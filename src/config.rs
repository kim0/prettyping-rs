use std::net::IpAddr;

use thiserror::Error;

pub const DEFAULT_LAST: u32 = 60;
const MAX_LAST: u32 = 10_000;
const MAX_COLUMNS: u16 = 10_000;
const MAX_LINES: u16 = 10_000;
const MAX_PACKET_SIZE: u32 = 65_507;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    Any,
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub host: String,
    pub color: bool,
    pub multicolor: bool,
    pub unicode: bool,
    pub legend: bool,
    pub globalstats: bool,
    pub recentstats: bool,
    pub terminal: Option<bool>,
    pub last: u32,
    pub columns: Option<u16>,
    pub lines: Option<u16>,
    pub rttmin: Option<u32>,
    pub rttmax: Option<u32>,
    pub family: AddressFamily,
    pub count: Option<u32>,
    pub interval_secs: Option<f64>,
    pub timeout_secs: Option<f64>,
    pub packet_size: Option<u32>,
    pub ttl: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct ConfigInput {
    pub host: String,
    pub color: bool,
    pub multicolor: bool,
    pub unicode: bool,
    pub legend: bool,
    pub globalstats: bool,
    pub recentstats: bool,
    pub terminal: Option<bool>,
    pub last: u32,
    pub columns: Option<u16>,
    pub lines: Option<u16>,
    pub rttmin: Option<u32>,
    pub rttmax: Option<u32>,
    pub force_ipv4: bool,
    pub force_ipv6: bool,
    pub count: Option<u32>,
    pub interval_secs: Option<f64>,
    pub timeout_secs: Option<f64>,
    pub packet_size: Option<u32>,
    pub ttl: Option<u16>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ConfigError {
    #[error("host must not be empty")]
    EmptyHost,
    #[error("--last must be between 0 and {MAX_LAST}")]
    LastOutOfRange,
    #[error("--columns must be between 1 and {MAX_COLUMNS}")]
    ColumnsOutOfRange,
    #[error("--lines must be between 1 and {MAX_LINES}")]
    LinesOutOfRange,
    #[error("--rttmin must be greater than 0")]
    RttMinOutOfRange,
    #[error("--rttmax must be greater than 0")]
    RttMaxOutOfRange,
    #[error("--rttmin must be strictly smaller than --rttmax")]
    InvalidRttRange,
    #[error("cannot use -4 and -6 together")]
    FamilyConflict,
    #[error("host address family conflicts with selected family flag")]
    HostFamilyConflict,
    #[error("-c/--count must be greater than 0")]
    CountOutOfRange,
    #[error("-i/--interval must be a finite value greater than 0")]
    IntervalOutOfRange,
    #[error("-W/--timeout must be a finite value greater than 0")]
    TimeoutOutOfRange,
    #[error("-s/--size must be at most {MAX_PACKET_SIZE}")]
    PacketSizeOutOfRange,
    #[error("-t/--ttl must be between 1 and 255")]
    TtlOutOfRange,
}

pub fn normalize(input: ConfigInput) -> Result<Config, ConfigError> {
    if input.host.trim().is_empty() {
        return Err(ConfigError::EmptyHost);
    }

    if input.last > MAX_LAST {
        return Err(ConfigError::LastOutOfRange);
    }

    if let Some(columns) = input.columns
        && (columns == 0 || columns > MAX_COLUMNS)
    {
        return Err(ConfigError::ColumnsOutOfRange);
    }

    if let Some(lines) = input.lines
        && (lines == 0 || lines > MAX_LINES)
    {
        return Err(ConfigError::LinesOutOfRange);
    }

    if let Some(rttmin) = input.rttmin
        && rttmin == 0
    {
        return Err(ConfigError::RttMinOutOfRange);
    }

    if let Some(rttmax) = input.rttmax
        && rttmax == 0
    {
        return Err(ConfigError::RttMaxOutOfRange);
    }

    if let (Some(rttmin), Some(rttmax)) = (input.rttmin, input.rttmax)
        && rttmin >= rttmax
    {
        return Err(ConfigError::InvalidRttRange);
    }

    let family = match (input.force_ipv4, input.force_ipv6) {
        (true, true) => return Err(ConfigError::FamilyConflict),
        (true, false) => AddressFamily::Ipv4,
        (false, true) => AddressFamily::Ipv6,
        (false, false) => AddressFamily::Any,
    };

    if let Ok(parsed_host) = input.host.parse::<IpAddr>() {
        match (family, parsed_host) {
            (AddressFamily::Ipv4, IpAddr::V6(_)) | (AddressFamily::Ipv6, IpAddr::V4(_)) => {
                return Err(ConfigError::HostFamilyConflict);
            }
            _ => {}
        }
    }

    if let Some(count) = input.count
        && count == 0
    {
        return Err(ConfigError::CountOutOfRange);
    }

    if let Some(interval_secs) = input.interval_secs
        && (!interval_secs.is_finite() || interval_secs <= 0.0)
    {
        return Err(ConfigError::IntervalOutOfRange);
    }

    if let Some(timeout_secs) = input.timeout_secs
        && (!timeout_secs.is_finite() || timeout_secs <= 0.0)
    {
        return Err(ConfigError::TimeoutOutOfRange);
    }

    if let Some(packet_size) = input.packet_size
        && packet_size > MAX_PACKET_SIZE
    {
        return Err(ConfigError::PacketSizeOutOfRange);
    }

    if let Some(ttl) = input.ttl
        && !(1..=255).contains(&ttl)
    {
        return Err(ConfigError::TtlOutOfRange);
    }

    Ok(Config {
        host: input.host,
        color: input.color,
        multicolor: input.multicolor,
        unicode: input.unicode,
        legend: input.legend,
        globalstats: input.globalstats,
        recentstats: input.recentstats,
        terminal: input.terminal,
        last: input.last,
        columns: input.columns,
        lines: input.lines,
        rttmin: input.rttmin,
        rttmax: input.rttmax,
        family,
        count: input.count,
        interval_secs: input.interval_secs,
        timeout_secs: input.timeout_secs,
        packet_size: input.packet_size,
        ttl: input.ttl,
    })
}
