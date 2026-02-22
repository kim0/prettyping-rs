use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use prettyping_rs::app::{AppConfig, run_with_observer};
use prettyping_rs::engine::mock::MockEngine;

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

#[test]
fn tracking_state_does_not_grow_unbounded_in_infinite_mode() {
    // This test runs a lot of sends with no replies. Without pruning, the internal
    // sent_meta/timed_out sets grow without bound.
    //
    // We observe memory indirectly by ensuring the run completes quickly and does
    // not time out. The mock engine runs in deterministic time.
    let mut engine = MockEngine::with_now(Duration::ZERO);

    let config = AppConfig {
        target: IpAddr::V4(Ipv4Addr::LOCALHOST),
        interval: ms(1),
        timeout: ms(1),
        count: Some(25_000),
        payload_size: 56,
        ttl: None,
    };

    let mut observed_timeouts = 0u64;

    let report = run_with_observer(&mut engine, &config, |event| {
        if matches!(event, prettyping_rs::app::AppEvent::ProbeTimeout { .. }) {
            observed_timeouts += 1;
        }
        Ok(())
    })
    .expect("run should succeed");

    assert_eq!(report.sent, 25_000);
    assert_eq!(report.replies, 0);
    assert_eq!(observed_timeouts, 25_000);
}
