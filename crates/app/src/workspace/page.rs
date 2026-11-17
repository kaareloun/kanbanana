//! Workspaces page (design v3 04): steps on the left, the add form on the right,
//! and the workspaces you have under the steps.

use std::collections::HashMap;

use eframe::egui::{
    Align, Align2, CentralPanel, Color32, CornerRadius, CursorIcon, Event, Frame, Id, Key, Layout, Margin,
    Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, pos2, vec2,
    text::{LayoutJob, TextFormat},
};
use model::{BoardData, Workspace};

use super::{ACCENTS, AddForm, KeyCheck, letter, slugify};
use crate::actions::Action;
use crate::theme::parts::{self, Btn, Icon};
use crate::theme::{Theme, W, font};

const LEFT_W: f32 = 360.0;
const FORM_W: f32 = 560.0;
const COL_GAP: f32 = 72.0;
const FORM_PAD: f32 = 28.0;
const FORM_GAP: f32 = 24.0;
const LABEL_H: f32 = 15.0;
const KEY_H: f32 = 44.0;
const FIELD_H: f32 = 40.0;
const SHORTCUT_W: f32 = 200.0;
const PREVIEW_H: f32 = 72.0;
const NUM_KEYS: [Key; 9] =
    [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];

pub fn show(
    ui: &mut Ui,
    t: &Theme,
    workspaces: &[Workspace],
    boards: &HashMap<String, BoardData>,
    form: &mut AddForm,
    actions: &mut Vec<Action>,
) {
    // The page takes the color the user picks for the new workspace.
    let t = &t.with_accent(form.accent);
    let name_id = Id::unique("lb-ws-name");
    keys(ui, form, workspaces, name_id, true, actions);

    CentralPanel::default().frame(Frame::NONE.fill(t.base)).show(ui, |ui| {
        let full = ui.max_rect();
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let avail = ui.max_rect();
            let total = LEFT_W + COL_GAP + FORM_W;
            let stacked = avail.width() < total + 64.0;
            let left = if stacked {
                avail.left() + ((avail.width() - FORM_W) / 2.0).max(32.0)
            } else {
                avail.left() + (avail.width() - total) / 2.0
            };
            let top = avail.top() + ((avail.height() - 600.0) / 2.0).clamp(64.0, 150.0);
            let intro_h = intro(ui, t, pos2(left, top), workspaces, boards, form, actions);
            let at = if stacked { pos2(left, top + intro_h + 40.0) } else { pos2(left + LEFT_W + COL_GAP, top) };
            let form_h = add_form(ui, t, at, FORM_W, workspaces, form, name_id, true, actions);
            let bottom = (top + intro_h).max(at.y + form_h) + 48.0;
            ui.allocate_rect(Rect::from_min_max(avail.min, pos2(avail.left() + 1.0, bottom)), Sense::hover());
        });
        if !workspaces.is_empty() {
            let r = Rect::from_min_max(pos2(full.right() - 240.0, full.top() + 14.0), pos2(full.right() - 40.0, full.top() + 46.0));
            ui.scope_builder(UiBuilder::new().max_rect(r).layout(Layout::right_to_left(Align::Center)), |ui| {
                if parts::button(ui, t, "Back to board", Btn::Ghost, Some("Esc"), true).clicked() {
                    actions.push(Action::CloseWorkspaces);
                }
            });
        }
    });
}

/// The add form without its buttons, for the onboarding flow. Returns its height.
pub(crate) fn embedded_form(
    ui: &mut Ui,
    t: &Theme,
    at: Pos2,
    w: f32,
    workspaces: &[Workspace],
    form: &mut AddForm,
    actions: &mut Vec<Action>,
) -> f32 {
    let name_id = Id::unique("lb-ws-name");
    keys(ui, form, workspaces, name_id, false, actions);
    add_form(ui, t, at, w, workspaces, form, name_id, false, actions)
}

fn keys(ui: &Ui, form: &mut AddForm, workspaces: &[Workspace], name_id: Id, escape_closes: bool, actions: &mut Vec<Action>) {
    let name_focused = ui.memory(|m| m.has_focus(name_id));
    if form.capturing {
        ui.input(|i| {
            if i.key_pressed(Key::Escape) {
                form.capturing = false;
            }
            if let Some(n) = NUM_KEYS.iter().position(|k| i.key_pressed(*k)) {
                form.shortcut = n as u8 + 1;
                form.capturing = false;
            }
        });
        return;
    }
    if escape_closes && ui.input(|i| i.key_pressed(Key::Escape)) && !name_focused {
        actions.push(Action::CloseWorkspaces);
    }
    if ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)) {
        submit(form, workspaces, actions);
        return;
    }
    if name_focused {
        return;
    }
    // The key field takes paste and typing whenever no other field has focus.
    let before = form.api_key.clone();
    let mut pasted = false;
    let ctrl = ui.input(|i| i.modifiers.command || i.modifiers.ctrl || i.modifiers.alt);
    for e in ui.input(|i| i.events.clone()) {
        match e {
            Event::Paste(s) => {
                form.api_key = s.trim().to_string();
                pasted = true;
            }
            Event::Text(s) if !ctrl => form.api_key.push_str(s.trim()),
            Event::Key { key: Key::Backspace, pressed: true, .. } => {
                form.api_key.pop();
            }
            _ => {}
        }
    }
    if form.api_key != before {
        if form.api_key.is_empty() {
            form.check = KeyCheck::Idle;
        } else if pasted || form.api_key.len() >= 48 {
            form.check = KeyCheck::Checking;
            actions.push(Action::CheckKey(form.api_key.clone()));
        } else {
            form.check = KeyCheck::Idle;
        }
    }
}

pub(crate) fn can_add(form: &AddForm, workspaces: &[Workspace]) -> bool {
    let slug = slugify(&form.name);
    matches!(form.check, KeyCheck::Ok { .. }) && !slug.is_empty() && workspaces.iter().all(|w| w.slug != slug)
}

pub(crate) fn submit(form: &mut AddForm, workspaces: &[Workspace], actions: &mut Vec<Action>) {
    if !can_add(form, workspaces) {
        return;
    }
    let workspace = Workspace {
        slug: slugify(&form.name),
        name: form.name.trim().to_string(),
        shortcut: form.shortcut,
        accent: form.accent,
        viewer_id: None,
        position: workspaces.iter().map(|w| w.position).max().unwrap_or(0) + 1,
    };
    actions.push(Action::AddWorkspace { workspace, api_key: form.api_key.clone() });
}

/// Paints wrapped text with its top left at `at`. Returns its height.
#[allow(clippy::too_many_arguments)]
pub(crate) fn para(ui: &Ui, at: Pos2, s: &str, w: W, size: f32, line_h: f32, width: f32, c: Color32) -> f32 {
    let mut job = LayoutJob::single_section(
        s.to_string(),
        TextFormat { font_id: font(w, size), color: c, line_height: Some(line_h), ..Default::default() },
    );
    job.wrap.max_width = width;
    let g = ui.painter().layout_job(job);
    let h = g.size().y;
    ui.painter().galley(at, g, c);
    h
}

/// Title, one line of intro, the three steps and the list of workspaces. Returns its height.
fn intro(
    ui: &mut Ui,
    t: &Theme,
    at: Pos2,
    workspaces: &[Workspace],
    boards: &HashMap<String, BoardData>,
    form: &mut AddForm,
    actions: &mut Vec<Action>,
) -> f32 {
    let p = ui.painter().clone();
    let x = at.x;
    let mut y = at.y;
    let first_run = workspaces.is_empty();
    if first_run {
        crate::brand::icon(&p, Rect::from_min_size(pos2(x, y), vec2(72.0, 72.0)));
        y += 72.0 + 20.0;
    }
    let title = if first_run { "Welcome to Kanbanana" } else { "Add a workspace" };
    let r = parts::text(&p, pos2(x, y), Align2::LEFT_TOP, title, W::Bold, 28.0, t.text);
    y = r.max.y + 24.0;
    let intro = if first_run {
        "Add a Linear workspace to start."
    } else {
        "Each workspace is one Linear organization with its own key."
    };
    y += para(ui, pos2(x, y), intro, W::Regular, 14.0, 22.0, 340.0, t.subtle) + 24.0;

    for n in 0..STEPS.len() {
        y += instruction(ui, t, pos2(x, y), n, 300.0, form) + 22.0;
    }

    if !workspaces.is_empty() {
        y += 14.0;
        let r = parts::text(&p, pos2(x, y), Align2::LEFT_TOP, "Your workspaces", W::SemiBold, 12.0, t.subtle);
        y = r.max.y + 10.0;
        let rows = Rect::from_min_size(pos2(x, y), vec2(LEFT_W, 56.0 * workspaces.len() as f32));
        p.rect_filled(rows, CornerRadius::same(12), t.surface);
        for (i, ws) in workspaces.iter().enumerate() {
            let row = Rect::from_min_size(pos2(x, y), vec2(LEFT_W, 56.0));
            ws_row(ui, t, row, ws, boards.get(&ws.slug), form, actions);
            if i + 1 < workspaces.len() {
                p.hline(row.x_range().shrink(12.0), row.bottom() - 0.5, Stroke::new(1.0, t.hl_low));
            }
            y += 56.0;
        }
    }
    y - at.y
}

/// How to get a key, in three steps.
const STEPS: [(&str, &str); 3] = [
    ("Create a key", "In Linear, open Settings, then Security and access, then Personal API keys."),
    ("Paste it here", "The key stays on this computer, in a file only you can read."),
    ("Pick a key and a color", "The shortcut switches to this workspace from anywhere in the app."),
];

/// Step `n` of `STEPS`: number circle, title and text `text_w` wide. The circle lights up
/// when the form has done the step. Returns its height.
pub(crate) fn instruction(ui: &Ui, t: &Theme, at: Pos2, n: usize, text_w: f32, form: &AddForm) -> f32 {
    let p = ui.painter();
    let (title, text) = STEPS[n];
    let done = [!form.api_key.is_empty(), matches!(form.check, KeyCheck::Ok { .. }), false][n];
    let c = pos2(at.x + 13.0, at.y + 13.0);
    let (fill, tc) = if done { (Theme::tint(t.accent_color, 0.14), t.accent_color) } else { (t.overlay, t.subtle) };
    p.circle_filled(c, 13.0, fill);
    parts::text(p, c, Align2::CENTER_CENTER, &(n + 1).to_string(), W::Bold, 12.0, tc);
    let r = parts::text(p, pos2(at.x + 40.0, at.y), Align2::LEFT_TOP, title, W::SemiBold, 14.0, t.text);
    let h = para(ui, pos2(at.x + 40.0, r.max.y + 4.0), text, W::Regular, 13.0, 20.0, text_w, t.subtle);
    (r.max.y + 4.0 + h - at.y).max(26.0)
}

fn ws_row(
    ui: &mut Ui,
    t: &Theme,
    row: Rect,
    ws: &Workspace,
    board: Option<&BoardData>,
    form: &mut AddForm,
    actions: &mut Vec<Action>,
) {
    let p = ui.painter().clone();
    let mid = row.center().y;
    let accent = t.accent(ws.accent);
    let tile = Rect::from_min_size(pos2(row.left() + 12.0, mid - 16.0), vec2(32.0, 32.0));
    p.rect_filled(tile, CornerRadius::same(10), Theme::tint(accent, 0.18));
    parts::text(&p, tile.center(), Align2::CENTER_CENTER, &letter(&ws.name), W::Bold, 14.0, accent);
    let meta = match board {
        Some(b) => {
            let teams = if b.teams.len() == 1 { "1 team".to_string() } else { format!("{} teams", b.teams.len()) };
            format!("{teams}, {} issues", b.issues.len())
        }
        None => "Not synced yet".to_string(),
    };
    let x = tile.right() + 12.0;
    parts::text(&p, pos2(x, mid - 1.0), Align2::LEFT_BOTTOM, &ws.name, W::SemiBold, 14.0, t.text);
    parts::text(&p, pos2(x, mid + 1.0), Align2::LEFT_TOP, &meta, W::Medium, 12.0, t.muted);

    let right = Rect::from_min_max(pos2(row.center().x - 20.0, row.top()), pos2(row.right() - 10.0, row.bottom()));
    ui.scope_builder(UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let confirm = form.pending_remove.as_deref() == Some(ws.slug.as_str());
        let text = if confirm { "Confirm remove" } else { "Remove" };
        if parts::button(ui, t, text, if confirm { Btn::Secondary } else { Btn::Ghost }, None, true).clicked() {
            if confirm {
                form.pending_remove = None;
                actions.push(Action::RemoveWorkspace(ws.slug.clone()));
            } else {
                form.pending_remove = Some(ws.slug.clone());
            }
        }
        if !confirm {
            let p = ui.painter().clone();
            let key = format!("{} {}", parts::ALT, ws.shortcut);
            let (r, _) = ui.allocate_exact_size(parts::kbd_size(&p, &key), Sense::hover());
            parts::kbd(&p, t, r.left_center(), &key);
        }
    });
}

fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 16 {
        return "•".repeat(chars.len());
    }
    let head: String = chars[..12].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}{}{tail}", "•".repeat(14))
}


fn field_label(p: &eframe::egui::Painter, t: &Theme, at: Pos2, s: &str) {
    parts::text(p, at, Align2::LEFT_TOP, s, W::SemiBold, 12.0, t.subtle);
}

fn input_box(p: &eframe::egui::Painter, t: &Theme, rect: Rect, focus: bool) {
    let stroke = if focus { Stroke::new(1.5, t.accent_color) } else { Stroke::new(1.0, t.hl_med) };
    p.rect(rect, CornerRadius::same(9), t.base, stroke, StrokeKind::Inside);
}

fn form_height(buttons: bool) -> f32 {
    let key = LABEL_H + 8.0 + KEY_H + 8.0 + 16.0;
    let row = LABEL_H + 8.0 + FIELD_H;
    let color = LABEL_H + 8.0 + 30.0;
    let base = 2.0 * FORM_PAD + key + row + color + PREVIEW_H + 3.0 * FORM_GAP;
    if buttons { base + FORM_GAP + 30.0 } else { base }
}

/// The form card, `width` wide with its top left at `at`. Returns its height.
#[allow(clippy::too_many_arguments)]
fn add_form(
    ui: &mut Ui,
    t: &Theme,
    at: Pos2,
    width: f32,
    workspaces: &[Workspace],
    form: &mut AddForm,
    name_id: Id,
    buttons: bool,
    actions: &mut Vec<Action>,
) -> f32 {
    let h = form_height(buttons);
    let card = Rect::from_min_size(at, vec2(width, h));
    let p = ui.painter().clone();
    p.rect_filled(card, CornerRadius::same(16), t.surface);
    let x = card.left() + FORM_PAD;
    let w = width - 2.0 * FORM_PAD;
    let mut y = card.top() + FORM_PAD;
    let name_focused = ui.memory(|m| m.has_focus(name_id));
    let key_active = !name_focused && !form.capturing;

    // API key. The page reads paste and typing into it whenever no other field has focus.
    field_label(&p, t, pos2(x, y), "API key");
    y += LABEL_H + 8.0;
    let rect = Rect::from_min_size(pos2(x, y), vec2(w, KEY_H));
    let resp = ui.interact(rect, Id::unique("lb-ws-key"), Sense::click()).on_hover_cursor(CursorIcon::Text);
    if resp.clicked() {
        form.capturing = false;
        ui.memory_mut(|m| m.surrender_focus(name_id));
    }
    input_box(&p, t, rect, key_active);
    let mid = rect.center().y;
    let kc = if key_active { t.accent_color } else { t.muted };
    parts::icon(&p, pos2(rect.left() + 14.0 + 7.5, mid), Icon::Key, 15.0, kc);
    let tx = pos2(rect.left() + 39.0, mid);
    if form.api_key.is_empty() {
        parts::text(&p, tx, Align2::LEFT_CENTER, "Paste your key", W::Medium, 14.0, t.muted);
    } else {
        parts::text(&p, tx, Align2::LEFT_CENTER, &mask(&form.api_key), W::MonoMedium, 14.0, t.text);
    }
    let paste_hint = format!("{} V", parts::CMD);
    let ks = parts::kbd_size(&p, &paste_hint);
    parts::kbd(&p, t, pos2(rect.right() - 14.0 - ks.x, mid), &paste_hint);
    y += KEY_H + 8.0;
    check_line(&p, t, pos2(x, y + 8.0), w, &form.check);
    y += 16.0 + FORM_GAP;

    // Name and shortcut.
    let name_w = w - 16.0 - SHORTCUT_W;
    field_label(&p, t, pos2(x, y), "Name");
    field_label(&p, t, pos2(x + name_w + 16.0, y), "Shortcut");
    y += LABEL_H + 8.0;
    let rect = Rect::from_min_size(pos2(x, y), vec2(name_w, FIELD_H));
    input_box(&p, t, rect, name_focused);
    let edit = TextEdit::singleline(&mut form.name)
        .id(name_id)
        .frame(Frame::NONE)
        .font(font(W::Medium, 14.0))
        .text_color(t.text)
        .hint_text("Workspace name")
        .margin(Margin::ZERO);
    ui.put(rect.shrink2(vec2(14.0, 11.0)), edit);
    let rect = Rect::from_min_size(pos2(x + name_w + 16.0, y), vec2(SHORTCUT_W, FIELD_H));
    let resp = ui.interact(rect, Id::unique("lb-ws-shortcut"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    if resp.clicked() {
        form.capturing = true;
        ui.memory_mut(|m| m.surrender_focus(name_id));
    }
    input_box(&p, t, rect, form.capturing);
    let k = parts::kbd(&p, t, pos2(rect.left() + 14.0, rect.center().y), &format!("{} {}", parts::ALT, form.shortcut));
    let hint = if form.capturing { "Press a number" } else { "Press keys" };
    parts::text(&p, pos2(k.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, hint, W::Medium, 12.0, t.muted);
    y += FIELD_H + FORM_GAP;

    // Color.
    field_label(&p, t, pos2(x, y), "Color");
    y += LABEL_H + 8.0;
    for (i, a) in ACCENTS.into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(x + i as f32 * 36.0, y), vec2(30.0, 30.0));
        let resp = ui.interact(r, Id::unique(("lb-ws-color", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let c = t.accent(a);
        if form.accent == a {
            p.rect_stroke(r, CornerRadius::same(10), Stroke::new(2.0, c), StrokeKind::Inside);
        } else if resp.hovered() {
            p.rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, t.hl_high), StrokeKind::Inside);
        }
        p.rect_filled(r.shrink(3.0), CornerRadius::same(7), c);
        if resp.clicked() {
            form.accent = a;
        }
    }
    y += 30.0 + FORM_GAP;

    preview(&p, t, pos2(x, y), form);
    y += PREVIEW_H + FORM_GAP;
    if !buttons {
        return h;
    }

    // Actions.
    let row = Rect::from_min_size(pos2(x, y), vec2(w, 30.0));
    ui.scope_builder(UiBuilder::new().max_rect(row).layout(Layout::right_to_left(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let ok = can_add(form, workspaces);
        let hint = format!("{} Enter", parts::CMD);
        if parts::button(ui, t, "Add workspace", Btn::Primary, Some(&hint), ok).clicked() && ok {
            submit(form, workspaces, actions);
        }
        if parts::button(ui, t, "Cancel", Btn::Ghost, Some("Esc"), true).clicked() {
            *form = AddForm::fresh(workspaces);
            actions.push(Action::CloseWorkspaces);
        }
    });
    h
}

/// The rail tile the new workspace gets, with its name and shortcut.
fn preview(p: &eframe::egui::Painter, t: &Theme, at: Pos2, form: &AddForm) {
    let name = form.name.trim();
    let (name, nc) = if name.is_empty() { ("Workspace name", t.muted) } else { (name, t.text) };
    let sub = format!("{} {} switches here", parts::ALT, form.shortcut);
    let tw = parts::text_size(p, name, W::SemiBold, 15.0).x.max(parts::text_size(p, &sub, W::Medium, 12.0).x);
    let rect = Rect::from_min_size(at, vec2(14.0 + 44.0 + 14.0 + tw + 14.0, PREVIEW_H));
    p.rect_filled(rect, CornerRadius::same(12), t.base);
    let tile = Rect::from_min_size(pos2(at.x + 14.0, at.y + 14.0), vec2(44.0, 44.0));
    let a = t.accent_color;
    p.rect(tile, CornerRadius::same(12), Theme::tint(a, 0.18), Stroke::new(1.5, a), StrokeKind::Inside);
    parts::text(p, tile.center(), Align2::CENTER_CENTER, &letter(name), W::Bold, 17.0, a);
    let tx = tile.right() + 14.0;
    let mid = tile.center().y;
    parts::text(p, pos2(tx, mid + 1.0), Align2::LEFT_BOTTOM, name, W::SemiBold, 15.0, nc);
    parts::text(p, pos2(tx, mid + 3.0), Align2::LEFT_TOP, &sub, W::Medium, 12.0, t.muted);
}

/// One line under the key: what Linear said about it.
fn check_line(p: &eframe::egui::Painter, t: &Theme, left_center: Pos2, width: f32, check: &KeyCheck) {
    let (icon, color, text) = match check {
        KeyCheck::Idle => (None, t.muted, "Linear checks the key as soon as you paste it.".to_string()),
        KeyCheck::Checking => (None, t.gold, "Checking the key with Linear.".to_string()),
        KeyCheck::Ok { workspace, user, teams } => {
            let teams = match teams.as_slice() {
                [] => String::new(),
                [one] => format!(", team {one}"),
                [rest @ .., last] => format!(", teams {} and {last}", rest.join(", ")),
            };
            (Some(Icon::Check), t.foam, format!("Works. {workspace}, signed in as {user}{teams}."))
        }
        KeyCheck::Error(msg) => (Some(Icon::X), t.love, msg.clone()),
    };
    let mut x = left_center.x;
    if let Some(ic) = icon {
        parts::icon(p, pos2(x + 7.0, left_center.y), ic, 14.0, color);
        x += 20.0;
    }
    let mut job = LayoutJob::single_section(text, TextFormat { font_id: font(W::Medium, 12.0), color, ..Default::default() });
    job.wrap = eframe::egui::text::TextWrapping::truncate_at_width(left_center.x + width - x);
    let g = p.layout_job(job);
    p.galley(pos2(x, left_center.y - g.size().y / 2.0), g, color);
}

#[cfg(test)]
mod tests {
    use super::mask;

    #[test]
    fn mask_keeps_head_and_tail() {
        let key = format!("lin_api_7Qm2{}Xk2Q", "a".repeat(32));
        assert_eq!(mask(&key), format!("lin_api_7Qm2{}Xk2Q", "•".repeat(14)));
        assert_eq!(mask("abc"), "•••");
    }
}
