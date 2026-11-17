//! A small painted board in the colors of one theme, for the theme cards.

use eframe::egui::{Color32, CornerRadius, Painter, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::theme::Theme;

const RAIL_W: f32 = 26.0;

/// Paints the rail, three columns and a few cards of `th` into `rect`.
pub fn board(p: &Painter, th: &Theme, rect: Rect) {
    p.rect_filled(rect, CornerRadius::same(10), th.base);
    let p = &p.with_clip_rect(rect.intersect(p.clip_rect()));

    // Rail: banana, the active workspace tile and a second one.
    let rail = Rect::from_min_size(rect.min, vec2(RAIL_W, rect.height()));
    p.vline(rail.right() - 0.5, rail.y_range(), Stroke::new(1.0, th.hl_low));
    let cx = rail.center().x;
    crate::brand::banana(p, Rect::from_center_size(pos2(cx, rect.top() + 14.0), vec2(13.0, 13.0)), 0.0, 1.0);
    for (i, c) in [th.iris, th.foam].into_iter().enumerate() {
        let tile = Rect::from_center_size(pos2(cx, rect.top() + 36.0 + i as f32 * 22.0), vec2(15.0, 15.0));
        let stroke = if i == 0 { Stroke::new(1.0, c) } else { Stroke::NONE };
        p.rect(tile, CornerRadius::same(4), Theme::tint(c, 0.2), stroke, StrokeKind::Inside);
    }

    // Top bar: workspace name and the sync dot.
    let left = rail.right() + 9.0;
    let right = rect.right() - 9.0;
    bar(p, pos2(left, rect.top() + 12.0), 30.0, 4.0, th.text);
    p.circle_filled(pos2(right - 22.0, rect.top() + 14.0), 2.0, th.foam);
    bar(p, pos2(right - 17.0, rect.top() + 12.0), 17.0, 3.0, th.muted);

    let cols: [(Color32, &[(usize, Color32)]); 3] = [
        (th.subtle, &[(2, th.love), (1, th.iris)]),
        (th.gold, &[(2, th.iris)]),
        (th.foam, &[(1, th.gold), (2, th.foam)]),
    ];
    let gap = 6.0;
    let col_w = (right - left - 2.0 * gap) / 3.0;
    let top = rect.top() + 28.0;
    for (i, (color, cards)) in cols.iter().enumerate() {
        let x = left + i as f32 * (col_w + gap);
        p.circle_stroke(pos2(x + 3.5, top + 3.0), 2.5, Stroke::new(1.0, *color));
        bar(p, pos2(x + 9.0, top + 1.0), col_w * 0.4, 4.0, th.text);
        let rule = Rect::from_min_size(pos2(x, top + 10.0), vec2(col_w, 1.5));
        p.rect_filled(rule, CornerRadius::same(1), *color);
        let well = Rect::from_min_max(pos2(x, rule.bottom() + 5.0), pos2(x + col_w, rect.bottom() - 8.0));
        p.rect_filled(well, CornerRadius::same(5), th.surface);
        let mut y = well.top() + 4.0;
        for (lines, tag) in cards.iter() {
            y = card(p, th, pos2(well.left() + 4.0, y), col_w - 8.0, *lines, *tag) + 4.0;
        }
    }
}

/// One card: title lines and a tag. Returns its bottom.
fn card(p: &Painter, th: &Theme, at: eframe::egui::Pos2, w: f32, lines: usize, tag: Color32) -> f32 {
    let h = 9.0 + lines as f32 * 6.0 + 10.0;
    let r = Rect::from_min_size(at, vec2(w, h));
    p.rect(r, CornerRadius::same(4), th.overlay, Stroke::new(1.0, th.hl_med), StrokeKind::Inside);
    for l in 0..lines {
        let lw = if l + 1 == lines && lines > 1 { w * 0.45 } else { w - 12.0 };
        bar(p, pos2(r.left() + 6.0, r.top() + 6.0 + l as f32 * 6.0), lw, 3.0, th.text);
    }
    let tag_r = Rect::from_min_size(pos2(r.left() + 6.0, r.bottom() - 9.0), vec2(16.0, 5.0));
    p.rect_filled(tag_r, CornerRadius::same(2), Theme::tint(tag, 0.55));
    r.bottom()
}

/// A rounded line that stands for text.
fn bar(p: &Painter, at: eframe::egui::Pos2, w: f32, h: f32, c: Color32) {
    p.rect_filled(Rect::from_min_size(at, vec2(w, h)), CornerRadius::same(2), c.gamma_multiply(0.75));
}
