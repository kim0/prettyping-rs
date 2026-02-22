use crate::app::AppEvent;
use crate::render::palette::Palette;
use crate::render::{
    format_global_stats_line_plain, format_recent_stats_line, trim_to_width, RenderConfig,
};
use crate::stats::Stats;

#[derive(Debug, Clone)]
pub struct PlainRenderer {
    config: RenderConfig,
    palette: Palette,
    stats: Stats,
    out: String,
    curr_col: usize,
}

impl PlainRenderer {
    #[must_use]
    pub fn new(config: RenderConfig) -> Self {
        let palette = Palette::from_flags(
            config.unicode,
            config.color,
            config.multicolor,
            config.rttmin,
            config.rttmax,
        );

        let mut out = String::new();
        if config.legend {
            out.push_str(&palette.legend_line());
            out.push('\n');
        }

        Self {
            stats: Stats::new(config.last),
            config,
            palette,
            out,
            curr_col: 0,
        }
    }

    pub fn render_event(&mut self, event: &AppEvent) {
        self.stats.apply(event);

        let Some(symbol) = self.event_symbol(event) else {
            return;
        };

        self.out.push_str(&symbol);
        self.curr_col = self.curr_col.saturating_add(1);

        let wrap_at = usize::try_from(self.config.last).unwrap_or(usize::MAX);
        if wrap_at > 0 && self.curr_col >= wrap_at {
            self.out.push('\n');
            self.curr_col = 0;
            self.append_stats_block();
        }
    }

    pub fn finish(&mut self) {
        if self.curr_col > 0 {
            self.out.push('\n');
            self.curr_col = 0;
        }
        self.append_stats_block();
    }

    #[must_use]
    pub fn output(&self) -> &str {
        &self.out
    }

    #[must_use]
    pub fn into_output(self) -> String {
        self.out
    }

    fn append_stats_block(&mut self) {
        if self.config.globalstats {
            let line = format_global_stats_line_plain(&self.stats.global_snapshot());
            self.out.push_str(&self.trim(line));
            self.out.push('\n');
        }

        if self.config.recentstats && self.config.last > 0 {
            let line = format_recent_stats_line(&self.stats.recent_snapshot());
            self.out.push_str(&self.trim(line));
            self.out.push('\n');
        }
    }

    fn event_symbol(&self, event: &AppEvent) -> Option<String> {
        match event {
            AppEvent::ProbeReply { rtt_ms, .. } => {
                let item = self
                    .palette
                    .item_for_rtt(u32::try_from(*rtt_ms).unwrap_or(u32::MAX));
                Some(self.palette.paint(item))
            }
            AppEvent::ProbeTimeout { .. } => {
                if self.config.color {
                    Some("\x1b[0;31m!\x1b[0m".to_owned())
                } else {
                    Some("!".to_owned())
                }
            }
            _ => None,
        }
    }

    fn trim(&self, line: String) -> String {
        self.config
            .columns
            .map(|width| trim_to_width(&line, usize::from(width)))
            .unwrap_or(line)
    }
}
