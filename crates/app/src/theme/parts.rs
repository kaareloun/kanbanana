//! Small painted parts the views share: icons, avatar, kbd hint, label, status and
//! priority icons, checkbox, segmented control and buttons. No icon font.

use std::f32::consts::{FRAC_PI_2, TAU};

use eframe::egui::{
    Align2, Color32, CornerRadius, CursorIcon, Painter, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use model::Priority;

use super::{Theme, W, font};

pub fn text(p: &Painter, pos: Pos2, anchor: Align2, s: &str, w: W, size: f32, c: Color32) -> Rect {
    p.text(pos, anchor, s, font(w, size), c)
}

pub fn text_size(p: &Painter, s: &str, w: W, size: f32) -> Vec2 {
    p.layout_no_wrap(s.to_string(), font(w, size), Color32::PLACEHOLDER).size()
}

// ------------------------------------------------------------------ geometry

/// Points on an arc. Angles in radians, screen space (y down, so positive is clockwise).
pub fn arc(c: Pos2, r: f32, a0: f32, a1: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

pub fn dashed_circle(p: &Painter, c: Pos2, r: f32, stroke: Stroke, dash: f32, gap: f32) {
    let pts = arc(c, r, -FRAC_PI_2, -FRAC_PI_2 + TAU, 48);
    p.extend(Shape::dashed_line(&pts, stroke, dash, gap));
}

pub fn dashed_rect(p: &Painter, rect: Rect, r: f32, stroke: Stroke, dash: f32, gap: f32) {
    let rect = rect.shrink(stroke.width / 2.0);
    let mut pts = Vec::new();
    let q = FRAC_PI_2;
    pts.extend(arc(pos2(rect.max.x - r, rect.min.y + r), r, -q, 0.0, 6));
    pts.extend(arc(pos2(rect.max.x - r, rect.max.y - r), r, 0.0, q, 6));
    pts.extend(arc(pos2(rect.min.x + r, rect.max.y - r), r, q, 2.0 * q, 6));
    pts.extend(arc(pos2(rect.min.x + r, rect.min.y + r), r, 2.0 * q, 3.0 * q, 6));
    pts.push(pts[0]);
    p.extend(Shape::dashed_line(&pts, stroke, dash, gap));
}

/// A filled pie slice from 12 o'clock, clockwise. Split in quarters so each piece is convex.
fn pie(p: &Painter, c: Pos2, r: f32, sweep: f32, fill: Color32) {
    let mut a = -FRAC_PI_2;
    let end = -FRAC_PI_2 + sweep;
    while a < end - 0.001 {
        let b = (a + FRAC_PI_2).min(end);
        let mut pts = vec![c];
        pts.extend(arc(c, r, a, b, 8));
        p.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
        a = b;
    }
}

// ------------------------------------------------------------------ icons

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    #[allow(dead_code)]
    ChevronDown,
    ChevronRight,
    Plus,
    X,
    #[allow(dead_code)]
    Refresh,
    Image,
    Comment,
    Check,
    Key,
    Branch,
    Mr,
    Copy,
    External,
    Asterisk,
    Link,
}

/// Paints an icon from design/src/build.py PATHS. Coordinates are in a 16 px box.
pub fn icon(p: &Painter, center: Pos2, ic: Icon, size: f32, c: Color32) {
    let k = size / 16.0;
    let pt = |x: f32, y: f32| center + vec2((x - 8.0) * k, (y - 8.0) * k);
    let st = Stroke::new(1.5, c);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| pt(x, y)).collect(), st));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(Shape::closed_line(pts.iter().map(|&(x, y)| pt(x, y)).collect(), st));
    };
    match ic {
        Icon::ChevronDown => line(&[(4.0, 6.0), (8.0, 10.0), (12.0, 6.0)]),
        Icon::ChevronRight => line(&[(6.0, 4.0), (10.0, 8.0), (6.0, 12.0)]),
        Icon::Plus => {
            line(&[(8.0, 3.0), (8.0, 13.0)]);
            line(&[(3.0, 8.0), (13.0, 8.0)]);
        }
        Icon::X => {
            line(&[(4.0, 4.0), (12.0, 12.0)]);
            line(&[(12.0, 4.0), (4.0, 12.0)]);
        }
        Icon::Refresh => refresh_icon(p, center, size, c, 0.0),
        Icon::Image => {
            closed(&[(2.5, 3.5), (13.5, 3.5), (13.5, 12.5), (2.5, 12.5)]);
            line(&[(2.5, 11.0), (6.0, 7.5), (9.0, 10.5), (10.5, 9.0), (13.5, 12.0)]);
        }
        Icon::Comment => closed(&[
            (2.5, 3.5),
            (13.5, 3.5),
            (13.5, 11.0),
            (7.0, 11.0),
            (4.0, 13.5),
            (4.0, 11.0),
            (2.5, 11.0),
        ]),
        Icon::Check => line(&[(3.5, 8.5), (6.5, 11.5), (12.5, 4.5)]),
        Icon::Key => {
            p.circle_stroke(pt(4.9, 11.1), 3.0 * k, st);
            line(&[(7.0, 9.0), (13.5, 2.5)]);
            line(&[(11.5, 4.5), (13.0, 6.0)]);
        }
        Icon::Branch => {
            for (x, y) in [(5.0, 3.5), (5.0, 12.5), (11.0, 4.5)] {
                p.circle_stroke(pt(x, y), 1.5 * k, st);
            }
            line(&[(5.0, 5.0), (5.0, 11.0)]);
            p.add(Shape::line(arc(pt(6.5, 6.1), 4.5 * k, -0.025, 1.637, 12), st));
        }
        Icon::Mr => {
            for (x, y) in [(4.5, 3.5), (4.5, 12.5), (11.5, 12.5)] {
                p.circle_stroke(pt(x, y), 1.5 * k, st);
            }
            line(&[(4.5, 5.0), (4.5, 11.0)]);
            line(&[(11.5, 11.0), (11.5, 6.5)]);
            p.add(Shape::line(arc(pt(9.5, 6.5), 2.0 * k, 0.0, -FRAC_PI_2, 8), st));
            line(&[(9.5, 4.5), (7.0, 4.5)]);
            line(&[(8.5, 3.0), (7.0, 4.5), (8.5, 6.0)]);
        }
        Icon::Copy => {
            closed(&[(5.5, 5.5), (13.0, 5.5), (13.0, 13.0), (5.5, 13.0)]);
            line(&[(3.0, 10.5), (3.0, 3.0), (10.5, 3.0)]);
        }
        Icon::External => {
            line(&[(9.0, 3.0), (13.0, 3.0), (13.0, 7.0)]);
            line(&[(13.0, 3.0), (7.5, 8.5)]);
            line(&[(11.0, 10.0), (11.0, 13.0), (3.0, 13.0), (3.0, 5.0), (6.0, 5.0)]);
        }
        Icon::Asterisk => {
            line(&[(8.0, 2.5), (8.0, 13.5)]);
            line(&[(3.2, 5.25), (12.8, 10.75)]);
            line(&[(12.8, 5.25), (3.2, 10.75)]);
        }
        Icon::Link => {
            let q = std::f32::consts::FRAC_PI_4;
            line(&[(7.0, 9.0), (9.0, 7.0)]);
            let mut a = vec![pt(6.5, 6.0)];
            a.extend(arc(pt(6.75, 9.25), 2.5 * k, 5.0 * q, q, 12));
            a.push(pt(10.0, 9.5));
            p.add(Shape::line(a, st));
            let mut b = vec![pt(9.5, 10.0)];
            b.extend(arc(pt(9.25, 6.75), 2.5 * k, q, -3.0 * q, 12));
            b.push(pt(6.0, 6.5));
            p.add(Shape::line(b, st));
        }
    }
}

/// The refresh icon turned by `angle` radians around its center. The sync spinner turns it.
pub fn refresh_icon(p: &Painter, center: Pos2, size: f32, c: Color32, angle: f32) {
    let k = size / 16.0;
    let (sin, cos) = angle.sin_cos();
    let pt = |x: f32, y: f32| {
        let (dx, dy) = ((x - 8.0) * k, (y - 8.0) * k);
        center + vec2(dx * cos - dy * sin, dx * sin + dy * cos)
    };
    let st = Stroke::new(1.5, c);
    p.add(Shape::line(arc(center, 5.0 * k, angle, angle + 1.75 * std::f32::consts::PI, 24), st));
    p.add(Shape::line(vec![pt(12.0, 1.5), pt(12.0, 4.8), pt(8.7, 4.8)], st));
}

/// A hover or active ring that fades in. `amount` is 0 to 1 from `motion::toggle`.
pub fn ring(p: &Painter, c: Pos2, radius: f32, width: f32, color: Color32, amount: f32) {
    if amount > 0.0 {
        p.circle_stroke(c, radius, Stroke::new(width, color.gamma_multiply(amount)));
    }
}

// ------------------------------------------------------------------ status, priority

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Backlog,
    Todo,
    Progress,
    Review,
    Done,
    Canceled,
}

pub fn status_icon(p: &Painter, t: &Theme, c: Pos2, kind: StatusKind, size: f32) {
    let r = size / 2.0;
    let ring = |col: Color32| p.circle_stroke(c, r - 0.75, Stroke::new(1.5, col));
    let k = size / 14.0;
    let pt = |x: f32, y: f32| c + vec2((x - 7.0) * k, (y - 7.0) * k);
    match kind {
        StatusKind::Backlog => dashed_circle(p, c, r - 0.75, Stroke::new(1.5, t.muted), 2.0, 2.0),
        StatusKind::Todo => {
            ring(t.subtle);
        }
        StatusKind::Progress => {
            ring(t.gold);
            pie(p, c, r / 2.0, std::f32::consts::PI, t.gold);
        }
        StatusKind::Review => {
            ring(t.foam);
            pie(p, c, r / 2.0, 1.5 * std::f32::consts::PI, t.foam);
        }
        StatusKind::Done => {
            p.circle_filled(c, r, t.iris);
            let st = Stroke::new(1.6, t.on_accent(t.iris));
            p.add(Shape::line(vec![pt(4.0, 7.2), pt(6.2, 9.4), pt(10.0, 5.0)], st));
        }
        StatusKind::Canceled => {
            p.circle_filled(c, r, t.muted);
            let st = Stroke::new(1.6, t.on_accent(t.muted));
            p.line_segment([pt(4.8, 4.8), pt(9.2, 9.2)], st);
            p.line_segment([pt(9.2, 4.8), pt(4.8, 9.2)], st);
        }
    }
}

/// Paints a 14 px priority icon with its top left corner at `min`.
pub fn priority_icon(p: &Painter, t: &Theme, min: Pos2, pr: Priority) {
    let at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(min + vec2(x, y), vec2(w, h));
    match pr {
        Priority::Urgent => {
            let r = at(0.0, 0.0, 14.0, 14.0);
            p.rect_filled(r, CornerRadius::same(3), t.love);
            text(p, r.center(), Align2::CENTER_CENTER, "!", W::Bold, 10.0, t.on_accent(t.love));
        }
        Priority::None => {
            for x in [1.0, 5.5, 10.0] {
                p.rect_filled(at(x, 6.25, 3.0, 1.5), CornerRadius::same(1), t.muted);
            }
        }
        _ => {
            let n = match pr {
                Priority::High => 3,
                Priority::Medium => 2,
                _ => 1,
            };
            for (i, h) in [5.0, 8.0, 11.0].into_iter().enumerate() {
                let on = match pr {
                    Priority::High => t.gold,
                    Priority::Medium => t.foam,
                    _ => t.subtle,
                };
                let c = if i < n { on } else { t.hl_high };
                p.rect_filled(at(1.0 + i as f32 * 4.5, 12.5 - h, 3.0, h), CornerRadius::same(1), c);
            }
        }
    }
}

// ------------------------------------------------------------------ people, labels, kbd

/// Initials circle. `None` paints the dashed "no assignee" circle.
pub fn avatar(p: &Painter, t: &Theme, c: Pos2, size: f32, who: Option<(&str, Color32)>) {
    match who {
        None => dashed_circle(p, c, size / 2.0 - 0.6, Stroke::new(1.2, t.muted), 2.0, 2.0),
        Some((initials, fill)) => {
            p.circle_filled(c, size / 2.0, fill);
            let fs = (size * 0.4).round();
            text(p, c, Align2::CENTER_CENTER, initials, W::SemiBold, fs, t.on_accent(fill));
        }
    }
}

/// Avatar with the user's Linear photo when it has loaded, else the initials circle.
/// The image loader fetches the photo; until then this paints the initials.
pub fn avatar_photo(
    p: &Painter,
    ctx: &eframe::egui::Context,
    t: &Theme,
    c: Pos2,
    size: f32,
    url: Option<&str>,
    who: Option<(&str, Color32)>,
) {
    if let Some(url) = url.filter(|u| !u.is_empty()) {
        let poll = eframe::egui::Image::new(url).load_for_size(ctx, vec2(size, size));
        if let Ok(eframe::egui::load::TexturePoll::Ready { texture }) = poll {
            let rect = Rect::from_center_size(c, vec2(size, size));
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            let shape = eframe::egui::epaint::RectShape::filled(rect, CornerRadius::same((size / 2.0) as u8), Color32::WHITE)
                .with_texture(texture.id, uv);
            p.add(shape);
            return;
        }
    }
    avatar(p, t, c, size, who);
}

pub fn kbd_size(p: &Painter, s: &str) -> Vec2 {
    let ts = text_size(p, s, W::MonoMedium, 10.0);
    vec2(ts.x + 10.0, ts.y + 2.0)
}

/// Keyboard hint box. Returns its rect.
/// Name of the command key in shortcut hints. egui's `modifiers.command` is Cmd on macOS
/// and Ctrl elsewhere, so the hints follow the same rule.
pub const CMD: &str = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
/// Name of the Alt key in shortcut hints. macOS calls it Option.
pub const ALT: &str = if cfg!(target_os = "macos") { "Opt" } else { "Alt" };

pub fn kbd(p: &Painter, t: &Theme, left_center: Pos2, s: &str) -> Rect {
    let size = kbd_size(p, s);
    let r = Rect::from_min_size(left_center - vec2(0.0, size.y / 2.0), size);
    p.rect(r, CornerRadius::same(4), t.overlay, Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
    text(p, r.center(), Align2::CENTER_CENTER, s, W::MonoMedium, 10.0, t.subtle);
    r
}

/// Dot plus name. Returns the right edge x.
#[allow(dead_code)]
pub fn label(p: &Painter, t: &Theme, left_center: Pos2, name: &str, c: Color32) -> f32 {
    p.circle_filled(left_center + vec2(4.0, 0.0), 4.0, c);
    let r = text(p, left_center + vec2(13.0, 0.0), Align2::LEFT_CENTER, name, W::Medium, 11.0, t.subtle);
    r.max.x
}

/// Tinted tag: text in its color on a low-opacity fill of the same color, with an
/// optional icon. Returns its rect. Labels, dev stages and the panel all use it.
pub fn tag(p: &Painter, t: &Theme, left_center: Pos2, s: &str, c: Color32, ic: Option<Icon>, mono: bool) -> Rect {
    let size = tag_size(p, s, ic.is_some(), mono);
    let r = Rect::from_min_size(left_center - vec2(0.0, size.y / 2.0), size);
    p.rect_filled(r, CornerRadius::same(5), Theme::tint(c, 0.16));
    let tc = t.tag_text(c);
    let mut x = r.min.x + 8.0;
    if let Some(ic) = ic {
        icon(p, pos2(x + 6.0, r.center().y), ic, 12.0, tc);
        x += 17.0;
    }
    let w = if mono { W::MonoMedium } else { W::SemiBold };
    text(p, pos2(x, r.center().y), Align2::LEFT_CENTER, s, w, 11.0, tc);
    r
}

pub fn tag_size(p: &Painter, s: &str, has_icon: bool, mono: bool) -> Vec2 {
    let w = if mono { W::MonoMedium } else { W::SemiBold };
    let ts = text_size(p, s, w, 11.0);
    vec2(16.0 + ts.x + if has_icon { 17.0 } else { 0.0 }, 20.0)
}

#[allow(dead_code)]
pub fn label_width(p: &Painter, name: &str) -> f32 {
    13.0 + text_size(p, name, W::Medium, 11.0).x
}

pub fn checkbox(p: &Painter, t: &Theme, rect: Rect, on: bool) {
    if on {
        p.rect_filled(rect, CornerRadius::same(3), t.accent_color);
        let k = rect.width() / 14.0;
        let pt = |x: f32, y: f32| rect.min + vec2(x * k, y * k);
        p.add(Shape::line(vec![pt(3.5, 7.2), pt(6.0, 9.6), pt(10.5, 4.6)], Stroke::new(1.6, t.on_accent(t.accent_color))));
    } else {
        p.rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.5, t.hl_high), StrokeKind::Inside);
    }
}

// ------------------------------------------------------------------ interactive parts

/// Segmented control. Returns the index the user clicked.
pub fn segmented(ui: &mut Ui, t: &Theme, id: &str, options: &[&str], active: usize) -> Option<usize> {
    let p = ui.painter().clone();
    let widths: Vec<f32> = options
        .iter()
        .map(|o| text_size(&p, o, W::SemiBold, 12.0).x + 24.0)
        .collect();
    let item_h = text_size(&p, "Ag", W::SemiBold, 12.0).y + 10.0;
    let total = vec2(widths.iter().sum::<f32>() + 2.0 * (options.len() as f32 - 1.0) + 6.0, item_h + 6.0);
    let (rect, _) = ui.allocate_exact_size(total, Sense::hover());
    p.rect_filled(rect, CornerRadius::same(8), t.surface);
    let mut x = rect.min.x + 3.0;
    let mut clicked = None;
    for (i, (o, w)) in options.iter().zip(&widths).enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.min.y + 3.0), vec2(*w, item_h));
        let resp = ui
            .interact(r, ui.scope_id().with((id, i)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let on = i == active;
        if on {
            p.rect_filled(r, CornerRadius::same(5), Theme::tint(t.accent_color, 0.14));
        }
        let (wt, c) = if on {
            (W::SemiBold, t.accent_color)
        } else if resp.hovered() {
            (W::Medium, t.text)
        } else {
            (W::Medium, t.subtle)
        };
        text(&p, r.center(), Align2::CENTER_CENTER, o, wt, 12.0, c);
        if resp.clicked() && !on {
            clicked = Some(i);
        }
        x += w + 2.0;
    }
    clicked
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    Primary,
    Secondary,
    Ghost,
}

/// Button with a kbd hint after the text. A disabled button still returns clicks,
/// so the caller checks `enabled` too.
pub fn button(
    ui: &mut Ui,
    t: &Theme,
    label_text: &str,
    kind: Btn,
    hint: Option<&str>,
    enabled: bool,
) -> Response {
    let p = ui.painter().clone();
    let ts = text_size(&p, label_text, W::SemiBold, 12.0);
    let hs = hint.map(|h| text_size(&p, h, W::MonoMedium, 10.0));
    let w = 24.0 + ts.x + hs.map_or(0.0, |h| 6.0 + h.x);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, ts.y + 12.0), Sense::click());
    let resp = if enabled { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp };
    let hover = enabled && resp.hovered();
    let (fill, stroke, tc) = match kind {
        Btn::Primary => (t.accent_color, Stroke::NONE, t.on_accent(t.accent_color)),
        Btn::Secondary => (if hover { t.hl_med } else { t.overlay }, Stroke::new(1.0, t.hl_med), t.text),
        Btn::Ghost => (
            if hover { t.hl_low } else { Color32::TRANSPARENT },
            Stroke::NONE,
            if hover { t.text } else { t.subtle },
        ),
    };
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
    p.rect(rect, CornerRadius::same(6), fade(fill), stroke, StrokeKind::Inside);
    let left = pos2(rect.min.x + 12.0, rect.center().y);
    text(&p, left, Align2::LEFT_CENTER, label_text, W::SemiBold, 12.0, fade(tc));
    if let Some(h) = hint {
        let hc = if kind == Btn::Primary { t.on_accent(t.accent_color) } else { t.muted };
        let at = left + vec2(ts.x + 6.0, 0.0);
        text(&p, at, Align2::LEFT_CENTER, h, W::MonoMedium, 10.0, fade(hc.gamma_multiply(0.7)));
    }
    resp
}

/// Square icon button with a hover fill.
#[allow(dead_code)]
pub fn icon_button(ui: &mut Ui, t: &Theme, ic: Icon, c: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), t.overlay);
    }
    icon(p, rect.center(), ic, 16.0, if resp.hovered() { t.text } else { c });
    resp
}
