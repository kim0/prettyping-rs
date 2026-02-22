#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::env;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use prettyping_rs::app::{run, AppConfig};
use prettyping_rs::engine::unix_surge::{UnixSurgeEngine, UnixSurgeEngineOptions};

const ENV_RUN_SMOKE: &str = "PRETTYPING_RS_RUN_UNIX_LIVE_SMOKE";
const ENV_TARGET: &str = "PRETTYPING_RS_UNIX_LIVE_TARGET";

#[test]
fn unix_backend_live_smoke_env_gated() {
    if env::var(ENV_RUN_SMOKE).ok().as_deref() != Some("1") {
        eprintln!(
            "skipping unix live smoke test (set {}=1 to enable)",
            ENV_RUN_SMOKE
        );
        return;
    }

    let target = env::var(ENV_TARGET)
        .ok()
        .and_then(|value| value.parse::<IpAddr>().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));

    let mut engine = UnixSurgeEngine::new(UnixSurgeEngineOptions {
        target,
        timeout: Duration::from_secs(1),
        ttl: None,
    })
    .expect("unix backend should initialize for live smoke test");

    let report = run(
        &mut engine,
        &AppConfig {
            target,
            interval: Duration::from_millis(250),
            timeout: Duration::from_secs(1),
            count: Some(1),
            payload_size: 16,
            ttl: None,
        },
    )
    .expect("runtime should complete in live smoke mode");

    assert_eq!(report.sent, 1, "smoke test must send one probe");
    assert!(
        report.replies >= 1,
        "expected at least one reply in live smoke mode (target: {target})"
    );
    assert_eq!(report.timeouts, 0, "live smoke should not timeout");
}
