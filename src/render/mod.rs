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

/// Trims a string to a visible width, keeping ANSI escape sequences intact.
///
/// This is primarily needed for rendering the startup legend, which may contain
/// color escape codes (SGR). Escape sequences do not count towards the visible
/// width.
pub(crate) fn trim_ansi_to_width(input: &str, width: usize) -> String {
    if width == 0 || input.is_empty() {
        return String::new();
    }

    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut idx = 0usize;
    let mut visible = 0usize;
    let mut saw_escape = false;

    while idx < bytes.len() {
        if bytes[idx] == b'\x1b' {
            saw_escape = true;

            // CSI: ESC [ ... <final byte>
            if idx + 1 < bytes.len() && bytes[idx + 1] == b'[' {
                let mut j = idx + 2;
                while j < bytes.len() {
                    let b = bytes[j];
                    // Final byte is in the range 0x40..=0x7E.
                    if (0x40..=0x7E).contains(&b) {
                        j += 1;
                        break;
                    }
                    j += 1;
                }

                out.push_str(&input[idx..j.min(bytes.len())]);
                idx = j.min(bytes.len());
                continue;
            }

            // Non-CSI 2-byte sequences like ESC 7 / ESC 8.
            if idx + 1 < bytes.len() {
                out.push_str(&input[idx..idx + 2]);
                idx += 2;
                continue;
            }

            // Trailing ESC without payload.
            out.push('\x1b');
            break;
        }

        let ch = input[idx..].chars().next().unwrap();
        if visible >= width {
            break;
        }

        out.push(ch);
        visible += 1;
        idx += ch.len_utf8();
    }

    // If we truncated after emitting escape sequences, reset to avoid color bleed.
    if saw_escape && idx < bytes.len() {
        out.push_str("\x1b[0m");
    }

    out
}
