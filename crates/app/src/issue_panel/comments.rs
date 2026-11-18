//! Activity (design v2 timeline): top-level comments in time order, replies indented
//! 30 px under their thread, an inline reply editor. Own comments edit in place, like
//! the description: they save 1 s after the last keystroke and when they lose focus.

use std::collections::HashMap;
use std::ops::Range;

use chrono::{Duration, Local, Utc};
use eframe::egui::{
    Align, Color32, CornerRadius, Frame, Id, Key, Layout, Margin, Modifiers, Response, Sense, Stroke, Ui, vec2,
};
use model::{BoardData, Comment, Issue};

use super::icons::Ic;
use super::{CommentsView, PanelAction, composer, paste, rich_view, widgets};
use crate::rich;
use crate::theme::parts::{self, Btn};
use crate::theme::{Theme, W};

const SAVE_AFTER: f64 = 1.0;

#[derive(Default)]
pub struct CommentsState {
    /// Open reply editor. Its target is the id of the thread's top comment.
    reply: Option<Draft>,
    /// Inline editor of an own comment.
    edit: Option<OwnEdit>,
    /// Saved bodies that the server did not confirm yet, by comment id.
    saving: HashMap<String, Saving>,
}

/// A save in flight. The view shows `text` while the comment still has an old body, so
/// it does not flicker back to the old text.
struct Saving {
    text: String,
    before: Vec<String>,
}

struct OwnEdit {
    issue_id: String,
    comment_id: String,
    text: String,
    /// The text of the last save, or the shown body when the editor opened.
    saved: String,
    last_edit: f64,
    /// Asks the editor to take focus and select this byte range on its next frame.
    focus: Option<Range<usize>>,
    /// The editor closes on focus loss only after it had focus once.
    had_focus: bool,
}

struct Draft {
    issue_id: String,
    target: String,
    text: String,
    /// Asks the editor to take focus on its next frame.
    focus: bool,
}

impl Draft {
    fn new(issue_id: &str, target: &str, text: &str) -> Draft {
        Draft { issue_id: issue_id.into(), target: target.into(), text: text.into(), focus: true }
    }
}

enum Outcome {
    Keep,
    Send,
    Cancel,
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    t: &Theme,
    st: &mut CommentsState,
    issue: &Issue,
    data: &BoardData,
    view: &CommentsView,
    next_request_id: &mut u64,
    out: &mut Vec<PanelAction>,
) {
    if st.reply.as_ref().is_some_and(|d| d.issue_id != issue.id) {
        st.reply = None;
    }
    if st.edit.as_ref().is_some_and(|e| e.issue_id != issue.id) {
        st.edit = None;
    }
    // A save ends when the server sends another body, or fails.
    st.saving.retain(|id, s| view.error.is_none() && view.list.iter().any(|c| &c.id == id && s.before.contains(&c.body)));

    let count = if view.loading && view.list.is_empty() { issue.comment_count.max(0) as usize } else { view.list.len() };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        widgets::text(ui, "Activity", W::SemiBold, 14.0, t.text);
        let n = if count == 1 { "1 comment".to_string() } else { format!("{count} comments") };
        widgets::text(ui, &n, W::Medium, 12.0, t.muted);
    });

    if view.loading {
        widgets::text(ui, "Loading comments", W::Medium, 12.0, t.muted);
    }
    if let Some(err) = &view.error {
        ui.horizontal_wrapped(|ui| {
            widgets::wrapped(ui, &format!("Comments did not load: {err}"), W::Medium, 12.0, 18.0, t.love);
        });
        if widgets::icon_link(ui, t, Ic::Reply, "Try again").clicked() {
            out.push(PanelAction::LoadComments);
        }
    }
    if !view.loading && view.error.is_none() && view.list.is_empty() {
        widgets::text(ui, "No comments yet.", W::Medium, 12.0, t.muted);
    }

    let ids: Vec<&str> = view.list.iter().map(|c| c.id.as_str()).collect();
    let mut tops: Vec<&Comment> = view
        .list
        .iter()
        .filter(|c| c.parent_id.as_deref().is_none_or(|p| !ids.contains(&p)))
        .collect();
    tops.sort_by_key(|c| c.created_at);

    let mut ctx = Ctx { t, st, issue, data, view_list: &view.list, next_request_id, out };
    for top in tops {
        let mut replies: Vec<&Comment> = view
            .list
            .iter()
            .filter(|c| c.parent_id.as_deref() == Some(top.id.as_str()))
            .collect();
        replies.sort_by_key(|c| c.created_at);
        let replying = ctx.st.reply.as_ref().is_some_and(|d| d.target == top.id);
        comment(ui, &mut ctx, top, &top.id);
        for r in &replies {
            indented(ui, REPLY_INDENT, |ui| comment(ui, &mut ctx, r, &top.id));
        }
        if replying {
            indented(ui, REPLY_INDENT + AVATAR + 12.0, |ui| reply_editor(ui, &mut ctx, &top.id));
        }
    }
}

const AVATAR: f32 = 26.0;
const REPLY_INDENT: f32 = 30.0;

fn indented(ui: &mut Ui, left: f32, add: impl FnOnce(&mut Ui)) {
    eframe::egui::Frame::NONE
        .inner_margin(Margin { left: left as i8, right: 0, top: 0, bottom: 0 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// Visits the text of every open editor, for upload results.
pub fn for_each_buffer(st: &mut CommentsState, mut f: impl FnMut(&mut String)) {
    if let Some(d) = &mut st.reply {
        f(&mut d.text);
    }
    if let Some(e) = &mut st.edit {
        f(&mut e.text);
    }
}

/// Demo screens only: opens the editor of an own comment with the caret at byte `caret`.
pub fn demo_edit(st: &mut CommentsState, issue_id: &str, c: &Comment, caret: usize) {
    st.edit = Some(OwnEdit::new(issue_id, c, &c.body, caret..caret));
}

impl OwnEdit {
    fn new(issue_id: &str, c: &Comment, body: &str, select: Range<usize>) -> OwnEdit {
        OwnEdit {
            issue_id: issue_id.into(),
            comment_id: c.id.clone(),
            text: body.into(),
            saved: body.into(),
            last_edit: 0.0,
            focus: Some(select),
            had_focus: false,
        }
    }

    /// The text to save, or None when it is the saved text, empty or waits for uploads.
    fn to_save(&self) -> Option<String> {
        let body = self.text.trim();
        let ok = body != self.saved.trim() && !body.is_empty() && paste::pending_uploads(body).is_empty();
        ok.then(|| body.to_string())
    }
}

/// Sends a new body for comment `c` and shows it until the server confirms it.
fn save(st: &mut CommentsState, out: &mut Vec<PanelAction>, c: &Comment, body: String) {
    let s = st.saving.entry(c.id.clone()).or_insert_with(|| Saving { text: String::new(), before: Vec::new() });
    let old = std::mem::replace(&mut s.text, body.clone());
    for b in [c.body.clone(), old] {
        if !b.is_empty() && b != body && !s.before.contains(&b) {
            s.before.push(b);
        }
    }
    if s.before.is_empty() {
        st.saving.remove(&c.id);
    }
    out.push(PanelAction::UpdateComment { comment_id: c.id.clone(), body });
}

/// The body to show: a save in flight, or the body from the server.
fn shown_body(st: &CommentsState, c: &Comment) -> String {
    match st.saving.get(&c.id) {
        Some(s) if s.before.contains(&c.body) => s.text.clone(),
        _ => c.body.clone(),
    }
}

/// The comments of design 04 on mock issue WEB-214, plus one with a table. Kaarel is
/// the mock viewer, so his comments are editable.
pub fn mock_comments() -> CommentsView {
    let now = Utc::now();
    let c = |id: &str, parent: Option<&str>, user: &str, ago: Duration, edited: bool, body: &str| Comment {
        id: id.into(),
        issue_id: "acme-i-web-214".into(),
        parent_id: parent.map(Into::into),
        user_id: Some(format!("acme-u-{user}")),
        body: body.into(),
        created_at: now - ago,
        updated_at: if edited { now - ago + Duration::minutes(5) } else { now - ago },
        edited,
    };
    CommentsView {
        loading: false,
        error: None,
        list: vec![
            c(
                "mock-c-1",
                None,
                "mt",
                Duration::hours(2),
                false,
                "Confirmed in the sandbox. The second webhook has status PAID because the payment provider \
                 sends the order snapshot, not the refund state.",
            ),
            c(
                "mock-c-2",
                Some("mock-c-1"),
                "ka",
                Duration::hours(1),
                true,
                "Then we ignore order.updated when the order already has a refund. \
                 I will add a check in the webhook handler.",
            ),
            c(
                "mock-c-3",
                None,
                "jk",
                Duration::minutes(40),
                false,
                "Same thing with partial refunds on parcel orders:\n\n\
                 ![orders-4502.png](https://uploads.linear.app/mock/orders-4502.png)",
            ),
            c(
                MOCK_OWN_TABLE,
                None,
                "ka",
                Duration::minutes(50),
                false,
                "Checked the webhooks in the sandbox. **Both arrive**, in this order:\n\n\
                 | \\# | Event | Order status |\n\
                 | -- | -- | -- |\n\
                 | 1 | `refund.created` | **Refunded** |\n\
                 | 2 | `order.updated` | Paid, ~~should stay Refunded~~ |\n\n\
                 Log: <https://sandbox.example.com/logs/4471>. Fix in [the webhook handler](https://github.com/acme/web/pull/88), \
                 review by @mt.",
            ),
        ],
    }
}

/// Id of the mock comment with a table, for the edit demo.
pub const MOCK_OWN_TABLE: &str = "mock-c-4";

/// What the comment rows need, bundled to keep the signatures short.
struct Ctx<'a> {
    t: &'a Theme,
    st: &'a mut CommentsState,
    issue: &'a Issue,
    data: &'a BoardData,
    view_list: &'a [Comment],
    next_request_id: &'a mut u64,
    out: &'a mut Vec<PanelAction>,
}

/// One comment: avatar, head, body or editor, actions.
fn comment(ui: &mut Ui, ctx: &mut Ctx, c: &Comment, thread_id: &str) {
    let t = ctx.t;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 6.0);
        let (rect, _) = ui.allocate_exact_size(vec2(AVATAR, AVATAR), Sense::hover());
        let user = c.user_id.as_deref().and_then(|id| ctx.data.users.iter().position(|u| u.id == id));
        let name = user.map_or("Unknown", |i| ctx.data.users[i].name.as_str());
        let who = user.map(|i| (crate::board::card::initials(name), avatar_color(t, i)));
        let url = user.and_then(|i| ctx.data.users[i].avatar_url.as_deref());
        let who = who.as_ref().map(|(s, col)| (s.as_str(), *col));
        parts::avatar_photo(ui.painter(), ui.ctx(), t, rect.center(), AVATAR, url, who);

        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            head(ui, t, c, name);

            let mine = c.user_id.is_some() && c.user_id == ctx.data.viewer_id;
            if mine {
                own_body(ui, ctx, c);
            } else {
                let body_id = Id::unique("lb-comment-body").with(&c.id);
                if let Some(url) = rich_view::show(ui, t, body_id, &c.body, rich::COMMENT, false).open_url {
                    ctx.out.push(PanelAction::OpenUrl(url));
                }
            }
            actions(ui, ctx, thread_id);
        });
    });
}

fn head(ui: &mut Ui, t: &Theme, c: &Comment, name: &str) {
    ui.allocate_ui_with_layout(vec2(ui.available_width(), AVATAR), Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_height(AVATAR);
        widgets::text(ui, name, W::SemiBold, 13.0, t.text);
        let when = super::time::relative(c.created_at, Utc::now());
        let full = c.created_at.with_timezone(&Local).format("%-d %b %Y %H:%M").to_string();
        widgets::text(ui, &when, W::Medium, 12.0, t.muted).on_hover_text(full);
        if c.edited {
            widgets::text(ui, "edited", W::Medium, 12.0, t.muted);
        }
    });
}

fn actions(ui: &mut Ui, ctx: &mut Ctx, thread_id: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        if widgets::icon_link(ui, ctx.t, Ic::Reply, "Reply").clicked() {
            match &mut ctx.st.reply {
                Some(d) if d.target == thread_id => d.focus = true,
                r => *r = Some(Draft::new(&ctx.issue.id, thread_id, "")),
            }
        }
    });
}

fn reply_editor(ui: &mut Ui, ctx: &mut Ctx, thread_id: &str) {
    let Some(d) = ctx.st.reply.as_mut() else { return };
    let id = Id::unique("lb-comment-reply").with(thread_id);
    match editor(ui, ctx.t, id, d, "Write a reply.", "Reply", ctx.next_request_id, ctx.out) {
        Outcome::Send => {
            let body = d.text.trim().to_string();
            ctx.out.push(PanelAction::CreateComment { body, parent_id: Some(thread_id.to_string()) });
            ctx.st.reply = None;
        }
        Outcome::Cancel => ctx.st.reply = None,
        Outcome::Keep => {}
    }
}

/// An own comment: read-only text that turns into the rich editor on click. Both look
/// the same, with a faint background on hover and a border while editing.
fn own_body(ui: &mut Ui, ctx: &mut Ctx, c: &Comment) {
    let t = ctx.t;
    let editing = ctx.st.edit.as_ref().is_some_and(|e| e.comment_id == c.id);
    // Negative outer margin keeps the text in line with the head, so the frame does not
    // move it when it shows. It also takes back the 1 px of the stroke.
    let frame = Frame::NONE
        .inner_margin(Margin::symmetric(8, 4))
        .outer_margin(Margin::symmetric(-9, -5))
        .corner_radius(CornerRadius::same(8))
        .stroke(Stroke::new(1.0, Color32::TRANSPARENT));
    let mut prepared = frame.begin(ui);
    let focused = {
        let inner = &mut prepared.content_ui;
        inner.set_width(inner.available_width());
        if editing {
            own_editor(inner, ctx, c)
        } else {
            own_view(inner, ctx, c);
            false
        }
    };
    let rect = prepared.content_ui.min_rect().expand2(vec2(8.0, 4.0));
    if focused {
        prepared.frame = prepared.frame.stroke(Stroke::new(1.0, t.hl_med));
    } else if ui.rect_contains_pointer(rect) {
        prepared.frame = prepared.frame.fill(t.hl_low);
    }
    prepared.end(ui);
}

fn own_view(ui: &mut Ui, ctx: &mut Ctx, c: &Comment) {
    let body = shown_body(ctx.st, c);
    let id = Id::unique("lb-comment-body").with(&c.id);
    let v = rich_view::show(ui, ctx.t, id, &body, rich::COMMENT, true);
    if let Some(url) = v.open_url {
        ctx.out.push(PanelAction::OpenUrl(url));
    } else if let Some(text) = v.toggled {
        save(ctx.st, ctx.out, c, text.trim().to_string());
    } else if let Some(at) = v.clicked_at {
        flush_edit(ctx);
        ctx.st.edit = Some(OwnEdit::new(&ctx.issue.id, c, &body, at..at));
    }
}

/// Saves the open own editor before another one replaces it.
fn flush_edit(ctx: &mut Ctx) {
    let Some(e) = ctx.st.edit.take() else { return };
    let comment = ctx.view_list.iter().find(|x| x.id == e.comment_id);
    if let (Some(body), Some(c)) = (e.to_save(), comment) {
        save(ctx.st, ctx.out, c, body);
    }
}

/// The editor of an own comment. Returns true while it has focus.
fn own_editor(ui: &mut Ui, ctx: &mut Ctx, c: &Comment) -> bool {
    let Some(e) = ctx.st.edit.as_mut() else { return false };
    let id = Id::unique("lb-comment-own").with(&c.id);
    // A sizing pass of the panel takes no focus, so the request waits for a live frame.
    let live = ui.is_enabled() && ui.is_visible();
    let select = if live { e.focus.take() } else { None };
    let opts = rich::Options {
        look: rich::COMMENT,
        placeholder: "",
        min_rows: 1,
        uploads: &[],
        request_focus: select.is_some(),
        select,
    };
    let mut r = rich::editor(ui, ctx.t, id, &mut e.text, &opts);
    if paste::handle(ui, &r.response, &mut e.text, ctx.next_request_id, ctx.out) {
        r.changed = true;
    }
    if let Some(url) = r.open_url.take() {
        ctx.out.push(PanelAction::OpenUrl(url));
    }
    let now = ui.input(|i| i.time);
    if r.changed {
        e.last_edit = now;
    }
    e.had_focus |= r.focused;
    // egui drops the focus on Esc before the frame runs, so the editor just lost it.
    if r.lost_focus && ui.input(|i| i.key_pressed(Key::Escape)) {
        ctx.st.edit = None;
        return false;
    }
    if let Some(body) = e.to_save() {
        let waited = now - e.last_edit;
        if !r.focused || waited >= SAVE_AFTER {
            e.saved = e.text.clone();
            save(ctx.st, ctx.out, c, body);
        } else {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(SAVE_AFTER - waited));
        }
    }
    let Some(e) = ctx.st.edit.as_mut() else { return false };
    if e.had_focus && !r.focused && paste::pending_uploads(&e.text).is_empty() {
        ctx.st.edit = None;
    }
    r.focused
}

/// Inline rich editor with Cancel and a send button. Ctrl+Enter sends, Esc cancels.
#[allow(clippy::too_many_arguments)]
fn editor(
    ui: &mut Ui,
    t: &Theme,
    id: Id,
    d: &mut Draft,
    hint: &str,
    send_label: &str,
    next_request_id: &mut u64,
    out: &mut Vec<PanelAction>,
) -> Outcome {
    let can_send = |s: &str| !s.trim().is_empty() && paste::pending_uploads(s).is_empty();
    // Taken before the editor, so Ctrl+Enter does not also add a line.
    let focused = ui.memory(|m| m.has_focus(id));
    let send_key = focused && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
    let focus = std::mem::take(&mut d.focus);
    let end = d.text.len();
    let opts = rich::Options {
        look: rich::COMMENT,
        placeholder: hint,
        min_rows: 1,
        uploads: &[],
        request_focus: focus,
        select: focus.then_some(end..end),
    };
    let mut frame = widgets::editor_frame(t, false, (10, 14, 10, 14), 10).begin(ui);
    let r = {
        let inner = &mut frame.content_ui;
        inner.set_width(inner.available_width());
        rich::editor(inner, t, id, &mut d.text, &opts)
    };
    if r.focused {
        frame.frame.stroke = Stroke::new(1.0, t.accent_color);
    }
    frame.end(ui);
    if let Some(url) = r.open_url.clone() {
        out.push(PanelAction::OpenUrl(url));
    }
    let resp = r.response.clone();

    let mut outcome = Outcome::Keep;
    if send_key && can_send(&d.text) {
        outcome = Outcome::Send;
    }
    // egui drops the focus on Esc before the frame runs, so the editor just lost it.
    if r.lost_focus && ui.input(|i| i.key_pressed(Key::Escape)) {
        outcome = Outcome::Cancel;
    }
    paste::handle(ui, &resp, &mut d.text, next_request_id, out);
    let can_send = can_send(&d.text);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if composer::attach_link(ui, t).clicked() {
            paste::attach_dialog(&mut d.text, next_request_id, out);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if send_button(ui, t, send_label, can_send).clicked() && can_send {
                outcome = Outcome::Send;
            }
            if parts::button(ui, t, "Cancel", Btn::Ghost, Some("Esc"), true).clicked() {
                outcome = Outcome::Cancel;
            }
        });
    });
    outcome
}

fn send_button(ui: &mut Ui, t: &Theme, label: &str, enabled: bool) -> Response {
    parts::button(ui, t, label, Btn::Primary, Some(&format!("{} Enter", crate::theme::parts::CMD)), enabled)
}

/// Same palette and order as the board cards, so a person keeps one color.
fn avatar_color(t: &Theme, user_index: usize) -> eframe::egui::Color32 {
    let palette = [t.iris, t.foam, t.gold, t.rose, t.pine, t.love];
    palette[user_index % palette.len()]
}
