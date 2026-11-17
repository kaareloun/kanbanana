//! Short ease-out transitions. They never block input and turn back mid-way when the target changes.

use eframe::egui::{Context, Id, emath::easing};

/// Duration of every transition, in seconds.
pub const SNAP: f32 = 0.14;

/// 0 to 1 toward `on`, eased out.
pub fn toggle(ctx: &Context, id: Id, on: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, SNAP, easing::cubic_out)
}

#[derive(Clone, Copy)]
struct Slide {
    from: f32,
    to: f32,
    start: f64,
}

impl Slide {
    fn at(&self, now: f64) -> f32 {
        let p = ((now - self.start) as f32 / SNAP).clamp(0.0, 1.0);
        if p >= 1.0 { self.to } else { self.from + (self.to - self.from) * easing::cubic_out(p) }
    }
}

/// A value that slides to `target`, eased out. The first call jumps to it.
pub fn value(ctx: &Context, id: Id, target: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let old = ctx.data(|d| d.get_temp::<Slide>(id));
    let slide = match old {
        Some(s) if s.to == target => s,
        Some(s) => Slide { from: s.at(now), to: target, start: now },
        None => Slide { from: target, to: target, start: f64::NEG_INFINITY },
    };
    ctx.data_mut(|d| d.insert_temp(id, slide));
    let v = slide.at(now);
    if v != slide.to {
        ctx.request_repaint();
    }
    v
}

/// Eased progress from 0 to 1 of a transition that started at `start` (egui input time).
pub fn since(ctx: &Context, start: f64) -> f32 {
    let now = ctx.input(|i| i.time);
    let p = ((now - start) as f32 / SNAP).clamp(0.0, 1.0);
    if p < 1.0 {
        ctx.request_repaint();
    }
    easing::cubic_out(p)
}

pub fn now(ctx: &Context) -> f64 {
    ctx.input(|i| i.time)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
