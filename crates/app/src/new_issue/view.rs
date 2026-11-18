//! Paints the new issue form and runs its keys. Centered over a scrim, styled like the popovers.

use eframe::egui::{
    self, Align, Align2, Area, Color32, CornerRadius, CursorIcon, Frame, Id, Key, Layout, LayerId, Margin,
    Modifiers, Order, Rect, Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui,
    emath::TSTransform, pos2, vec2,
};
use model::{BoardData, IssueChange, IssueDraft, Priority};

use super::{NewIssueState, as_issue, set_team};
use crate::actions::Action;
use crate::board::layout::status_kind;
use crate::issue_panel::keys::{IssueKey, assign_me, take_issue_key};
use crate::issue_panel::{PickerKind, pickers, user_avatar, user_badge};
use crate::rich;
use crate::theme::parts::{self, Btn, CMD, Icon, StatusKind};
use crate::theme::{Theme, W, font, motion};

const WIDTH: f32 = 680.0;
const PAD: i8 = 22;
const CHIP_H: f32 = 28.0;
const TITLE_ID: &str = "lb-new-title";
const DESC_ID: &str = "lb-new-desc";

/// What one chip shows, left to right.
enum Bit {
    Status(StatusKind),
    Priority(Priority),
    /// Initials and color, and the photo URL. None paints the dashed circle.
    Avatar(Option<(String, Color32)>, Option<String>),
    Dot(Color32),
    /// An empty ring, for "no labels".
    Ring,
    Text(String, Color32),
    Mono(String),
    Chevron,
}

/// Things the user did this frame that change the form state.
#[derive(Default)]
struct Did {
    submit: bool,
    cancel: bool,
    discard: bool,
    keep: bool,
    chip: Option<PickerKind>,
}

pub fn show(ctx: &egui::Context, t: &Theme, state: &mut NewIssueState, data: &BoardData, out: &mut Vec<Action>) {
    if !state.open {
        return;
    }
    let screen = ctx.content_rect();
    let shown = motion::since(ctx, state.opened_at);
    let picker_open = state.picker.is_some();
    let mut did = Did::default();

    // The open picker reads Esc and Enter itself.
    if !picker_open {
        ctx.input_mut(|i| {
            did.submit = i.consume_key(Modifiers::COMMAND, Key::Enter);
            did.cancel = i.consume_key(Modifiers::NONE, Key::Escape);
        });
        if !ctx.text_edit_focused() {
            match take_issue_key(ctx) {
                Some(IssueKey::Picker(kind)) => did.chip = Some(kind),
                Some(IssueKey::AssignMe) => {
                    if let Some(c) = assign_me(&as_issue(&state.draft), data) {
                        apply_change(&mut state.draft, c);
                    }
                }
                Some(IssueKey::Priority(p)) => state.draft.priority = p,
                None => {}
            }
        }
    }

    let scrim_clicked = Area::new(Id::unique("lb-new-scrim"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .fade_in(false)
        .show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0.0, Theme::tint(t.base, 0.7 * shown));
            resp.clicked()
        })
        .inner;
    if scrim_clicked && !picker_open {
        did.cancel = true;
    }

    let w = WIDTH.min(screen.width() - 48.0);
    let top = (screen.top() + (screen.height() * 0.14).max(48.0))
        .min(screen.bottom() - 24.0 - state.height)
        .max(screen.top() + 24.0);
    let x = screen.center().x - w / 2.0;
    let id = Id::unique("lb-new-issue");
    // Fades in and grows from 97 % around its top center.
    let origin = pos2(screen.center().x, top).to_vec2();
    let scale = motion::lerp(0.97, 1.0, shown);
    let grow = TSTransform::from_translation(origin) * TSTransform::from_scaling(scale) * TSTransform::from_translation(-origin);
    ctx.set_transform_layer(LayerId::new(Order::Foreground, id), grow);
    let area = Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(pos2(x, top))
        .fade_in(false)
        .constrain(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(shown);
            Frame::NONE
                .fill(t.overlay)
                .stroke(Stroke::new(1.0, t.hl_med))
                .corner_radius(14)
                .shadow(t.shadow())
                .show(ui, |ui| {
                    ui.set_width(w);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    body(ui, t, state, data, &mut did);
                });
        });
    // The scrim takes clicks, and egui lifts a clicked area to the top. The form stays above it.
    ctx.move_to_top(area.response.layer_id);
    state.height = area.response.rect.height();

    if let Some(kind) = did.chip {
        let same = state.picker.as_ref().is_some_and(|p| p.kind == kind);
        state.picker = if same { None } else { Some(pickers::new_picker(kind, &as_issue(&state.draft), data)) };
    }
    if let Some(p) = state.picker.as_mut() {
        let kind = p.kind;
        let issue = as_issue(&state.draft);
        let anchor = state.chips[kind as usize];
        let r = pickers::popover(ctx, t, p, anchor, screen, Order::Tooltip, &issue, data);
        if let Some(key) = r.picked {
            pick(&mut state.draft, data, kind, &key);
        }
        if r.close {
            state.picker = None;
            state.focus_title = true;
        }
    }

    if did.keep {
        state.confirm_discard = false;
    }
    if did.discard || (did.cancel && (state.confirm_discard || !state.has_text())) {
        state.close();
    } else if did.cancel {
        state.confirm_discard = true;
    }
    if did.submit && state.open {
        submit(state, out);
    }
}

fn body(ui: &mut Ui, t: &Theme, state: &mut NewIssueState, data: &BoardData, did: &mut Did) {
    let open = state.picker.as_ref().map(|p| p.kind);
    let team = data.teams.iter().find(|tm| tm.id == state.draft.team_id);

    Frame::NONE.inner_margin(Margin { left: PAD - 6, right: 14, top: 14, bottom: 0 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let bits = match team {
                Some(tm) => vec![Bit::Mono(tm.key.clone()), Bit::Text(tm.name.clone(), t.text), Bit::Chevron],
                None => vec![Bit::Text("Pick a team".into(), t.subtle), Bit::Chevron],
            };
            let r = chip(ui, t, "team", &bits, open == Some(PickerKind::Team));
            state.chips[PickerKind::Team as usize] = r.rect;
            if r.clicked() {
                did.chip = Some(PickerKind::Team);
            }
            let p = ui.painter().clone();
            let (r, _) = ui.allocate_exact_size(vec2(28.0, CHIP_H), Sense::hover());
            parts::icon(&p, r.center(), Icon::ChevronRight, 12.0, t.muted);
            let (r, _) = ui.allocate_exact_size(vec2(80.0, CHIP_H), Sense::hover());
            parts::text(&p, r.left_center(), Align2::LEFT_CENTER, "New issue", W::Medium, 13.0, t.subtle);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if parts::icon_button(ui, t, Icon::X, t.muted).on_hover_text("Close (Esc)").clicked() {
                    did.cancel = true;
                }
            });
        });
    });

    Frame::NONE.inner_margin(Margin { left: PAD, right: PAD, top: 14, bottom: 0 }).show(ui, |ui| {
        let resp = TextEdit::singleline(&mut state.draft.title)
            .id(Id::unique(TITLE_ID))
            .frame(Frame::NONE)
            .margin(0)
            .font(font(W::SemiBold, 20.0))
            .text_color(t.text)
            .hint_text(RichText::new("Issue title").font(font(W::SemiBold, 20.0)).color(t.muted))
            .desired_width(f32::INFINITY)
            .show(ui)
            .response;
        // The first frame of an area is a sizing pass that takes no focus.
        if state.focus_title && !ui.is_sizing_pass() {
            resp.request_focus();
            state.focus_title = false;
        }
        // Enter in the title goes on to the description, like Tab.
        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.command) {
            ui.memory_mut(|m| m.request_focus(Id::unique(DESC_ID)));
        }
        if resp.changed() {
            state.error = None;
        }
    });

    Frame::NONE.inner_margin(Margin { left: PAD, right: PAD, top: 12, bottom: 14 }).show(ui, |ui| {
        ScrollArea::vertical().id_salt("lb-new-desc-scroll").max_height(300.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let opts = rich::Options {
                look: rich::DESCRIPTION,
                placeholder: "Add a description",
                min_rows: 4,
                ..Default::default()
            };
            rich::editor(ui, t, Id::unique(DESC_ID), &mut state.draft.description, &opts);
        });
    });

    Frame::NONE.inner_margin(Margin { left: PAD, right: PAD, top: 0, bottom: 18 }).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for (kind, bits) in chip_bits(t, &state.draft, data) {
                let r = chip(ui, t, kind_salt(kind), &bits, open == Some(kind));
                state.chips[kind as usize] = r.rect;
                if r.clicked() {
                    did.chip = Some(kind);
                }
            }
        });
    });

    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.hl_med));

    Frame::NONE.inner_margin(Margin { left: PAD, right: 16, top: 12, bottom: 12 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_min_height(30.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let ready = !state.draft.title.trim().is_empty() && state.pending.is_none();
                let label = if state.pending.is_some() { "Creating" } else { "Create issue" };
                let hint = format!("{CMD} Enter");
                if parts::button(ui, t, label, Btn::Primary, Some(&hint), ready).clicked() && ready {
                    did.submit = true;
                }
                ui.add_space(10.0);
                if create_more(ui, t, state.create_more).clicked() {
                    state.create_more = !state.create_more;
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if state.confirm_discard {
                        text(ui, "Discard this issue?", W::SemiBold, t.text);
                        ui.add_space(4.0);
                        if parts::button(ui, t, "Discard", Btn::Secondary, Some("Esc"), true).clicked() {
                            did.discard = true;
                        }
                        if parts::button(ui, t, "Keep editing", Btn::Ghost, None, true).clicked() {
                            did.keep = true;
                            state.focus_title = true;
                        }
                    } else if let Some(err) = &state.error {
                        let l = ui.add(egui::Label::new(RichText::new(err).font(font(W::Medium, 12.0)).color(t.love)).truncate());
                        l.on_hover_text(err.as_str());
                    }
                });
            });
        });
    });
}

fn kind_salt(kind: PickerKind) -> &'static str {
    match kind {
        PickerKind::Status => "status",
        PickerKind::Priority => "priority",
        PickerKind::Assignee => "assignee",
        PickerKind::Labels => "labels",
        PickerKind::Team => "team",
    }
}

fn chip_bits(t: &Theme, d: &IssueDraft, data: &BoardData) -> Vec<(PickerKind, Vec<Bit>)> {
    let status = match d.state_id.as_ref().and_then(|id| data.states.iter().find(|s| &s.id == id)) {
        Some(s) => vec![Bit::Status(status_kind(s.kind, &s.name)), Bit::Text(s.name.clone(), t.text)],
        None => vec![Bit::Status(StatusKind::Todo), Bit::Text("Status".into(), t.subtle)],
    };
    let priority = match d.priority {
        Priority::None => vec![Bit::Priority(Priority::None), Bit::Text("Priority".into(), t.subtle)],
        p => vec![Bit::Priority(p), Bit::Text(p.label().into(), t.text)],
    };
    let assignee = match d.assignee_id.as_deref().and_then(|u| user_badge(data, t, u)) {
        Some((ini, c, name)) => {
            let url = d.assignee_id.as_deref().and_then(|u| user_avatar(data, u));
            vec![Bit::Avatar(Some((ini, c)), url), Bit::Text(name, t.text)]
        }
        None => vec![Bit::Avatar(None, None), Bit::Text("Assignee".into(), t.subtle)],
    };
    let mut labels = Vec::new();
    for l in d.label_ids.iter().filter_map(|id| data.labels.iter().find(|l| &l.id == id)) {
        labels.push(Bit::Dot(t.nearest(&l.color)));
        labels.push(Bit::Text(l.name.clone(), t.text));
    }
    if labels.is_empty() {
        labels = vec![Bit::Ring, Bit::Text("Labels".into(), t.subtle)];
    }
    vec![
        (PickerKind::Status, status),
        (PickerKind::Priority, priority),
        (PickerKind::Assignee, assignee),
        (PickerKind::Labels, labels),
    ]
}

fn bit_width(p: &egui::Painter, b: &Bit) -> f32 {
    match b {
        Bit::Status(_) | Bit::Priority(_) => 14.0,
        Bit::Avatar(..) => 18.0,
        Bit::Dot(_) | Bit::Ring => 8.0,
        Bit::Text(s, _) => parts::text_size(p, s, W::Medium, 12.0).x,
        Bit::Mono(s) => parts::text_size(p, s, W::MonoMedium, 11.0).x,
        Bit::Chevron => 10.0,
    }
}

fn paint_bit(p: &egui::Painter, t: &Theme, x: f32, cy: f32, b: &Bit) {
    match b {
        Bit::Status(k) => parts::status_icon(p, t, pos2(x + 7.0, cy), *k, 14.0),
        Bit::Priority(pr) => parts::priority_icon(p, t, pos2(x, cy - 7.0), *pr),
        Bit::Avatar(who, url) => {
            let who = who.as_ref().map(|(i, c)| (i.as_str(), *c));
            parts::avatar_photo(p, p.ctx(), t, pos2(x + 9.0, cy), 18.0, url.as_deref(), who);
        }
        Bit::Dot(c) => {
            p.circle_filled(pos2(x + 4.0, cy), 4.0, *c);
        }
        Bit::Ring => {
            p.circle_stroke(pos2(x + 4.0, cy), 3.5, Stroke::new(1.2, t.muted));
        }
        Bit::Text(s, c) => {
            parts::text(p, pos2(x, cy), Align2::LEFT_CENTER, s, W::Medium, 12.0, *c);
        }
        Bit::Mono(s) => {
            parts::text(p, pos2(x, cy), Align2::LEFT_CENTER, s, W::MonoMedium, 11.0, t.subtle);
        }
        Bit::Chevron => parts::icon(p, pos2(x + 5.0, cy), Icon::ChevronDown, 12.0, t.muted),
    }
}

/// A property chip: outlined, with a hover fill. The open one has the accent outline.
fn chip(ui: &mut Ui, t: &Theme, salt: &str, bits: &[Bit], open: bool) -> Response {
    let p = ui.painter().clone();
    let gap = 6.0;
    let inner: f32 = bits.iter().map(|b| bit_width(&p, b)).sum::<f32>() + gap * (bits.len().saturating_sub(1)) as f32;
    let (rect, resp) = ui.allocate_exact_size(vec2(inner + 20.0, CHIP_H), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let id = Id::unique(("lb-new-chip", salt));
    let h = motion::toggle(ui.ctx(), id, resp.hovered());
    let radius = CornerRadius::same(8);
    if h > 0.0 {
        p.rect_filled(rect, radius, Theme::tint(t.hl_low, h));
    }
    let stroke = if open { Stroke::new(1.0, t.accent_color) } else { Stroke::new(1.0, t.hl_med) };
    p.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
    let mut x = rect.min.x + 10.0;
    for b in bits {
        paint_bit(&p, t, x, rect.center().y, b);
        x += bit_width(&p, b) + gap;
    }
    resp
}

/// "Create more" toggle: a checkbox and its label, one click target.
fn create_more(ui: &mut Ui, t: &Theme, on: bool) -> Response {
    let p = ui.painter().clone();
    let tw = parts::text_size(&p, "Create more", W::Medium, 12.0).x;
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0 + 8.0 + tw, 24.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Keep the form open after create");
    let cy = rect.center().y;
    parts::checkbox(&p, t, Rect::from_center_size(pos2(rect.min.x + 7.0, cy), vec2(14.0, 14.0)), on);
    let c = if resp.hovered() { t.text } else { t.subtle };
    parts::text(&p, pos2(rect.min.x + 22.0, cy), Align2::LEFT_CENTER, "Create more", W::Medium, 12.0, c);
    resp
}

fn text(ui: &mut Ui, s: &str, w: W, c: Color32) {
    ui.label(RichText::new(s).font(font(w, 12.0)).color(c));
}

/// Applies a picked row to the draft.
fn pick(draft: &mut IssueDraft, data: &BoardData, kind: PickerKind, key: &str) {
    if kind == PickerKind::Team {
        set_team(draft, data, key);
    } else if let Some(c) = pickers::change_for(kind, key, &as_issue(draft)) {
        apply_change(draft, c);
    }
}

fn apply_change(d: &mut IssueDraft, c: IssueChange) {
    match c {
        IssueChange::State { state_id, .. } => d.state_id = Some(state_id),
        IssueChange::Priority(p) => d.priority = p,
        IssueChange::Assignee(a) => d.assignee_id = a,
        IssueChange::Labels(l) => d.label_ids = l,
        IssueChange::Title(s) => d.title = s,
        IssueChange::Description(s) => d.description = s,
        IssueChange::SortOrder(_) => {}
    }
}

fn submit(state: &mut NewIssueState, out: &mut Vec<Action>) {
    if state.pending.is_some() {
        return;
    }
    if state.draft.title.trim().is_empty() {
        state.error = Some("Add a title first.".into());
        state.focus_title = true;
        return;
    }
    let request_id = state.next_request;
    state.next_request += 1;
    state.pending = Some(request_id);
    state.error = None;
    state.confirm_discard = false;
    out.push(Action::CreateIssue { request_id, draft: state.draft.clone() });
}
