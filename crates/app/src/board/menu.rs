//! Right-click menu of a card: open, copy the id and title, copy a prompt, open in Linear.

use eframe::egui::{
    Align2, Area, Context, CornerRadius, CursorIcon, Id, Key, LayerId, Order, Pos2, Rect, Sense, Stroke,
    StrokeKind, emath::TSTransform, pos2, vec2,
};
use model::Issue;

use crate::actions::{self, Action};
use crate::theme::parts::{self, CMD, Icon};
use crate::theme::{Theme, W, motion};

const MIN_WIDTH: f32 = 290.0;
const PAD: f32 = 6.0;
const ROW_H: f32 = 32.0;
const ROW_GAP: f32 = 2.0;

pub struct Menu {
    pub issue_id: String,
    pos: Pos2,
    /// True in the frame that opened the menu, so the opening click does not close it.
    fresh: bool,
    /// Row that looks hovered in screenshots.
    demo_hover: Option<usize>,
    /// When the menu opened, for its fade in. Set on the first frame.
    start: Option<f64>,
}

impl Menu {
    pub fn new(issue_id: String, pos: Pos2) -> Menu {
        Menu { issue_id, pos, fresh: true, demo_hover: None, start: None }
    }

    /// A menu for `--screen` shots, with one row drawn as hovered.
    pub fn demo(issue_id: &str, pos: Pos2, hover: usize) -> Menu {
        Menu { demo_hover: Some(hover), start: Some(f64::NEG_INFINITY), ..Menu::new(issue_id.into(), pos) }
    }
}

#[derive(Clone, Copy)]
enum Item {
    Open,
    CopyTitle,
    CopyPrompt,
    Linear,
}

fn rows() -> [(Item, Icon, &'static str, Option<String>); 4] {
    [
        (Item::Open, Icon::External, "Open", Some("Enter".into())),
        (Item::CopyTitle, Icon::Copy, "Copy ID and title", Some(format!("{CMD} C"))),
        (Item::CopyPrompt, Icon::Asterisk, "Copy prompt for Claude", Some(format!("{CMD} Shift C"))),
        (Item::Linear, Icon::Link, "Open in Linear", None),
    ]
}

fn run(item: Item, issue: &Issue, actions: &mut Vec<Action>) {
    match item {
        Item::Open => actions.push(Action::SelectIssue(Some(issue.id.clone()))),
        Item::CopyTitle => actions.push(Action::Copy(actions::copy_title(issue))),
        Item::CopyPrompt => actions.push(Action::Copy(actions::copy_prompt(issue))),
        Item::Linear => {
            if let Some(url) = &issue.url {
                actions.push(Action::OpenUrl(url.clone()));
            }
        }
    }
}

/// Paints the open menu and runs the row the user picks. Closes on a pick, Escape or a click outside.
pub fn show(ctx: &Context, t: &Theme, menu: &mut Option<Menu>, issues: &[Issue], actions: &mut Vec<Action>) {
    let Some(m) = menu.as_mut() else { return };
    let Some(issue) = issues.iter().find(|i| i.id == m.issue_id) else {
        *menu = None;
        return;
    };
    let rows = rows();
    let h = 2.0 * PAD + rows.len() as f32 * ROW_H + rows.len() as f32 * ROW_GAP + 1.0;
    let mut picked = None;
    // Fades in and grows from 96 % around the click point.
    let shown = motion::since(ctx, *m.start.get_or_insert_with(|| motion::now(ctx)));
    let id = Id::unique("lb-card-menu");
    let scale = motion::lerp(0.96, 1.0, shown);
    ctx.set_transform_layer(LayerId::new(Order::Foreground, id), TSTransform::new(m.pos.to_vec2() * (1.0 - scale), scale));
    let area = Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(m.pos)
        .constrain(true)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(shown);
            let p = ui.painter().clone();
            let p = &p;
            let widest = rows
                .iter()
                .map(|(_, _, label, hint)| {
                    let hw = hint.as_deref().map_or(0.0, |h| 24.0 + parts::text_size(p, h, W::MonoMedium, 11.0).x);
                    35.0 + parts::text_size(p, label, W::Medium, 13.0).x + hw + 10.0
                })
                .fold(0.0, f32::max);
            let width = (widest + 2.0 * PAD).max(MIN_WIDTH);
            let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
            p.add(t.shadow().as_shape(rect, CornerRadius::same(12)));
            p.rect(rect, CornerRadius::same(12), t.overlay, Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
            let mut y = rect.min.y + PAD;
            for (i, (item, icon, label, hint)) in rows.iter().enumerate() {
                if matches!(item, Item::Linear) {
                    let x = rect.x_range().shrink(PAD + 4.0);
                    p.hline(x, y + 0.5, Stroke::new(1.0, t.hl_med));
                    y += 1.0 + ROW_GAP;
                }
                let row = Rect::from_min_size(pos2(rect.min.x + PAD, y), vec2(width - 2.0 * PAD, ROW_H));
                let enabled = !matches!(item, Item::Linear) || issue.url.is_some();
                let resp = ui.interact(row, Id::unique(("lb-card-menu-row", i)), Sense::click());
                let resp = if enabled { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp };
                let hover = enabled && (resp.hovered() || m.demo_hover == Some(i));
                if hover {
                    p.rect_filled(row, CornerRadius::same(7), t.hl_med);
                }
                let c = match (enabled, hover) {
                    (false, _) => t.muted,
                    (true, true) => t.text,
                    (true, false) => t.subtle,
                };
                let mid = row.center().y;
                parts::icon(p, pos2(row.min.x + 10.0 + 7.5, mid), *icon, 15.0, c);
                parts::text(p, pos2(row.min.x + 35.0, mid), Align2::LEFT_CENTER, label, W::Medium, 13.0, c);
                if let Some(hint) = hint {
                    parts::text(p, pos2(row.max.x - 10.0, mid), Align2::RIGHT_CENTER, hint, W::MonoMedium, 11.0, t.muted);
                }
                if enabled && resp.clicked() {
                    picked = Some(*item);
                }
                y += ROW_H + ROW_GAP;
            }
        });

    let (esc, enter, pressed, pointer) = ctx.input(|i| {
        (i.key_pressed(Key::Escape), i.key_pressed(Key::Enter), i.pointer.any_pressed(), i.pointer.interact_pos())
    });
    if enter {
        picked = Some(Item::Open);
    }
    let outside = pressed && pointer.is_some_and(|p| !area.response.rect.contains(p));
    if let Some(item) = picked {
        run(item, issue, actions);
        *menu = None;
    } else if esc || (outside && !m.fresh) {
        *menu = None;
    } else {
        m.fresh = false;
    }
}
