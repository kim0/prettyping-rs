use std::ffi::OsString;

use clap::error::ErrorKind;
use clap::{ArgAction, CommandFactory, Parser};

use crate::config::{self, Config, ConfigInput, DEFAULT_LAST};

const CLI_AFTER_HELP: &str = "Compatibility notes:\n  - Removed legacy flags: --awkbin, --pingbin (hard error).\n  - Unsupported legacy flags: -f, -R, -q, -a (hard error).\n  - Legacy -v is accepted and ignored for compatibility.";

const CLI_HELP_TEXT: &str = "Usage: prettyping-rs [prettyping parameters] <host>\n\nThis is a Rust port of prettyping. It prints each ping response as a compact\ncharacter graph, with optional color and stats lines.\n\nprettyping parameters:\n  --[no]color        Enable/disable color output. (default: enabled)\n  --[no]multicolor   Enable/disable multi-color output. Has no effect if\n                       color or unicode is disabled. (default: enabled)\n  --[no]unicode      Enable/disable unicode characters. (default: enabled)\n  --[no]legend       Enable/disable the latency legend. (default: enabled)\n  --[no]globalstats  Enable/disable the global statistics line. (default: enabled)\n  --[no]recentstats  Enable/disable the last n statistics line. (default: enabled)\n  --[no]terminal     Force the output designed for a terminal. (default: auto)\n  --last <n>         Use the last n pings at the statistics line. (default: 60)\n  --columns <n>      Override auto-detection of terminal dimensions.\n  --lines <n>        Override auto-detection of terminal dimensions.\n  --rttmin <n>       Minimum RTT represented in the graph. (default: auto)\n  --rttmax <n>       Maximum RTT represented in the graph. (default: auto)\n\nping parameters handled by prettyping-rs:\n  -4, --ipv4         Use IPv4 only.\n  -6, --ipv6         Use IPv6 only.\n  -c, --count <n>    Stop after sending n probes.\n  -i, --interval <s> Interval between probes in seconds.\n  -W, --timeout <s>  Per-probe timeout in seconds.\n  -s, --size <n>     Payload size in bytes.\n  -t, --ttl <n>      IP time-to-live (hop limit).\n\n";

#[derive(Debug, Parser)]
#[command(
    name = "prettyping-rs",
    about = "Rust port of prettyping",
    after_help = CLI_AFTER_HELP,
    disable_version_flag = true
)]
struct RawCliArgs {
    /// Destination host or IP
    #[arg(value_name = "HOST")]
    host: String,

    // Kept prettyping flags
    #[arg(long = "color", action = ArgAction::SetTrue)]
    color: bool,
    #[arg(long = "nocolor", action = ArgAction::SetTrue)]
    nocolor: bool,
    #[arg(long = "multicolor", action = ArgAction::SetTrue)]
    multicolor: bool,
    #[arg(long = "nomulticolor", action = ArgAction::SetTrue)]
    nomulticolor: bool,
    #[arg(long = "unicode", action = ArgAction::SetTrue)]
    unicode: bool,
    #[arg(long = "nounicode", action = ArgAction::SetTrue)]
    nounicode: bool,
    #[arg(long = "legend", action = ArgAction::SetTrue)]
    legend: bool,
    #[arg(long = "nolegend", action = ArgAction::SetTrue)]
    nolegend: bool,
    #[arg(long = "globalstats", action = ArgAction::SetTrue)]
    globalstats: bool,
    #[arg(long = "noglobalstats", action = ArgAction::SetTrue)]
    noglobalstats: bool,
    #[arg(long = "recentstats", action = ArgAction::SetTrue)]
    recentstats: bool,
    #[arg(long = "norecentstats", action = ArgAction::SetTrue)]
    norecentstats: bool,
    #[arg(long = "terminal", action = ArgAction::SetTrue)]
    terminal: bool,
    #[arg(long = "noterminal", action = ArgAction::SetTrue)]
    noterminal: bool,
    #[arg(long = "last", default_value_t = DEFAULT_LAST)]
    last: u32,
    #[arg(long = "columns")]
    columns: Option<u16>,
    #[arg(long = "lines")]
    lines: Option<u16>,
    #[arg(long = "rttmin")]
    rttmin: Option<u32>,
    #[arg(long = "rttmax")]
    rttmax: Option<u32>,

    // Native ping flags (explicitly handled in Rust)
    #[arg(short = '4', long = "ipv4", action = ArgAction::SetTrue)]
    ipv4: bool,
    #[arg(short = '6', long = "ipv6", action = ArgAction::SetTrue)]
    ipv6: bool,
    #[arg(short = 'c', long = "count")]
    count: Option<u32>,
    #[arg(short = 'i', long = "interval")]
    interval_secs: Option<f64>,
    #[arg(short = 'W', long = "timeout")]
    timeout_secs: Option<f64>,
    #[arg(short = 's', long = "size")]
    packet_size: Option<u32>,
    #[arg(short = 't', long = "ttl")]
    ttl: Option<u16>,

    // Removed legacy passthrough flags - parsed only to emit explicit errors.
    #[arg(long = "awkbin", value_name = "EXEC", hide = true)]
    removed_awkbin: Option<String>,
    #[arg(long = "pingbin", value_name = "EXEC", hide = true)]
    removed_pingbin: Option<String>,

    // Unsupported legacy flags - parsed only to emit explicit errors.
    #[arg(short = 'a', action = ArgAction::SetTrue, hide = true)]
    legacy_audible: bool,
    #[arg(short = 'f', action = ArgAction::SetTrue, hide = true)]
    legacy_flood: bool,
    #[arg(short = 'R', action = ArgAction::SetTrue, hide = true)]
    legacy_record_route: bool,
    #[arg(short = 'q', action = ArgAction::SetTrue, hide = true)]
    legacy_quiet: bool,

    // Accepted legacy compatibility no-op.
    #[arg(short = 'v', action = ArgAction::SetTrue, hide = true)]
    legacy_verbose_ignored: bool,
}

pub fn parse_config_from_args<I, T>(args: I) -> Result<Config, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let normalized = normalize_legacy_long_spellings(args);
    if wants_help(&normalized) {
        return Err(help_error());
    }

    let raw = RawCliArgs::try_parse_from(normalized)?;

    if raw.removed_awkbin.is_some() {
        return Err(usage_error(
            "--awkbin was removed in prettyping-rs (pure Rust engine has no awk subprocess)",
        ));
    }
    if raw.removed_pingbin.is_some() {
        return Err(usage_error(
            "--pingbin was removed in prettyping-rs (pure Rust engine has no ping subprocess)",
        ));
    }

    if raw.legacy_audible {
        return Err(usage_error("unsupported legacy flag: -a"));
    }
    if raw.legacy_flood {
        return Err(usage_error("unsupported legacy flag: -f"));
    }
    if raw.legacy_record_route {
        return Err(usage_error("unsupported legacy flag: -R"));
    }
    if raw.legacy_quiet {
        return Err(usage_error("unsupported legacy flag: -q"));
    }

    let _ = raw.legacy_verbose_ignored;

    let input = ConfigInput {
        host: raw.host,
        color: !raw.nocolor || raw.color,
        multicolor: !raw.nomulticolor || raw.multicolor,
        unicode: !raw.nounicode || raw.unicode,
        legend: !raw.nolegend || raw.legend,
        globalstats: !raw.noglobalstats || raw.globalstats,
        recentstats: !raw.norecentstats || raw.recentstats,
        terminal: if raw.terminal {
            Some(true)
        } else if raw.noterminal {
            Some(false)
        } else {
            None
        },
        last: raw.last,
        columns: raw.columns,
        lines: raw.lines,
        rttmin: raw.rttmin,
        rttmax: raw.rttmax,
        force_ipv4: raw.ipv4,
        force_ipv6: raw.ipv6,
        count: raw.count,
        interval_secs: raw.interval_secs,
        timeout_secs: raw.timeout_secs,
        packet_size: raw.packet_size,
        ttl: raw.ttl,
    };

    config::normalize(input).map_err(|err| usage_error(err.to_string()))
}

pub fn parse_config_from_env() -> Result<Config, clap::Error> {
    parse_config_from_args(std::env::args_os())
}

fn usage_error(message: impl Into<String>) -> clap::Error {
    let mut cmd = RawCliArgs::command();
    cmd.error(ErrorKind::ValueValidation, message.into())
}

fn wants_help(args: &[OsString]) -> bool {
    args.iter()
        .any(|arg| arg == "--help" || arg == "-h" || arg == "-help" || arg == "--h")
}

fn help_error() -> clap::Error {
    let mut out = String::new();
    out.push_str(CLI_HELP_TEXT);
    out.push_str(CLI_AFTER_HELP);
    clap::Error::raw(ErrorKind::DisplayHelp, out)
}

fn normalize_legacy_long_spellings<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    args.into_iter()
        .map(Into::into)
        .map(|arg| {
            arg.to_str()
                .and_then(rewrite_legacy_single_dash_flag)
                .map(OsString::from)
                .unwrap_or(arg)
        })
        .collect()
}

fn rewrite_legacy_single_dash_flag(flag: &str) -> Option<&'static str> {
    match flag {
        "-help" => Some("--help"),
        "-color" => Some("--color"),
        "-nocolor" => Some("--nocolor"),
        "-multicolor" => Some("--multicolor"),
        "-nomulticolor" => Some("--nomulticolor"),
        "-unicode" => Some("--unicode"),
        "-nounicode" => Some("--nounicode"),
        "-legend" => Some("--legend"),
        "-nolegend" => Some("--nolegend"),
        "-globalstats" => Some("--globalstats"),
        "-noglobalstats" => Some("--noglobalstats"),
        "-recentstats" => Some("--recentstats"),
        "-norecentstats" => Some("--norecentstats"),
        "-terminal" => Some("--terminal"),
        "-noterminal" => Some("--noterminal"),
        "-last" => Some("--last"),
        "-columns" => Some("--columns"),
        "-lines" => Some("--lines"),
        "-rttmin" => Some("--rttmin"),
        "-rttmax" => Some("--rttmax"),
        "-awkbin" => Some("--awkbin"),
        "-pingbin" => Some("--pingbin"),
        _ => None,
    }
}
