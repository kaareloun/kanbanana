//! The onboarding screen: app icon, step indicator, the step and a footer with the buttons.

use std::f32::consts::TAU;
use std::path::Path;

use eframe::egui::{
    Align, Align2, CentralPanel, CornerRadius, CursorIcon, Frame, Id, Key, Layout, Rect, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, pos2, vec2,
    text::{LayoutJob, TextFormat},
};
use model::Workspace;

use super::claude::{self, Status};
use super::{OnboardingState, Step, theme_grid};
use crate::actions::Action;
use crate::settings::page::code_box;
use crate::theme::parts::{self, Btn, Icon};
use crate::theme::{Theme, W, motion};
use crate::workspace::{AddForm, page as ws_page};

const COL_W: f32 = 720.0;
const FOOTER_H: f32 = 76.0;
/// How far a step slides in, in points.
const SLIDE: f32 = 28.0;

pub struct OnboardingProps<'a> {
    pub workspaces: &'a [Workspace],
    /// Index of the base theme in `THEMES`.
    pub theme: usize,
    pub exe: Option<&'a Path>,
}

pub fn show(
    ui: &mut Ui,
    t: &Theme,
    props: &OnboardingProps,
    state: &mut OnboardingState,
    form: &mut AddForm,
    actions: &mut Vec<Action>,
) {
    if state.step == Step::Claude && state.claude.status == Status::Unknown {
        actions.push(Action::CheckClaude);
    }
    CentralPanel::default().frame(Frame::NONE.fill(t.base)).show(ui, |ui| {
        let full = ui.max_rect();
        let w = COL_W.min(full.width() - 64.0);
        let x = full.center().x - w / 2.0;
        let body = Rect::from_min_max(full.min, pos2(full.right(), full.bottom() - FOOTER_H));
        let mut body_ui = ui.new_child(UiBuilder::new().id_salt("lb-onb-body").max_rect(body));
        body_ui.set_clip_rect(body);
        ScrollArea::vertical().auto_shrink([false, false]).show(&mut body_ui, |ui| {
            let avail = ui.max_rect();
            let mut y = avail.top() + 48.0;
            let p = ui.painter().clone();
            crate::brand::icon(&p, Rect::from_center_size(pos2(full.center().x, y + 22.0), vec2(44.0, 44.0)));
            y += 44.0 + 24.0;
            y = stepper(ui, t, full.center().x, y, state) + 40.0;

            // The new step fades in and slides in from the side the user moved to.
            let k = motion::since(ui.ctx(), state.changed_at);
            let sx = x + state.dir * SLIDE * (1.0 - k);
            let mut c = ui.new_child(UiBuilder::new().id_salt(("lb-onb-step", state.step)).max_rect(avail));
            c.set_opacity(k);
            let end = match state.step {
                Step::Theme => theme_step(&mut c, t, sx, y, w, props, actions),
                Step::Workspace => workspace_step(&mut c, t, sx, y, w, props, state, form, actions),
                Step::Claude => claude_step(&mut c, t, sx, y, w, props, state, actions),
            };
            ui.allocate_rect(Rect::from_min_max(avail.min, pos2(avail.left() + 1.0, end + 32.0)), Sense::hover());
        });
        let foot = Rect::from_min_max(pos2(x, full.bottom() - FOOTER_H), pos2(x + w, full.bottom()));
        footer(ui, t, foot, props, state, form, actions);
    });
}

/// Three numbered steps with a thin line between them. Returns the y under it.
fn stepper(ui: &mut Ui, t: &Theme, cx: f32, y: f32, state: &mut OnboardingState) -> f32 {
    const R: f32 = 12.0;
    const LINE: f32 = 56.0;
    let p = ui.painter().clone();
    let labels = ["Theme", "Workspace", "Claude Code"];
    let widths: Vec<f32> = labels.iter().map(|l| 2.0 * R + 10.0 + parts::text_size(&p, l, W::SemiBold, 13.0).x).collect();
    let total = widths.iter().sum::<f32>() + LINE * 2.0 + 2.0 * 2.0 * 14.0;
    let mut x = cx - total / 2.0;
    let mid = y + R;
    let current = state.step.index();
    for (i, (label, iw)) in labels.iter().zip(&widths).enumerate() {
        let item = Rect::from_min_size(pos2(x, y), vec2(*iw, 2.0 * R));
        let c = pos2(x + R, mid);
        let done = i < current;
        let resp = ui.interact(item, Id::unique(("lb-onb-stepper", i)), Sense::click());
        let resp = if done { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp };
        if done {
            p.circle_filled(c, R, Theme::tint(t.accent_color, 0.16));
            parts::icon(&p, c, Icon::Check, 13.0, t.accent_color);
        } else if i == current {
            p.circle_filled(c, R, t.accent_color);
            parts::text(&p, c, Align2::CENTER_CENTER, &(i + 1).to_string(), W::Bold, 12.0, t.on_accent(t.accent_color));
        } else {
            p.circle_filled(c, R, t.overlay);
            parts::text(&p, c, Align2::CENTER_CENTER, &(i + 1).to_string(), W::Bold, 12.0, t.muted);
        }
        let tc = if i == current {
            t.text
        } else if done && resp.hovered() {
            t.text
        } else if done {
            t.subtle
        } else {
            t.muted
        };
        parts::text(&p, pos2(c.x + R + 10.0, mid), Align2::LEFT_CENTER, label, W::SemiBold, 13.0, tc);
        if done && resp.clicked() {
            state.go(Step::ALL[i], motion::now(ui.ctx()));
        }
        x += iw;
        if i + 1 < labels.len() {
            let color = if done { Theme::tint(t.accent_color, 0.6) } else { t.hl_med };
            p.hline(x + 14.0..=x + 14.0 + LINE, mid, Stroke::new(1.0, color));
            x += LINE + 28.0;
        }
    }
    y + 2.0 * R
}

/// Title and one or two lines under it, centered. Returns the y under them.
fn heading(ui: &Ui, t: &Theme, x: f32, y: f32, w: f32, title: &str, lines: &[&str]) -> f32 {
    let p = ui.painter();
    let cx = x + w / 2.0;
    let r = parts::text(p, pos2(cx, y), Align2::CENTER_TOP, title, W::Bold, 26.0, t.text);
    let mut y = r.max.y + 12.0;
    for l in lines {
        let r = parts::text(p, pos2(cx, y), Align2::CENTER_TOP, l, W::Regular, 14.0, t.subtle);
        y = r.max.y + 4.0;
    }
    y + 28.0
}

// ------------------------------------------------------------------ step 1

fn theme_step(ui: &mut Ui, t: &Theme, x: f32, y: f32, w: f32, props: &OnboardingProps, actions: &mut Vec<Action>) -> f32 {
    let lines = ["A kanban board for your Linear issues. Pick a look, you can change it later in Settings."];
    let y = heading(ui, t, x, y, w, "Welcome to Kanbanana", &lines);
    // Three rows of this height fit the window at 1440x900 without a scroll.
    theme_grid::show(ui, t, x, y, w, 116.0, props.theme, "lb-onb-theme", actions)
}

// ------------------------------------------------------------------ step 2

#[allow(clippy::too_many_arguments)]
fn workspace_step(
    ui: &mut Ui,
    t: &Theme,
    x: f32,
    y: f32,
    w: f32,
    props: &OnboardingProps,
    state: &OnboardingState,
    form: &mut AddForm,
    actions: &mut Vec<Action>,
) -> f32 {
    let title = if props.workspaces.is_empty() || state.linked.is_some() {
        "Link your first workspace"
    } else {
        "Link a workspace"
    };
    let lines = ["Each workspace is one Linear organization with its own key."];
    let mut y = heading(ui, t, x, y, w, title, &lines);
    if let Some(name) = &state.linked {
        let card = Rect::from_min_size(pos2(x, y), vec2(w, 76.0));
        let p = ui.painter();
        p.rect_filled(card, CornerRadius::same(14), t.surface);
        let c = pos2(card.left() + 36.0, card.center().y);
        p.circle_filled(c, 16.0, Theme::tint(t.foam, 0.16));
        parts::icon(p, c, Icon::Check, 16.0, t.foam);
        let tx = c.x + 30.0;
        let s = format!("{name} is linked");
        parts::text(p, pos2(tx, card.center().y - 2.0), Align2::LEFT_BOTTOM, &s, W::SemiBold, 15.0, t.text);
        let sub = "The first sync runs in the background.";
        parts::text(p, pos2(tx, card.center().y + 2.0), Align2::LEFT_TOP, sub, W::Medium, 13.0, t.muted);
        return card.bottom();
    }
    let col_w = (w - 2.0 * 24.0) / 3.0;
    let mut h: f32 = 0.0;
    for n in 0..3 {
        let at = pos2(x + n as f32 * (col_w + 24.0), y);
        h = h.max(ws_page::instruction(ui, t, at, n, col_w - 40.0, form));
    }
    y += h + 28.0;
    y + ws_page::embedded_form(ui, t, pos2(x, y), w, props.workspaces, form, actions)
}

// ------------------------------------------------------------------ step 3

#[allow(clippy::too_many_arguments)]
fn claude_step(
    ui: &mut Ui,
    t: &Theme,
    x: f32,
    y: f32,
    w: f32,
    props: &OnboardingProps,
    state: &OnboardingState,
    actions: &mut Vec<Action>,
) -> f32 {
    let lines = [
        "Claude reports the branch, MR and deploy stage of the issue it works on.",
        "It can read your issues and, when you ask, change them.",
    ];
    let y = heading(ui, t, x, y, w, "Connect Claude Code", &lines);
    let link = &state.claude;
    let p = ui.painter().clone();
    // The card height is known only at the end, so its fill goes in a slot painted under the rest.
    let bg = p.add(eframe::egui::Shape::Noop);
    let pad = 22.0;
    let ix = x + pad;
    let iw = w - 2.0 * pad;
    let row = Rect::from_min_size(pos2(ix, y + pad), vec2(iw, 40.0));
    let mid = row.center().y;
    let ic = pos2(ix + 16.0, mid);
    let busy = link.adding || link.status == Status::Checking;
    let (title, sub) = if link.adding {
        ("Adding to Claude Code", "This takes a few seconds.")
    } else {
        match link.status {
            Status::Unknown | Status::Checking => ("Looking for Claude Code", "This takes a few seconds."),
            Status::NotFound => ("Claude Code not found", "Install it, then check again."),
            Status::NotConnected => ("Not connected yet", "Kanbanana goes in your user config, so every project gets it."),
            Status::Connected => ("Connected", "Restart Claude Code, then ask it to start an issue."),
        }
    };
    if busy {
        ui.ctx().request_repaint();
        let angle = (motion::now(ui.ctx()) % 1.0) as f32 * TAU;
        crate::brand::banana(&p, Rect::from_center_size(ic, vec2(24.0, 24.0)), angle, 1.0);
    } else {
        let (icon, c) = match link.status {
            Status::Connected => (Icon::Check, t.foam),
            Status::NotFound => (Icon::X, t.gold),
            _ => (Icon::Plus, t.subtle),
        };
        p.circle_filled(ic, 16.0, Theme::tint(c, 0.16));
        parts::icon(&p, ic, icon, 15.0, c);
    }
    let tx = ic.x + 30.0;
    parts::text(&p, pos2(tx, mid - 1.0), Align2::LEFT_BOTTOM, title, W::SemiBold, 15.0, t.text);
    parts::text(&p, pos2(tx, mid + 2.0), Align2::LEFT_TOP, sub, W::Medium, 13.0, t.muted);
    let mut right = ui.new_child(
        UiBuilder::new().id_salt("lb-onb-claude-btn").max_rect(row).layout(Layout::right_to_left(Align::Center)),
    );
    if !busy {
        match link.status {
            Status::NotConnected => {
                if parts::button(&mut right, t, "Add to Claude Code", Btn::Primary, None, true).clicked() {
                    actions.push(Action::AddToClaude);
                }
            }
            Status::NotFound => {
                if parts::button(&mut right, t, "Check again", Btn::Secondary, None, true).clicked() {
                    actions.push(Action::CheckClaude);
                }
            }
            _ => {}
        }
    }
    let mut y = row.bottom() + 18.0;
    if let Some(err) = link.error.as_deref().filter(|_| !link.adding) {
        y = error_box(ui, t, pos2(ix, y), iw, err) + 18.0;
    }
    if link.status == Status::NotFound && !busy {
        y = code_box(ui, t, pos2(ix, y), iw, claude::INSTALL, "lb-onb-install", actions) + 18.0;
    }
    let card = Rect::from_min_max(pos2(x, row.top() - pad), pos2(x + w, y - 18.0 + pad));
    p.set(bg, eframe::egui::epaint::RectShape::filled(card, CornerRadius::same(14), t.surface));

    if link.status == Status::Connected {
        return card.bottom();
    }
    let y = card.bottom() + 28.0;
    let r = parts::text(&p, pos2(x, y), Align2::LEFT_TOP, "Or run it yourself", W::SemiBold, 12.0, t.subtle);
    let command = props.exe.map_or_else(|| "claude mcp add --scope user kanbanana -- kanbanana mcp".into(), claude::add_command);
    code_box(ui, t, pos2(x, r.bottom() + 10.0), w, &command, "lb-onb-cmd", actions)
}

/// The output of a failed command in a small mono box. Returns its bottom.
fn error_box(ui: &Ui, t: &Theme, at: eframe::egui::Pos2, w: f32, text: &str) -> f32 {
    let p = ui.painter();
    let mut job = LayoutJob::single_section(
        text.to_string(),
        TextFormat { font_id: crate::theme::font(W::MonoMedium, 12.0), color: t.love, line_height: Some(19.0), ..Default::default() },
    );
    job.wrap.max_width = w - 28.0;
    job.wrap.max_rows = 6;
    let g = p.layout_job(job);
    let rect = Rect::from_min_size(at, vec2(w, g.size().y + 22.0));
    p.rect(rect, CornerRadius::same(9), t.base, Stroke::new(1.0, Theme::tint(t.love, 0.45)), StrokeKind::Inside);
    p.galley(pos2(rect.left() + 14.0, rect.top() + 11.0), g, t.love);
    rect.bottom()
}

// ------------------------------------------------------------------ footer

fn footer(
    ui: &mut Ui,
    t: &Theme,
    rect: Rect,
    props: &OnboardingProps,
    state: &mut OnboardingState,
    form: &mut AddForm,
    actions: &mut Vec<Action>,
) {
    ui.painter().hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, t.hl_low));
    let now = motion::now(ui.ctx());
    let mut left = ui.new_child(UiBuilder::new().id_salt("lb-onb-back").max_rect(rect).layout(Layout::left_to_right(Align::Center)));
    if state.step != Step::Theme && parts::button(&mut left, t, "Back", Btn::Ghost, None, true).clicked() {
        let back = Step::ALL[state.step.index() - 1];
        state.go(back, now);
    }
    let mut right = ui.new_child(UiBuilder::new().id_salt("lb-onb-next").max_rect(rect).layout(Layout::right_to_left(Align::Center)));
    right.spacing_mut().item_spacing.x = 10.0;
    match state.step {
        Step::Theme => {
            let enter = ui.input(|i| i.key_pressed(Key::Enter));
            if parts::button(&mut right, t, "Continue", Btn::Primary, Some("Enter"), true).clicked() || enter {
                state.go(Step::Workspace, now);
            }
        }
        Step::Workspace => {
            let linked = state.linked.is_some();
            let ok = linked || ws_page::can_add(form, props.workspaces);
            let hint = format!("{} Enter", parts::CMD);
            let hint = if linked { None } else { Some(hint.as_str()) };
            if parts::button(&mut right, t, "Continue", Btn::Primary, hint, ok).clicked() && ok {
                if linked {
                    state.go(Step::Claude, now);
                } else {
                    // The app moves on to the next step when it has added the workspace.
                    ws_page::submit(form, props.workspaces, actions);
                }
            }
            if !linked && parts::button(&mut right, t, "Skip for now", Btn::Ghost, None, true).clicked() {
                state.go(Step::Claude, now);
            }
        }
        Step::Claude => {
            if state.claude.status == Status::Connected {
                if parts::button(&mut right, t, "Finish", Btn::Primary, None, true).clicked() {
                    actions.push(Action::FinishOnboarding);
                }
            } else if parts::button(&mut right, t, "Finish without Claude Code", Btn::Secondary, None, true).clicked() {
                actions.push(Action::FinishOnboarding);
            }
        }
    }
}
