//! The board: top bar, group tabs, column headers, scrolling wells, drag and drop and the card menu.

pub mod card;
mod columns;
mod drag;
pub mod layout;
mod menu;
mod nav;
mod tabs;
mod topbar;

use std::collections::{HashMap, HashSet};

use eframe::egui::{CentralPanel, Event, Frame, Key, Margin, Panel, Pos2, Rect, Stroke, Ui, Vec2};
use model::{BoardData, Issue, Workspace};

use self::layout::{Column, GroupBy, Tab};
pub use self::menu::Menu;
use self::nav::Move;
use crate::actions::{self, Action};
use crate::issue_panel::keys::{IssueKey, assign_me, set_priority, take_issue_key, take_plain};
use crate::theme::parts::StatusKind;
use crate::sync::SyncStatus;
use crate::theme::Theme;

#[derive(Default)]
pub struct BoardView {
    pub group: GroupBy,
    pub mine: bool,
    /// Active tab key per workspace and grouping.
    tabs: HashMap<(String, GroupBy), String>,
    /// The Backlog shows as a full column instead of the narrow strip.
    pub backlog_open: bool,
    pub drag: Option<Drag>,
    pub menu: Option<Menu>,
    /// Card under the pointer in this frame. The copy shortcuts use it.
    hovered: Option<String>,
    /// The dropped card on its way into its slot.
    settle: Option<Settle>,
    /// Card ids per column in the last frame, to find cards that moved.
    seen: HashMap<String, HashSet<String>>,
    /// Cards that fade in at a new place, with their start time.
    fades: HashMap<String, f64>,
    /// Visible card rects of the last frame, with the issue id.
    cards: Vec<(Rect, String)>,
    /// Card with the keyboard focus ring. Only keys and new issues set it.
    pub focus: Option<String>,
    /// Scrolls the focused card into view on the next frame.
    focus_scroll: bool,
    /// Well of the focused card, for the horizontal scroll after the wells drew.
    focus_well: Option<Rect>,
}

/// A dropped card that glides from where it was let go into its slot.
pub struct Settle {
    pub issue_id: String,
    pub from: Rect,
    pub angle: f32,
    pub scale: f32,
    pub start: f64,
}

impl BoardView {
    pub fn set_tab(&mut self, workspace: &str, group: GroupBy, key: &str) {
        self.tabs.insert((workspace.to_string(), group), key.to_string());
    }

    /// Where the card of this issue is on screen: where it was let go, else its slot in the last frame.
    pub fn card_rect(&self, issue_id: &str) -> Option<Rect> {
        match &self.settle {
            Some(s) if s.issue_id == issue_id => Some(s.from),
            _ => self.cards.iter().find(|(_, id)| id == issue_id).map(|(r, _)| *r),
        }
    }

    /// Puts the focus ring on a card and scrolls it into view.
    pub fn focus_on(&mut self, issue_id: &str) {
        self.focus = Some(issue_id.to_string());
        self.focus_scroll = true;
    }

    /// Team and project of the active tab, as defaults for a new issue.
    pub fn tab_scope(&self, workspace: &str, data: &BoardData) -> (Option<String>, Option<String>) {
        let tabs = layout::tabs(data, self.group, self.mine);
        let key = self.tab(workspace, &tabs);
        let team = key.strip_prefix("team:").map(str::to_string);
        let project = key.strip_prefix("project:").filter(|p| data.projects.iter().any(|x| x.id == *p)).map(str::to_string);
        (team, project)
    }

    /// Changes the tab, the mine filter and the Backlog fold, so the card of `issue` shows.
    pub fn reveal(&mut self, workspace: &str, data: &BoardData, issue: &Issue) {
        if self.mine && !layout::is_visible(data, issue, true) {
            self.mine = false;
        }
        let tabs = layout::tabs(data, self.group, self.mine);
        let key = self.tab(workspace, &tabs);
        if !layout::in_tab(data, issue, self.group, &key) {
            let next = match self.group {
                GroupBy::Team => format!("team:{}", issue.team_id),
                GroupBy::Project => layout::ALL.to_string(),
            };
            self.set_tab(workspace, self.group, &next);
        }
        let kind = data.states.iter().find(|s| s.id == issue.state_id).map(|s| layout::status_kind(s.kind, &s.name));
        if kind == Some(StatusKind::Backlog) {
            self.backlog_open = true;
        }
    }

    /// The remembered tab, or the first one when it is gone.
    fn tab(&self, workspace: &str, tabs: &[Tab]) -> String {
        let key = self.tabs.get(&(workspace.to_string(), self.group));
        match key {
            Some(k) if tabs.iter().any(|t| &t.key == k) => k.clone(),
            _ => layout::ALL.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
    pub issue_id: String,
    /// Pointer position inside the card when the drag started.
    pub grab: Vec2,
    /// Fixed pointer for screenshots. A demo drag never drops.
    pub demo_pointer: Option<Pos2>,
}

pub struct BoardProps<'a> {
    pub workspace: &'a Workspace,
    pub data: &'a BoardData,
    pub sync: &'a SyncStatus,
    pub selected: Option<&'a str>,
    /// A modal is open. The board takes no keys and no clicks through the scrim.
    pub blocked: bool,
}

pub fn show(ui: &mut Ui, t: &Theme, props: &BoardProps, view: &mut BoardView, actions: &mut Vec<Action>) {
    let data = props.data;
    let slug = props.workspace.slug.as_str();
    let tabs = layout::tabs(data, view.group, view.mine);
    let tab = view.tab(slug, &tabs);
    let cols = layout::columns(data, view.group, &tab, view.mine);
    view.hovered = None;
    let menu_was_open = view.menu.is_some();

    let subtitle = if view.mine {
        let me = data
            .viewer_id
            .as_ref()
            .and_then(|id| data.users.iter().find(|u| &u.id == id))
            .map(|u| u.name.split_whitespace().next().unwrap_or(&u.name).to_string())
            .unwrap_or_else(|| "Me".into());
        format!("{me}, {}", plural(tabs[0].count, "issue"))
    } else {
        plural(data.teams.len(), "team")
    };
    let info = topbar::TopbarInfo { workspace: &props.workspace.name, subtitle, last_sync: data.last_sync };
    let bar = Frame::NONE.fill(t.base).inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 0 });
    Panel::top("lb-topbar")
        .exact_size(60.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(bar)
        .show(ui, |ui| topbar::show(ui, t, &info, props.sync, actions));

    let bar = Frame::NONE.fill(t.base).inner_margin(Margin { left: 24, right: 24, top: 0, bottom: 0 });
    let out = Panel::top("lb-tabs")
        .exact_size(tabs::HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(bar)
        .show(ui, |ui| tabs::show(ui, t, view.group, &tabs, &tab, view.mine));
    let r = out.response.rect;
    ui.painter().hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, t.hl_low));
    match out.inner {
        Some(tabs::Pick::Group(g)) => {
            view.group = g;
            view.drag = None;
        }
        Some(tabs::Pick::Tab(key)) => {
            view.set_tab(slug, view.group, &key);
            view.drag = None;
        }
        Some(tabs::Pick::ToggleMine) => {
            view.mine = !view.mine;
            view.drag = None;
        }
        None => {}
    }

    CentralPanel::default()
        .frame(Frame::NONE.fill(t.base))
        .show(ui, |ui| columns::show(ui, t, props, &tab, &cols, view, actions));

    menu::show(ui.ctx(), t, &mut view.menu, &data.issues, actions);
    keys(ui, view, props, &cols, menu_was_open, actions);
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

fn keys(ui: &Ui, view: &mut BoardView, props: &BoardProps, cols: &[Column], menu_was_open: bool, actions: &mut Vec<Action>) {
    let ctx = ui.ctx();
    if props.blocked || ctx.text_edit_focused() {
        return;
    }
    let data = props.data;
    let (copy, copy_prompt) = ui.input(|i| {
        let copy = i.events.iter().any(|e| matches!(e, Event::Copy))
            || (i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::C));
        let prompt = i.modifiers.command && i.modifiers.shift && i.key_pressed(Key::C);
        (copy, prompt)
    });
    if copy || copy_prompt {
        let id = view
            .menu
            .as_ref()
            .map(|m| m.issue_id.as_str())
            .or(view.focus.as_deref())
            .or(view.hovered.as_deref())
            .or(props.selected);
        if let Some(issue) = id.and_then(|id| data.issues.iter().find(|i| i.id == id)) {
            let text = if copy_prompt { actions::copy_prompt(issue) } else { actions::copy_title(issue) };
            actions.push(Action::Copy(text));
        }
    }
    // The card menu reads Enter and Esc itself.
    if menu_was_open {
        return;
    }
    let help = ctx.input(|i| {
        let text = i.events.iter().any(|e| matches!(e, Event::Text(s) if s == "?"));
        !i.modifiers.command && (text || i.key_pressed(Key::Questionmark))
    });
    if help {
        actions.push(Action::ShowShortcuts);
        return;
    }
    let (new, mine, refresh, open, esc, mv) = ctx.input_mut(|i| {
        let mv = if take_plain(i, Key::J) || take_plain(i, Key::ArrowDown) {
            Some(Move::Down)
        } else if take_plain(i, Key::K) || take_plain(i, Key::ArrowUp) {
            Some(Move::Up)
        } else if take_plain(i, Key::ArrowLeft) {
            Some(Move::Left)
        } else if take_plain(i, Key::ArrowRight) {
            Some(Move::Right)
        } else {
            None
        };
        let open = take_plain(i, Key::Enter) || take_plain(i, Key::O);
        (take_plain(i, Key::C), take_plain(i, Key::M), take_plain(i, Key::R), open, i.key_pressed(Key::Escape), mv)
    });
    if new {
        actions.push(Action::NewIssue);
    }
    if mine {
        view.mine = !view.mine;
    }
    if refresh {
        actions.push(Action::Refresh);
    }
    if esc {
        if view.drag.is_some() {
            view.drag = None;
        } else {
            view.focus = None;
        }
    }
    if let Some(mv) = mv {
        move_focus(ctx, view, props, cols, mv, actions);
    }
    // The last input wins: a mouse move over another card drops the focus ring, and
    // the keys then act on the hovered card, like in Linear.
    let moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO);
    if moved && view.hovered.is_some() && view.hovered != view.focus {
        view.focus = None;
    }
    let target = view.focus.clone().or(view.hovered.clone());
    let Some(issue) = target.and_then(|id| data.issues.iter().find(|i| i.id == id)) else { return };
    if open {
        actions.push(Action::SelectIssue(Some(issue.id.clone())));
    }
    // With the panel open, the panel took these keys already for the open issue.
    if props.selected.is_some() {
        return;
    }
    let change = match take_issue_key(ctx) {
        Some(IssueKey::Picker(kind)) => {
            actions.push(Action::OpenIssueWith { issue_id: issue.id.clone(), picker: kind });
            None
        }
        Some(IssueKey::AssignMe) => assign_me(issue, data),
        Some(IssueKey::Priority(p)) => set_priority(issue, p),
        None => None,
    };
    if let Some(change) = change {
        actions.push(Action::ChangeIssue { issue_id: issue.id.clone(), change });
    }
}

/// Moves the focus ring. A first move starts at the open issue. With the panel open,
/// the panel follows the focus.
fn move_focus(ctx: &eframe::egui::Context, view: &mut BoardView, props: &BoardProps, cols: &[Column], mv: Move, actions: &mut Vec<Action>) {
    let data = props.data;
    let shown: Vec<Vec<&str>> = cols
        .iter()
        .filter(|c| view.backlog_open || c.kind != StatusKind::Backlog)
        .map(|c| c.cards.iter().map(|&i| data.issues[i].id.as_str()).collect())
        .collect();
    let from = view.focus.as_deref().or(props.selected);
    let next = match (from, view.hovered.as_deref()) {
        (None, Some(h)) => Some(h.to_string()),
        _ => nav::step(&shown, from, mv),
    };
    let Some(next) = next else { return };
    if props.selected.is_some() && props.selected != Some(next.as_str()) {
        actions.push(Action::SelectIssue(Some(next.clone())));
    }
    view.focus_on(&next);
    ctx.request_repaint();
}
