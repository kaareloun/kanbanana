//! Kanbanana brand art: the banana and the app icon, plus the small banana moments.

use std::f32::consts::TAU;

use eframe::egui::{
    self, Color32, Context, Id, LayerId, Mesh, Order, Painter, Pos2, Rect, TextureFilter, TextureId,
    TextureOptions, emath::{Rot2, easing}, load::TexturePoll, pos2, vec2,
};

use crate::theme::motion;

pub const ICON_PNG: &[u8] = include_bytes!("../../../../assets/brand/icon.png");
const BANANA_PNG: &[u8] = include_bytes!("../../../../assets/brand/banana.png");
const BANANA_URI: &str = "bytes://kanbanana/banana.png";
const ICON_URI: &str = "bytes://kanbanana/icon.png";

/// Length of the done pop, in seconds.
const POP_SECS: f32 = 0.45;
const POP_SIZE: f32 = 32.0;
/// Length of the rail mark wiggle, in seconds.
const WIGGLE_SECS: f32 = 0.3;

/// The art is 256 and 512 px. Mipmaps keep it smooth at 20 to 72 px.
fn texture(ctx: &Context, uri: &'static str, bytes: &'static [u8], size: egui::Vec2) -> Option<TextureId> {
    let opts = TextureOptions::LINEAR.with_mipmap_mode(Some(TextureFilter::Linear));
    let img = egui::Image::from_bytes(uri, bytes).texture_options(opts);
    match img.load_for_size(ctx, size) {
        Ok(TexturePoll::Ready { texture }) => Some(texture.id),
        _ => None,
    }
}

/// Starts the decode of both images, so the first frames that show them have them.
pub fn preload(ctx: &Context) {
    texture(ctx, BANANA_URI, BANANA_PNG, vec2(64.0, 64.0));
    texture(ctx, ICON_URI, ICON_PNG, vec2(72.0, 72.0));
}

/// The banana in `rect`, turned by `angle` radians around its center. Returns false until it has loaded.
pub fn banana(p: &Painter, rect: Rect, angle: f32, alpha: f32) -> bool {
    let Some(tex) = texture(p.ctx(), BANANA_URI, BANANA_PNG, rect.size()) else { return false };
    let mut mesh = Mesh::with_texture(tex);
    // The art has a wide transparent margin. This square fits the banana, so `rect` is the banana size.
    let uv = Rect::from_min_max(pos2(35.0, 34.0), pos2(220.0, 219.0)) / 256.0;
    mesh.add_rect_with_uv(rect, uv, Color32::WHITE.gamma_multiply(alpha));
    if angle != 0.0 {
        mesh.rotate(Rot2::from_angle(angle), rect.center());
    }
    p.add(mesh);
    true
}

/// The app icon. The art has its own rounded corners.
pub fn icon(p: &Painter, rect: Rect) {
    if let Some(tex) = texture(p.ctx(), ICON_URI, ICON_PNG, rect.size()) {
        p.image(tex, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

#[derive(Clone, Copy, Default)]
struct Wiggle {
    hovered: bool,
    start: f64,
}

/// Angle of the rail mark. A new hover starts one short wiggle that dies out.
pub fn wiggle(ctx: &Context, id: Id, hovered: bool) -> f32 {
    let now = motion::now(ctx);
    let mut w = ctx.data(|d| d.get_temp::<Wiggle>(id)).unwrap_or(Wiggle { hovered: false, start: f64::NEG_INFINITY });
    if hovered && !w.hovered {
        w.start = now;
    }
    w.hovered = hovered;
    ctx.data_mut(|d| d.insert_temp(id, w));
    let p = ((now - w.start) as f32 / WIGGLE_SECS).clamp(0.0, 1.0);
    if p >= 1.0 {
        return 0.0;
    }
    ctx.request_repaint();
    8f32.to_radians() * (1.0 - p) * (p * 2.0 * TAU).sin()
}

#[derive(Clone, Default)]
struct Pops(Vec<(Pos2, f64)>);

fn pops_id() -> Id {
    Id::unique("lb-banana-pops")
}

/// Starts a banana pop at `at`, e.g. when a card lands in a completed state.
pub fn pop(ctx: &Context, at: Pos2) {
    let now = motion::now(ctx);
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Pops>(pops_id()).0.push((at, now)));
    ctx.request_repaint();
}

/// Paints the live pops over everything. They only paint, so they never take input.
pub fn paint_pops(ctx: &Context) {
    let now = motion::now(ctx);
    let mut pops = ctx.data(|d| d.get_temp::<Pops>(pops_id())).unwrap_or_default();
    if pops.0.is_empty() {
        return;
    }
    pops.0.retain(|(_, start)| now - start < f64::from(POP_SECS));
    let p = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("lb-banana-pop")));
    for (at, start) in &pops.0 {
        let k = ((now - start) as f32 / POP_SECS).clamp(0.0, 1.0);
        let e = easing::cubic_out(k);
        let size = POP_SIZE * motion::lerp(0.6, 1.0, easing::cubic_out((k * 2.0).min(1.0)));
        let c = *at - vec2(0.0, 24.0 * e);
        let alpha = 1.0 - easing::cubic_in(k);
        banana(&p, Rect::from_center_size(c, vec2(size, size)), -0.2 * e, alpha);
    }
    if !pops.0.is_empty() {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(pops_id(), pops));
}
