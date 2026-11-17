//! Settings page: same frame as the Workspaces page, one centered column of sections.

use std::collections::HashMap;

use chrono::Utc;
use eframe::egui::{
    Align, Align2, CentralPanel, Color32, CornerRadius, Frame, Key, Layout, Painter, Pos2, Rect, ScrollArea,
    Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2,
    text::{LayoutJob, TextFormat, TextWrapping},
    vec2,
};
use model::{BoardData, StateType, Workspace};

use super::{SettingsState, viewer};
use crate::actions::{self, Action};
use crate::board::card::initials;
use crate::onboarding::theme_grid;
use super::ago;
use crate::theme::parts::{self, Btn, Icon};
use crate::theme::{Theme, W, font, motion};
use crate::workspace::letter;
use crate::workspace::page::para;

const MAX_W: f32 = 760.0;
const PAD: f32 = 20.0;
const SECTION_GAP: f32 = 44.0;
const ACCOUNT_H: f32 = 76.0;

/// One line per MCP tool. The first sentence of each description in crates/mcp/src/tools.rs.
const TOOLS: [(&str, &str); 7] = [
    ("current_issue", "Finds the Linear issue that the current git work belongs to."),
    ("set_dev", "Records how far the code work on an issue got, so the board shows it."),
    ("list_issues", "Lists cached issues with status, priority and assignee."),
    ("get_issue", "Reads one issue: fields, description, dev state and comments."),
    ("update_issue", "Changes the issue in Linear. Only when you ask for the change."),
    ("add_comment", "Writes a comment on the issue in Linear. Only when you ask."),
    ("list_workspaces", "Lists the workspaces added in Kanbanana."),
];

pub struct SettingsProps<'a> {
    pub workspaces: &'a [Workspace],
    pub boards: &'a HashMap<String, BoardData>,
    pub active: Option<&'a str>,
    /// Index of the base theme in `THEMES`.
    pub theme: usize,
}

pub fn show(ui: &mut Ui, t: &Theme, props: &SettingsProps, state: &mut SettingsState, actions: &mut Vec<Action>) {
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        actions.push(Action::CloseSettings);
    }
    CentralPanel::default().frame(Frame::NONE.fill(t.base)).show(ui, |ui| {
        let full = ui.max_rect();
        // Fades in and slides up 12 px, like a sheet.
        let k = motion::since(ui.ctx(), state.opened_at);
        ui.set_opacity(k);
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let avail = ui.max_rect();
            let w = MAX_W.min(avail.width() - 64.0);
            let x = avail.left() + (avail.width() - w) / 2.0;
            let mut y = avail.top() + 64.0 + 12.0 * (1.0 - k);
            let p = ui.painter().clone();
            let r = parts::text(&p, pos2(x, y), Align2::LEFT_TOP, "Settings", W::Bold, 28.0, t.text);
            y = r.max.y + 36.0;
            y = accounts(ui, t, x, y, w, props, state, actions) + SECTION_GAP;
            y = claude(ui, t, x, y, w, props, state, actions) + SECTION_GAP;
            let top = y;
            y = appearance(ui, t, x, y, w, props, actions) + SECTION_GAP;
            if std::mem::take(&mut state.scroll_to_appearance) {
                let r = Rect::from_min_max(pos2(x, top - 24.0), pos2(x + w, y));
                ui.scroll_to_rect_animation(r, Some(Align::TOP), eframe::egui::style::ScrollAnimation::none());
            }
            y = about(ui, t, x, y, w, state, actions) + 64.0;
            ui.allocate_rect(Rect::from_min_max(avail.min, pos2(avail.left() + 1.0, y)), Sense::hover());
        });
        let r = Rect::from_min_max(pos2(full.right() - 240.0, full.top() + 14.0), pos2(full.right() - 40.0, full.top() + 46.0));
        let mut bar = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::right_to_left(Align::Center)));
        if parts::button(&mut bar, t, "Back to board", Btn::Ghost, Some("Esc"), true).clicked() {
            actions.push(Action::CloseSettings);
        }
    });
}

/// Section title in SemiBold 16 with a short line under it. Returns the y below them.
fn heading(ui: &Ui, t: &Theme, x: f32, y: f32, w: f32, title: &str, line: &str) -> f32 {
    let p = ui.painter();
    let r = parts::text(p, pos2(x, y), Align2::LEFT_TOP, title, W::SemiBold, 16.0, t.text);
    let h = para(ui, pos2(x, r.max.y + 6.0), line, W::Regular, 13.0, 20.0, w, t.subtle);
    r.max.y + 6.0 + h + 16.0
}

/// A child Ui over `rect` that lays out from the right. It does not move the page cursor.
fn row_ui(ui: &mut Ui, rect: Rect, salt: impl std::hash::Hash + std::fmt::Debug) -> Ui {
    ui.new_child(UiBuilder::new().id_salt(salt).max_rect(rect).layout(Layout::right_to_left(Align::Center)))
}

/// Text cut to `width` with an ellipsis. Returns the painted rect and whether it was cut.
#[allow(clippy::too_many_arguments)]
fn line(p: &Painter, at: Pos2, anchor: Align2, s: &str, w: W, size: f32, width: f32, c: Color32) -> (Rect, bool) {
    let mut job = LayoutJob::single_section(s.to_string(), TextFormat { font_id: font(w, size), color: c, ..Default::default() });
    job.wrap = TextWrapping::truncate_at_width(width);
    let g = p.layout_job(job);
    let elided = g.elided;
    let rect = anchor.anchor_size(at, g.size());
    p.galley(rect.min, g, c);
    (rect, elided)
}

// ------------------------------------------------------------------ accounts

#[allow(clippy::too_many_arguments)]
fn accounts(
    ui: &mut Ui,
    t: &Theme,
    x: f32,
    mut y: f32,
    w: f32,
    props: &SettingsProps,
    state: &mut SettingsState,
    actions: &mut Vec<Action>,
) -> f32 {
    let line_text = "Each workspace is one Linear organization with its own key. The photo in the rail is you in the active one.";
    y = heading(ui, t, x, y, w, "Accounts", line_text);
    for ws in props.workspaces {
        let card = Rect::from_min_size(pos2(x, y), vec2(w, ACCOUNT_H));
        account(ui, t, card, ws, props.boards.get(&ws.slug), props.active == Some(ws.slug.as_str()), state, actions);
        y += ACCOUNT_H + 10.0;
    }
    if props.workspaces.is_empty() {
        let p = ui.painter();
        parts::text(p, pos2(x, y), Align2::LEFT_TOP, "No workspace yet.", W::Medium, 13.0, t.muted);
        y += 30.0;
    }
    y += 4.0;
    let r = Rect::from_min_size(pos2(x, y), vec2(w, 30.0));
    let mut row = ui.new_child(UiBuilder::new().id_salt("lb-set-add").max_rect(r).layout(Layout::left_to_right(Align::Center)));
    if parts::button(&mut row, t, "Add workspace", Btn::Secondary, None, true).clicked() {
        actions.push(Action::OpenWorkspaces);
    }
    y + 30.0
}

#[allow(clippy::too_many_arguments)]
fn account(
    ui: &mut Ui,
    t: &Theme,
    card: Rect,
    ws: &Workspace,
    board: Option<&BoardData>,
    active: bool,
    state: &mut SettingsState,
    actions: &mut Vec<Action>,
) {
    let p = ui.painter().clone();
    let accent = t.accent(ws.accent);
    let stroke = if active { Stroke::new(1.0, Theme::tint(accent, 0.45)) } else { Stroke::NONE };
    p.rect(card, CornerRadius::same(12), t.surface, stroke, StrokeKind::Inside);
    let mid = card.center().y;
    let tile = Rect::from_min_size(pos2(card.left() + 16.0, mid - 22.0), vec2(44.0, 44.0));
    p.rect_filled(tile, CornerRadius::same(12), Theme::tint(accent, 0.18));
    parts::text(&p, tile.center(), Align2::CENTER_CENTER, &letter(&ws.name), W::Bold, 17.0, accent);

    let tx = tile.right() + 14.0;
    let name = parts::text(&p, pos2(tx, mid - 3.0), Align2::LEFT_BOTTOM, &ws.name, W::SemiBold, 15.0, t.text);
    parts::kbd(&p, t, pos2(name.right() + 10.0, name.center().y), &format!("{} {}", parts::ALT, ws.shortcut));
    let me = viewer(ws, board);
    let sub_y = mid + 12.0;
    match me {
        Some(u) => {
            let who = initials(&u.name);
            let c = pos2(tx + 9.0, sub_y);
            parts::avatar_photo(&p, ui.ctx(), t, c, 18.0, u.avatar_url.as_deref(), Some((who.as_str(), accent)));
            let s = format!("Signed in as {}", u.name);
            parts::text(&p, pos2(tx + 26.0, sub_y), Align2::LEFT_CENTER, &s, W::Medium, 12.0, t.subtle);
        }
        None => {
            parts::text(&p, pos2(tx, sub_y), Align2::LEFT_CENTER, "Signed in after the first sync", W::Medium, 12.0, t.muted);
        }
    }

    let right = Rect::from_min_max(pos2(card.center().x, card.top()), pos2(card.right() - 16.0, card.bottom()));
    let mut row = row_ui(ui, right, ("lb-set-acc", &ws.slug));
    row.spacing_mut().item_spacing.x = 8.0;
    let confirm = state.pending_remove.as_deref() == Some(ws.slug.as_str());
    let text = if confirm { "Confirm remove" } else { "Remove" };
    if parts::button(&mut row, t, text, if confirm { Btn::Secondary } else { Btn::Ghost }, None, true).clicked() {
        if confirm {
            state.pending_remove = None;
            actions.push(Action::RemoveWorkspace(ws.slug.clone()));
        } else {
            state.pending_remove = Some(ws.slug.clone());
        }
    }
    row.add_space(16.0);
    let (count, synced) = match board {
        Some(b) => (
            if b.issues.len() == 1 { "1 issue".to_string() } else { format!("{} issues", b.issues.len()) },
            b.last_sync.map(|at| format!("Synced {}", ago(at, Utc::now()))).unwrap_or_else(|| "Not synced yet".into()),
        ),
        None => ("No issues yet".to_string(), "Not synced yet".to_string()),
    };
    let (r, _) = row.allocate_exact_size(vec2(130.0, 36.0), Sense::hover());
    parts::text(&p, pos2(r.right(), mid - 3.0), Align2::RIGHT_BOTTOM, &count, W::Medium, 12.0, t.text);
    parts::text(&p, pos2(r.right(), mid + 3.0), Align2::RIGHT_TOP, &synced, W::Medium, 12.0, t.muted);
}

// ------------------------------------------------------------------ claude code

#[allow(clippy::too_many_arguments)]
fn claude(
    ui: &mut Ui,
    t: &Theme,
    x: f32,
    y: f32,
    w: f32,
    props: &SettingsProps,
    state: &SettingsState,
    actions: &mut Vec<Action>,
) -> f32 {
    let line_text = "Claude reports the branch, MR and deploy stage of the issue it works on, and the board shows it. \
        It can read your issues and, when you ask, comment on them or change them.";
    let top = heading(ui, t, x, y, w, "Claude Code", line_text);
    let p = ui.painter().clone();
    // The card height is known only at the end, so its fill goes in a slot painted under the rest.
    let bg = p.add(eframe::egui::Shape::Noop);
    let ix = x + PAD;
    let iw = w - 2.0 * PAD;
    let mut y = top + PAD;

    let exe = state.exe.as_ref().map_or("kanbanana".to_string(), |p| p.display().to_string());
    let command = format!("claude mcp add --scope user kanbanana -- {exe} mcp");
    y = step(&p, t, ix, y, 1, "Add the server to Claude Code", None);
    y = code_box(ui, t, pos2(ix + 40.0, y), iw - 40.0, &command, "lb-set-cmd", actions) + 20.0;

    let prompt = example_prompt(props);
    y = step(&p, t, ix, y, 2, "Restart Claude Code", Some("Ask it to start an issue, for example"));
    y = code_box(ui, t, pos2(ix + 40.0, y), iw - 40.0, &prompt, "lb-set-prompt", actions) + 22.0;

    p.hline(ix..=ix + iw, y, Stroke::new(1.0, t.hl_low));
    y += 18.0;
    let r = parts::text(&p, pos2(ix, y), Align2::LEFT_TOP, "Tools Claude Code gets", W::SemiBold, 12.0, t.subtle);
    y = r.max.y + 10.0;
    for (name, desc) in TOOLS {
        let mid = y + 11.0;
        parts::text(&p, pos2(ix, mid), Align2::LEFT_CENTER, name, W::MonoMedium, 12.0, t.tag_text(t.accent_color));
        let (r, cut) = line(&p, pos2(ix + 140.0, mid), Align2::LEFT_CENTER, desc, W::Regular, 13.0, iw - 140.0, t.subtle);
        if cut {
            ui.interact(r, ui.scope_id().with(("lb-set-tool", name)), Sense::hover()).on_hover_text(desc);
        }
        y += 26.0;
    }
    y += PAD - 4.0;
    let card = Rect::from_min_max(pos2(x, top), pos2(x + w, y));
    p.set(bg, eframe::egui::epaint::RectShape::filled(card, CornerRadius::same(12), t.surface));
    y
}

/// "start PLT-93 in acme" for an issue of the active workspace: one of yours in progress if
/// there is one, else any issue.
fn example_prompt(props: &SettingsProps) -> String {
    let slug = props.active.unwrap_or("acme");
    let Some(b) = props.boards.get(slug) else {
        return format!("start WEB-214 in {slug}");
    };
    let started = |i: &&model::Issue| {
        b.states.iter().any(|s| s.id == i.state_id && s.kind == StateType::Started)
            && i.assignee_id.is_some()
            && i.assignee_id == b.viewer_id
    };
    let issue = b.issues.iter().find(started).or_else(|| b.issues.first());
    issue.map_or_else(|| format!("start WEB-214 in {slug}"), actions::copy_prompt)
}

/// Number circle and title, with an optional line under the title. Returns the y below.
fn step(p: &Painter, t: &Theme, x: f32, y: f32, n: usize, title: &str, sub: Option<&str>) -> f32 {
    let c = pos2(x + 13.0, y + 13.0);
    p.circle_filled(c, 13.0, Theme::tint(t.accent_color, 0.14));
    parts::text(p, c, Align2::CENTER_CENTER, &n.to_string(), W::Bold, 12.0, t.accent_color);
    let r = parts::text(p, pos2(x + 40.0, c.y), Align2::LEFT_CENTER, title, W::SemiBold, 14.0, t.text);
    let mut bottom = r.max.y.max(c.y + 13.0);
    if let Some(s) = sub {
        let r = parts::text(p, pos2(x + 40.0, r.max.y + 4.0), Align2::LEFT_TOP, s, W::Regular, 13.0, t.subtle);
        bottom = r.max.y;
    }
    bottom + 10.0
}

/// Mono text in a box with a Copy button. Long text wraps. Returns the y below the box.
pub(crate) fn code_box(ui: &mut Ui, t: &Theme, at: Pos2, w: f32, text: &str, salt: &str, actions: &mut Vec<Action>) -> f32 {
    let p = ui.painter().clone();
    let copy_w = 76.0;
    let mut job = LayoutJob::single_section(
        text.to_string(),
        TextFormat { font_id: font(W::MonoMedium, 12.0), color: t.text, line_height: Some(19.0), ..Default::default() },
    );
    job.wrap.max_width = w - 28.0 - copy_w;
    let g = p.layout_job(job);
    let h = (g.size().y + 22.0).max(44.0);
    let rect = Rect::from_min_size(at, vec2(w, h));
    p.rect(rect, CornerRadius::same(9), t.base, Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
    p.galley(pos2(rect.left() + 14.0, rect.center().y - g.size().y / 2.0), g, t.text);
    let br = Rect::from_min_max(pos2(rect.right() - 8.0 - copy_w, rect.top()), pos2(rect.right() - 8.0, rect.bottom()));
    let mut row = row_ui(ui, br, salt);
    if copy_button(&mut row, t).clicked() {
        actions.push(Action::Copy(text.to_string()));
    }
    rect.bottom()
}

/// Small button with the copy icon and "Copy". The hover fill fades.
fn copy_button(ui: &mut Ui, t: &Theme) -> eframe::egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(68.0, 28.0), Sense::click());
    let resp = resp.on_hover_cursor(eframe::egui::CursorIcon::PointingHand);
    let hover = motion::toggle(ui.ctx(), resp.id.with("hover"), resp.hovered());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(6), t.overlay.lerp_to_gamma(t.hl_med, hover), Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
    let c = t.subtle.lerp_to_gamma(t.text, hover);
    parts::icon(p, pos2(rect.left() + 17.0, rect.center().y), Icon::Copy, 13.0, c);
    parts::text(p, pos2(rect.left() + 29.0, rect.center().y), Align2::LEFT_CENTER, "Copy", W::SemiBold, 12.0, c);
    resp
}

// ------------------------------------------------------------------ appearance

fn appearance(ui: &mut Ui, t: &Theme, x: f32, y: f32, w: f32, props: &SettingsProps, actions: &mut Vec<Action>) -> f32 {
    let line_text = "Pick a theme. Each workspace keeps its own accent color.";
    let y = heading(ui, t, x, y, w, "Appearance", line_text);
    theme_grid::show(ui, t, x, y, w, 132.0, props.theme, "lb-set-theme", actions)
}

// ------------------------------------------------------------------ about

fn about(ui: &mut Ui, t: &Theme, x: f32, y: f32, w: f32, state: &SettingsState, actions: &mut Vec<Action>) -> f32 {
    let top = heading(ui, t, x, y, w, "About", "Where Kanbanana keeps its data.");
    let p = ui.painter().clone();
    let brand_top = top;
    let top = brand(&p, t, x, top) + 20.0;
    let r = Rect::from_min_max(pos2(x + w - 240.0, brand_top), pos2(x + w + 8.0, top - 20.0));
    let mut row = row_ui(ui, r, "lb-set-onboarding");
    if parts::button(&mut row, t, "Run onboarding again", Btn::Ghost, None, true).clicked() {
        actions.push(Action::RunOnboarding);
    }
    const ROW_H: f32 = 48.0;
    let card = Rect::from_min_size(pos2(x, top), vec2(w, 2.0 * ROW_H));
    p.rect_filled(card, CornerRadius::same(12), t.surface);
    let rows: [(&str, String, Option<&std::path::PathBuf>); 2] = [
        ("Cache", path_text(state.cache.as_ref()), state.cache.as_ref()),
        ("Keys", path_text(state.credentials.as_ref()), state.credentials.as_ref()),
    ];
    for (i, (label, value, path)) in rows.iter().enumerate() {
        let row = Rect::from_min_size(pos2(x, top + i as f32 * ROW_H), vec2(w, ROW_H));
        let mid = row.center().y;
        parts::text(&p, pos2(row.left() + PAD, mid), Align2::LEFT_CENTER, label, W::SemiBold, 13.0, t.subtle);
        let vx = row.left() + PAD + 90.0;
        let vw = w - PAD - 90.0 - PAD - 120.0;
        let (r, cut) = line(&p, pos2(vx, mid), Align2::LEFT_CENTER, value, W::MonoMedium, 12.0, vw, t.text);
        if cut {
            ui.interact(r, ui.scope_id().with(("lb-set-path", i)), Sense::hover()).on_hover_text(value.as_str());
        }
        if let Some(path) = path {
            let br = Rect::from_min_max(pos2(row.right() - PAD - 120.0, row.top()), pos2(row.right() - PAD + 8.0, row.bottom()));
            let mut r = row_ui(ui, br, ("lb-set-open", i));
            if parts::button(&mut r, t, "Open folder", Btn::Ghost, None, true).clicked() {
                actions.push(Action::OpenFolder((*path).clone()));
            }
        }
        if i + 1 < rows.len() {
            p.hline(row.x_range().shrink(PAD), row.bottom() - 0.5, Stroke::new(1.0, t.hl_low));
        }
    }
    let y = card.bottom() + 14.0;
    parts::icon(&p, pos2(x + 7.0, y + 8.0), Icon::Key, 14.0, t.muted);
    let r = parts::text(&p, pos2(x + 22.0, y + 8.0), Align2::LEFT_CENTER, "Keys stay on this computer in a file only you can read.", W::Medium, 12.0, t.muted);
    r.max.y
}

/// The app icon, the name with the version on its baseline, and the tagline. Returns the y under it.
fn brand(p: &Painter, t: &Theme, x: f32, y: f32) -> f32 {
    const ICON: f32 = 56.0;
    crate::brand::icon(p, Rect::from_min_size(pos2(x, y), vec2(ICON, ICON)));
    let tx = x + ICON + 16.0;
    let name = parts::text(p, pos2(tx, y + 8.0), Align2::LEFT_TOP, "Kanbanana", W::SemiBold, 18.0, t.text);
    let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
    parts::text(p, pos2(name.max.x + 10.0, name.bottom() - 2.0), Align2::LEFT_BOTTOM, &version, W::MonoMedium, 12.0, t.muted);
    let tagline = "A kanban board for your Linear issues. Slightly bananas.";
    parts::text(p, pos2(tx, name.bottom() + 6.0), Align2::LEFT_TOP, tagline, W::Medium, 13.0, t.muted);
    y + ICON
}

fn path_text(p: Option<&std::path::PathBuf>) -> String {
    p.map_or("Unknown".to_string(), |p| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_prompt_prefers_your_started_issue() {
        let ws = crate::mock::workspaces();
        let boards: HashMap<String, BoardData> =
            ws.iter().map(|w| (w.slug.clone(), crate::mock::board(&w.slug))).collect();
        let props = SettingsProps { workspaces: &ws, boards: &boards, active: Some("acme"), theme: 0 };
        assert_eq!(example_prompt(&props), "start WEB-214 in acme");
        let props = SettingsProps { active: Some("none"), ..props };
        assert_eq!(example_prompt(&props), "start WEB-214 in none");
    }
}
