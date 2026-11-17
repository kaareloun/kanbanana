//! Panel icons. Shared ones come from the theme; the rest follow design/src/build.py
//! PATHS, in a 16 px box.

use std::f32::consts::FRAC_PI_2;

use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, vec2};

use crate::theme::parts;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ic {
    Copy,
    External,
    Pencil,
    Reply,
    Branch,
    Mr,
    Image,
    Plus,
    X,
    Check,
    Asterisk,
}

pub fn paint(p: &Painter, center: Pos2, ic: Ic, size: f32, c: Color32) {
    let k = size / 16.0;
    let pt = |x: f32, y: f32| center + vec2((x - 8.0) * k, (y - 8.0) * k);
    let st = Stroke::new(1.5, c);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| pt(x, y)).collect(), st));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(Shape::closed_line(pts.iter().map(|&(x, y)| pt(x, y)).collect(), st));
    };
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| {
        p.add(Shape::line(parts::arc(pt(cx, cy), r * k, a0, a1, 12), st));
    };
    let theme_icon = |i: parts::Icon| parts::icon(p, center, i, size, c);
    match ic {
        Ic::Pencil => closed(&[(3.0, 13.0), (3.5, 10.5), (10.5, 3.5), (12.5, 5.5), (5.5, 12.5)]),
        Ic::Reply => {
            line(&[(6.0, 4.0), (2.5, 7.5), (6.0, 11.0)]);
            line(&[(2.5, 7.5), (9.5, 7.5)]);
            arc(9.5, 11.0, 3.5, -FRAC_PI_2, 0.0);
            line(&[(13.0, 11.0), (13.0, 12.5)]);
        }
        Ic::Copy => theme_icon(parts::Icon::Copy),
        Ic::External => theme_icon(parts::Icon::External),
        Ic::Branch => theme_icon(parts::Icon::Branch),
        Ic::Mr => theme_icon(parts::Icon::Mr),
        Ic::Asterisk => theme_icon(parts::Icon::Asterisk),
        Ic::Image => theme_icon(parts::Icon::Image),
        Ic::Plus => theme_icon(parts::Icon::Plus),
        Ic::X => theme_icon(parts::Icon::X),
        Ic::Check => theme_icon(parts::Icon::Check),
    }
}
