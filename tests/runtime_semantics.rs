use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use prettyping_rs::app::{AppConfig, AppEvent, run};
use prettyping_rs::engine::mock::MockEngine;
use prettyping_rs::engine::{PingEvent, PingReply, TimedEvent};

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn base_config(count: Option<u64>, interval_ms: u64, timeout_ms: u64) -> AppConfig {
    AppConfig {
        target: IpAddr::V4(Ipv4Addr::LOCALHOST),
        interval: ms(interval_ms),
        timeout: ms(timeout_ms),
        count,
        payload_size: 56,
        ttl: None,
    }
}

#[test]
fn handles_reordered_replies_deterministically() {
    let mut engine = MockEngine::with_now(Duration::ZERO);
    engine.queue_events([
        TimedEvent {
            at: ms(1_500),
            event: PingEvent::Reply(PingReply::for_seq(2)),
        },
        TimedEvent {
            at: ms(2_100),
            event: PingEvent::Reply(PingReply::for_seq(3)),
        },
        TimedEvent {
            at: ms(2_500),
            event: PingEvent::Reply(PingReply::for_seq(1)),
        },
    ]);

    let report = run(&mut engine, &base_config(Some(3), 1_000, 5_000)).expect("run should pass");

    assert_eq!(report.sent, 3);
    assert_eq!(report.replies, 3);
    assert_eq!(report.timeouts, 0);

    let reply_sequences: Vec<u64> = report
        .events
        .iter()
        .filter_map(|event| match event {
            AppEvent::ProbeReply { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(reply_sequences, vec![2, 3, 1]);

    let sent_sequences: Vec<u64> = engine
        .sent_requests()
        .iter()
        .map(|request| request.seq)
        .collect();
    assert_eq!(sent_sequences, vec![1, 2, 3]);
}

#[test]
fn counts_duplicate_replies_without_double_counting_success() {
    let mut engine = MockEngine::with_now(Duration::ZERO);
    engine.queue_events([
        TimedEvent {
            at: ms(100),
            event: PingEvent::Reply(PingReply::for_seq(1)),
        },
        TimedEvent {
            at: ms(1_600),
            event: PingEvent::Reply(PingReply::for_seq(1)),
        },
        TimedEvent {
            at: ms(2_600),
            event: PingEvent::Reply(PingReply::for_seq(2)),
        },
    ]);

    let report = run(&mut engine, &base_config(Some(2), 1_000, 5_000)).expect("run should pass");

    assert_eq!(report.sent, 2);
    assert_eq!(report.replies, 2);
    assert_eq!(report.duplicate_replies, 1);
    assert_eq!(report.late_replies, 0);

    let duplicates: Vec<bool> = report
        .events
        .iter()
        .filter_map(|event| match event {
            AppEvent::ProbeReply { duplicate, .. } => Some(*duplicate),
            _ => None,
        })
        .collect();
    assert_eq!(duplicates, vec![false, true, false]);
}

#[test]
fn marks_late_replies_after_timeout() {
    let mut engine = MockEngine::with_now(Duration::ZERO);
    engine.queue_events([
        TimedEvent {
            at: ms(1_500),
            event: PingEvent::Reply(PingReply::for_seq(1)),
        },
        TimedEvent {
            at: ms(2_500),
            event: PingEvent::Reply(PingReply::for_seq(2)),
        },
    ]);

    let report = run(&mut engine, &base_config(Some(2), 2_000, 1_000)).expect("run should pass");

    assert_eq!(report.sent, 2);
    assert_eq!(report.replies, 2);
    assert_eq!(report.timeouts, 1);
    assert_eq!(report.late_replies, 1);

    let has_seq1_timeout = report.events.iter().any(|event| {
        matches!(
            event,
            AppEvent::ProbeTimeout {
                seq: 1,
                sent_at,
                deadline
            } if *sent_at == ms(0) && *deadline == ms(1_000)
        )
    });
    assert!(has_seq1_timeout, "seq=1 timeout event should be present");

    let has_late_reply = report.events.iter().any(|event| {
        matches!(
            event,
            AppEvent::ProbeReply {
                seq: 1,
                late: true,
                duplicate: false,
                ..
            }
        )
    });
    assert!(has_late_reply, "late reply for seq=1 should be present");
}

#[test]
fn emits_timeout_when_no_reply_arrives() {
    let mut engine = MockEngine::with_now(Duration::ZERO);

    let report = run(&mut engine, &base_config(Some(1), 1_000, 700)).expect("run should pass");

    assert_eq!(report.sent, 1);
    assert_eq!(report.replies, 0);
    assert_eq!(report.timeouts, 1);
    assert!(!report.interrupted);

    assert!(
        report
            .events
            .iter()
            .any(|event| matches!(event, AppEvent::ProbeTimeout { seq: 1, .. }))
    );
}

#[test]
fn stops_on_interrupt_event() {
    let mut engine = MockEngine::with_now(Duration::ZERO);
    engine.queue_event(TimedEvent {
        at: ms(1_200),
        event: PingEvent::Interrupt,
    });

    let report = run(&mut engine, &base_config(None, 1_000, 5_000)).expect("run should pass");

    assert!(report.interrupted);
    assert_eq!(report.sent, 2);
    assert_eq!(report.timeouts, 0);

    let interrupt_event = report.events.iter().find_map(|event| match event {
        AppEvent::Interrupted { at } => Some(*at),
        _ => None,
    });
    assert_eq!(interrupt_event, Some(ms(1_200)));

    let sent_sequences: Vec<u64> = engine
        .sent_requests()
        .iter()
        .map(|request| request.seq)
        .collect();
    assert_eq!(sent_sequences, vec![1, 2]);
}
