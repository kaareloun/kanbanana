//! Paints what the hidden marks stand for: bullets, numbers, checkboxes, quote bars,
//! code backgrounds and images. Positions come from the galley of the TextEdit.

use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Context, CornerRadius, CursorIcon, Galley, Id, Pos2, Rect, Sense, Shape, Stroke, StrokeKind,
    TextureOptions, Ui, Vec2,
    layers::ShapeIdx,
    load::{SizeHint, SizedTexture, TexturePoll},
    pos2,
    text::CCursor,
    vec2,
};

use super::Look;
use super::layout::{self, CHECK_INDENT, CODE_GAP, LIST_INDENT};
use super::parse::{Block, Image, Line, Mark};
use crate::theme::parts::{self, Icon};
use crate::theme::{Theme, W};

const IMG_PAD: f32 = 6.0;
const CAPTION_H: f32 = 22.0;
const MAX_IMG_H: f32 = 480.0;
const UPLOAD_H: f32 = 56.0;

/// What the editor knows about one image.
#[derive(Clone, Debug)]
pub enum Pic {
    Ready(SizedTexture),
    Loading,
    Failed,
    /// `![Uploading name...]()`, the upload still runs.
    Uploading,
}

pub fn pic(ctx: &Context, img: &Image) -> Pic {
    if img.url.is_empty() {
        return Pic::Uploading;
    }
    match ctx.try_load_texture(&img.url, TextureOptions::LINEAR, SizeHint::default()) {
        Ok(TexturePoll::Ready { texture }) => Pic::Ready(texture),
        Ok(TexturePoll::Pending { .. }) => {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            Pic::Loading
        }
        Err(_) => Pic::Failed,
    }
}

/// Size of the picture box at editor width `wrap`. Screenshots fill the width.
fn pic_size(p: &Pic, wrap: f32) -> Vec2 {
    match p {
        Pic::Ready(tex) => {
            let s = tex.size;
            if s.x <= 0.0 || s.y <= 0.0 {
                return vec2(wrap, 44.0);
            }
            let mut w = if s.x >= wrap * 0.5 { wrap } else { s.x.min(wrap) };
            let mut h = s.y * w / s.x;
            if h > MAX_IMG_H {
                w *= MAX_IMG_H / h;
                h = MAX_IMG_H;
            }
            vec2(w, h)
        }
        Pic::Loading => vec2(wrap, 120.0),
        Pic::Failed => vec2(wrap, 44.0),
        Pic::Uploading => vec2(wrap, UPLOAD_H),
    }
}

/// Row height of an image line: the picture, its caption and some space around.
pub fn image_line_h(pics: &HashMap<String, Pic>, img: &Image, wrap: f32) -> f32 {
    let p = pics.get(&img.url).cloned().unwrap_or(Pic::Loading);
    let caption = if matches!(p, Pic::Uploading) { 0.0 } else { CAPTION_H };
    IMG_PAD * 2.0 + pic_size(&p, wrap).y + caption
}

/// Clicks on the overlays.
#[derive(Default)]
pub struct Clicks {
    /// Index of a checklist line whose box was clicked.
    pub toggle: Option<usize>,
    pub open_url: Option<String>,
}

pub struct Ctx<'a> {
    pub t: &'a Theme,
    pub look: &'a Look,
    pub id: Id,
    pub text: &'a str,
    pub lines: &'a [Line],
    pub galley: &'a Galley,
    /// Screen position of the galley origin.
    pub origin: Pos2,
    pub width: f32,
    pub pics: &'a HashMap<String, Pic>,
    pub uploads: &'a [(String, Option<f32>)],
    /// Links open on click when the editor has no focus, and on Cmd+click when it has.
    pub links_live: bool,
}

impl Ctx<'_> {
    /// The row rect of byte offset `b`, on screen.
    fn row(&self, b: usize) -> Rect {
        let c = self.text[..b.min(self.text.len())].chars().count();
        self.galley.pos_from_cursor(CCursor::new(c)).translate(self.origin.to_vec2())
    }
}

/// Paints the backgrounds into `bg`, a slot below the text, and the rest on top.
pub fn paint(ui: &mut Ui, c: &Ctx, bg: ShapeIdx) -> Clicks {
    let mut below = Vec::new();
    let mut clicks = Clicks::default();
    let t = c.t;
    let p = ui.painter().clone();
    let mut block_start: Option<f32> = None;
    for (i, line) in c.lines.iter().enumerate() {
        let first = c.row(line.marker.end);
        let cy = first.center().y;
        let level = line.depth.saturating_sub(1) as f32 * LIST_INDENT;
        let x = c.origin.x + level;
        match line.block {
            Block::Bullet => {
                p.circle_filled(pos2(x + 9.0, cy), 2.5, t.subtle);
            }
            Block::Numbered(n) => {
                let b = layout::base(line.block, c.look, t);
                parts::text(&p, pos2(x + LIST_INDENT - 7.0, cy), Align2::RIGHT_CENTER, &format!("{n}."), W::Regular, b.size, t.subtle);
            }
            Block::Check(done) => {
                let r = Rect::from_center_size(pos2(x + CHECK_INDENT / 2.0 - 2.0, cy), vec2(15.0, 15.0));
                let resp = ui.interact(r.expand(3.0), c.id.with(("check", line.range.start)), Sense::click());
                parts::checkbox(&p, t, r, done);
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    clicks.toggle = Some(i);
                }
            }
            Block::Quote => {
                let last = c.row(line.range.end);
                let r = Rect::from_min_max(pos2(c.origin.x, first.min.y + 3.0), pos2(c.origin.x + 3.0, last.max.y - 3.0));
                p.rect_filled(r, CornerRadius::same(2), t.hl_high);
            }
            Block::Fence => match block_start.take() {
                None => block_start = Some(first.min.y),
                Some(top) => below.push(code_block(t, c, top, first.max.y)),
            },
            Block::Image => {
                if let Some(img) = &line.image {
                    image(ui, c, img, first.min.y);
                }
            }
            _ => {}
        }
        inline(ui, c, line, &mut below, &mut clicks);
    }
    if let (Some(top), Some(last)) = (block_start, c.lines.last()) {
        below.push(code_block(t, c, top, c.row(last.range.end).max.y));
    }
    ui.painter().set(bg, Shape::Vec(below));
    clicks
}

fn code_block(t: &Theme, c: &Ctx, top: f32, bottom: f32) -> Shape {
    let r = Rect::from_min_max(pos2(c.origin.x, top), pos2(c.origin.x + c.width, bottom));
    Shape::Rect(egui::epaint::RectShape::new(r, CornerRadius::same(8), t.overlay, Stroke::new(1.0, t.hl_med), StrokeKind::Inside))
}

/// Inline code backgrounds and link hit areas of one line.
fn inline(ui: &mut Ui, c: &Ctx, line: &Line, below: &mut Vec<Shape>, clicks: &mut Clicks) {
    for s in &line.spans {
        match &s.mark {
            Mark::Code if !s.range.is_empty() => {
                let (a, b) = (c.row(s.range.start), c.row(s.range.end));
                let b = if (a.min.y - b.min.y).abs() < 1.0 { b.min.x - CODE_GAP / 2.0 } else { c.origin.x + c.width };
                let size = layout::base(line.block, c.look, c.t).size;
                let cy = a.center().y;
                let r = Rect::from_min_max(pos2(a.min.x - CODE_GAP / 2.0, cy - size * 0.72), pos2(b, cy + size * 0.72));
                below.push(Shape::Rect(egui::epaint::RectShape::new(
                    r,
                    CornerRadius::same(4),
                    c.t.overlay,
                    Stroke::new(1.0, c.t.hl_med),
                    StrokeKind::Inside,
                )));
            }
            Mark::Link { url } if c.links_live => {
                let (a, b) = (c.row(s.range.start), c.row(s.range.end));
                let right = if (a.min.y - b.min.y).abs() < 1.0 { b.min.x } else { c.origin.x + c.width };
                let r = Rect::from_min_max(a.min, pos2(right, a.max.y));
                let resp = ui.interact(r, c.id.with(("link", s.range.start)), Sense::click());
                let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(&c.text[url.clone()]);
                if resp.clicked() {
                    clicks.open_url = Some(c.text[url.clone()].to_string());
                }
            }
            _ => {}
        }
    }
}

fn image(ui: &Ui, c: &Ctx, img: &Image, top: f32) {
    let t = c.t;
    let p = ui.painter();
    let pic = c.pics.get(&img.url).cloned().unwrap_or(Pic::Loading);
    let size = pic_size(&pic, c.width);
    let r = Rect::from_min_size(pos2(c.origin.x, top + IMG_PAD), size);
    match &pic {
        Pic::Ready(tex) => {
            egui::Image::from_texture(*tex).corner_radius(8).paint_at(ui, r);
            p.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
        }
        Pic::Uploading => {
            upload_box(ui, c, r, img);
            return;
        }
        Pic::Loading | Pic::Failed => {
            p.rect(r, CornerRadius::same(8), t.hl_low, Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
            let msg = if matches!(pic, Pic::Loading) { "Loading image" } else { "Image did not load" };
            parts::icon(p, r.left_center() + vec2(20.0, 0.0), Icon::Image, 14.0, t.subtle);
            parts::text(p, r.left_center() + vec2(36.0, 0.0), Align2::LEFT_CENTER, msg, W::Medium, 12.0, t.muted);
        }
    }
    let at = pos2(r.min.x, r.max.y + 5.0);
    parts::text(p, at, Align2::LEFT_TOP, &img.name, W::MonoMedium, 11.0, t.muted);
}

/// Placeholder of a running upload: icon, file name and a progress bar.
fn upload_box(ui: &Ui, c: &Ctx, r: Rect, img: &Image) {
    let t = c.t;
    let p = ui.painter();
    let name = img.name.trim_start_matches("Uploading ").trim_end_matches("...");
    let progress = c.uploads.iter().find(|(n, _)| n == name).and_then(|(_, f)| *f);
    p.rect(r, CornerRadius::same(8), t.overlay, Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
    let thumb = Rect::from_min_size(r.min + vec2(10.0, 10.0), vec2(36.0, r.height() - 20.0));
    p.rect_filled(thumb, CornerRadius::same(6), t.hl_med);
    parts::icon(p, thumb.center(), Icon::Image, 14.0, t.subtle);
    let x = thumb.right() + 12.0;
    parts::text(p, pos2(x, r.center().y - 8.0), Align2::LEFT_CENTER, name, W::MonoMedium, 12.0, t.text);
    let label = match progress {
        Some(f) => format!("Uploading {}%", (f * 100.0).round() as i32),
        None => "Uploading".to_string(),
    };
    let bar = Rect::from_min_size(pos2(x, r.center().y + 9.0), vec2(160.0_f32.min(r.right() - x - 90.0).max(40.0), 3.0));
    parts::text(p, pos2(bar.right() + 10.0, bar.center().y), Align2::LEFT_CENTER, &label, W::Medium, 11.0, t.muted);
    p.rect_filled(bar, CornerRadius::same(2), t.hl_med);
    let fill = match progress {
        Some(f) => Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), bar.height())),
        None => {
            let time = ui.input(|i| i.time) as f32;
            let seg = bar.width() * 0.3;
            ui.ctx().request_repaint();
            Rect::from_min_size(pos2(bar.min.x + (time * 0.8).fract() * (bar.width() + seg) - seg, bar.min.y), vec2(seg, 3.0))
        }
    };
    p.rect_filled(fill.intersect(bar), CornerRadius::same(2), t.accent_color);
}
