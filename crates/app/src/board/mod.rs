//! The board: top bar, group tabs, column headers, scrolling wells, drag and drop and the card menu.

pub mod card;
mod columns;
mod drag;
pub mod layout;
mod menu;
mod tabs;
mod topbar;

use std::collections::{HashMap, HashSet};

use eframe::egui::{CentralPanel, Event, Frame, Key, Margin, Panel, Pos2, Rect, Stroke, Ui, Vec2};
use model::{BoardData, Workspace};

use self::layout::{GroupBy, Tab};
pub use self::menu::Menu;
use crate::actions::{self, Action};
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
}

pub fn show(ui: &mut Ui, t: &Theme, props: &BoardProps, view: &mut BoardView, actions: &mut Vec<Action>) {
    let data = props.data;
    let slug = props.workspace.slug.as_str();
    let tabs = layout::tabs(data, view.group, view.mine);
    let tab = view.tab(slug, &tabs);
    let cols = layout::columns(data, view.group, &tab, view.mine);
    view.hovered = None;

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
    keys(ui, view, props, actions);
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

fn keys(ui: &Ui, view: &mut BoardView, props: &BoardProps, actions: &mut Vec<Action>) {
    if ui.ctx().text_edit_focused() {
        return;
    }
    let (copy, copy_prompt, plain) = ui.input(|i| {
        let copy = i.events.iter().any(|e| matches!(e, Event::Copy))
            || (i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::C));
        let prompt = i.modifiers.command && i.modifiers.shift && i.key_pressed(Key::C);
        (copy, prompt, !i.modifiers.any())
    });
    if copy || copy_prompt {
        let id = view.menu.as_ref().map(|m| m.issue_id.as_str()).or(view.hovered.as_deref()).or(props.selected);
        if let Some(issue) = id.and_then(|id| props.data.issues.iter().find(|i| i.id == id)) {
            let text = if copy_prompt { actions::copy_prompt(issue) } else { actions::copy_title(issue) };
            actions.push(Action::Copy(text));
        }
    }
    if !plain {
        return;
    }
    ui.input(|i| {
        if i.key_pressed(Key::M) {
            view.mine = !view.mine;
        }
        if i.key_pressed(Key::R) {
            actions.push(Action::Refresh);
        }
        if i.key_pressed(Key::Escape) && view.drag.is_some() {
            view.drag = None;
        }
    });
}
