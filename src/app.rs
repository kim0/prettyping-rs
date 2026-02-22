use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::time::Duration;

use thiserror::Error;

use crate::engine::{
    EngineTime, PingEngine, PingEngineError, PingEvent, ProbeRequest, SequenceNumber,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub target: IpAddr,
    pub interval: Duration,
    pub timeout: Duration,
    pub count: Option<u64>,
    pub payload_size: usize,
    pub ttl: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    ProbeSent {
        seq: SequenceNumber,
        at: Duration,
    },
    ProbeReply {
        seq: SequenceNumber,
        sent_at: Duration,
        received_at: Duration,
        rtt_ms: u64,
        duplicate: bool,
        late: bool,
    },
    ProbeTimeout {
        seq: SequenceNumber,
        sent_at: Duration,
        deadline: Duration,
    },
    Interrupted {
        at: Duration,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppReport {
    pub events: Vec<AppEvent>,
    pub sent: u64,
    pub replies: u64,
    pub timeouts: u64,
    pub duplicate_replies: u64,
    pub late_replies: u64,
    pub interrupted: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppError {
    #[error("interval must be greater than 0")]
    InvalidInterval,
    #[error("timeout must be greater than 0")]
    InvalidTimeout,
    #[error("count must be greater than 0 when provided")]
    InvalidCount,
    #[error("duration overflow while scheduling probes")]
    ClockOverflow,
    #[error("observer failed: {message}")]
    Observer { message: String },
    #[error(transparent)]
    Engine(#[from] PingEngineError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InFlight {
    sent_at: Duration,
    deadline: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SentMeta {
    sent_at: Duration,
}

pub fn run<E>(engine: &mut E, config: &AppConfig) -> Result<AppReport, AppError>
where
    E: PingEngine,
{
    run_with_observer(engine, config, |_| Ok(()))
}

pub fn run_with_observer<E, F>(
    engine: &mut E,
    config: &AppConfig,
    mut observer: F,
) -> Result<AppReport, AppError>
where
    E: PingEngine,
    F: FnMut(&AppEvent) -> Result<(), AppError>,
{
    validate_config(config)?;

    let mut report = AppReport::default();
    let mut sent_total: u64 = 0;
    let mut next_seq: SequenceNumber = 1;
    let mut next_send_at = engine.now();

    let mut in_flight: BTreeMap<SequenceNumber, InFlight> = BTreeMap::new();
    let mut sent_meta: BTreeMap<SequenceNumber, SentMeta> = BTreeMap::new();
    let mut replied: BTreeSet<SequenceNumber> = BTreeSet::new();
    let mut timed_out: BTreeSet<SequenceNumber> = BTreeSet::new();

    schedule_due(
        engine,
        config,
        &mut observer,
        &mut report,
        &mut sent_total,
        &mut next_seq,
        &mut next_send_at,
        &mut in_flight,
        &mut sent_meta,
        &mut replied,
        &mut timed_out,
    )?;

    loop {
        if report.interrupted || is_finished(config.count, sent_total, &in_flight) {
            return Ok(report);
        }

        let deadline = match next_deadline(config.count, sent_total, next_send_at, &in_flight) {
            Some(deadline) => deadline,
            None => return Ok(report),
        };

        let events = engine.poll_until(deadline)?;
        for timed_event in events {
            match timed_event.event {
                PingEvent::Reply(reply) => {
                    let seq = reply.seq;
                    if let Some(inflight) = in_flight.remove(&seq) {
                        let did_insert = replied.insert(seq);
                        if did_insert {
                            report.replies = report.replies.saturating_add(1);
                            record_event(
                                &mut report,
                                AppEvent::ProbeReply {
                                    seq,
                                    sent_at: inflight.sent_at,
                                    received_at: timed_event.at,
                                    rtt_ms: duration_to_ms(
                                        timed_event.at.saturating_sub(inflight.sent_at),
                                    ),
                                    duplicate: false,
                                    late: false,
                                },
                                &mut observer,
                            )?;
                        }
                    } else if let Some(meta) = sent_meta.get(&seq) {
                        let duplicate = replied.contains(&seq);
                        let late = timed_out.contains(&seq);
                        if duplicate {
                            report.duplicate_replies = report.duplicate_replies.saturating_add(1);
                        } else {
                            let _ = replied.insert(seq);
                            report.replies = report.replies.saturating_add(1);
                        }
                        if late {
                            report.late_replies = report.late_replies.saturating_add(1);
                        }

                        record_event(
                            &mut report,
                            AppEvent::ProbeReply {
                                seq,
                                sent_at: meta.sent_at,
                                received_at: timed_event.at,
                                rtt_ms: duration_to_ms(timed_event.at.saturating_sub(meta.sent_at)),
                                duplicate,
                                late,
                            },
                            &mut observer,
                        )?;
                    }
                }
                PingEvent::Interrupt => {
                    report.interrupted = true;
                    record_event(
                        &mut report,
                        AppEvent::Interrupted { at: timed_event.at },
                        &mut observer,
                    )?;
                    break;
                }
            }
        }

        if report.interrupted {
            return Ok(report);
        }

        schedule_due(
            engine,
            config,
            &mut observer,
            &mut report,
            &mut sent_total,
            &mut next_seq,
            &mut next_send_at,
            &mut in_flight,
            &mut sent_meta,
            &mut replied,
            &mut timed_out,
        )?;
    }
}

fn validate_config(config: &AppConfig) -> Result<(), AppError> {
    if config.interval.is_zero() {
        return Err(AppError::InvalidInterval);
    }
    if config.timeout.is_zero() {
        return Err(AppError::InvalidTimeout);
    }
    if matches!(config.count, Some(0)) {
        return Err(AppError::InvalidCount);
    }
    Ok(())
}

fn next_deadline(
    count: Option<u64>,
    sent_total: u64,
    next_send_at: Duration,
    in_flight: &BTreeMap<SequenceNumber, InFlight>,
) -> Option<Duration> {
    let send_deadline = if should_send_more(count, sent_total) {
        Some(next_send_at)
    } else {
        None
    };

    let timeout_deadline = in_flight.values().map(|entry| entry.deadline).min();

    match (send_deadline, timeout_deadline) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn schedule_due<E>(
    engine: &mut E,
    config: &AppConfig,
    observer: &mut impl FnMut(&AppEvent) -> Result<(), AppError>,
    report: &mut AppReport,
    sent_total: &mut u64,
    next_seq: &mut SequenceNumber,
    next_send_at: &mut Duration,
    in_flight: &mut BTreeMap<SequenceNumber, InFlight>,
    sent_meta: &mut BTreeMap<SequenceNumber, SentMeta>,
    replied: &mut BTreeSet<SequenceNumber>,
    timed_out: &mut BTreeSet<SequenceNumber>,
) -> Result<(), AppError>
where
    E: PingEngine,
{
    let now = engine.now();

    let expired: Vec<SequenceNumber> = in_flight
        .iter()
        .filter_map(|(seq, inflight)| (inflight.deadline <= now).then_some(*seq))
        .collect();

    for seq in expired {
        if let Some(inflight) = in_flight.remove(&seq) {
            let _ = timed_out.insert(seq);
            report.timeouts = report.timeouts.saturating_add(1);
            record_event(
                report,
                AppEvent::ProbeTimeout {
                    seq,
                    sent_at: inflight.sent_at,
                    deadline: inflight.deadline,
                },
                observer,
            )?;
        }
    }

    prune_tracking_state(*next_seq, sent_meta, replied, timed_out);

    while should_send_more(config.count, *sent_total) && *next_send_at <= now {
        let seq = *next_seq;
        let sent_at: EngineTime = now;

        let request = ProbeRequest {
            seq,
            target: config.target,
            sent_at,
            payload_size: config.payload_size,
            ttl: config.ttl,
        };

        engine.send_probe(request)?;

        let deadline = add_duration(sent_at, config.timeout)?;

        in_flight.insert(seq, InFlight { sent_at, deadline });
        sent_meta.insert(seq, SentMeta { sent_at });
        report.sent = report.sent.saturating_add(1);
        record_event(report, AppEvent::ProbeSent { seq, at: sent_at }, observer)?;

        *sent_total = sent_total.saturating_add(1);
        *next_seq = next_seq.saturating_add(1);
        *next_send_at = add_duration(*next_send_at, config.interval)?;
    }

    Ok(())
}

fn record_event(
    report: &mut AppReport,
    event: AppEvent,
    observer: &mut impl FnMut(&AppEvent) -> Result<(), AppError>,
) -> Result<(), AppError> {
    observer(&event)?;
    report.events.push(event);
    Ok(())
}

fn should_send_more(count: Option<u64>, sent_total: u64) -> bool {
    match count {
        Some(limit) => sent_total < limit,
        None => true,
    }
}

fn is_finished(
    count: Option<u64>,
    sent_total: u64,
    in_flight: &BTreeMap<SequenceNumber, InFlight>,
) -> bool {
    matches!(count, Some(limit) if sent_total >= limit) && in_flight.is_empty()
}

fn add_duration(base: Duration, delta: Duration) -> Result<Duration, AppError> {
    base.checked_add(delta).ok_or(AppError::ClockOverflow)
}

fn duration_to_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn prune_tracking_state(
    next_seq: SequenceNumber,
    sent_meta: &mut BTreeMap<SequenceNumber, SentMeta>,
    replied: &mut BTreeSet<SequenceNumber>,
    timed_out: &mut BTreeSet<SequenceNumber>,
) {
    // Keep a bounded window so the default infinite ping mode does not grow memory.
    // This window only needs to be large enough to classify late/duplicate replies.
    const KEEP_WINDOW: SequenceNumber = 10_000;

    let keep_from = next_seq.saturating_sub(KEEP_WINDOW);

    sent_meta.retain(|seq, _| *seq >= keep_from);
    replied.retain(|seq| *seq >= keep_from);
    timed_out.retain(|seq| *seq >= keep_from);
}
