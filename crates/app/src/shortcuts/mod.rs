//! Keyboard shortcuts overlay. `?` opens it, Esc, `?` or a click outside closes it.

use eframe::egui::{
    self, Align, Align2, Area, Event, Frame, Id, Key, LayerId, Layout, Margin, Modifiers, Order, Sense, Stroke,
    Ui, emath::TSTransform, pos2, vec2,
};

use crate::theme::parts::{self, ALT, CMD, Icon};
use crate::theme::{Theme, W, motion};

const WIDTH: f32 = 760.0;
const ROW_H: f32 = 30.0;

/// The overlay. `opened_at` is the egui time of the open, for the fade in.
#[derive(Default)]
pub struct Shortcuts {
    pub opened_at: Option<f64>,
}

impl Shortcuts {
    pub fn is_open(&self) -> bool {
        self.opened_at.is_some()
    }

    pub fn toggle(&mut self, now: f64) {
        self.opened_at = if self.is_open() { None } else { Some(now) };
    }
}

/// One row: what it does and its keys. Each key string is one kbd box.
type Row = (&'static str, Vec<String>);

fn k(s: &str) -> String {
    s.to_string()
}

/// Every shortcut, in groups. README.md has the same list.
fn groups() -> Vec<(&'static str, Vec<Row>)> {
    vec![
        (
            "Navigation",
            vec![
                ("Next card in the column", vec![k("J"), k("↓")]),
                ("Previous card in the column", vec![k("K"), k("↑")]),
                ("Next or previous column", vec![k("→"), k("←")]),
                ("Open the focused issue", vec![k("Enter"), k("O")]),
                ("Close the panel, then clear focus", vec![k("Esc")]),
            ],
        ),
        (
            "Issue",
            vec![
                ("New issue", vec![k("C")]),
                ("Status", vec![k("S")]),
                ("Priority", vec![k("P")]),
                ("Assignee", vec![k("A")]),
                ("Labels", vec![k("L")]),
                ("Assign to me", vec![k("I")]),
                ("Urgent, high, medium, low", vec![k("1"), k("2"), k("3"), k("4")]),
                ("No priority", vec![k("0")]),
                ("Edit the description", vec![k("E")]),
                ("Copy ID and title", vec![format!("{CMD} C")]),
                ("Copy prompt for Claude", vec![format!("{CMD} Shift C")]),
                ("Create the issue in the form", vec![format!("{CMD} Enter")]),
            ],
        ),
        (
            "Board",
            vec![
                ("Assigned to me", vec![k("M")]),
                ("Sync now", vec![k("R")]),
                ("Keyboard shortcuts", vec![k("?")]),
            ],
        ),
        (
            "Workspaces",
            vec![("Switch workspace", vec![format!("{ALT} 1 to 9")]), ("Settings", vec![format!("{CMD} ,")])],
        ),
    ]
}

pub fn show(ctx: &egui::Context, t: &Theme, state: &mut Shortcuts) {
    let Some(opened_at) = state.opened_at else { return };
    let shown = motion::since(ctx, opened_at);
    let close_key = ctx.input_mut(|i| {
        let q = i.events.iter().any(|e| matches!(e, Event::Text(s) if s == "?"));
        // The `?` text event stays, so a focused text field still gets it. None has focus here.
        i.consume_key(Modifiers::NONE, Key::Escape) || q
    });
    let screen = ctx.content_rect();
    let clicked = Area::new(Id::unique("lb-keys-scrim"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .fade_in(false)
        .show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0.0, Theme::tint(t.base, 0.7 * shown));
            resp.clicked()
        })
        .inner;

    let w = WIDTH.min(screen.width() - 48.0);
    let top = screen.top() + (screen.height() * 0.1).max(32.0);
    let x = screen.center().x - w / 2.0;
    let id = Id::unique("lb-keys");
    let origin = pos2(screen.center().x, top).to_vec2();
    let scale = motion::lerp(0.97, 1.0, shown);
    let grow = TSTransform::from_translation(origin) * TSTransform::from_scaling(scale) * TSTransform::from_translation(-origin);
    ctx.set_transform_layer(LayerId::new(Order::Foreground, id), grow);
    let mut close = close_key || clicked;
    let area = Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(pos2(x, top))
        .fade_in(false)
        .constrain(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(shown);
            Frame::NONE
                .fill(t.overlay)
                .stroke(Stroke::new(1.0, t.hl_med))
                .corner_radius(14)
                .shadow(t.shadow())
                .inner_margin(Margin { left: 24, right: 16, top: 16, bottom: 20 })
                .show(ui, |ui| {
                    ui.set_width(w - 40.0);
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    ui.horizontal(|ui| {
                        ui.set_min_height(28.0);
                        let p = ui.painter().clone();
                        let (r, _) = ui.allocate_exact_size(vec2(300.0, 28.0), Sense::hover());
                        parts::text(&p, r.left_center(), Align2::LEFT_CENTER, "Keyboard shortcuts", W::SemiBold, 16.0, t.text);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if parts::icon_button(ui, t, Icon::X, t.muted).on_hover_text("Close (Esc)").clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    let p = ui.painter().clone();
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width() - 8.0, 18.0), Sense::hover());
                    let note = "They work when no text field has focus.";
                    parts::text(&p, r.left_center(), Align2::LEFT_CENTER, note, W::Medium, 12.0, t.muted);
                    ui.add_space(14.0);
                    let all = groups();
                    let col_w = (ui.available_width() - 8.0 - 32.0) / 2.0;
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 32.0;
                        // The long Issue group gets the right column alone.
                        let (right, left): (Vec<_>, Vec<_>) = all.iter().partition(|(title, _)| *title == "Issue");
                        for side in [left, right] {
                            ui.vertical(|ui| {
                                ui.set_width(col_w);
                                for (n, (title, rows)) in side.iter().enumerate() {
                                    if n > 0 {
                                        ui.add_space(18.0);
                                    }
                                    group(ui, t, title, rows, col_w);
                                }
                            });
                        }
                    });
                });
        });
    ctx.move_to_top(area.response.layer_id);
    if close {
        state.opened_at = None;
    }
}

fn group(ui: &mut Ui, t: &Theme, title: &str, rows: &[Row], width: f32) {
    let p = ui.painter().clone();
    let (r, _) = ui.allocate_exact_size(vec2(width, 22.0), Sense::hover());
    parts::text(&p, r.left_center(), Align2::LEFT_CENTER, title, W::SemiBold, 12.0, t.subtle);
    ui.add_space(4.0);
    for (label, keys) in rows {
        let (r, _) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::hover());
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, t.hl_low));
        let cy = r.center().y;
        parts::text(&p, pos2(r.left(), cy), Align2::LEFT_CENTER, label, W::Medium, 13.0, t.text);
        let mut x = r.right();
        for key in keys.iter().rev() {
            let s = parts::kbd_size(&p, key);
            x -= s.x;
            parts::kbd(&p, t, pos2(x, cy), key);
            x -= 4.0;
        }
    }
}
