//! Board top bar: workspace name, sync status and refresh.

use std::f32::consts::TAU;

use chrono::{DateTime, Utc};

use eframe::egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Layout, Rect, Sense, Ui, pos2,
    text::{LayoutJob, TextFormat, TextWrapping},
    vec2,
};

use crate::actions::Action;
use crate::brand;
use crate::settings::ago;
use crate::sync::SyncStatus;
use crate::theme::parts;
use crate::theme::{Theme, W, font, motion};

pub struct TopbarInfo<'a> {
    pub workspace: &'a str,
    /// "3 teams", or "Kaarel, 9 issues" with the mine filter on.
    pub subtitle: String,
    pub last_sync: Option<DateTime<Utc>>,
}

/// Width of the sync status slot. It fits "Synced 59 min ago" and "Rate limit, retry 14:05",
/// so a change of the text never moves the other items.
const SYNC_W: f32 = 156.0;

pub fn show(ui: &mut Ui, t: &Theme, info: &TopbarInfo, sync: &SyncStatus, actions: &mut Vec<Action>) {
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        title(ui, t, info);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if refresh_button(ui, t, matches!(sync, SyncStatus::Syncing)) {
                actions.push(Action::Refresh);
            }
            sync_status(ui, t, sync, info.last_sync);
        });
    });
}

/// Name in Bold 20 with the subtitle on the same baseline.
fn title(ui: &mut Ui, t: &Theme, info: &TopbarInfo) {
    let p = ui.painter().clone();
    let ns = parts::text_size(&p, info.workspace, W::Bold, 20.0);
    let ss = parts::text_size(&p, &info.subtitle, W::Medium, 12.0);
    let (r, _) = ui.allocate_exact_size(vec2(ns.x + 10.0 + ss.x, ns.y), Sense::hover());
    let name = parts::text(&p, r.left_center(), Align2::LEFT_CENTER, info.workspace, W::Bold, 20.0, t.text);
    let at = pos2(name.max.x + 10.0, name.bottom() - 1.0);
    parts::text(&p, at, Align2::LEFT_BOTTOM, &info.subtitle, W::Medium, 12.0, t.muted);
}

/// Refresh button at a fixed spot. While a sync runs, a banana spins in it once per second.
fn refresh_button(ui: &mut Ui, t: &Theme, syncing: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(29.0, 29.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Refresh (R)");
    let hover = motion::toggle(ui.ctx(), resp.id.with("hover"), resp.hovered());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), t.surface.lerp_to_gamma(t.overlay, hover));
    let c: Color32 = t.subtle.lerp_to_gamma(t.text, hover);
    let angle = if syncing {
        ui.ctx().request_repaint();
        (motion::now(ui.ctx()) % 1.0) as f32 * TAU
    } else {
        0.0
    };
    let banana = Rect::from_center_size(rect.center(), vec2(18.0, 18.0));
    if !(syncing && brand::banana(p, banana, angle, 1.0)) {
        parts::refresh_icon(p, rect.center(), 15.0, c, angle);
    }
    resp.clicked()
}

fn sync_status(ui: &mut Ui, t: &Theme, sync: &SyncStatus, last: Option<DateTime<Utc>>) {
    let (dot, label) = status_text(t, sync, last);
    let tc = if matches!(sync, SyncStatus::Error(_)) { t.love } else { t.muted };
    let p = ui.painter().clone();
    let (rect, resp) = ui.allocate_exact_size(vec2(SYNC_W, 20.0), Sense::hover());
    let mid = rect.center().y;
    // The text sits at the right edge of the slot. Long text ends in an ellipsis.
    let mut job = LayoutJob::single_section(label.clone(), TextFormat { font_id: font(W::Medium, 12.0), color: tc, ..Default::default() });
    job.wrap = TextWrapping::truncate_at_width(SYNC_W - 15.0);
    let g = p.layout_job(job);
    let elided = g.elided;
    let x = rect.right() - g.size().x;
    p.galley(pos2(x, mid - g.size().y / 2.0), g, tc);
    p.circle_filled(pos2(x - 11.5, mid), 3.5, dot);
    if elided {
        resp.on_hover_text(label);
    }
}

fn status_text(t: &Theme, sync: &SyncStatus, last: Option<DateTime<Utc>>) -> (Color32, String) {
    match sync {
        SyncStatus::Syncing => (t.gold, "Syncing".to_string()),
        SyncStatus::Error(e) => (t.love, e.clone()),
        SyncStatus::Idle => match last {
            Some(at) => (t.foam, format!("Synced {}", ago(at, Utc::now()))),
            None => (t.muted, "Not synced yet".to_string()),
        },
    }
}
