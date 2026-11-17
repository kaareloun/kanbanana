//! Title (inline editor) and the property list. A click on a value opens its picker.

use eframe::egui::{
    Align2, Color32, CornerRadius, CursorIcon, Key, Modifiers, Painter, Rect, Sense, Stroke, StrokeKind,
    TextEdit, Ui, pos2, vec2,
};
use chrono::{DateTime, Local, Utc};
use model::{BoardData, Issue, IssueChange, Priority};

use super::icons::{self, Ic};
use super::{PanelAction, PanelState, PickerKind, TITLE_ID, anim, dev, id, time, user_avatar, user_badge, widgets};
use crate::board::layout::status_kind;
use crate::theme::parts::{self, StatusKind};
use crate::theme::{Theme, W};

const TITLE_SIZE: f32 = 22.0;
const TITLE_LINE: f32 = 30.0;
const TITLE_ICON: f32 = 18.0;
const ROW_H: f32 = 34.0;
const LABEL_W: f32 = 104.0;
const VALUE_GAP: f32 = 8.0;

/// Status circle and the title. The title is always an inline editor that looks like the
/// heading. Enter or focus loss saves. Esc (in `keys`) reverts.
pub fn title(ui: &mut Ui, t: &Theme, state: &mut PanelState, issue: &Issue, data: &BoardData, out: &mut Vec<PanelAction>) {
    let tid = id(TITLE_ID);
    let focused = ui.memory(|m| m.has_focus(tid));
    // The title is one line, so Enter saves instead of adding a line break.
    let enter = focused && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
    let mut buf = state.title_edit.clone().unwrap_or_else(|| issue.title.clone());
    let kind = data
        .states
        .iter()
        .find(|s| s.id == issue.state_id)
        .map_or(StatusKind::Todo, |s| status_kind(s.kind, &s.name));

    let resp = ui
        .horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let (r, _) = ui.allocate_exact_size(vec2(TITLE_ICON, TITLE_LINE), Sense::hover());
            parts::status_icon(ui.painter(), t, pos2(r.center().x, r.top() + TITLE_LINE / 2.0), kind, TITLE_ICON);
            let mut layouter = widgets::plain_layouter(W::SemiBold, TITLE_SIZE, TITLE_LINE, t.text);
            TextEdit::multiline(&mut buf)
                .id(tid)
                .frame(eframe::egui::Frame::NONE)
                .margin(0)
                .desired_rows(1)
                .desired_width(f32::INFINITY)
                .layouter(&mut layouter)
                .show(ui)
                .response
        })
        .inner;

    let ctx = ui.ctx().clone();
    let shown = anim::hover(&ctx, tid, resp.hovered() || resp.has_focus());
    let focus = anim::toggle(&ctx, tid.with("focus"), resp.has_focus(), anim::HOVER);
    if shown > 0.0 {
        let c = Theme::tint(anim::mix(t.hl_low, t.hl_med, focus), shown);
        let frame = resp.rect.expand2(vec2(8.0, 3.0));
        ui.painter().rect_stroke(frame, CornerRadius::same(8), Stroke::new(1.0, c), StrokeKind::Outside);
    }

    if state.title_focus {
        resp.request_focus();
        state.title_focus = false;
        state.title_edit = Some(buf);
    } else if resp.has_focus() && !enter {
        state.title_edit = Some(buf);
    } else if state.title_edit.is_some() {
        state.title_edit = Some(buf);
        save_title(state, issue, out);
        if enter {
            resp.surrender_focus();
        }
    }
}

pub fn save_title(state: &mut PanelState, issue: &Issue, out: &mut Vec<PanelAction>) {
    if let Some(s) = state.title_edit.take() {
        let s = s.replace('\n', " ").trim().to_string();
        if !s.is_empty() && s != issue.title {
            out.push(PanelAction::Change(IssueChange::Title(s)));
        }
    }
}

/// Parts a value shows, left to right.
enum Part {
    Status(StatusKind),
    Priority(Priority),
    /// Initials and color, and the photo URL.
    Avatar(Option<(String, Color32)>, Option<String>),
    Text(String, Color32),
    Tag(String, Color32),
    Icon(Ic, Color32),
}

fn part_width(p: &Painter, part: &Part) -> f32 {
    match part {
        Part::Status(_) => 15.0,
        Part::Priority(_) | Part::Icon(..) => 14.0,
        Part::Avatar(..) => 20.0,
        Part::Text(s, _) => parts::text_size(p, s, W::Medium, 13.0).x,
        Part::Tag(s, _) => parts::tag_size(p, s, false, false).x,
    }
}

fn paint_part(p: &Painter, t: &Theme, x: f32, cy: f32, part: &Part) {
    match part {
        Part::Status(k) => parts::status_icon(p, t, pos2(x + 7.5, cy), *k, 15.0),
        Part::Priority(pr) => parts::priority_icon(p, t, pos2(x, cy - 7.0), *pr),
        Part::Avatar(who, url) => {
            let who = who.as_ref().map(|(i, c)| (i.as_str(), *c));
            parts::avatar_photo(p, p.ctx(), t, pos2(x + 10.0, cy), 20.0, url.as_deref(), who);
        }
        Part::Text(s, c) => {
            parts::text(p, pos2(x, cy), Align2::LEFT_CENTER, s, W::Medium, 13.0, *c);
        }
        Part::Tag(s, c) => {
            parts::tag(p, t, pos2(x, cy), s, *c, None, false);
        }
        Part::Icon(ic, c) => icons::paint(p, pos2(x + 7.0, cy), *ic, 14.0, *c),
    }
}

/// One property row with its label. Returns the row rect.
pub fn row(ui: &mut Ui, t: &Theme, label: &str) -> Rect {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    parts::text(ui.painter(), r.left_center(), Align2::LEFT_CENTER, label, W::Medium, 12.0, t.muted);
    r
}

/// A clickable value: hover fill, accent ring while its picker is open.
fn value(ui: &mut Ui, t: &Theme, row: Rect, items: &[Part], open: bool, salt: &str) -> (Rect, bool) {
    let p = ui.painter().clone();
    let inner: f32 = items.iter().map(|i| part_width(&p, i)).sum::<f32>() + VALUE_GAP * (items.len() as f32 - 1.0);
    let rect = Rect::from_center_size(
        pos2(row.left() + LABEL_W - 6.0 + (inner + 12.0) / 2.0, row.center().y),
        vec2(inner + 12.0, 28.0),
    );
    let resp = ui.interact(rect, id("lb-panel-prop").with(salt), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let h = anim::hover(ui.ctx(), resp.id, resp.hovered());
    if open {
        p.rect(rect, CornerRadius::same(6), t.hl_low, Stroke::new(1.0, t.accent_color), StrokeKind::Inside);
    } else if h > 0.0 {
        p.rect_filled(rect, CornerRadius::same(6), Theme::tint(t.hl_low, h));
    }
    let mut x = rect.min.x + 6.0;
    for it in items {
        paint_part(&p, t, x, rect.center().y, it);
        x += part_width(&p, it) + VALUE_GAP;
    }
    (rect, resp.clicked())
}

pub fn list(ui: &mut Ui, t: &Theme, state: &mut PanelState, issue: &Issue, data: &BoardData, out: &mut Vec<PanelAction>) {
    let open = state.picker.as_ref().map(|p| p.kind);
    let st = data.states.iter().find(|s| s.id == issue.state_id);
    let status = match st {
        Some(s) => vec![Part::Status(status_kind(s.kind, &s.name)), Part::Text(s.name.clone(), t.text)],
        None => vec![Part::Status(StatusKind::Todo), Part::Text("No status".into(), t.subtle)],
    };
    let priority = vec![Part::Priority(issue.priority), Part::Text(issue.priority.label().into(), t.text)];
    let assignee = match issue.assignee_id.as_deref().and_then(|u| user_badge(data, t, u)) {
        Some((ini, c, name)) => {
            let url = issue.assignee_id.as_deref().and_then(|u| user_avatar(data, u));
            vec![Part::Avatar(Some((ini, c)), url), Part::Text(name, t.text)]
        }
        None => vec![Part::Avatar(None, None), Part::Text("Unassigned".into(), t.subtle)],
    };
    let mut labels: Vec<Part> = issue
        .label_ids
        .iter()
        .filter_map(|lid| data.labels.iter().find(|l| &l.id == lid))
        .map(|l| Part::Tag(l.name.clone(), t.nearest(&l.color)))
        .collect();
    if labels.is_empty() {
        labels.push(Part::Text("Add labels".into(), t.muted));
    }
    labels.push(Part::Icon(Ic::Plus, t.muted));

    let rows = [
        ("Status", PickerKind::Status, status),
        ("Priority", PickerKind::Priority, priority),
        ("Assignee", PickerKind::Assignee, assignee),
        ("Labels", PickerKind::Labels, labels),
    ];
    let mut clicked = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, (label, kind, items)) in rows.iter().enumerate() {
            let r = row(ui, t, label);
            let (rect, hit) = value(ui, t, r, items, open == Some(*kind), label);
            state.chip_rects[i] = rect;
            if hit {
                clicked = Some(*kind);
            }
        }
        dev::row(ui, t, issue, data, LABEL_W, out);
        let created = issue.created_at.with_timezone(&Local);
        date_row(ui, t, "Created", &created.format("%-d %b %Y").to_string(), issue.created_at);
        date_row(ui, t, "Updated", &time::relative(issue.updated_at, super::now()), issue.updated_at);
    });
    if let Some(kind) = clicked {
        if open == Some(kind) {
            state.picker = None;
        } else {
            state.open_picker(kind, issue, data);
        }
    }
}

/// Read-only date row. Hover shows the full local date and time.
fn date_row(ui: &mut Ui, t: &Theme, label: &str, shown: &str, at: DateTime<Utc>) {
    let r = row(ui, t, label);
    let p = ui.painter().clone();
    let tr = parts::text(&p, pos2(r.left() + LABEL_W, r.center().y), Align2::LEFT_CENTER, shown, W::Medium, 13.0, t.subtle);
    let full = at.with_timezone(&Local).format("%-d %b %Y %H:%M").to_string();
    ui.interact(tr, id("lb-panel-date").with(label), Sense::hover()).on_hover_text(full);
}
