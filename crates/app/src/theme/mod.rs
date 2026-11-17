//! Theme tokens, fonts, egui visuals and small painted parts.

mod fonts;
pub mod motion;
pub mod parts;

pub use fonts::{W, font, install_fonts};

use eframe::egui::{self, Color32, Shadow, Stroke, Visuals};
use model::{Accent, DevStage};

/// Color tokens. Names match design/src/build.py THEMES.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub base: Color32,
    pub surface: Color32,
    pub overlay: Color32,
    pub muted: Color32,
    pub subtle: Color32,
    pub text: Color32,
    pub love: Color32,
    pub gold: Color32,
    pub rose: Color32,
    pub pine: Color32,
    pub foam: Color32,
    pub iris: Color32,
    pub hl_low: Color32,
    pub hl_med: Color32,
    pub hl_high: Color32,
    /// Accent of the active workspace. Selection, focus, primary buttons and the rail
    /// marker use it, so each workspace has its own color.
    pub accent_color: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

impl Theme {
    pub const MAIN: Theme = Theme {
        dark: true,
        base: hex(0x191724),
        surface: hex(0x1f1d2e),
        overlay: hex(0x26233a),
        muted: hex(0x6e6a86),
        subtle: hex(0x908caa),
        text: hex(0xe0def4),
        love: hex(0xeb6f92),
        gold: hex(0xf6c177),
        rose: hex(0xebbcba),
        pine: hex(0x31748f),
        foam: hex(0x9ccfd8),
        iris: hex(0xc4a7e7),
        hl_low: hex(0x21202e),
        hl_med: hex(0x403d52),
        hl_high: hex(0x524f67),
        accent_color: hex(0xc4a7e7),
    };

    pub const MOON: Theme = Theme {
        dark: true,
        base: hex(0x232136),
        surface: hex(0x2a273f),
        overlay: hex(0x393552),
        muted: hex(0x6e6a86),
        subtle: hex(0x908caa),
        text: hex(0xe0def4),
        love: hex(0xeb6f92),
        gold: hex(0xf6c177),
        rose: hex(0xea9a97),
        pine: hex(0x3e8fb0),
        foam: hex(0x9ccfd8),
        iris: hex(0xc4a7e7),
        hl_low: hex(0x2a283e),
        hl_med: hex(0x44415a),
        hl_high: hex(0x56526e),
        accent_color: hex(0xc4a7e7),
    };

    pub const DAWN: Theme = Theme {
        dark: false,
        base: hex(0xfaf4ed),
        surface: hex(0xfffaf3),
        overlay: hex(0xf2e9e1),
        muted: hex(0x9893a5),
        subtle: hex(0x797593),
        text: hex(0x575279),
        love: hex(0xb4637a),
        gold: hex(0xea9d34),
        rose: hex(0xd7827e),
        pine: hex(0x286983),
        foam: hex(0x56949f),
        iris: hex(0x907aa9),
        hl_low: hex(0xf4ede8),
        hl_med: hex(0xdfdad9),
        hl_high: hex(0xcecacd),
        accent_color: hex(0x907aa9),
    };

    /// Catppuccin Mocha, from catppuccin.com. Mantle is the board, base the wells, surface0 the cards.
    pub const CATPPUCCIN_MOCHA: Theme = Theme {
        dark: true,
        base: hex(0x181825),
        surface: hex(0x1e1e2e),
        overlay: hex(0x313244),
        muted: hex(0x6c7086),
        subtle: hex(0x9399b2),
        text: hex(0xcdd6f4),
        love: hex(0xf38ba8),
        gold: hex(0xf9e2af),
        rose: hex(0xf2cdcd),
        pine: hex(0x89b4fa),
        foam: hex(0x94e2d5),
        iris: hex(0xcba6f7),
        hl_low: hex(0x232334),
        hl_med: hex(0x45475a),
        hl_high: hex(0x585b70),
        accent_color: hex(0xcba6f7),
    };

    /// Catppuccin Latte, the light flavor.
    pub const CATPPUCCIN_LATTE: Theme = Theme {
        dark: false,
        base: hex(0xe6e9ef),
        surface: hex(0xeff1f5),
        overlay: hex(0xdce0e8),
        // Overlay1, not overlay0. Overlay0 is too faint as date text on the cards.
        muted: hex(0x8c8fa1),
        subtle: hex(0x6c6f85),
        text: hex(0x4c4f69),
        love: hex(0xd20f39),
        gold: hex(0xdf8e1d),
        rose: hex(0xdd7878),
        pine: hex(0x1e66f5),
        foam: hex(0x179299),
        iris: hex(0x8839ef),
        hl_low: hex(0xe3e6ed),
        hl_med: hex(0xccd0da),
        hl_high: hex(0xbcc0cc),
        accent_color: hex(0x8839ef),
    };

    /// Tokyo Night (night variant).
    pub const TOKYO_NIGHT: Theme = Theme {
        dark: true,
        base: hex(0x16161e),
        surface: hex(0x1a1b26),
        overlay: hex(0x24283b),
        muted: hex(0x565f89),
        subtle: hex(0x737aa2),
        text: hex(0xc0caf5),
        love: hex(0xf7768e),
        gold: hex(0xe0af68),
        rose: hex(0xff9e64),
        pine: hex(0x7aa2f7),
        foam: hex(0x7dcfff),
        iris: hex(0xbb9af7),
        hl_low: hex(0x1f2335),
        hl_med: hex(0x3b4261),
        hl_high: hex(0x545c7e),
        accent_color: hex(0xbb9af7),
    };

    /// Gruvbox dark, hard contrast.
    pub const GRUVBOX_DARK: Theme = Theme {
        dark: true,
        base: hex(0x1d2021),
        surface: hex(0x282828),
        overlay: hex(0x32302f),
        muted: hex(0x7c6f64),
        subtle: hex(0xa89984),
        text: hex(0xebdbb2),
        love: hex(0xfb4934),
        gold: hex(0xfabd2f),
        rose: hex(0xfe8019),
        pine: hex(0x83a598),
        foam: hex(0x8ec07c),
        iris: hex(0xd3869b),
        hl_low: hex(0x242424),
        hl_med: hex(0x504945),
        hl_high: hex(0x665c54),
        accent_color: hex(0xd3869b),
    };

    /// Nord. Polar Night for the surfaces, Frost and Aurora for the accents.
    pub const NORD: Theme = Theme {
        dark: true,
        base: hex(0x2e3440),
        surface: hex(0x3b4252),
        overlay: hex(0x434c5e),
        // Brightened nord3. A darker gray does not read as date text on the cards.
        muted: hex(0x7b88a1),
        subtle: hex(0xa3abb9),
        text: hex(0xe5e9f0),
        love: hex(0xbf616a),
        gold: hex(0xebcb8b),
        rose: hex(0xd08770),
        pine: hex(0x5e81ac),
        foam: hex(0x88c0d0),
        iris: hex(0xb48ead),
        hl_low: hex(0x353b49),
        hl_med: hex(0x4c566a),
        hl_high: hex(0x5a657c),
        accent_color: hex(0xb48ead),
    };

    /// Dracula.
    pub const DRACULA: Theme = Theme {
        dark: true,
        base: hex(0x21222c),
        surface: hex(0x282a36),
        overlay: hex(0x343746),
        muted: hex(0x6272a4),
        subtle: hex(0xa0a6c4),
        text: hex(0xf8f8f2),
        love: hex(0xff5555),
        gold: hex(0xf1fa8c),
        rose: hex(0xff79c6),
        pine: hex(0x50fa7b),
        foam: hex(0x8be9fd),
        iris: hex(0xbd93f9),
        hl_low: hex(0x2a2c38),
        hl_med: hex(0x44475a),
        hl_high: hex(0x565a70),
        accent_color: hex(0xbd93f9),
    };

    pub fn by_name(name: &str) -> Option<Theme> {
        match name {
            "main" => Some(Theme::MAIN),
            "moon" => Some(Theme::MOON),
            "dawn" => Some(Theme::DAWN),
            "catppuccin-mocha" => Some(Theme::CATPPUCCIN_MOCHA),
            "catppuccin-latte" => Some(Theme::CATPPUCCIN_LATTE),
            "tokyo-night" => Some(Theme::TOKYO_NIGHT),
            "gruvbox-dark" => Some(Theme::GRUVBOX_DARK),
            "nord" => Some(Theme::NORD),
            "dracula" => Some(Theme::DRACULA),
            _ => None,
        }
    }

    pub fn accent(&self, a: Accent) -> Color32 {
        match a {
            Accent::Iris => self.iris,
            Accent::Foam => self.foam,
            Accent::Gold => self.gold,
            Accent::Rose => self.rose,
            Accent::Pine => self.pine,
            Accent::Love => self.love,
        }
    }

    /// The same theme with the accent of a workspace.
    pub fn with_accent(mut self, a: Accent) -> Theme {
        self.accent_color = self.accent(a);
        self
    }

    /// A color at low opacity, for tinted fills behind text in the same color.
    pub fn tint(c: Color32, alpha: f32) -> Color32 {
        c.gamma_multiply(alpha)
    }

    /// Text color for a tag of this color on a card. A dark pine reads as foam.
    /// Other colors that are too faint move toward the text color until they read.
    pub fn tag_text(&self, c: Color32) -> Color32 {
        let bg = self.overlay;
        if self.dark && c == self.pine && contrast(c, bg) < TAG_CONTRAST {
            return self.foam;
        }
        self.readable(c, bg)
    }

    /// `c`, or `c` mixed toward the text color until it has `TAG_CONTRAST` on `bg`.
    pub fn readable(&self, c: Color32, bg: Color32) -> Color32 {
        let mut out = c;
        let mut k = 0.0;
        while contrast(out, bg) < TAG_CONTRAST && k < 1.0 {
            k += 0.1;
            out = c.lerp_to_gamma(self.text, k);
        }
        out
    }

    pub fn stage(&self, s: DevStage) -> Color32 {
        match s {
            DevStage::Local => self.gold,
            DevStage::Mr => self.rose,
            DevStage::Merged | DevStage::Staging => self.iris,
            DevStage::Prod => self.foam,
        }
    }

    /// Text color that reads well on a filled accent: base or text, whichever has more contrast.
    /// Light themes have a dark text and a light base, so this works for both kinds.
    pub fn on_accent(&self, fill: Color32) -> Color32 {
        if contrast(self.base, fill) >= contrast(self.text, fill) { self.base } else { self.text }
    }

    /// Maps a Linear hex color to the closest accent token.
    /// The match runs against the main palette, so a label keeps its token in every theme.
    pub fn nearest(&self, hex_color: &str) -> Color32 {
        let Some(c) = parse_hex(hex_color) else {
            return self.subtle;
        };
        let m = Theme::MAIN;
        let pairs = [
            (m.love, self.love),
            (m.gold, self.gold),
            (m.rose, self.rose),
            (m.pine, self.pine),
            (m.foam, self.foam),
            (m.iris, self.iris),
            (m.subtle, self.subtle),
            (m.muted, self.muted),
        ];
        let dist = |a: Color32| {
            let d = |x: u8, y: u8| (x as i32 - y as i32).pow(2);
            d(a.r(), c.r()) + d(a.g(), c.g()) + d(a.b(), c.b())
        };
        pairs
            .iter()
            .min_by_key(|(k, _)| dist(*k))
            .map(|(_, v)| *v)
            .unwrap_or(self.subtle)
    }

    /// Black at `alpha` for shadows. Light themes get 40 percent of it, because a full
    /// black shadow on a light page reads as a dirty gray band.
    pub fn shade(&self, alpha: u8) -> Color32 {
        let a = if self.dark { alpha } else { (alpha as f32 * 0.4) as u8 };
        Color32::from_black_alpha(a)
    }

    pub fn shadow(&self) -> Shadow {
        Shadow { offset: [0, 12], blur: 32, spread: 0, color: self.shade(128) }
    }

    pub fn shadow_sm(&self) -> Shadow {
        Shadow { offset: [0, 6], blur: 16, spread: 0, color: self.shade(89) }
    }

    pub fn visuals(&self) -> Visuals {
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        v.dark_mode = self.dark;
        v.override_text_color = Some(self.text);
        v.panel_fill = self.base;
        v.window_fill = self.surface;
        v.window_stroke = Stroke::new(1.0, self.hl_med);
        v.extreme_bg_color = self.base;
        v.faint_bg_color = self.hl_low;
        v.code_bg_color = self.overlay;
        v.hyperlink_color = self.accent_color;
        v.warn_fg_color = self.gold;
        v.error_fg_color = self.love;
        v.selection.bg_fill = self.accent_color.gamma_multiply(0.35);
        v.selection.stroke = Stroke::new(1.0, self.accent_color);
        v.text_cursor.stroke = Stroke::new(2.0, self.accent_color);
        v.window_shadow = self.shadow();
        v.popup_shadow = self.shadow_sm();

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.base;
        w.noninteractive.weak_bg_fill = self.base;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.hl_low);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text);
        w.inactive.bg_fill = self.overlay;
        w.inactive.weak_bg_fill = self.overlay;
        w.inactive.bg_stroke = Stroke::new(1.0, self.hl_med);
        w.inactive.fg_stroke = Stroke::new(1.0, self.subtle);
        w.hovered.bg_fill = self.hl_med;
        w.hovered.weak_bg_fill = self.hl_med;
        w.hovered.bg_stroke = Stroke::new(1.0, self.hl_high);
        w.hovered.fg_stroke = Stroke::new(1.0, self.text);
        w.active.bg_fill = self.hl_high;
        w.active.weak_bg_fill = self.hl_high;
        w.active.bg_stroke = Stroke::new(1.0, self.accent_color);
        w.active.fg_stroke = Stroke::new(1.0, self.text);
        w.open = w.active;
        v
    }
}

/// Contrast that tag text needs on its card. A bit under WCAG AA, like the approved design.
const TAG_CONTRAST: f32 = 3.5;

/// WCAG relative luminance: 0 for black, 1 for white.
fn luminance(c: Color32) -> f32 {
    let ch = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
}

/// WCAG contrast ratio of two opaque colors, from 1 to 21.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(hex(v))
}

/// Sets fonts and visuals once at startup.
pub fn install(ctx: &egui::Context, t: &Theme) {
    install_fonts(ctx);
    ctx.set_visuals(t.visuals());
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.interaction.tooltip_delay = 0.2;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_maps_linear_colors_to_tokens() {
        let t = Theme::MAIN;
        assert_eq!(t.nearest("#eb6f92"), t.love);
        assert_eq!(t.nearest("#f2c94c"), t.gold);
        assert_eq!(t.nearest("bad"), t.subtle);
        assert_eq!(Theme::DAWN.nearest("#c4a7e7"), Theme::DAWN.iris);
    }

    #[test]
    fn accent_text_reads_in_every_theme() {
        for (name, _, t) in crate::settings::THEMES {
            for a in [Accent::Iris, Accent::Foam, Accent::Gold, Accent::Rose, Accent::Pine, Accent::Love] {
                let fill = t.accent(a);
                // Rose in the two light palettes is a soft mid tone. Neither ink gets much over 2.5 on it.
                assert!(contrast(t.on_accent(fill), fill) >= 2.4, "{name} {a:?} button text");
                assert!(contrast(t.tag_text(fill), t.overlay) >= TAG_CONTRAST, "{name} {a:?} tag text");
            }
            assert!(contrast(t.text, t.overlay) >= 4.5, "{name} card text");
            assert_eq!(t.dark, luminance(t.base) < 0.2, "{name} dark flag");
        }
    }

    #[test]
    fn on_accent_keeps_the_rose_pine_choices() {
        let m = Theme::MAIN;
        assert_eq!(m.on_accent(m.iris), m.base);
        assert_eq!(m.on_accent(m.pine), m.text);
        assert_eq!(m.tag_text(m.pine), m.foam);
        assert_eq!(Theme::DAWN.on_accent(Theme::DAWN.iris), Theme::DAWN.base);
    }
}
