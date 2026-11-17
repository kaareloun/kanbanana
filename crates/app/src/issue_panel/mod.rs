//! Issue side panel (design 03 to 05). It reads the issue and the board data and pushes
//! `PanelAction`s. It never does network I/O.

mod anim;
mod comments;
mod composer;
mod description;
mod dev;
mod editor;
mod header;
mod icons;
mod keys;
mod mock;
mod paste;
mod pickers;
mod props;
mod rich_view;
mod time;
mod widgets;

use std::collections::BTreeMap;

use chrono::Utc;
use eframe::egui::{
    self, Area, Color32, Id, Margin, Order, Rect, ScrollArea, Sense, Shadow, Stroke, Ui, UiBuilder, pos2,
    style::ScrollAnimation,
};
use model::{BoardData, Comment, Issue, IssueChange};

use crate::actions::Action;
use crate::theme::Theme;

pub use comments::mock_comments;
pub use mock::{MOCK_DESCRIPTION, demo};

pub const WIDTH: f32 = 660.0;
const PAD_X: f32 = 36.0;
const HEADER_H: f32 = 60.0;
const GAP: f32 = 18.0;

const TITLE_ID: &str = "lb-panel-title";
const DESC_ID: &str = "lb-panel-desc";
const FILTER_ID: &str = "lb-panel-filter";
const OPEN_ID: &str = "lb-panel-open";

fn id(s: &str) -> Id {
    Id::unique(s)
}

#[derive(Clone, Debug, PartialEq)]
pub enum PanelAction {
    Change(IssueChange),
    /// A change for an issue that may not be the open one, for example an unsaved
    /// description edit that is saved when the user opens another issue.
    ChangeFor { issue_id: String, change: IssueChange },
    LoadComments,
    CreateComment { body: String, parent_id: Option<String> },
    UpdateComment { comment_id: String, body: String },
    Upload { request_id: u64, filename: String, content_type: String, bytes: Vec<u8> },
    Close,
    OpenUrl(String),
    CopyText(String),
}

/// Comments of the open issue, as the app knows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommentsView {
    pub loading: bool,
    pub error: Option<String>,
    pub list: Vec<Comment>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DescMode {
    View,
    Edit { buffer: String, preview: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    Status,
    Priority,
    Assignee,
    Labels,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Picker {
    pub kind: PickerKind,
    pub filter: String,
    /// Index into the filtered options. Up and Down move it.
    pub cursor: usize,
    /// Label ids in the order of the moment the picker opened, so rows do not jump on toggle.
    order: Vec<String>,
    focus: bool,
    /// egui time of the first frame, for the pop-in.
    opened_at: Option<f64>,
}

/// An upload the engine still works on.
#[derive(Clone, Debug, PartialEq)]
struct Upload {
    filename: String,
    /// 0 to 1 when the engine reports it.
    progress: Option<f32>,
    /// Started from the description editor, so the editor shows its progress row.
    description: bool,
}

impl Upload {
    fn new(filename: &str, description: bool) -> Upload {
        Upload { filename: filename.to_string(), progress: None, description }
    }
}

pub struct PanelState {
    /// The selected issue. The app sets it; the panel resets its editors when it changes.
    pub issue_id: Option<String>,
    opened: Option<String>,
    /// Issue that got its `LoadComments`.
    loaded: Option<String>,
    title_edit: Option<String>,
    title_focus: bool,
    pub desc: DescMode,
    desc_focus: bool,
    /// Cursor (char index) the editor takes when it opens. The demo screens set it.
    desc_cursor: Option<usize>,
    pub picker: Option<Picker>,
    chip_rects: [Rect; 4],
    pub comments: comments::CommentsState,
    pub composer: composer::ComposerState,
    pub next_request_id: u64,
    uploads: BTreeMap<u64, Upload>,
    upload_error: Option<String>,
    composer_h: f32,
    /// Keeps the comments head at the top of the view until the user scrolls.
    pub scroll_to_comments: bool,
    /// Focused widget at the end of the last frame. egui drops text focus on Esc before
    /// the next frame runs, so the Esc handler needs the previous one.
    last_focus: Option<Id>,
    /// Actions the next frame sends, for the demo screens.
    pending: Vec<PanelAction>,
    /// egui time of the last switch to another issue, for the content swap animation.
    switched_at: Option<f64>,
    /// The panel slides out. An issue opened now slides in again instead of swapping.
    closing: bool,
    /// Comments of the last frame, so the panel keeps them while it slides out.
    last_comments: CommentsView,
    /// Scrolls the body to the top on the next frame.
    reset_scroll: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        PanelState {
            issue_id: None,
            opened: None,
            loaded: None,
            title_edit: None,
            title_focus: false,
            desc: DescMode::View,
            desc_focus: false,
            desc_cursor: None,
            picker: None,
            chip_rects: [Rect::NOTHING; 4],
            comments: Default::default(),
            composer: Default::default(),
            next_request_id: 1,
            uploads: BTreeMap::new(),
            upload_error: None,
            composer_h: 74.0,
            scroll_to_comments: false,
            last_focus: None,
            pending: Vec::new(),
            switched_at: None,
            closing: false,
            last_comments: CommentsView::default(),
            reset_scroll: false,
        }
    }
}

impl PanelState {
    fn reset_for(&mut self, issue_id: &str) {
        self.opened = Some(issue_id.to_string());
        self.title_edit = None;
        self.desc = DescMode::View;
        self.desc_cursor = None;
        self.picker = None;
        self.comments = Default::default();
        self.upload_error = None;
        self.scroll_to_comments = false;
        self.reset_scroll = true;
    }

    pub fn open_picker(&mut self, kind: PickerKind, issue: &Issue, data: &BoardData) {
        let order = match kind {
            PickerKind::Labels => pickers::label_order(issue, data),
            _ => Vec::new(),
        };
        self.picker = Some(Picker { kind, filter: String::new(), cursor: 0, order, focus: true, opened_at: None });
    }

    /// Marks an upload as started, for the progress row. The app does not need to call it.
    pub fn track_upload(&mut self, request_id: u64, filename: &str, description: bool) {
        self.uploads.entry(request_id).or_insert_with(|| Upload::new(filename, description));
    }
}

/// The upload finished. Replaces its placeholder with the image markdown in every buffer.
pub fn on_uploaded(state: &mut PanelState, request_id: u64, asset_url: &str) {
    finish(state, request_id, Some(asset_url));
}

pub fn on_upload_failed(state: &mut PanelState, request_id: u64, message: &str) {
    let name = finish(state, request_id, None);
    state.upload_error = Some(format!("{name} did not upload: {message}"));
}

pub fn on_upload_progress(state: &mut PanelState, request_id: u64, fraction: f32) {
    if let Some(u) = state.uploads.get_mut(&request_id) {
        u.progress = Some(fraction.clamp(0.0, 1.0));
    }
}

fn finish(state: &mut PanelState, request_id: u64, url: Option<&str>) -> String {
    let name = state.uploads.remove(&request_id).map(|u| u.filename).unwrap_or_default();
    if let DescMode::Edit { buffer, .. } = &mut state.desc {
        paste::finish_upload(buffer, request_id, &name, url);
    }
    composer::for_each_buffer(&mut state.composer, |s| paste::finish_upload(s, request_id, &name, url));
    comments::for_each_buffer(&mut state.comments, |s| paste::finish_upload(s, request_id, &name, url));
    name
}

/// Draws the panel over the right side of the window, full height, with a scrim over the
/// board. The scrim only paints: the board under it takes the clicks, so a click on a card
/// opens that issue at once.
pub fn show_panel(
    ctx: &egui::Context,
    t: &Theme,
    state: &mut PanelState,
    issue: &Issue,
    data: &BoardData,
    comments: &CommentsView,
    out: &mut Vec<PanelAction>,
) {
    let open = anim::toggle(ctx, id(OPEN_ID), true, anim::DUR);
    if state.opened.as_deref() != Some(issue.id.as_str()) {
        if let Some(old) = state.opened.clone() {
            save_title_for(state, &old, data, out);
            if !state.closing {
                state.switched_at = Some(anim::now(ctx));
            }
        }
        state.reset_for(&issue.id);
    }
    state.closing = false;
    if state.loaded.as_deref() != Some(issue.id.as_str()) {
        state.loaded = Some(issue.id.clone());
        out.push(PanelAction::LoadComments);
    }
    if state.last_comments != *comments {
        state.last_comments = comments.clone();
    }
    state.issue_id = Some(issue.id.clone());
    out.append(&mut state.pending);
    let first_new = out.len();

    keys::handle(ctx, state, issue, data, out);
    let rect = draw(ctx, t, state, issue, data, comments, open, true, out);
    pickers::show(ctx, t, state, issue, data, rect, out);

    state.last_focus = if ctx.text_edit_focused() { ctx.memory(|m| m.focused()) } else { None };
    for a in &out[first_new..] {
        if let PanelAction::Upload { request_id, filename, .. } = a {
            state.track_upload(*request_id, filename, false);
        }
    }
}

/// Call every frame while no issue is selected. It draws the last issue while the panel
/// slides out, click-through. Returns the issue id that the pushed edits belong to.
pub fn show_closing(
    ctx: &egui::Context,
    t: &Theme,
    state: &mut PanelState,
    data: &BoardData,
    out: &mut Vec<PanelAction>,
) -> Option<String> {
    let open = anim::toggle(ctx, id(OPEN_ID), false, anim::DUR);
    state.closing = true;
    state.picker = None;
    let issue = state.opened.as_ref().and_then(|o| data.issues.iter().find(|i| &i.id == o))?;
    if open <= 0.0 {
        return None;
    }
    let comments = std::mem::take(&mut state.last_comments);
    let mut all = Vec::new();
    draw(ctx, t, state, issue, data, &comments, open, false, &mut all);
    state.last_comments = comments;
    // Only edits leave the closing panel, for example a title that saves on focus loss.
    out.extend(all.into_iter().filter(|a| matches!(a, PanelAction::Change(_) | PanelAction::ChangeFor { .. })));
    Some(issue.id.clone())
}

/// Saves an unsaved title edit of an issue the panel leaves.
fn save_title_for(state: &mut PanelState, issue_id: &str, data: &BoardData, out: &mut Vec<PanelAction>) {
    let Some(s) = state.title_edit.take() else { return };
    let s = s.replace('\n', " ").trim().to_string();
    let old = data.issues.iter().find(|i| i.id == issue_id).map(|i| i.title.as_str());
    if !s.is_empty() && old.is_some_and(|o| o != s) {
        out.push(PanelAction::ChangeFor { issue_id: issue_id.to_string(), change: IssueChange::Title(s) });
    }
}

/// Paints scrim and panel. `open` (0 to 1) slides and fades it. `live` false makes it
/// click-through, for the slide out. Returns the panel rect.
#[allow(clippy::too_many_arguments)]
fn draw(
    ctx: &egui::Context,
    t: &Theme,
    state: &mut PanelState,
    issue: &Issue,
    data: &BoardData,
    comments: &CommentsView,
    open: f32,
    live: bool,
    out: &mut Vec<PanelAction>,
) -> Rect {
    let screen = ctx.content_rect();
    let rect = Rect::from_min_max(pos2(screen.right() - WIDTH, screen.top()), screen.max)
        .translate(egui::vec2(anim::SLIDE * (1.0 - open), 0.0));
    let scrim = Rect::from_min_max(pos2(screen.left() + crate::workspace::rail::WIDTH, screen.top()), pos2(rect.left(), screen.bottom()));
    let swap = state.switched_at.map_or(1.0, |s| anim::since(ctx, s));
    if swap >= 1.0 {
        state.switched_at = None;
    }
    Area::new(id("lb-issue-panel"))
        .fixed_pos(rect.min)
        .order(Order::Middle)
        .fade_in(false)
        .constrain(false)
        .interactable(live)
        .show(ctx, |ui| {
            ui.multiply_opacity(open);
            ui.painter().with_clip_rect(scrim).rect_filled(scrim, 0.0, Theme::tint(t.base, 0.4));
            let shadow = Shadow { offset: [-12, 0], blur: 40, spread: 0, ..t.shadow() };
            ui.painter().with_clip_rect(scrim.union(rect)).add(shadow.as_shape(rect, 0));
            ui.set_clip_rect(rect);
            ui.painter().rect_filled(rect, 0.0, t.surface);
            // Clicks on the panel background must not reach the board below.
            ui.allocate_rect(rect, Sense::click());

            // Another issue: the content fades in and slides a few px from the right.
            ui.multiply_opacity(0.2 + 0.8 * swap);
            let content = rect.translate(egui::vec2(anim::SWAP * (1.0 - swap), 0.0));
            let header = Rect::from_min_size(content.min, egui::vec2(WIDTH, HEADER_H));
            child(ui, header, |ui| header::show(ui, t, issue, data, out));

            let composer_top = content.bottom() - state.composer_h;
            let body = Rect::from_min_max(pos2(content.left(), header.bottom()), pos2(content.right(), composer_top));
            child(ui, body, |ui| body_ui(ui, t, state, issue, data, comments, out));

            let r = Rect::from_min_max(pos2(content.left(), composer_top), content.max);
            ui.painter().rect_filled(r.with_min_x(rect.left()), 0.0, t.surface);
            let h = child(ui, r, |ui| {
                egui::Frame::NONE
                    .inner_margin(Margin { left: PAD_X as i8, right: PAD_X as i8, top: 14, bottom: 16 })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        composer::show(ui, t, &mut state.composer, issue, data, &mut state.next_request_id, out);
                    });
            });
            if (h - state.composer_h).abs() > 0.5 {
                state.composer_h = h;
                ctx.request_repaint();
            }
        });
    rect
}

/// Runs `add` in a child ui that fills `rect`. Returns the height the content used.
fn child(ui: &mut Ui, rect: Rect, add: impl FnOnce(&mut Ui)) -> f32 {
    let mut c = ui.new_child(UiBuilder::new().max_rect(rect));
    c.set_clip_rect(rect.intersect(ui.clip_rect()));
    add(&mut c);
    c.min_rect().height()
}

fn body_ui(
    ui: &mut Ui,
    t: &Theme,
    state: &mut PanelState,
    issue: &Issue,
    data: &BoardData,
    comments: &CommentsView,
    out: &mut Vec<PanelAction>,
) {
    if state.scroll_to_comments && ui.input(|i| i.smooth_scroll_delta.y != 0.0 || i.pointer.any_pressed()) {
        state.scroll_to_comments = false;
    }
    let mut area = ScrollArea::vertical().id_salt("lb-panel-body").auto_shrink([false, false]);
    if std::mem::take(&mut state.reset_scroll) {
        area = area.scroll_offset(egui::Vec2::ZERO);
    }
    area.show(ui, |ui| {
        egui::Frame::NONE
            .inner_margin(Margin { left: PAD_X as i8, right: PAD_X as i8, top: 8, bottom: 24 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = egui::vec2(8.0, GAP);
                props::title(ui, t, state, issue, data, out);
                props::list(ui, t, state, issue, data, out);
                rule(ui, t);
                description::show(ui, t, state, issue, out);
                rule(ui, t);
                let top = ui.cursor().top();
                comments::show(
                    ui,
                    t,
                    &mut state.comments,
                    issue,
                    data,
                    comments,
                    &mut state.next_request_id,
                    out,
                );
                if state.scroll_to_comments {
                    let target = Rect::from_min_size(pos2(ui.max_rect().left(), top - 24.0), egui::vec2(1.0, 24.0));
                    ui.scroll_to_rect_animation(target, Some(egui::Align::TOP), ScrollAnimation::none());
                }
            });
    });
}

fn rule(ui: &mut Ui, t: &Theme) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.hl_low));
}

/// Maps panel actions to app actions for the parts that the app already has.
/// Comments and uploads need the sync engine, so they are dropped here.
pub fn to_app_actions(_ctx: &egui::Context, issue_id: &str, panel: Vec<PanelAction>, actions: &mut Vec<Action>) {
    for a in panel {
        match a {
            PanelAction::Change(change) => actions.push(Action::ChangeIssue { issue_id: issue_id.to_string(), change }),
            PanelAction::ChangeFor { issue_id, change } => actions.push(Action::ChangeIssue { issue_id, change }),
            PanelAction::Close => actions.push(Action::SelectIssue(None)),
            PanelAction::OpenUrl(url) => actions.push(Action::OpenUrl(url)),
            PanelAction::CopyText(text) => actions.push(Action::Copy(text)),
            PanelAction::LoadComments
            | PanelAction::CreateComment { .. }
            | PanelAction::UpdateComment { .. }
            | PanelAction::Upload { .. } => {}
        }
    }
}

/// Linear photo URL of a user, if the user has one.
pub fn user_avatar(data: &BoardData, user_id: &str) -> Option<String> {
    data.users.iter().find(|u| u.id == user_id)?.avatar_url.clone()
}

/// Initials, avatar color and name of a user. The palette order matches the board cards.
pub fn user_badge(data: &BoardData, t: &Theme, user_id: &str) -> Option<(String, Color32, String)> {
    let palette = [t.iris, t.foam, t.gold, t.rose, t.pine, t.love];
    let (i, u) = data.users.iter().enumerate().find(|(_, u)| u.id == user_id)?;
    Some((crate::board::card::initials(&u.name), palette[i % palette.len()], u.name.clone()))
}

fn now() -> chrono::DateTime<Utc> {
    Utc::now()
}
