#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolColor {
    Default,
    Green,
    Yellow,
    Red,
    YellowOnGreen,
    RedOnYellow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteItem {
    pub ch: char,
    pub color: SymbolColor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    items: Vec<PaletteItem>,
    pub rtt_min: u32,
    pub rtt_max: u32,
}

impl Palette {
    #[must_use]
    pub fn from_flags(
        unicode: bool,
        color: bool,
        multicolor: bool,
        rttmin: Option<u32>,
        rttmax: Option<u32>,
    ) -> Self {
        let multi = color && multicolor;

        let (mut items, mut rtt_min, mut rtt_max) = if unicode {
            if multi {
                (unicode_multicolor_items(), 10, 230)
            } else {
                (unicode_simple_items(color), 25, 175)
            }
        } else if multi {
            (ascii_multicolor_items(), 20, 220)
        } else {
            (ascii_simple_items(color), 75, 225)
        };

        if let (Some(min), Some(max)) = (rttmin, rttmax) {
            rtt_min = min;
            rtt_max = max;
        } else if let Some(min) = rttmin {
            rtt_min = min;
            let span = u32::try_from(items.len().saturating_sub(1)).unwrap_or(u32::MAX);
            rtt_max = min.saturating_mul(span.max(1));
        } else if let Some(max) = rttmax {
            rtt_max = max;
            let span = u32::try_from(items.len().saturating_sub(1))
                .unwrap_or(1)
                .max(1);
            rtt_min = max / span;
        }

        if rtt_max <= rtt_min {
            rtt_max = rtt_min.saturating_add(1);
        }

        if !color {
            for item in &mut items {
                item.color = SymbolColor::Default;
            }
        }

        Self {
            items,
            rtt_min,
            rtt_max,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[must_use]
    pub fn item_for_rtt(&self, rtt_ms: u32) -> PaletteItem {
        if rtt_ms < self.rtt_min {
            return self.items[0];
        }
        if rtt_ms >= self.rtt_max {
            return *self.items.last().unwrap_or(&self.items[0]);
        }

        let len = self.items.len();
        if len <= 2 {
            return self.items[0];
        }

        let numerator = u64::from(rtt_ms.saturating_sub(self.rtt_min));
        let range = u64::from(self.rtt_max.saturating_sub(self.rtt_min)).max(1);
        let bins = u64::try_from(len.saturating_sub(2)).unwrap_or(0);
        let idx = 1 + usize::try_from((numerator.saturating_mul(bins)) / range).unwrap_or(0);
        self.items[idx.min(len - 1)]
    }

    #[must_use]
    pub fn legend_line(&self) -> String {
        if self.items.len() <= 1 {
            return String::new();
        }

        let mut out = String::new();
        out.push_str(&format!("0 {}", self.items[0].ch));

        let len = self.items.len();
        for index in 1..len {
            let lower_bound = self.rtt_min.saturating_add(
                (u64::from(index as u32 - 1).saturating_mul(u64::from(self.rtt_range()))
                    / u64::from((len as u32).saturating_sub(2).max(1))) as u32,
            );
            out.push(' ');
            out.push_str(&format!("{} {}", lower_bound, self.items[index].ch));
        }

        out.push_str(" inf");
        out
    }

    #[must_use]
    pub fn legend_line_painted(&self) -> String {
        if self.items.len() <= 1 {
            return String::new();
        }

        let mut out = String::new();
        out.push_str("0 ");
        out.push_str(&self.paint(self.items[0]));

        let len = self.items.len();
        for index in 1..len {
            let lower_bound = self.rtt_min.saturating_add(
                (u64::from(index as u32 - 1).saturating_mul(u64::from(self.rtt_range()))
                    / u64::from((len as u32).saturating_sub(2).max(1))) as u32,
            );
            out.push(' ');
            out.push_str(&lower_bound.to_string());
            out.push(' ');
            out.push_str(&self.paint(self.items[index]));
        }

        out.push_str(" inf");
        out
    }

    #[must_use]
    pub fn paint(&self, item: PaletteItem) -> String {
        match item.color {
            SymbolColor::Default => item.ch.to_string(),
            SymbolColor::Green => format!("\x1b[0;32m{}\x1b[0m", item.ch),
            SymbolColor::Yellow => format!("\x1b[0;33m{}\x1b[0m", item.ch),
            SymbolColor::Red => format!("\x1b[0;31m{}\x1b[0m", item.ch),
            SymbolColor::YellowOnGreen => format!("\x1b[42;33m{}\x1b[0m", item.ch),
            SymbolColor::RedOnYellow => format!("\x1b[43;31m{}\x1b[0m", item.ch),
        }
    }

    #[must_use]
    pub fn rtt_range(&self) -> u32 {
        self.rtt_max.saturating_sub(self.rtt_min).max(1)
    }
}

fn unicode_multicolor_items() -> Vec<PaletteItem> {
    let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut items = Vec::with_capacity(24);

    for ch in chars {
        items.push(PaletteItem {
            ch,
            color: SymbolColor::Green,
        });
    }
    for ch in chars {
        items.push(PaletteItem {
            ch,
            color: SymbolColor::YellowOnGreen,
        });
    }
    for ch in chars {
        items.push(PaletteItem {
            ch,
            color: SymbolColor::RedOnYellow,
        });
    }

    items
}

fn unicode_simple_items(color: bool) -> Vec<PaletteItem> {
    let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    chars
        .into_iter()
        .map(|ch| PaletteItem {
            ch,
            color: if color {
                SymbolColor::Green
            } else {
                SymbolColor::Default
            },
        })
        .collect()
}

fn ascii_multicolor_items() -> Vec<PaletteItem> {
    let greens = ['_', '.', 'o', 'O'];
    let yellows = ['_', '.', 'o', 'O'];
    let reds = ['_', '.', 'o', 'O'];

    let mut items = Vec::with_capacity(12);
    for ch in greens {
        items.push(PaletteItem {
            ch,
            color: SymbolColor::Green,
        });
    }
    for ch in yellows {
        items.push(PaletteItem {
            ch,
            color: SymbolColor::Yellow,
        });
    }
    for ch in reds {
        items.push(PaletteItem {
            ch,
            color: SymbolColor::Red,
        });
    }

    items
}

fn ascii_simple_items(color: bool) -> Vec<PaletteItem> {
    ['_', '.', 'o', 'O']
        .into_iter()
        .map(|ch| PaletteItem {
            ch,
            color: if color {
                SymbolColor::Green
            } else {
                SymbolColor::Default
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Palette;

    #[test]
    fn ascii_simple_defaults_match_expected_ranges() {
        let palette = Palette::from_flags(false, false, false, None, None);
        assert_eq!(palette.len(), 4);
        assert_eq!(palette.rtt_min, 75);
        assert_eq!(palette.rtt_max, 225);

        assert_eq!(palette.item_for_rtt(10).ch, '_');
        assert_eq!(palette.item_for_rtt(90).ch, '.');
        assert_eq!(palette.item_for_rtt(170).ch, 'o');
        assert_eq!(palette.item_for_rtt(260).ch, 'O');
    }

    #[test]
    fn explicit_rtt_bounds_override_defaults() {
        let palette = Palette::from_flags(true, true, true, Some(50), Some(100));
        assert_eq!(palette.rtt_min, 50);
        assert_eq!(palette.rtt_max, 100);
    }
}
