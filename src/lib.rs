use std::net::IpAddr;
use std::time::Duration;

use clap::error::ErrorKind;

pub mod app;
pub mod cli;
pub mod config;
pub mod engine;
pub mod net;

const DEFAULT_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const DEFAULT_PAYLOAD_SIZE: usize = 56;

pub fn run() -> Result<(), clap::Error> {
    let config = cli::parse_config_from_env()?;
    let target = resolve_target(&config)?;
    let app_config = map_runtime_config(&config, target)?;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        run_with_unix_backend(app_config)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = app_config;
        Err(runtime_error(
            "runtime backend is not implemented on this platform yet (M4 pending)",
        ))
    }
}

fn resolve_target(config: &config::Config) -> Result<IpAddr, clap::Error> {
    let resolved = net::dns::resolve_once(&config.host, config.family).map_err(|err| {
        runtime_error(format!("failed to resolve target '{}': {err}", config.host))
    })?;

    resolved
        .addresses
        .first()
        .copied()
        .ok_or_else(|| runtime_error(format!("no resolved addresses found for '{}'", config.host)))
}

fn map_runtime_config(
    config: &config::Config,
    target: IpAddr,
) -> Result<app::AppConfig, clap::Error> {
    let interval =
        option_secs_to_duration(config.interval_secs, DEFAULT_INTERVAL, "-i/--interval")?;
    let timeout = option_secs_to_duration(config.timeout_secs, DEFAULT_TIMEOUT, "-W/--timeout")?;

    let payload_size = match config.packet_size {
        Some(packet_size) => usize::try_from(packet_size)
            .map_err(|_| runtime_error("-s/--size is too large for this platform pointer width"))?,
        None => DEFAULT_PAYLOAD_SIZE,
    };

    let ttl = match config.ttl {
        Some(ttl) => Some(
            u8::try_from(ttl).map_err(|_| runtime_error("-t/--ttl must be between 1 and 255"))?,
        ),
        None => None,
    };

    Ok(app::AppConfig {
        target,
        interval,
        timeout,
        count: config.count.map(u64::from),
        payload_size,
        ttl,
    })
}

fn option_secs_to_duration(
    configured: Option<f64>,
    fallback: Duration,
    flag_name: &str,
) -> Result<Duration, clap::Error> {
    let Some(seconds) = configured else {
        return Ok(fallback);
    };

    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(runtime_error(format!(
            "{flag_name} must be a finite value greater than 0"
        )));
    }

    Duration::try_from_secs_f64(seconds)
        .map_err(|_| runtime_error(format!("{flag_name} value is out of range")))
}

fn runtime_error(message: impl Into<String>) -> clap::Error {
    clap::Error::raw(ErrorKind::Io, message.into())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_with_unix_backend(app_config: app::AppConfig) -> Result<(), clap::Error> {
    let mut engine =
        engine::unix_surge::UnixSurgeEngine::new(engine::unix_surge::UnixSurgeEngineOptions {
            target: app_config.target,
            timeout: app_config.timeout,
            ttl: app_config.ttl,
        })
        .map_err(|err| runtime_error(err.to_string()))?;

    app::run(&mut engine, &app_config)
        .map(|_| ())
        .map_err(|err| runtime_error(format!("ping runtime failed: {err}")))
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    use crate::cli::parse_config_from_args;

    use super::map_runtime_config;

    #[test]
    fn maps_native_ping_flags_into_runtime_contract() {
        let config = parse_config_from_args([
            "prettyping-rs",
            "-c",
            "7",
            "-i",
            "0.25",
            "-W",
            "2.5",
            "-s",
            "80",
            "-t",
            "64",
            "example.com",
        ])
        .expect("cli parsing should pass");

        let runtime = map_runtime_config(&config, IpAddr::V4(Ipv4Addr::LOCALHOST))
            .expect("runtime mapping should pass");

        assert_eq!(runtime.count, Some(7));
        assert_eq!(runtime.interval, Duration::from_millis(250));
        assert_eq!(runtime.timeout, Duration::from_millis(2_500));
        assert_eq!(runtime.payload_size, 80);
        assert_eq!(runtime.ttl, Some(64));
    }

    #[test]
    fn uses_runtime_defaults_when_native_ping_flags_are_missing() {
        let config = parse_config_from_args(["prettyping-rs", "example.com"])
            .expect("cli parsing should pass");

        let runtime = map_runtime_config(&config, IpAddr::V4(Ipv4Addr::LOCALHOST))
            .expect("runtime mapping should pass");

        assert_eq!(runtime.count, None);
        assert_eq!(runtime.interval, Duration::from_secs(1));
        assert_eq!(runtime.timeout, Duration::from_secs(1));
        assert_eq!(runtime.payload_size, 56);
        assert_eq!(runtime.ttl, None);
    }
}
