pub mod palette;
pub mod plain;
pub mod terminal;

use crate::config::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderConfig {
    pub color: bool,
    pub multicolor: bool,
    pub unicode: bool,
    pub legend: bool,
    pub globalstats: bool,
    pub recentstats: bool,
    pub last: u32,
    pub columns: Option<u16>,
    pub lines: Option<u16>,
    pub rttmin: Option<u32>,
    pub rttmax: Option<u32>,
}

impl From<&Config> for RenderConfig {
    fn from(value: &Config) -> Self {
        Self {
            color: value.color,
            multicolor: value.multicolor,
            unicode: value.unicode,
            legend: value.legend,
            globalstats: value.globalstats,
            recentstats: value.recentstats,
            last: value.last,
            columns: value.columns,
            lines: value.lines,
            rttmin: value.rttmin,
            rttmax: value.rttmax,
        }
    }
}

pub(crate) fn format_global_stats_line_terminal(
    snapshot: &crate::stats::GlobalStatsSnapshot,
) -> String {
    format!(
        "{:>2}/{:>3} ({:>2}%) lost; {:>4}/{:>4}/{:>4}ms; last: {:>4}ms",
        snapshot.loss.lost,
        snapshot.loss.total,
        snapshot.loss.percent,
        snapshot.rtt.min_ms,
        snapshot.rtt.avg_ms,
        snapshot.rtt.max_ms,
        snapshot.last_rtt_ms
    )
}

pub(crate) fn format_global_stats_line_plain(
    snapshot: &crate::stats::GlobalStatsSnapshot,
) -> String {
    format!(
        "{:>2}/{:>3} ({:>2}%) lost; {:>4}/{:>4}/{:>4}ms",
        snapshot.loss.lost,
        snapshot.loss.total,
        snapshot.loss.percent,
        snapshot.rtt.min_ms,
        snapshot.rtt.avg_ms,
        snapshot.rtt.max_ms
    )
}

pub(crate) fn format_recent_stats_line(snapshot: &crate::stats::RecentStatsSnapshot) -> String {
    format!(
        "{:>2}/{:>3} ({:>2}%) lost; {:>4}/{:>4}/{:>4}/{:>4}ms (last {})",
        snapshot.loss.lost,
        snapshot.loss.total,
        snapshot.loss.percent,
        snapshot.rtt.min_ms,
        snapshot.rtt.avg_ms,
        snapshot.rtt.max_ms,
        snapshot.rtt.mdev_ms,
        snapshot.rtt.count
    )
}

pub(crate) fn trim_to_width(line: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    line.chars().take(width).collect()
}
