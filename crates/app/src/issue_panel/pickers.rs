//! Status, priority, assignee and labels pickers. A popover under the value with a filter.

use eframe::egui::{
    self, Align2, Area, Color32, CornerRadius, CursorIcon, Key, Margin, Modifiers, Order, Rect, ScrollArea,
    Sense, Stroke, TextEdit, emath::TSTransform, pos2, vec2,
};
use model::{BoardData, Issue, IssueChange, Priority};

use super::icons::{self, Ic};
use super::{FILTER_ID, PanelAction, PanelState, Picker, PickerKind, anim, id, user_badge, widgets};
use crate::board::layout::status_kind;
use crate::theme::parts::{self, StatusKind};
use crate::theme::{Theme, W, font};

const POP_W: f32 = 260.0;
const ROW_H: f32 = 31.0;

enum Look {
    Status(StatusKind),
    Priority(Priority),
    /// Initials and color, and the photo URL.
    User(Option<(String, Color32)>, Option<String>),
    Label(Color32),
}

struct Opt {
    key: String,
    name: String,
    look: Look,
    selected: bool,
}

/// Team labels and workspace labels. The ones on the issue come first.
pub fn label_order(issue: &Issue, data: &BoardData) -> Vec<String> {
    let usable = |team: &Option<String>| team.as_ref().is_none_or(|t| *t == issue.team_id);
    let mut ids: Vec<String> = issue
        .label_ids
        .iter()
        .filter(|id| data.labels.iter().any(|l| &l.id == *id && usable(&l.team_id)))
        .cloned()
        .collect();
    for l in data.labels.iter().filter(|l| usable(&l.team_id)) {
        if !ids.contains(&l.id) {
            ids.push(l.id.clone());
        }
    }
    ids
}

fn options(t: &Theme, p: &Picker, issue: &Issue, data: &BoardData) -> Vec<Opt> {
    let mut v: Vec<Opt> = match p.kind {
        PickerKind::Status => {
            let mut states: Vec<_> = data.states.iter().filter(|s| s.team_id == issue.team_id).collect();
            states.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.position.total_cmp(&b.position)));
            states
                .into_iter()
                .map(|s| Opt {
                    key: s.id.clone(),
                    name: s.name.clone(),
                    look: Look::Status(status_kind(s.kind, &s.name)),
                    selected: s.id == issue.state_id,
                })
                .collect()
        }
        PickerKind::Priority => Priority::ALL
            .iter()
            .map(|pr| Opt {
                key: pr.to_linear().to_string(),
                name: pr.label().into(),
                look: Look::Priority(*pr),
                selected: *pr == issue.priority,
            })
            .collect(),
        PickerKind::Assignee => {
            let mut v = vec![Opt {
                key: String::new(),
                name: "No assignee".into(),
                look: Look::User(None, None),
                selected: issue.assignee_id.is_none(),
            }];
            for u in data.users.iter().filter(|u| u.active) {
                let badge = user_badge(data, t, &u.id).map(|(i, c, _)| (i, c));
                v.push(Opt {
                    key: u.id.clone(),
                    name: u.name.clone(),
                    look: Look::User(badge, u.avatar_url.clone()),
                    selected: issue.assignee_id.as_ref() == Some(&u.id),
                });
            }
            v
        }
        PickerKind::Labels => p
            .order
            .iter()
            .filter_map(|lid| data.labels.iter().find(|l| &l.id == lid))
            .map(|l| Opt {
                key: l.id.clone(),
                name: l.name.clone(),
                look: Look::Label(t.nearest(&l.color)),
                selected: issue.label_ids.contains(&l.id),
            })
            .collect(),
    };
    let f = p.filter.trim().to_lowercase();
    if !f.is_empty() {
        v.retain(|o| o.name.to_lowercase().contains(&f));
    }
    v
}

fn texts(kind: PickerKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        PickerKind::Status => ("Filter statuses", "S", "Enter picks. Esc closes."),
        PickerKind::Priority => ("Filter priorities", "P", "Enter picks. Esc closes."),
        PickerKind::Assignee => ("Filter people", "A", "Enter picks. Esc closes."),
        PickerKind::Labels => ("Filter labels", "L", "Enter toggles. Esc closes."),
    }
}

pub fn show(
    ctx: &egui::Context,
    t: &Theme,
    state: &mut PanelState,
    issue: &Issue,
    data: &BoardData,
    panel: Rect,
    out: &mut Vec<PanelAction>,
) {
    let Some(picker) = state.picker.as_mut() else { return };
    let anchor = state.chip_rects[picker.kind as usize];
    let opts = options(t, picker, issue, data);
    picker.cursor = picker.cursor.min(opts.len().saturating_sub(1));

    let (down, up, enter, esc) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    if esc {
        state.picker = None;
        return;
    }
    if down && !opts.is_empty() {
        picker.cursor = (picker.cursor + 1) % opts.len();
    }
    if up && !opts.is_empty() {
        picker.cursor = (picker.cursor + opts.len() - 1) % opts.len();
    }
    let mut pick = if enter { Some(picker.cursor) } else { None };

    let x = anchor.left().min(panel.right() - 8.0 - POP_W).max(panel.left() + 8.0);
    let y = anchor.bottom().clamp(panel.top(), panel.bottom() - 120.0) + 6.0;
    let (hint_text, key, footer) = texts(picker.kind);
    let start = *picker.opened_at.get_or_insert_with(|| anim::now(ctx));
    let pop = anim::since(ctx, start);
    let shown = Area::new(id("lb-panel-picker"))
        .order(Order::Foreground)
        .fade_in(false)
        .fixed_pos(pos2(x, y))
        .constrain(true)
        .show(ctx, |ui| {
            ui.multiply_opacity(pop);
            egui::Frame::NONE
                .fill(t.overlay)
                .stroke(Stroke::new(1.0, t.hl_med))
                .corner_radius(12)
                .shadow(t.shadow())
                .show(ui, |ui| {
                    ui.set_width(POP_W);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    egui::Frame::NONE
                        .inner_margin(Margin { left: 12, right: 12, top: 10, bottom: 10 })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let kw = parts::kbd_size(ui.painter(), key).x;
                                let resp = TextEdit::singleline(&mut picker.filter)
                                    .id(id(FILTER_ID))
                                    .frame(egui::Frame::NONE)
                                    .margin(0)
                                    .font(font(W::Regular, 13.0))
                                    .text_color(t.text)
                                    .hint_text(widgets::hint(t, hint_text))
                                    .desired_width(ui.available_width() - kw - 8.0)
                                    .show(ui)
                                    .response;
                                let nothing_focused = ui.memory(|m| m.focused().is_none());
                                if picker.focus || nothing_focused {
                                    resp.request_focus();
                                    picker.focus = false;
                                }
                                if resp.changed() {
                                    picker.cursor = 0;
                                }
                                ui.add_space(8.0);
                                widgets::kbd(ui, t, key);
                            });
                        });
                    line(ui, t);
                    egui::Frame::NONE.inner_margin(4).show(ui, |ui| {
                        ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            if opts.is_empty() {
                                let (r, _) = ui.allocate_exact_size(vec2(POP_W - 8.0, ROW_H), Sense::hover());
                                parts::text(ui.painter(), r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, "No matches", W::Medium, 13.0, t.muted);
                            }
                            for (i, o) in opts.iter().enumerate() {
                                if row(ui, t, picker.kind, o, i == picker.cursor, &mut picker.cursor, i) {
                                    pick = Some(i);
                                }
                            }
                        });
                    });
                    line(ui, t);
                    egui::Frame::NONE
                        .inner_margin(Margin { left: 12, right: 12, top: 8, bottom: 10 })
                        .show(ui, |ui| {
                            widgets::text(ui, footer, W::Medium, 11.0, t.muted);
                        });
                });
        })
        .response;
    // Pops in from 96% around its top left corner. The transform also maps input, so it
    // stays clickable while it grows.
    let origin = pos2(x, y).to_vec2();
    let scale = 0.96 + 0.04 * pop;
    let grow = TSTransform::from_translation(origin) * TSTransform::from_scaling(scale) * TSTransform::from_translation(-origin);
    ctx.set_transform_layer(shown.layer_id, grow);
    let area = shown.rect;

    let outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer.interact_pos().is_some_and(|p| !area.contains(p) && !anchor.contains(p))
    });
    let kind = picker.kind;
    if let Some(o) = pick.and_then(|i| opts.get(i)) {
        apply(state, kind, o, issue, out);
    } else if outside {
        state.picker = None;
    }
}

fn line(ui: &mut egui::Ui, t: &Theme) {
    let (r, _) = ui.allocate_exact_size(vec2(POP_W, 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.hl_med));
}

fn row(ui: &mut egui::Ui, t: &Theme, kind: PickerKind, o: &Opt, on: bool, cursor: &mut usize, i: usize) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(POP_W - 8.0, ROW_H), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
        *cursor = i;
    }
    let p = ui.painter();
    if on {
        p.rect_filled(r, CornerRadius::same(6), t.hl_med);
    }
    let cy = r.center().y;
    let mut x = r.left() + 10.0;
    if kind == PickerKind::Labels {
        parts::checkbox(p, t, Rect::from_center_size(pos2(x + 7.0, cy), vec2(14.0, 14.0)), o.selected);
        x += 24.0;
    }
    match &o.look {
        Look::Status(k) => {
            parts::status_icon(p, t, pos2(x + 7.0, cy), *k, 14.0);
            x += 24.0;
        }
        Look::Priority(pr) => {
            parts::priority_icon(p, t, pos2(x, cy - 7.0), *pr);
            x += 24.0;
        }
        Look::User(who, url) => {
            let who = who.as_ref().map(|(i, c)| (i.as_str(), *c));
            parts::avatar_photo(p, ui.ctx(), t, pos2(x + 9.0, cy), 18.0, url.as_deref(), who);
            x += 28.0;
        }
        Look::Label(c) => {
            p.circle_filled(pos2(x + 4.0, cy), 4.0, *c);
            x += 18.0;
        }
    }
    parts::text(p, pos2(x, cy), Align2::LEFT_CENTER, &o.name, W::Medium, 13.0, t.text);
    if kind != PickerKind::Labels && o.selected {
        icons::paint(p, pos2(r.right() - 17.0, cy), Ic::Check, 14.0, t.subtle);
    }
    resp.clicked()
}

fn apply(state: &mut PanelState, kind: PickerKind, o: &Opt, issue: &Issue, out: &mut Vec<PanelAction>) {
    let change = match kind {
        PickerKind::Status => (o.key != issue.state_id).then(|| IssueChange::State { state_id: o.key.clone(), sort_order: None }),
        PickerKind::Priority => {
            let pr = Priority::from_linear(o.key.parse().unwrap_or(0));
            (pr != issue.priority).then_some(IssueChange::Priority(pr))
        }
        PickerKind::Assignee => {
            let a = (!o.key.is_empty()).then(|| o.key.clone());
            (a != issue.assignee_id).then_some(IssueChange::Assignee(a))
        }
        PickerKind::Labels => {
            let mut ids = issue.label_ids.clone();
            if let Some(pos) = ids.iter().position(|l| *l == o.key) {
                ids.remove(pos);
            } else {
                ids.push(o.key.clone());
            }
            Some(IssueChange::Labels(ids))
        }
    };
    if let Some(c) = change {
        out.push(PanelAction::Change(c));
    }
    if kind != PickerKind::Labels {
        state.picker = None;
    }
}
