use std::process::Command;

use prettyping_rs::cli::parse_config_from_args;
use prettyping_rs::config::AddressFamily;

#[test]
fn defaults_match_prettyping_behavior() {
    let cfg =
        parse_config_from_args(["prettyping-rs", "example.com"]).expect("parse should succeed");

    assert_eq!(cfg.host, "example.com");
    assert!(cfg.color);
    assert!(cfg.multicolor);
    assert!(cfg.unicode);
    assert!(cfg.legend);
    assert!(cfg.globalstats);
    assert!(cfg.recentstats);
    assert_eq!(cfg.last, 60);
    assert_eq!(cfg.family, AddressFamily::Any);
    assert_eq!(cfg.terminal, None);
}

#[test]
fn maps_kept_flags_and_native_ping_flags() {
    let cfg = parse_config_from_args([
        "prettyping-rs",
        "-nocolor",
        "--nomulticolor",
        "--nounicode",
        "--nolegend",
        "--noglobalstats",
        "--norecentstats",
        "--noterminal",
        "--last",
        "120",
        "--columns",
        "140",
        "--lines",
        "40",
        "--rttmin",
        "25",
        "--rttmax",
        "350",
        "-6",
        "-c",
        "10",
        "-i",
        "0.5",
        "-W",
        "1.2",
        "-s",
        "64",
        "-t",
        "42",
        "example.com",
    ])
    .expect("parse should succeed");

    assert!(!cfg.color);
    assert!(!cfg.multicolor);
    assert!(!cfg.unicode);
    assert!(!cfg.legend);
    assert!(!cfg.globalstats);
    assert!(!cfg.recentstats);
    assert_eq!(cfg.terminal, Some(false));
    assert_eq!(cfg.last, 120);
    assert_eq!(cfg.columns, Some(140));
    assert_eq!(cfg.lines, Some(40));
    assert_eq!(cfg.rttmin, Some(25));
    assert_eq!(cfg.rttmax, Some(350));
    assert_eq!(cfg.family, AddressFamily::Ipv6);
    assert_eq!(cfg.count, Some(10));
    assert_eq!(cfg.interval_secs, Some(0.5));
    assert_eq!(cfg.timeout_secs, Some(1.2));
    assert_eq!(cfg.packet_size, Some(64));
    assert_eq!(cfg.ttl, Some(42));
}

#[test]
fn removed_legacy_flags_return_usage_error() {
    let err = parse_config_from_args(["prettyping-rs", "--awkbin", "awk", "example.com"])
        .expect_err("--awkbin must be rejected");

    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("--awkbin was removed"));

    let err = parse_config_from_args(["prettyping-rs", "--pingbin", "ping", "example.com"])
        .expect_err("--pingbin must be rejected");

    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("--pingbin was removed"));
}

#[test]
fn unsupported_legacy_flags_are_rejected() {
    for flag in ["-f", "-R", "-q", "-a"] {
        let err =
            parse_config_from_args(["prettyping-rs", flag, "example.com"]).expect_err("must fail");
        assert_eq!(err.exit_code(), 2, "flag {flag} should exit with code 2");
        assert!(
            err.to_string().contains("unsupported legacy flag"),
            "flag {flag} should produce explicit unsupported message"
        );
    }
}

#[test]
fn legacy_verbose_flag_is_accepted_and_ignored() {
    let cfg = parse_config_from_args(["prettyping-rs", "-v", "example.com"])
        .expect("-v should be accepted");
    assert_eq!(cfg.host, "example.com");
}

#[test]
fn rejects_invalid_rtt_range() {
    let err = parse_config_from_args([
        "prettyping-rs",
        "--rttmin",
        "50",
        "--rttmax",
        "50",
        "example.com",
    ])
    .expect_err("equal bounds must fail");

    assert_eq!(err.exit_code(), 2);
    assert!(err
        .to_string()
        .contains("--rttmin must be strictly smaller than --rttmax"));
}

#[test]
fn rejects_family_conflicts() {
    let err = parse_config_from_args(["prettyping-rs", "-4", "-6", "example.com"])
        .expect_err("must fail");
    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("cannot use -4 and -6 together"));

    let err = parse_config_from_args(["prettyping-rs", "-4", "::1"]).expect_err("must fail");
    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("host address family conflicts"));
}

#[test]
fn help_mentions_removed_and_unsupported_flags() {
    let err = parse_config_from_args(["prettyping-rs", "--help"]).expect_err("help exits early");
    let help = err.to_string();

    assert_eq!(err.exit_code(), 0);
    assert!(help.contains("Removed legacy flags: --awkbin, --pingbin"));
    assert!(help.contains("Unsupported legacy flags: -f, -R, -q, -a"));
}

#[test]
fn process_usage_errors_exit_with_code_2() {
    let bin = env!("CARGO_BIN_EXE_prettyping-rs");

    let awkbin_output = Command::new(bin)
        .arg("--awkbin")
        .arg("awk")
        .arg("example.com")
        .output()
        .expect("binary execution must work");

    assert_eq!(awkbin_output.status.code(), Some(2));

    let awkbin_stderr = String::from_utf8_lossy(&awkbin_output.stderr);
    assert!(awkbin_stderr.contains("--awkbin was removed"));

    let pingbin_output = Command::new(bin)
        .arg("--pingbin")
        .arg("ping")
        .arg("example.com")
        .output()
        .expect("binary execution must work");

    assert_eq!(pingbin_output.status.code(), Some(2));

    let pingbin_stderr = String::from_utf8_lossy(&pingbin_output.stderr);
    assert!(pingbin_stderr.contains("--pingbin was removed"));
}
