use crate::app::AppEvent;
use crate::render::palette::Palette;
use crate::render::{
    format_global_stats_line_terminal, format_recent_stats_line, trim_ansi_to_width, trim_to_width,
    RenderConfig,
};
use crate::stats::Stats;

const ESC_SAVE_POS: &str = "\x1b7";
const ESC_RESTORE_POS: &str = "\x1b8";
const ESC_ERASE_LINE: &str = "\x1b[2K";
const ESC_CURSOR_UP: &str = "\x1b[A";

#[derive(Debug, Clone)]
pub struct TerminalRenderer {
    config: RenderConfig,
    palette: Palette,
    stats: Stats,
    out: String,
    curr_col: usize,
    width: usize,
    reserved_lines: usize,
}

impl TerminalRenderer {
    #[must_use]
    pub fn new(config: RenderConfig) -> Self {
        let palette = Palette::from_flags(
            config.unicode,
            config.color,
            config.multicolor,
            config.rttmin,
            config.rttmax,
        );

        let width = config.columns.map(usize::from).unwrap_or(80).max(1);

        let mut out = String::new();
        if config.legend {
            out.push_str(&trim_ansi_to_width(&palette.legend_line_painted(), width));
            out.push('\n');
        }

        let reserved_lines =
            usize::from(config.globalstats) + usize::from(config.recentstats && config.last > 0);

        let last = config.last;

        Self {
            config,
            palette,
            stats: Stats::new(last),
            out,
            curr_col: 0,
            width,
            reserved_lines,
        }
    }

    pub fn update_size(&mut self, columns: Option<u16>, lines: Option<u16>) {
        if let Some(columns) = columns {
            self.width = usize::from(columns).max(1);
            self.config.columns = Some(columns);
        }

        if let Some(lines) = lines {
            self.config.lines = Some(lines);
        }
    }

    pub fn render_event(&mut self, event: &AppEvent) {
        self.stats.apply(event);

        let Some(symbol) = self.event_symbol(event) else {
            return;
        };

        if self.curr_col >= self.width.saturating_sub(1) {
            self.new_graph_line();
        }

        if self.curr_col == 0 {
            self.reserve_stats_lines();
        }

        self.out.push_str(&symbol);
        self.curr_col = self.curr_col.saturating_add(1);
        self.render_stats_overlay();
    }

    pub fn finish(&mut self) {
        if self.curr_col > 0 {
            self.out.push('\n');

            // When stats overlay lines are reserved below the graph line,
            // one newline lands on the first overlay line. Move below the
            // full overlay so the shell prompt appears on a clean line.
            for _ in 0..self.reserved_lines {
                self.out.push('\n');
            }

            self.curr_col = 0;
        }
    }

    #[must_use]
    pub fn output(&self) -> &str {
        &self.out
    }

    pub fn output_mut(&mut self) -> &mut String {
        &mut self.out
    }

    #[must_use]
    pub fn into_output(self) -> String {
        self.out
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

    fn new_graph_line(&mut self) {
        self.out.push('\n');
        self.curr_col = 0;
    }

    fn reserve_stats_lines(&mut self) {
        if self.reserved_lines == 0 {
            return;
        }

        for _ in 0..self.reserved_lines {
            self.out.push_str("\x1b[0m\n");
        }
        for _ in 0..self.reserved_lines {
            self.out.push_str(ESC_CURSOR_UP);
        }
        self.out.push_str(ESC_ERASE_LINE);
    }

    fn render_stats_overlay(&mut self) {
        if self.reserved_lines == 0 {
            return;
        }

        self.out.push_str(ESC_SAVE_POS);

        if self.config.globalstats {
            self.out.push('\n');
            self.out.push_str(ESC_ERASE_LINE);
            let line = format_global_stats_line_terminal(&self.stats.global_snapshot());
            self.out.push_str(&trim_to_width(&line, self.width));
        }

        if self.config.recentstats && self.config.last > 0 {
            self.out.push('\n');
            self.out.push_str(ESC_ERASE_LINE);
            let line = format_recent_stats_line(&self.stats.recent_snapshot());
            self.out.push_str(&trim_to_width(&line, self.width));
        }

        self.out.push_str(ESC_RESTORE_POS);
    }
}
