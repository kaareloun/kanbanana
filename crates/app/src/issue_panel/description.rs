//! Description: always an inline rich editor, like in Linear. It saves 1 s after the last
//! keystroke and when it loses focus.

use std::cell::Cell;

use eframe::egui::{Color32, CornerRadius, Frame, Margin, Stroke, Ui};
use model::{Issue, IssueChange};

use super::{DESC_ID, DescMode, PanelAction, PanelState, editor, id, paste, widgets};
use crate::rich::commands::char_to_byte;
use crate::theme::{Theme, W};

const SAVE_AFTER: f64 = 1.0;

/// The editor text between frames, kept for the issue it belongs to. A sync that changes
/// the issue does not overwrite it while the user types.
#[derive(Clone, Default)]
struct Buf {
    issue_id: String,
    text: String,
    /// The issue description as last seen.
    server: String,
    /// The server text changed and the editor did not take it yet.
    server_new: bool,
    /// The text of the last save, or the server text the editor took.
    saved: String,
    last_edit: f64,
    /// This code put the text into `state.desc` last frame, so `on_uploaded` can change it.
    mirrored: bool,
}

impl Buf {
    fn unsaved(&self) -> bool {
        self.text.trim_end() != self.saved.trim_end()
    }
}

thread_local! {
    static DEMO_SELECT: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

/// Demo screens only: the char range the editor selects when it opens.
pub fn demo_select(anchor: usize, caret: usize) {
    DEMO_SELECT.with(|c| c.set(Some((anchor, caret))));
}

pub fn show(ui: &mut Ui, t: &Theme, state: &mut PanelState, issue: &Issue, out: &mut Vec<PanelAction>) {
    let key = id("lb-desc-buffer");
    let mut b: Buf = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    let ed = id(DESC_ID);
    let focused = ui.memory(|m| m.has_focus(ed));
    let now = ui.input(|i| i.time);
    let panel_text = match &state.desc {
        DescMode::Edit { buffer, .. } => Some(buffer.clone()),
        DescMode::View => None,
    };

    if b.issue_id != issue.id {
        flush(&b, out);
        let text = panel_text.unwrap_or_else(|| issue.description.clone());
        let d = issue.description.clone();
        b = Buf { issue_id: issue.id.clone(), text, server: d.clone(), saved: d, last_edit: now, ..Default::default() };
    } else {
        if b.mirrored
            && let Some(text) = panel_text
        {
            b.text = text;
        }
        if issue.description != b.server {
            b.server = issue.description.clone();
            b.server_new = true;
        }
        if b.server_new && !focused && !b.unsaved() {
            b.text = b.server.clone();
            b.saved = b.server.clone();
            b.server_new = false;
        }
    }

    // The first frame of the panel area is a sizing pass that takes no focus. The
    // requests wait for a frame that can take them.
    let live = ui.is_enabled() && ui.is_visible();
    let request_focus = live && std::mem::take(&mut state.desc_focus);
    let demo = if live { DEMO_SELECT.with(Cell::take) } else { None };
    let cursor = if live { state.desc_cursor.take() } else { None };
    let select = match (demo, cursor) {
        (Some((a, c)), _) => Some(char_to_byte(&b.text, a)..char_to_byte(&b.text, c)),
        (None, Some(c)) => Some(char_to_byte(&b.text, c)..char_to_byte(&b.text, c)),
        (None, None) => None,
    };

    widgets::section_head(ui, t, "Description", |_| {});
    // Negative outer margin keeps the text in line with the rest of the panel, so the
    // frame does not move it when it shows.
    let frame = Frame::NONE
        .inner_margin(Margin::symmetric(12, 8))
        .outer_margin(Margin::symmetric(-12, -8))
        .corner_radius(CornerRadius::same(10))
        .stroke(Stroke::new(1.0, Color32::TRANSPARENT));
    let mut prepared = frame.begin(ui);
    let r = {
        let inner = &mut prepared.content_ui;
        inner.set_width(inner.available_width());
        editor::show(inner, t, state, &mut b.text, request_focus, select, out)
    };
    let rect = prepared.content_ui.min_rect().expand2(eframe::egui::vec2(12.0, 8.0));
    if r.focused {
        prepared.frame = prepared.frame.stroke(Stroke::new(1.0, t.hl_med));
    } else if ui.rect_contains_pointer(rect) {
        prepared.frame = prepared.frame.fill(t.hl_low);
    }
    prepared.end(ui);
    if let Some(err) = &state.upload_error {
        widgets::wrapped(ui, err, W::Medium, 12.0, 18.0, t.love);
    }

    if r.changed {
        b.last_edit = now;
    }
    if b.unsaved() {
        let waited = now - b.last_edit;
        if r.lost_focus || !r.focused || waited >= SAVE_AFTER {
            out.push(PanelAction::Change(IssueChange::Description(b.text.clone())));
            b.saved = b.text.clone();
        } else {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(SAVE_AFTER - waited));
        }
    }

    // `on_uploaded` writes into `state.desc`, so the text stays there while it may change.
    b.mirrored = r.focused || !paste::pending_uploads(&b.text).is_empty();
    state.desc = if b.mirrored { DescMode::Edit { buffer: b.text.clone(), preview: false } } else { DescMode::View };
    ui.data_mut(|d| d.insert_temp(key, b));
}

/// Saves an unsaved edit of an issue that is no longer open.
fn flush(b: &Buf, out: &mut Vec<PanelAction>) {
    if !b.issue_id.is_empty() && b.unsaved() {
        out.push(PanelAction::ChangeFor { issue_id: b.issue_id.clone(), change: IssueChange::Description(b.text.clone()) });
    }
}
