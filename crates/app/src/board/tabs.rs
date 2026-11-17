//! Group tab row: Team or Project switch, one tab per team or project, and the
//! "Assigned to me" filter at the right end. All three choose what the board shows.

use eframe::egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Id, Layout, Rect, Sense, Ui, pos2, vec2,
};

use super::layout::{GroupBy, Tab, TabKind};
use crate::theme::{motion, parts};
use crate::theme::{Theme, W};

pub const HEIGHT: f32 = 48.0;

/// What the user clicked in the row.
pub enum Pick {
    Group(GroupBy),
    Tab(String),
    ToggleMine,
}

/// A stable color per team or project. Projects start one step later,
/// so the first project does not share the first team's color.
pub fn color(t: &Theme, kind: TabKind) -> Option<Color32> {
    let teams = [t.iris, t.foam, t.rose, t.gold, t.pine, t.love];
    let projects = [t.foam, t.gold, t.iris, t.rose, t.pine, t.love];
    match kind {
        TabKind::All => None,
        TabKind::Team(i) => Some(teams[i % teams.len()]),
        TabKind::Project(i) => Some(projects[i % projects.len()]),
        TabKind::NoProject => Some(t.muted),
    }
}

pub fn show(ui: &mut Ui, t: &Theme, group: GroupBy, tabs: &[Tab], active: &str, mine: bool) -> Option<Pick> {
    let mut pick = None;
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 20.0;
        let on = if group == GroupBy::Team { 0 } else { 1 };
        if let Some(i) = parts::segmented(ui, t, "group-by", &["Team", "Project"], on) {
            pick = Some(Pick::Group(if i == 0 { GroupBy::Team } else { GroupBy::Project }));
        }
        let (r, _) = ui.allocate_exact_size(vec2(1.0, 20.0), Sense::hover());
        ui.painter().rect_filled(r, CornerRadius::ZERO, t.hl_med);
        let mut line = None;
        for tab in tabs {
            let on = tab.key == active;
            let (clicked, rect) = tab_button(ui, t, tab, on);
            if clicked {
                pick = Some(Pick::Tab(tab.key.clone()));
            }
            if on {
                line = Some(rect);
            }
        }
        // The underline slides from the old active tab to the new one.
        if let Some(r) = line {
            let ctx = ui.ctx().clone();
            let x = motion::value(&ctx, Id::unique("lb-tab-line-x"), r.min.x);
            let w = motion::value(&ctx, Id::unique("lb-tab-line-w"), r.width());
            let line = Rect::from_min_max(pos2(x, r.max.y - 4.0), pos2(x + w, r.max.y - 2.0));
            ui.painter().rect_filled(line, CornerRadius::same(1), t.accent_color);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if mine_toggle(ui, t, mine) {
                pick = Some(Pick::ToggleMine);
            }
        });
    });
    pick
}

fn mine_toggle(ui: &mut Ui, t: &Theme, on: bool) -> bool {
    let p = ui.painter().clone();
    let label = "Assigned to me";
    let ts = parts::text_size(&p, label, W::SemiBold, 12.0);
    let ks = parts::kbd_size(&p, "M");
    let w = 10.0 + 15.0 + 8.0 + ts.x + 8.0 + ks.x + 10.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let hover = motion::toggle(ui.ctx(), resp.id.with("hover"), resp.hovered());
    let fill = if on { t.surface } else { t.hl_low.gamma_multiply(hover) };
    p.rect_filled(rect, CornerRadius::same(8), fill);
    let mid = rect.center().y;
    let cb = Rect::from_min_size(pos2(rect.min.x + 10.0, mid - 7.5), vec2(15.0, 15.0));
    parts::checkbox(&p, t, cb, on);
    let tc = if on { t.text } else { t.subtle.lerp_to_gamma(t.text, hover) };
    let r = parts::text(&p, pos2(cb.max.x + 8.0, mid), Align2::LEFT_CENTER, label, W::SemiBold, 12.0, tc);
    parts::kbd(&p, t, pos2(r.max.x + 8.0, mid), "M");
    resp.clicked()
}


/// Returns true on a click on an inactive tab, and the tab rect.
fn tab_button(ui: &mut Ui, t: &Theme, tab: &Tab, on: bool) -> (bool, Rect) {
    let p = ui.painter().clone();
    let color = color(t, tab.kind);
    let count = tab.count.to_string();
    let ns = parts::text_size(&p, &tab.name, W::SemiBold, 13.0);
    let cs = parts::text_size(&p, &count, W::MonoMedium, 11.0);
    let sq = if color.is_some() { 10.0 + 8.0 } else { 0.0 };
    let w = 4.0 + sq + ns.x + 8.0 + cs.x + 4.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, HEIGHT), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let mid = rect.center().y;
    let mut x = rect.min.x + 4.0;
    if let Some(c) = color {
        let r = Rect::from_center_size(pos2(x + 5.0, mid), vec2(10.0, 10.0));
        p.rect_filled(r, CornerRadius::same(3), c);
        x += sq;
    }
    let lit = motion::toggle(ui.ctx(), Id::unique(("lb-tab-lit", &tab.key)), on || resp.hovered());
    let active = motion::toggle(ui.ctx(), Id::unique(("lb-tab-on", &tab.key)), on);
    let nc = t.subtle.lerp_to_gamma(t.text, lit);
    let r = parts::text(&p, pos2(x, mid), Align2::LEFT_CENTER, &tab.name, W::SemiBold, 13.0, nc);
    let cc = t.muted.lerp_to_gamma(t.accent_color, active);
    parts::text(&p, pos2(r.max.x + 8.0, mid + 0.5), Align2::LEFT_CENTER, &count, W::MonoMedium, 11.0, cc);
    (resp.clicked() && !on, rect)
}
