use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use thiserror::Error;

use super::{
    EngineTime, PingEngine, PingEngineError, PingEvent, PingReply, ProbeRequest, SequenceNumber,
    TimedEvent,
};

const UNIX_PERMISSION_DENIED_ERRNO: [i32; 2] = [1, 13]; // EPERM, EACCES
const U16_MODULO: u64 = u16::MAX as u64 + 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnixSurgeEngineOptions {
    pub target: IpAddr,
    pub timeout: Duration,
    pub ttl: Option<u8>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum UnixSurgeError {
    #[error("invalid unix backend timeout: must be greater than 0")]
    InvalidTimeout,
    #[error("failed to initialize unix ping backend: {message}")]
    InitFailed { message: String },
}

enum BackendMessage {
    Reply(TimedEvent),
    Fatal(String),
}

pub struct UnixSurgeEngine {
    target: IpAddr,
    timeout: Duration,
    ttl: Option<u8>,
    now: EngineTime,
    started_at: Instant,
    runtime: tokio::runtime::Runtime,
    client: Arc<surge_ping::Client>,
    identifier: surge_ping::PingIdentifier,
    inbox_tx: Sender<BackendMessage>,
    inbox_rx: Receiver<BackendMessage>,
    pending: BTreeMap<EngineTime, Vec<TimedEvent>>,
    fatal_error: Option<String>,
}

impl UnixSurgeEngine {
    pub fn new(options: UnixSurgeEngineOptions) -> Result<Self, UnixSurgeError> {
        if options.timeout.is_zero() {
            return Err(UnixSurgeError::InvalidTimeout);
        }

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| UnixSurgeError::InitFailed {
                message: format!("failed to create tokio runtime: {err}"),
            })?;

        let mut config_builder = surge_ping::Config::builder().kind(match options.target {
            IpAddr::V4(_) => surge_ping::ICMP::V4,
            IpAddr::V6(_) => surge_ping::ICMP::V6,
        });
        if let Some(ttl) = options.ttl {
            config_builder = config_builder.ttl(u32::from(ttl));
        }
        let config = config_builder.build();

        // surge-ping requires a Tokio runtime context for initialization.
        let _tokio_guard = runtime.enter();

        let client = Arc::new(surge_ping::Client::new(&config).map_err(|err| {
            UnixSurgeError::InitFailed {
                message: format_io_error_with_guidance("could not open ICMP socket", &err),
            }
        })?);

        let identifier = default_ping_identifier();
        let (inbox_tx, inbox_rx) = mpsc::channel();

        Ok(Self {
            target: options.target,
            timeout: options.timeout,
            ttl: options.ttl,
            now: Duration::ZERO,
            started_at: Instant::now(),
            runtime,
            client,
            identifier,
            inbox_tx,
            inbox_rx,
            pending: BTreeMap::new(),
            fatal_error: None,
        })
    }

    fn drain_inbox(&mut self) {
        while let Ok(message) = self.inbox_rx.try_recv() {
            self.handle_backend_message(message);
        }
    }

    fn handle_backend_message(&mut self, message: BackendMessage) {
        match message {
            BackendMessage::Reply(mut timed_event) => {
                // Keep the engine timeline monotonic even if a reply is observed late.
                // We never want to store (or later drop) events that appear to be in the past.
                if timed_event.at < self.now {
                    timed_event.at = self.now;
                }
                self.pending
                    .entry(timed_event.at)
                    .or_default()
                    .push(timed_event);
            }
            BackendMessage::Fatal(message) => {
                if self.fatal_error.is_none() {
                    self.fatal_error = Some(message);
                }
            }
        }
    }

    fn take_ready_events(&mut self, deadline: EngineTime) -> Option<Vec<TimedEvent>> {
        let selected_time = self
            .pending
            .range(self.now..=deadline)
            .next()
            .map(|(at, _)| *at);

        selected_time.map(|at| {
            self.now = at;
            self.pending.remove(&at).unwrap_or_default()
        })
    }

    fn real_time_remaining_until(&self, deadline: EngineTime) -> Duration {
        if let Some(target_instant) = self.started_at.checked_add(deadline) {
            return target_instant.saturating_duration_since(Instant::now());
        }
        Duration::ZERO
    }

    fn pop_fatal_error(&mut self) -> Option<PingEngineError> {
        self.fatal_error
            .take()
            .map(|message| PingEngineError::PollFailed { message })
    }
}

impl PingEngine for UnixSurgeEngine {
    fn now(&self) -> EngineTime {
        self.now
    }

    fn send_probe(&mut self, request: ProbeRequest) -> Result<(), PingEngineError> {
        if request.sent_at != self.now {
            return Err(PingEngineError::InvalidProbeRequest {
                seq: request.seq,
                message: format!(
                    "sent_at {:?} does not match unix backend now {:?}",
                    request.sent_at, self.now
                ),
            });
        }

        if request.target != self.target {
            return Err(PingEngineError::InvalidProbeRequest {
                seq: request.seq,
                message: format!(
                    "target changed from {} to {} after backend initialization",
                    self.target, request.target
                ),
            });
        }

        if request.ttl != self.ttl {
            return Err(PingEngineError::InvalidProbeRequest {
                seq: request.seq,
                message:
                    "ttl changed after backend initialization; unix backend requires a stable ttl"
                        .to_string(),
            });
        }

        let sequence_u16 = fold_sequence_to_u16(request.seq);

        let payload = vec![0_u8; request.payload_size];
        let client = self.client.clone();
        let identifier = self.identifier;
        let timeout = self.timeout;
        let inbox = self.inbox_tx.clone();
        let target = self.target;
        let requested_seq = request.seq;
        let started_at = self.started_at;

        self.runtime.spawn(async move {
            let mut pinger = client.pinger(target, identifier).await;
            let expected_identifier = pinger.ident.map(|ident| ident.into_u16());
            pinger.timeout(timeout);

            match pinger
                .ping(surge_ping::PingSequence(sequence_u16), &payload)
                .await
            {
                Ok((packet, _duration)) => {
                    let reply_sequence_u16 = packet.get_sequence().into_u16();
                    let expected_reply_sequence = fold_sequence_to_u16(requested_seq);
                    if reply_sequence_u16 != expected_reply_sequence {
                        return;
                    }

                    if let Some(expected) = expected_identifier {
                        let reply_ident = packet.get_identifier().into_u16();
                        if reply_ident != expected {
                            return;
                        }
                    }

                    let (source, ttl) = match packet {
                        surge_ping::IcmpPacket::V4(packet_v4) => (
                            Some(IpAddr::V4(packet_v4.get_source())),
                            packet_v4.get_ttl(),
                        ),
                        surge_ping::IcmpPacket::V6(packet_v6) => {
                            (Some(IpAddr::V6(packet_v6.get_source())), None)
                        }
                    };

                    let timed_event = TimedEvent {
                        at: started_at.elapsed(),
                        event: PingEvent::Reply(PingReply {
                            seq: requested_seq,
                            ttl,
                            payload_size: payload.len(),
                            source,
                        }),
                    };
                    let _ = inbox.send(BackendMessage::Reply(timed_event));
                }
                Err(surge_ping::SurgeError::Timeout { .. }) => {}
                Err(surge_ping::SurgeError::IOError(io_err)) => {
                    let message = format_io_error_with_guidance(
                        "failed while sending/receiving ICMP",
                        &io_err,
                    );
                    let _ = inbox.send(BackendMessage::Fatal(message));
                }
                Err(other) => {
                    let _ = inbox.send(BackendMessage::Fatal(format!(
                        "unix backend ping task failed for seq {requested_seq}: {other}"
                    )));
                }
            }
        });

        Ok(())
    }

    fn poll_until(&mut self, deadline: EngineTime) -> Result<Vec<TimedEvent>, PingEngineError> {
        if deadline < self.now {
            return Err(PingEngineError::NonMonotonicPoll);
        }

        loop {
            let effective_deadline = deadline.max(self.started_at.elapsed());

            self.drain_inbox();

            if let Some(error) = self.pop_fatal_error() {
                return Err(error);
            }

            if let Some(events) = self.take_ready_events(effective_deadline) {
                return Ok(events);
            }

            if effective_deadline == self.now {
                return Ok(Vec::new());
            }

            let remaining = self.real_time_remaining_until(effective_deadline);
            if remaining.is_zero() {
                self.now = effective_deadline;
                return Ok(Vec::new());
            }

            match self.inbox_rx.recv_timeout(remaining) {
                Ok(message) => self.handle_backend_message(message),
                Err(RecvTimeoutError::Timeout) => {
                    self.now = effective_deadline;
                    return Ok(Vec::new());
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(PingEngineError::PollFailed {
                        message: "unix backend event channel disconnected".to_string(),
                    });
                }
            }
        }
    }
}

fn default_ping_identifier() -> surge_ping::PingIdentifier {
    let modulo = u32::from(u16::MAX) + 1;
    let value = std::process::id() % modulo;
    surge_ping::PingIdentifier(u16::try_from(value).unwrap_or(0))
}

fn fold_sequence_to_u16(seq: SequenceNumber) -> u16 {
    u16::try_from(seq % U16_MODULO).unwrap_or(0)
}

fn format_io_error_with_guidance(context: &str, error: &std::io::Error) -> String {
    let mut message = format!("{context}: {error}");
    if let Some(guidance) = permission_guidance(error) {
        message.push('\n');
        message.push_str(&guidance);
    }
    message
}

fn permission_guidance(error: &std::io::Error) -> Option<String> {
    let has_permission_denied_errno = error
        .raw_os_error()
        .map(|code| UNIX_PERMISSION_DENIED_ERRNO.contains(&code))
        .unwrap_or(false);
    let is_permission_denied =
        error.kind() == std::io::ErrorKind::PermissionDenied || has_permission_denied_errno;

    if !is_permission_denied {
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        return Some(
            "Linux denied ICMP socket access. Add your group to ping_group_range and retry.\nExample (temporary): sudo sysctl -w net.ipv4.ping_group_range=\"0 2147483647\"\nCheck your group id with: id -g\nPersist via: /etc/sysctl.d/*.conf"
                .to_string(),
        );
    }

    #[cfg(target_os = "macos")]
    {
        return Some(
            "macOS denied ICMP socket access. This usually means sandbox/policy restrictions.\nRun from a normal Terminal session (not sandboxed), or try elevated privileges to verify permission boundaries."
                .to_string(),
        );
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::permission_guidance;

    #[test]
    fn permission_guidance_is_actionable() {
        let err = io::Error::from_raw_os_error(1);
        let message = permission_guidance(&err).expect("permission guidance should exist");

        #[cfg(target_os = "linux")]
        {
            assert!(message.contains("ping_group_range"));
            assert!(message.contains("id -g"));
        }

        #[cfg(target_os = "macos")]
        {
            assert!(message.contains("Terminal"));
            assert!(message.contains("elevated"));
        }
    }
}
