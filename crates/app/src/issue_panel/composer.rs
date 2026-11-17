//! Bottom composer (design v3 03). Keeps one draft per issue.
//! Upload placeholders stay at the end of the draft and show as chips, not as text.

use std::collections::HashMap;

use eframe::egui::{
    Align, CornerRadius, Id, Key, Layout, Margin, Modifiers, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2,
    vec2,
};
use model::{BoardData, Issue};

use super::icons::{self, Ic};
use super::{PanelAction, paste, widgets};
use crate::rich;
use crate::theme::parts::{self, Btn, CMD};
use crate::theme::{Theme, W};

const HINT: &str = "Leave a comment. Paste an image to attach it.";
const AVATAR: f32 = 26.0;
/// The input grows with the draft up to six lines, then scrolls.
const MAX_TEXT_H: f32 = 6.0 * rich::COMMENT.line;

#[derive(Default)]
pub struct ComposerState {
    /// Draft text by issue id. It can end with upload placeholders.
    drafts: HashMap<String, String>,
}

/// One row: viewer avatar, input, Comment button. The row centers on the input.
pub fn show(
    ui: &mut Ui,
    t: &Theme,
    st: &mut ComposerState,
    issue: &Issue,
    data: &BoardData,
    next_request_id: &mut u64,
    out: &mut Vec<PanelAction>,
) {
    let text = st.drafts.entry(issue.id.clone()).or_default();
    let (mut visible, mut pending) = split_pending(text);
    let id = Id::unique("lb-panel-composer").with(&issue.id);
    let mut send = false;

    let p = ui.painter().clone();
    let key = format!("{CMD} Enter");
    let btn = vec2(
        30.0 + parts::text_size(&p, "Comment", W::SemiBold, 12.0).x + parts::text_size(&p, &key, W::MonoMedium, 10.0).x,
        parts::text_size(&p, "Comment", W::SemiBold, 12.0).y + 12.0,
    );
    let full = ui.available_rect_before_wrap();
    let box_rect = Rect::from_min_max(
        pos2(full.left() + AVATAR + 12.0, full.top()),
        pos2(full.right() - btn.x - 12.0, full.bottom()),
    );
    let mut child = ui.new_child(UiBuilder::new().max_rect(box_rect).layout(Layout::top_down(Align::Min)));
    let mut box_ui = widgets::editor_frame(t, false, (11, 14, 11, 14), 10).begin(&mut child);
    {
        let ui = &mut box_ui.content_ui;
        ui.spacing_mut().item_spacing.y = 10.0;
        // Taken before the editor, so Ctrl+Enter does not also add a line.
        if ui.memory(|m| m.has_focus(id)) {
            send = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        }
        let opts = rich::Options { look: rich::COMMENT, placeholder: HINT, min_rows: 1, ..Default::default() };
        let r = ScrollArea::vertical()
            .id_salt(id.with("scroll"))
            .max_height(MAX_TEXT_H)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                rich::editor(ui, t, id, &mut visible, &opts)
            })
            .inner;
        if r.focused {
            box_ui.frame.stroke = Stroke::new(1.0, t.accent_color);
        }
        if let Some(url) = r.open_url.clone() {
            out.push(PanelAction::OpenUrl(url));
        }
        let resp = r.response;
        pending.extend(paste::take_images(ui, &resp, next_request_id, out));
        let mut cancel = None;
        for name in &pending {
            if chip(ui, t, name) {
                cancel = Some(name.clone());
            }
        }
        if let Some(name) = cancel {
            pending.retain(|n| *n != name);
        }
    }
    let input = box_ui.end(&mut child).rect;
    ui.allocate_rect(Rect::from_min_max(full.min, pos2(full.right(), input.bottom())), Sense::hover());

    let me = data.viewer_id.as_deref().and_then(|v| super::user_badge(data, t, v));
    let who = me.as_ref().map(|(ini, c, _)| (ini.as_str(), *c));
    let url = data.viewer_id.as_deref().and_then(|v| super::user_avatar(data, v));
    let c = pos2(full.left() + AVATAR / 2.0, input.center().y);
    parts::avatar_photo(&p, ui.ctx(), t, c, AVATAR, url.as_deref(), who);

    let can_send = !visible.trim().is_empty() && pending.is_empty();
    let btn_rect = Rect::from_center_size(pos2(full.right() - btn.x / 2.0, input.center().y), btn);
    let clicked = ui
        .scope_builder(UiBuilder::new().max_rect(btn_rect), |ui| {
            parts::button(ui, t, "Comment", Btn::Primary, Some(&key), can_send).clicked()
        })
        .inner;
    send |= clicked;

    if send && can_send {
        out.push(PanelAction::CreateComment { body: visible.trim().to_string(), parent_id: None });
        st.drafts.remove(&issue.id);
        ui.ctx().memory_mut(|m| m.surrender_focus(id));
        return;
    }
    *text = join_pending(&visible, &pending);
}

/// The draft of an issue, for upload results.
#[allow(dead_code)] // Panel API. The panel uses `for_each_buffer` today.
pub fn buffer_mut<'a>(st: &'a mut ComposerState, issue_id: &str) -> Option<&'a mut String> {
    st.drafts.get_mut(issue_id)
}

/// Visits the drafts of all issues. An upload can finish after the user opened another issue.
pub fn for_each_buffer(st: &mut ComposerState, mut f: impl FnMut(&mut String)) {
    st.drafts.values_mut().for_each(|s| f(s));
}

/// Sets a draft for screenshots. `pending_upload_name` adds an upload chip.
pub fn set_draft_for_mock(st: &mut ComposerState, issue_id: &str, text: &str, pending_upload_name: Option<&str>) {
    let pending: Vec<String> = pending_upload_name.map(String::from).into_iter().collect();
    st.drafts.insert(issue_id.to_string(), join_pending(text, &pending));
}

/// Splits a draft into the text the user edits and the names of pending uploads.
fn split_pending(text: &str) -> (String, Vec<String>) {
    let names = paste::pending_uploads(text);
    let mut visible = text.to_string();
    for n in &names {
        paste::finish_upload(&mut visible, 0, n, None);
    }
    (visible, names)
}

fn join_pending(visible: &str, names: &[String]) -> String {
    let mut s = visible.to_string();
    for n in names {
        s.push('\n');
        s.push_str(&paste::placeholder(n));
    }
    s
}

/// Upload chip: thumbnail, file name, progress, cancel. Returns true on cancel.
fn chip(ui: &mut Ui, t: &Theme, name: &str) -> bool {
    let mut cancel = false;
    eframe::egui::Frame::NONE
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.hl_low))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 6.0);
                let (r, _) = ui.allocate_exact_size(vec2(44.0, 30.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(3), t.overlay);
                icons::paint(ui.painter(), r.center(), Ic::Image, 14.0, t.muted);
                ui.vertical(|ui| {
                    widgets::text(ui, name, W::MonoMedium, 12.0, t.subtle);
                    widgets::progress(ui, t, 120.0, 3.0, None);
                });
                widgets::text(ui, &widgets::upload_label(None), W::Medium, 11.0, t.muted);
                cancel = widgets::icon_button(ui, t, Ic::X, t.muted).on_hover_text("Cancel upload").clicked();
            });
        });
    cancel
}

/// Image icon plus "Attach image", muted until hovered.
pub(super) fn attach_link(ui: &mut Ui, t: &Theme) -> eframe::egui::Response {
    widgets::icon_link(ui, t, Ic::Image, "Attach image")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_join_round_trip() {
        let names = vec!["paste-1.png".to_string(), "b.png".to_string()];
        let text = join_pending("Pushed the fix:", &names);
        assert_eq!(text, "Pushed the fix:\n![Uploading paste-1.png...]()\n![Uploading b.png...]()");
        assert_eq!(split_pending(&text), ("Pushed the fix:".to_string(), names));
    }

    #[test]
    fn finished_upload_moves_into_the_visible_text() {
        let mut text = join_pending("Fix", &["a.png".to_string()]);
        paste::finish_upload(&mut text, 1, "a.png", Some("https://x/a.png"));
        assert_eq!(split_pending(&text), ("Fix\n![a.png](https://x/a.png)".to_string(), vec![]));
    }
}
