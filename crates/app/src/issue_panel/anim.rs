//! Panel motion: short ease-out animations. None of them block input.

use eframe::egui::{Color32, Context, Id, emath::easing};

/// Duration of slides, fades and pops in seconds. Tune all panel motion here.
pub const DUR: f32 = 0.14;
/// Hover fades run a bit shorter than moves.
pub const HOVER: f32 = 0.1;
/// The panel slides in from this far right.
pub const SLIDE: f32 = 40.0;
/// Content shift when the panel switches to another issue.
pub const SWAP: f32 = 8.0;

pub fn ease(t: f32) -> f32 {
    easing::cubic_out(t)
}

/// 0 to 1, follows `on` with a fade. Interruptible: a reverse starts from the current value.
pub fn toggle(ctx: &Context, id: Id, on: bool, secs: f32) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, secs, easing::cubic_out)
}

/// Hover amount of a widget, 0 to 1.
pub fn hover(ctx: &Context, id: Id, hovered: bool) -> f32 {
    toggle(ctx, id.with("lb-hover"), hovered, HOVER)
}

pub fn now(ctx: &Context) -> f64 {
    ctx.input(|i| i.time)
}

/// Eased progress of a one-shot animation that started at `start` (egui time).
pub fn since(ctx: &Context, start: f64) -> f32 {
    let t = ((now(ctx) - start) as f32 / DUR).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    ease(t)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t)
}
