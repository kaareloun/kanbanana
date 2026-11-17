//! Small painted parts the panel sections share.

use std::sync::Arc;

use eframe::egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Galley, Layout, Rect, Response, Sense, Stroke,
    TextBuffer, Ui, pos2, vec2,
    text::{LayoutJob, TextFormat},
};

use super::anim;
use super::icons::{self, Ic};
use crate::theme::parts;
use crate::theme::{Theme, W, font};

/// Section title (SemiBold 14) on the left, `right` content on the right edge.
pub fn section_head(ui: &mut Ui, t: &Theme, title: &str, right: impl FnOnce(&mut Ui)) -> Rect {
    let w = ui.available_width();
    let p = ui.painter().clone();
    let s = parts::text_size(&p, title, W::SemiBold, 14.0);
    let r = ui
        .allocate_ui_with_layout(vec2(w, 28.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_size(vec2(w, 28.0));
            ui.allocate_exact_size(vec2(s.x, 1.0), Sense::hover());
            ui.with_layout(Layout::right_to_left(Align::Center), right);
        })
        .response
        .rect;
    // Painted after the right side, so the title centers on the final row height.
    parts::text(&p, r.left_center(), Align2::LEFT_CENTER, title, W::SemiBold, 14.0, t.text);
    r
}

/// Labeled header button with an icon on an overlay fill (design v3 `head_btn`).
pub fn head_button(ui: &mut Ui, t: &Theme, ic: Ic, label: &str) -> Response {
    let p = ui.painter().clone();
    let ts = parts::text_size(&p, label, W::SemiBold, 12.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(20.0 + 14.0 + 6.0 + ts.x, 28.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let h = anim::hover(ui.ctx(), resp.id, resp.hovered());
    let (fill, c) = (anim::mix(t.overlay, t.hl_med, h), anim::mix(t.subtle, t.text, h));
    p.rect_filled(rect, CornerRadius::same(8), fill);
    icons::paint(&p, pos2(rect.min.x + 17.0, rect.center().y), ic, 14.0, c);
    parts::text(&p, pos2(rect.min.x + 30.0, rect.center().y), Align2::LEFT_CENTER, label, W::SemiBold, 12.0, c);
    resp
}

/// Plain text item.
pub fn text(ui: &mut Ui, s: &str, w: W, size: f32, c: Color32) -> Response {
    let p = ui.painter().clone();
    let sz = parts::text_size(&p, s, w, size);
    let (r, resp) = ui.allocate_exact_size(sz, Sense::hover());
    parts::text(&p, r.left_center(), Align2::LEFT_CENTER, s, w, size, c);
    resp
}

/// Wrapped text with a fixed line height, as wide as the ui.
pub fn wrapped(ui: &mut Ui, s: &str, w: W, size: f32, line_h: f32, c: Color32) -> Response {
    let width = ui.available_width();
    let g = galley(ui, s, w, size, line_h, c, width);
    let (r, resp) = ui.allocate_exact_size(vec2(width, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, c);
    resp
}

pub fn galley(ui: &Ui, s: &str, w: W, size: f32, line_h: f32, c: Color32, width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(
        s.to_string(),
        TextFormat { font_id: font(w, size), color: c, line_height: Some(line_h), ..Default::default() },
    );
    job.wrap.max_width = width;
    ui.painter().layout_job(job)
}

/// Icon plus text link, like the comment actions. Muted until hovered.
pub fn icon_link(ui: &mut Ui, t: &Theme, ic: Ic, label: &str) -> Response {
    let p = ui.painter().clone();
    let ts = parts::text_size(&p, label, W::Medium, 12.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(18.0 + ts.x, ts.y.max(14.0)), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let c = anim::mix(t.muted, t.text, anim::hover(ui.ctx(), resp.id, resp.hovered()));
    icons::paint(&p, pos2(rect.min.x + 7.0, rect.center().y), ic, 14.0, c);
    parts::text(&p, pos2(rect.min.x + 18.0, rect.center().y), Align2::LEFT_CENTER, label, W::Medium, 12.0, c);
    resp
}

/// 28 px square icon button.
pub fn icon_button(ui: &mut Ui, t: &Theme, ic: Ic, c: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let h = anim::hover(ui.ctx(), resp.id, resp.hovered());
    let p = ui.painter();
    if h > 0.0 {
        p.rect_filled(rect, CornerRadius::same(6), Theme::tint(t.overlay, h));
    }
    icons::paint(p, rect.center(), ic, 16.0, anim::mix(c, t.text, h));
    resp
}

pub fn kbd(ui: &mut Ui, t: &Theme, s: &str) -> Response {
    let p = ui.painter().clone();
    let (r, resp) = ui.allocate_exact_size(parts::kbd_size(&p, s), Sense::hover());
    parts::kbd(&p, t, r.left_center(), s);
    resp
}

/// Progress bar. `None` draws a moving segment for uploads without progress reports.
pub fn progress(ui: &mut Ui, t: &Theme, width: f32, height: f32, fraction: Option<f32>) {
    let (r, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let p = ui.painter();
    let cr = CornerRadius::same(2);
    p.rect_filled(r, cr, t.hl_med);
    let fill = match fraction {
        Some(f) => Rect::from_min_size(r.min, vec2(r.width() * f.clamp(0.0, 1.0), height)),
        None => {
            let time = ui.input(|i| i.time) as f32;
            let seg = r.width() * 0.3;
            let x = (time * 0.8).fract() * (r.width() + seg) - seg;
            ui.ctx().request_repaint();
            Rect::from_min_size(pos2(r.min.x + x, r.min.y), vec2(seg, height))
        }
    };
    p.rect_filled(fill.intersect(r), cr, t.foam);
}

pub fn upload_label(fraction: Option<f32>) -> String {
    match fraction {
        Some(f) if f >= 1.0 => "Uploaded".into(),
        Some(f) => format!("Uploading {}%", (f * 100.0).round() as i32),
        None => "Uploading".into(),
    }
}

/// Text box frame for editors: base fill, accent stroke when focused.
pub fn editor_frame(t: &Theme, focused: bool, pad: (i8, i8, i8, i8), radius: u8) -> eframe::egui::Frame {
    let (top, right, bottom, left) = pad;
    eframe::egui::Frame::NONE
        .fill(t.base)
        .stroke(Stroke::new(1.0, if focused { t.accent_color } else { t.hl_med }))
        .corner_radius(CornerRadius::same(radius))
        .inner_margin(eframe::egui::Margin { left, right, top, bottom })
}

/// Lays out editor text with one font, color and line height.
pub fn plain_layouter(w: W, size: f32, line_h: f32, c: Color32) -> impl FnMut(&Ui, &dyn TextBuffer, f32) -> Arc<Galley> {
    move |ui: &Ui, buf: &dyn TextBuffer, wrap: f32| {
        let mut job = LayoutJob::single_section(
            buf.as_str().to_string(),
            TextFormat { font_id: font(w, size), color: c, line_height: Some(line_h), ..Default::default() },
        );
        job.wrap.max_width = wrap;
        ui.fonts_mut(|f| f.layout_job(job))
    }
}

pub fn hint(t: &Theme, s: &str) -> eframe::egui::RichText {
    eframe::egui::RichText::new(s).font(font(W::Regular, 13.0)).color(t.muted)
}
