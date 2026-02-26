use std::collections::BTreeMap;
use std::io;
use std::net::IpAddr;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use ping_async::IcmpEchoStatus;
use thiserror::Error;

use super::{
    EngineTime, PingEngine, PingEngineError, PingEvent, PingReply, ProbeRequest, TimedEvent,
};

const WINDOWS_ERROR_ACCESS_DENIED: i32 = 5;
const WINDOWS_ERROR_NETWORK_UNREACHABLE: i32 = 1_231;
const WINDOWS_ERROR_HOST_UNREACHABLE: i32 = 1_232;
const WINDOWS_ERROR_PROTOCOL_UNREACHABLE: i32 = 1_233;
const WINDOWS_ERROR_PORT_UNREACHABLE: i32 = 1_234;
const WINDOWS_WSAEACCES: i32 = 10_013;
const WINDOWS_WSAENETUNREACH: i32 = 10_051;
const WINDOWS_WSAEHOSTUNREACH: i32 = 10_065;
const WINDOWS_IP_GENERAL_FAILURE: i32 = 11_050;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsPingAsyncEngineOptions {
    pub target: IpAddr,
    pub timeout: Duration,
    pub ttl: Option<u8>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WindowsPingAsyncError {
    #[error("invalid windows backend timeout: must be greater than 0")]
    InvalidTimeout,
    #[error("failed to initialize windows ping backend: {message}")]
    InitFailed { message: String },
}

enum BackendMessage {
    Reply(TimedEvent),
    Fatal(String),
}

pub struct WindowsPingAsyncEngine {
    target: IpAddr,
    ttl: Option<u8>,
    now: EngineTime,
    started_at: Instant,
    runtime: tokio::runtime::Runtime,
    requestor: ping_async::IcmpEchoRequestor,
    inbox_tx: Sender<BackendMessage>,
    inbox_rx: Receiver<BackendMessage>,
    pending: BTreeMap<EngineTime, Vec<TimedEvent>>,
    fatal_error: Option<String>,
}

impl WindowsPingAsyncEngine {
    pub fn new(options: WindowsPingAsyncEngineOptions) -> Result<Self, WindowsPingAsyncError> {
        if options.timeout.is_zero() {
            return Err(WindowsPingAsyncError::InvalidTimeout);
        }

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| WindowsPingAsyncError::InitFailed {
                message: format!("failed to create tokio runtime: {err}"),
            })?;

        let requestor = ping_async::IcmpEchoRequestor::new(
            options.target,
            None,
            options.ttl,
            Some(options.timeout),
        )
        .map_err(|err| WindowsPingAsyncError::InitFailed {
            message: format_windows_io_error("could not initialize Windows ICMP requestor", &err),
        })?;

        let (inbox_tx, inbox_rx) = mpsc::channel();

        Ok(Self {
            target: options.target,
            ttl: options.ttl,
            now: Duration::ZERO,
            started_at: Instant::now(),
            runtime,
            requestor,
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

impl PingEngine for WindowsPingAsyncEngine {
    fn now(&self) -> EngineTime {
        self.now
    }

    fn send_probe(&mut self, request: ProbeRequest) -> Result<(), PingEngineError> {
        if request.sent_at != self.now {
            return Err(PingEngineError::InvalidProbeRequest {
                seq: request.seq,
                message: format!(
                    "sent_at {:?} does not match windows backend now {:?}",
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
                message: "ttl changed after backend initialization; windows backend requires a stable ttl"
                    .to_string(),
            });
        }

        let requestor = self.requestor.clone();
        let inbox = self.inbox_tx.clone();
        let started_at = self.started_at;
        let payload_size = request.payload_size;
        let requested_seq = request.seq;

        self.runtime.spawn(async move {
            match requestor.send().await {
                Ok(reply) => match reply.status() {
                    IcmpEchoStatus::Success => {
                        let timed_event = TimedEvent {
                            at: started_at.elapsed(),
                            event: PingEvent::Reply(PingReply {
                                seq: requested_seq,
                                ttl: None,
                                payload_size,
                                source: None,
                            }),
                        };
                        let _ = inbox.send(BackendMessage::Reply(timed_event));
                    }
                    // Keep continuous ping behavior resilient on Windows.
                    // Treat per-probe reachability statuses as timeout-like outcomes
                    // instead of aborting the entire session.
                    IcmpEchoStatus::TimedOut
                    | IcmpEchoStatus::Unreachable
                    | IcmpEchoStatus::Unknown => {}
                },
                Err(err) => {
                    if is_transient_probe_io_error(&err) {
                        return;
                    }
                    let message = format_windows_io_error(
                        &format!("windows backend failed while probing seq {requested_seq}"),
                        &err,
                    );
                    let _ = inbox.send(BackendMessage::Fatal(message));
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
                        message: "windows backend event channel disconnected".to_string(),
                    });
                }
            }
        }
    }
}

fn format_windows_io_error(context: &str, error: &io::Error) -> String {
    let mut message = format!("{context}: {error}");
    if let Some(guidance) = windows_error_guidance(error) {
        message.push('\n');
        message.push_str(&guidance);
    }
    message
}

fn windows_error_guidance(error: &io::Error) -> Option<String> {
    match error.raw_os_error() {
        Some(WINDOWS_ERROR_ACCESS_DENIED | WINDOWS_WSAEACCES) => {
            return Some(
                "Windows denied ICMP access (permission/firewall policy).\nRun once in an elevated terminal to validate permissions, then review Windows Firewall and endpoint security ICMP rules."
                    .to_string(),
            );
        }
        Some(WINDOWS_ERROR_NETWORK_UNREACHABLE | WINDOWS_WSAENETUNREACH) => {
            return Some(
                "Network unreachable.\nVerify adapter connectivity, VPN state, default gateway, and route table (`route print`)."
                    .to_string(),
            );
        }
        Some(WINDOWS_ERROR_HOST_UNREACHABLE | WINDOWS_WSAEHOSTUNREACH) => {
            return Some(
                "Host unreachable.\nConfirm target IP is correct/reachable from this subnet and that upstream firewall/routing allows ICMP."
                    .to_string(),
            );
        }
        Some(WINDOWS_ERROR_PROTOCOL_UNREACHABLE | WINDOWS_ERROR_PORT_UNREACHABLE) => {
            return Some(
                "Protocol/port unreachable reported by network stack.\nThis often indicates filtering or policy controls; verify ICMP echo is allowed on local and upstream firewalls."
                    .to_string(),
            );
        }
        Some(WINDOWS_IP_GENERAL_FAILURE) => {
            return Some(
                "Windows reported a general IP failure.\nCheck interface status, firewall profile, and endpoint security network protections before retrying."
                    .to_string(),
            );
        }
        _ => {}
    }

    match error.kind() {
        io::ErrorKind::PermissionDenied => Some(
            "Permission denied while opening/using Windows ICMP APIs.\nValidate local policy (firewall/GPO/security agent) and retry from an elevated terminal."
                .to_string(),
        ),
        io::ErrorKind::TimedOut => Some(
            "Operation timed out in Windows ICMP APIs.\nCheck host reachability and firewall rules for outbound/inbound ICMP echo."
                .to_string(),
        ),
        io::ErrorKind::NotConnected => Some(
            "No active network path is available.\nVerify interface link state, IP config (`ipconfig`), and routing."
                .to_string(),
        ),
        io::ErrorKind::ConnectionRefused => Some(
            "Connection was refused by the network stack.\nInspect host firewall and upstream filtering for ICMP echo."
                .to_string(),
        ),
        _ => None,
    }
}

fn is_transient_probe_io_error(error: &io::Error) -> bool {
    let is_transient_raw_code = error.raw_os_error().is_some_and(|code| {
        matches!(
            code,
            WINDOWS_ERROR_NETWORK_UNREACHABLE
                | WINDOWS_ERROR_HOST_UNREACHABLE
                | WINDOWS_WSAENETUNREACH
                | WINDOWS_WSAEHOSTUNREACH
        )
    });

    is_transient_raw_code || error.kind() == io::ErrorKind::NotConnected
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{is_transient_probe_io_error, windows_error_guidance};

    #[test]
    fn maps_access_denied_to_firewall_guidance() {
        let err = io::Error::from_raw_os_error(5);
        let message = windows_error_guidance(&err).expect("guidance should exist");

        assert!(message.contains("ICMP"));
        assert!(message.contains("Firewall"));
    }

    #[test]
    fn maps_network_unreachable_to_route_guidance() {
        let err = io::Error::from_raw_os_error(1_231);
        let message = windows_error_guidance(&err).expect("guidance should exist");

        assert!(message.contains("Network unreachable"));
        assert!(message.contains("route print"));
    }

    #[test]
    fn transient_network_errors_are_non_fatal() {
        let err = io::Error::from_raw_os_error(1_231);
        assert!(is_transient_probe_io_error(&err));
    }

    #[test]
    fn access_denied_is_not_transient() {
        let err = io::Error::from_raw_os_error(5);
        assert!(!is_transient_probe_io_error(&err));
    }

    #[test]
    fn not_connected_kind_is_treated_as_transient() {
        let err = io::Error::from(io::ErrorKind::NotConnected);
        assert!(is_transient_probe_io_error(&err));
    }

    #[test]
    fn raw_wsa_not_connected_errno_is_treated_as_transient() {
        let err = io::Error::from_raw_os_error(10_057);
        assert!(is_transient_probe_io_error(&err));
    }
}
