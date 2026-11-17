//! Panel shortcuts. Runs before the panel draws, so it can take keys away from text edits.

use eframe::egui::{self, Key, Modifiers};
use model::{BoardData, Issue};

use super::{DESC_ID, PanelAction, PanelState, PickerKind, id};

pub fn handle(ctx: &egui::Context, state: &mut PanelState, issue: &Issue, data: &BoardData, out: &mut Vec<PanelAction>) {
    // The open picker reads Up, Down, Enter and Esc itself.
    if state.picker.is_some() {
        return;
    }
    let focused = ctx.memory(|m| m.focused());
    let typing = ctx.text_edit_focused();

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        let editor = if typing { focused } else { state.last_focus };
        let typing = editor.is_some();
        if state.title_edit.is_some() {
            take(ctx, Modifiers::NONE, Key::Escape);
            state.title_edit = None;
            surrender(ctx, focused);
        } else if editor == Some(id(DESC_ID)) {
            // Esc only blurs the description. It saves on focus loss.
            take(ctx, Modifiers::NONE, Key::Escape);
            surrender(ctx, focused);
        } else if !typing {
            take(ctx, Modifiers::NONE, Key::Escape);
            out.push(PanelAction::Close);
        }
        // Otherwise a comment editor or the composer has focus and handles Esc itself.
    }

    // The board handles Cmd+C and Cmd+Shift+C for the selected issue.
    if typing || ctx.input(|i| i.modifiers.any()) {
        return;
    }
    let pressed = |k: Key| ctx.input(|i| i.key_pressed(k));
    if pressed(Key::E) {
        state.desc_focus = true;
    }
    for (k, kind) in [
        (Key::S, PickerKind::Status),
        (Key::P, PickerKind::Priority),
        (Key::A, PickerKind::Assignee),
        (Key::L, PickerKind::Labels),
    ] {
        if pressed(k) {
            state.open_picker(kind, issue, data);
        }
    }
}

fn take(ctx: &egui::Context, m: Modifiers, k: Key) -> bool {
    ctx.input_mut(|i| i.consume_key(m, k))
}

fn surrender(ctx: &egui::Context, focused: Option<egui::Id>) {
    if let Some(f) = focused {
        ctx.memory_mut(|m| m.surrender_focus(f));
    }
}
