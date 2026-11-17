//! The theme picker: one card per theme, three per row, with a painted board preview.

use eframe::egui::{Align2, CornerRadius, CursorIcon, Id, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::preview;
use crate::actions::Action;
use crate::settings::THEMES;
use crate::theme::parts::{self, Icon};
use crate::theme::{Theme, W, motion};

const COLS: usize = 3;
const GAP: f32 = 14.0;
const INSET: f32 = 7.0;
const LABEL_H: f32 = 34.0;

/// Paints the grid at `x, y` in width `w`. `active` is the index in `THEMES`.
/// A click on another card pushes `SetTheme`. Returns the y under the grid.
#[allow(clippy::too_many_arguments)]
pub fn show(ui: &mut Ui, t: &Theme, x: f32, y: f32, w: f32, preview_h: f32, active: usize, salt: &str, actions: &mut Vec<Action>) -> f32 {
    let card_w = (w - (COLS as f32 - 1.0) * GAP) / COLS as f32;
    let card_h = INSET + preview_h + LABEL_H;
    let mut bottom = y;
    for (i, (name, label, th)) in THEMES.iter().enumerate() {
        let (row, col) = (i / COLS, i % COLS);
        let at = pos2(x + col as f32 * (card_w + GAP), y + row as f32 * (card_h + GAP));
        let card = Rect::from_min_size(at, vec2(card_w, card_h));
        let resp = ui.interact(card, Id::unique((salt, i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let on = active == i;
        let hover = motion::toggle(ui.ctx(), resp.id.with("hover"), resp.hovered());
        let lit = motion::toggle(ui.ctx(), resp.id.with("on"), on);
        let p = ui.painter();
        let edge = t.hl_low.lerp_to_gamma(t.hl_high, hover).lerp_to_gamma(t.accent_color, lit);
        p.rect(card, CornerRadius::same(12), t.surface, Stroke::new(1.0 + lit, edge), StrokeKind::Inside);
        let shot = Rect::from_min_size(card.min + vec2(INSET, INSET), vec2(card_w - 2.0 * INSET, preview_h));
        preview::board(p, th, shot);
        p.rect_stroke(shot, CornerRadius::same(10), Stroke::new(1.0, t.hl_low), StrokeKind::Inside);
        let mid = shot.bottom() + LABEL_H / 2.0;
        let tc = if on { t.text } else { t.subtle.lerp_to_gamma(t.text, hover) };
        parts::text(p, pos2(card.left() + 12.0, mid), Align2::LEFT_CENTER, label, W::SemiBold, 13.0, tc);
        let c = pos2(card.right() - 19.0, mid);
        if lit > 0.0 {
            p.circle_filled(c, 9.0, t.accent_color.gamma_multiply(lit));
            parts::icon(p, c, Icon::Check, 11.0, t.on_accent(t.accent_color).gamma_multiply(lit));
        }
        if resp.clicked() && !on {
            actions.push(Action::SetTheme(name.to_string()));
        }
        bottom = card.bottom();
    }
    bottom
}
