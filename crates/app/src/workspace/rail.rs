//! Left rail: the banana mark, one tile per workspace, "Alt N" ("Opt N" on macOS) under each, "+" near the
//! bottom and the signed-in user's photo under it, which opens Settings. Without a known user the
//! photo is gone and "+" sits at the bottom.

use std::collections::HashMap;

use eframe::egui::{
    Align2, CornerRadius, CursorIcon, Id, LayerId, Order, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use model::{BoardData, User, Workspace};

use super::letter;
use crate::actions::Action;
use crate::theme::parts::{self, Icon};
use crate::theme::{Theme, W, motion};

pub const WIDTH: f32 = 72.0;
const TILE: f32 = 44.0;
const TOP: f32 = 18.0;
const AVATAR: f32 = 32.0;
const MARK: f32 = 28.0;

/// Where the app is, for the rail highlights.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RailMode {
    Board,
    Adding,
    Settings,
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    t: &Theme,
    workspaces: &[Workspace],
    boards: &HashMap<String, BoardData>,
    active: Option<&str>,
    me: Option<&User>,
    mode: RailMode,
    actions: &mut Vec<Action>,
) {
    let adding = mode == RailMode::Adding;
    let rect = ui.max_rect();
    let p = ui.painter().clone();
    p.vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0, t.hl_low));
    let cx = rect.center().x;
    let mut y = brand_mark(ui, t, pos2(cx, rect.top() + TOP));
    let mut marker = None;

    for ws in workspaces {
        let tile = Rect::from_min_size(pos2(cx - TILE / 2.0, y), vec2(TILE, TILE));
        let resp = ui
            .interact(tile, Id::unique(("lb-rail", &ws.slug)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let on = active == Some(ws.slug.as_str());
        let accent = t.accent(ws.accent);
        let hover = motion::toggle(ui.ctx(), Id::unique(("lb-rail-hover", &ws.slug)), resp.hovered());
        let lit = motion::toggle(ui.ctx(), Id::unique(("lb-rail-on", &ws.slug)), on);
        let alpha = motion::lerp(motion::lerp(0.08, 0.13, hover), 0.18, lit);
        let stroke = Stroke::new(1.5, accent.gamma_multiply(lit));
        p.rect(tile, CornerRadius::same(12), Theme::tint(accent, alpha), stroke, StrokeKind::Inside);
        parts::text(&p, tile.center(), Align2::CENTER_CENTER, &letter(&ws.name), W::Bold, 17.0, accent);
        if on {
            marker = Some((tile.top() + 10.0, accent));
        }
        let key = format!("{} {}", parts::ALT, ws.shortcut);
        let kc = if on { t.subtle } else { t.muted };
        let r = parts::text(&p, pos2(cx, tile.bottom() + 6.0), Align2::CENTER_TOP, &key, W::MonoMedium, 9.0, kc);
        if resp.clicked() {
            actions.push(Action::SwitchWorkspace(ws.slug.clone()));
        }
        if resp.hovered() {
            let teams = boards.get(&ws.slug).map(|b| b.teams.len());
            tooltip(ui, t, pos2(rect.right() - 6.0, tile.top() + 8.0), &ws.name, teams, &key);
        }
        y = r.bottom() + 16.0;
    }
    // The marker slides to the active workspace.
    if let Some((top, accent)) = marker {
        let top = motion::value(ui.ctx(), Id::unique("lb-rail-marker"), top);
        let r = Rect::from_min_size(pos2(rect.left(), top), vec2(3.0, 24.0));
        p.rect_filled(r, CornerRadius::same(2), accent);
    }

    let me_center = pos2(cx, rect.bottom() - TOP - AVATAR / 2.0);
    // The photo fades in after the first sync finds the user, and "+" slides up to make room.
    let shown = motion::toggle(ui.ctx(), Id::unique("lb-rail-me-shown"), me.is_some());
    let add_bottom = motion::lerp(rect.bottom() - TOP, me_center.y - AVATAR / 2.0 - 14.0, shown);
    let add = Rect::from_min_size(pos2(cx - TILE / 2.0, add_bottom - TILE), vec2(TILE, TILE));
    let resp = ui
        .interact(add, Id::unique("lb-rail-add"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Add a workspace");
    if adding {
        let stroke = Stroke::new(1.5, t.accent_color);
        p.rect(add, CornerRadius::same(12), Theme::tint(t.accent_color, 0.18), stroke, StrokeKind::Inside);
    } else {
        if resp.hovered() {
            p.rect_filled(add, CornerRadius::same(12), t.hl_low);
        }
        parts::dashed_rect(&p, add, 12.0, Stroke::new(1.0, t.hl_med), 3.0, 3.0);
    }
    let ic = if adding { t.accent_color } else if resp.hovered() { t.text } else { t.subtle };
    parts::icon(&p, add.center(), Icon::Plus, 16.0, ic);
    if resp.clicked() {
        actions.push(Action::OpenWorkspaces);
    }
    if let Some(me) = me {
        profile(ui, t, me_center, me, mode == RailMode::Settings, shown, actions);
    }
}

/// The banana with a hairline under it. Returns the y where the workspace tiles start.
fn brand_mark(ui: &Ui, t: &Theme, top: eframe::egui::Pos2) -> f32 {
    let r = Rect::from_min_size(pos2(top.x - MARK / 2.0, top.y), vec2(MARK, MARK));
    let resp = ui.interact(r, Id::unique("lb-rail-brand"), Sense::hover()).on_hover_text("Kanbanana");
    let angle = crate::brand::wiggle(ui.ctx(), Id::unique("lb-rail-brand-wiggle"), resp.hovered());
    crate::brand::banana(ui.painter(), r, angle, 1.0);
    let line_y = r.bottom() + 12.5;
    ui.painter().hline(top.x - 14.0..=top.x + 14.0, line_y, Stroke::new(1.0, t.hl_med));
    line_y + 16.0
}

/// The signed-in user of the active workspace, at `alpha` opacity while it fades in.
fn profile(ui: &Ui, t: &Theme, c: eframe::egui::Pos2, me: &User, on: bool, alpha: f32, actions: &mut Vec<Action>) {
    let mut p = ui.painter().clone();
    p.multiply_opacity(alpha);
    let p = &p;
    let hit = Rect::from_center_size(c, vec2(AVATAR + 6.0, AVATAR + 6.0));
    let resp = ui.interact(hit, Id::unique("lb-rail-me"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let hover = motion::toggle(ui.ctx(), Id::unique("lb-rail-me-hover"), resp.hovered() || on);
    let initials = crate::board::card::initials(&me.name);
    let who = Some((initials.as_str(), t.accent_color));
    parts::avatar_photo(p, ui.ctx(), t, c, AVATAR, me.avatar_url.as_deref(), who);
    parts::ring(p, c, AVATAR / 2.0 + 2.5, 1.5, t.accent_color, hover);
    if resp.hovered() {
        let key = format!("{} ,", parts::CMD);
        let at = pos2(ui.max_rect().right() - 6.0, c.y - 13.0);
        tip(ui, t, at, &me.name, Some("Settings"), &key);
    }
    if resp.clicked() {
        actions.push(Action::OpenSettings);
    }
}

fn tooltip(ui: &Ui, t: &Theme, at: eframe::egui::Pos2, name: &str, teams: Option<usize>, key: &str) {
    let teams = teams.map(|n| if n == 1 { "1 team".to_string() } else { format!("{n} teams") });
    tip(ui, t, at, name, teams.as_deref(), key);
}

/// Name in SemiBold, an optional muted part and a kbd hint, right of the rail.
fn tip(ui: &Ui, t: &Theme, at: eframe::egui::Pos2, name: &str, extra: Option<&str>, key: &str) {
    let p = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, Id::unique("lb-rail-tip")));
    let ns = parts::text_size(&p, name, W::SemiBold, 12.0);
    let ts = extra.map(|s| parts::text_size(&p, s, W::Medium, 12.0));
    let ks = parts::kbd_size(&p, key);
    let w = 10.0 + ns.x + ts.map_or(0.0, |s| 8.0 + s.x) + 8.0 + ks.x + 10.0;
    let h = ns.y.max(ks.y) + 12.0;
    let r = Rect::from_min_size(at, vec2(w, h));
    p.add(t.shadow_sm().as_shape(r, CornerRadius::same(6)));
    p.rect(r, CornerRadius::same(6), t.overlay, Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
    let mid = r.center().y;
    let mut x = parts::text(&p, pos2(r.min.x + 10.0, mid), Align2::LEFT_CENTER, name, W::SemiBold, 12.0, t.text).max.x;
    if let Some(s) = extra {
        x = parts::text(&p, pos2(x + 8.0, mid), Align2::LEFT_CENTER, s, W::Medium, 12.0, t.muted).max.x;
    }
    parts::kbd(&p, t, pos2(x + 8.0, mid), key);
}
