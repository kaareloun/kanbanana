//! Activity (design v2 timeline): top-level comments in time order, replies indented
//! 30 px under their thread, inline reply and edit editors.

use chrono::{Duration, Local, Utc};
use eframe::egui::{Align, Id, Key, Layout, Margin, Modifiers, Response, Sense, Stroke, Ui, vec2};
use model::{BoardData, Comment, Issue};

use super::icons::Ic;
use super::{CommentsView, PanelAction, composer, paste, rich_view, widgets};
use crate::rich;
use crate::theme::parts::{self, Btn};
use crate::theme::{Theme, W};

#[derive(Default)]
pub struct CommentsState {
    /// Open reply editor. Its target is the id of the thread's top comment.
    reply: Option<Draft>,
    /// Open edit editor. Its target is the id of the edited comment.
    edit: Option<Draft>,
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
    for d in [&mut st.reply, &mut st.edit] {
        if d.as_ref().is_some_and(|d| d.issue_id != issue.id) {
            *d = None;
        }
    }

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

    let mut ctx = Ctx { t, st, issue, data, next_request_id, out };
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
    for d in [&mut st.reply, &mut st.edit].into_iter().flatten() {
        f(&mut d.text);
    }
}

/// The three comments of design 04 on mock issue WEB-214. Kaarel is the mock viewer.
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
        ],
    }
}

/// What the comment rows need, bundled to keep the signatures short.
struct Ctx<'a> {
    t: &'a Theme,
    st: &'a mut CommentsState,
    issue: &'a Issue,
    data: &'a BoardData,
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

            let editing = ctx.st.edit.as_ref().is_some_and(|d| d.target == c.id);
            if editing {
                edit_editor(ui, ctx, c);
            } else {
                let body_id = Id::unique("lb-comment-body").with(&c.id);
                if let Some(url) = rich_view::show(ui, t, body_id, &c.body, rich::COMMENT) {
                    ctx.out.push(PanelAction::OpenUrl(url));
                }
                actions(ui, ctx, c, thread_id);
            }
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

fn actions(ui: &mut Ui, ctx: &mut Ctx, c: &Comment, thread_id: &str) {
    let mine = c.user_id.is_some() && c.user_id == ctx.data.viewer_id;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        if widgets::icon_link(ui, ctx.t, Ic::Reply, "Reply").clicked() {
            match &mut ctx.st.reply {
                Some(d) if d.target == thread_id => d.focus = true,
                r => *r = Some(Draft::new(&ctx.issue.id, thread_id, "")),
            }
        }
        if mine && widgets::icon_link(ui, ctx.t, Ic::Pencil, "Edit").clicked() {
            ctx.st.edit = Some(Draft::new(&ctx.issue.id, &c.id, &c.body));
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

fn edit_editor(ui: &mut Ui, ctx: &mut Ctx, c: &Comment) {
    let Some(d) = ctx.st.edit.as_mut() else { return };
    let id = Id::unique("lb-comment-edit").with(&c.id);
    match editor(ui, ctx.t, id, d, "Edit the comment", "Save", ctx.next_request_id, ctx.out) {
        Outcome::Send => {
            if d.text.trim() != c.body.trim() {
                let body = d.text.trim().to_string();
                ctx.out.push(PanelAction::UpdateComment { comment_id: c.id.clone(), body });
            }
            ctx.st.edit = None;
        }
        Outcome::Cancel => ctx.st.edit = None,
        Outcome::Keep => {}
    }
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
