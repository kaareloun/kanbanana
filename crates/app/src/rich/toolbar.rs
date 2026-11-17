//! The floating format bar above a selection, and the link popover.

use eframe::egui::{
    self, Align2, Area, Context, CornerRadius, CursorIcon, Frame, Id, Key, Margin, Order, Pos2, Rect, Sense, Stroke,
    TextEdit, Ui, pos2, vec2,
};

use crate::theme::parts::{self, CMD, Icon};
use crate::theme::{Theme, W, font};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Bold,
    Code,
    Link,
    Heading,
    Bullet,
    Check,
}

fn popover(t: &Theme) -> Frame {
    Frame::NONE
        .fill(t.overlay)
        .stroke(Stroke::new(1.0, t.hl_med))
        .corner_radius(CornerRadius::same(10))
        .shadow(t.shadow_sm())
        .inner_margin(Margin::same(4))
}

/// Shows the bar centered above `anchor`. Returns the clicked tool.
pub fn show(ctx: &Context, t: &Theme, id: Id, anchor: Pos2, headings: bool) -> Option<Tool> {
    let mut picked = None;
    Area::new(id.with("toolbar"))
        .order(Order::Foreground)
        .fade_in(false)
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(anchor - vec2(0.0, 8.0))
        .show(ctx, |ui| {
            popover(t).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let mut tools = vec![
                        (Tool::Bold, format!("Bold  {CMD} B")),
                        (Tool::Code, format!("Code  {CMD} E")),
                        (Tool::Link, format!("Link  {CMD} K")),
                    ];
                    if headings {
                        tools.push((Tool::Heading, "Heading".to_string()));
                    }
                    tools.push((Tool::Bullet, format!("Bulleted list  {CMD} Shift 8")));
                    tools.push((Tool::Check, format!("Checklist  {CMD} Shift 9")));
                    for (k, tip) in tools {
                        if k == Tool::Bullet || k == Tool::Link {
                            sep(ui, t);
                        }
                        if button(ui, t, k, &tip) {
                            picked = Some(k);
                        }
                    }
                });
            });
        });
    picked
}

fn sep(ui: &mut Ui, t: &Theme) {
    let (r, _) = ui.allocate_exact_size(vec2(7.0, 18.0), Sense::hover());
    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, t.hl_med));
}

fn button(ui: &mut Ui, t: &Theme, k: Tool, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 28.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, CornerRadius::same(6), t.hl_med);
    }
    let c = if resp.hovered() { t.text } else { t.subtle };
    let label = |s: &str, w: W, size: f32| {
        parts::text(p, r.center(), Align2::CENTER_CENTER, s, w, size, c);
    };
    match k {
        Tool::Bold => label("B", W::Bold, 14.0),
        Tool::Code => label("</>", W::MonoMedium, 12.0),
        Tool::Heading => label("H", W::Bold, 14.0),
        Tool::Link => parts::icon(p, r.center(), Icon::Link, 15.0, c),
        Tool::Bullet => {
            for i in 0..3 {
                let y = r.center().y - 4.5 + i as f32 * 4.5;
                p.circle_filled(pos2(r.center().x - 5.0, y), 1.3, c);
                p.hline(r.center().x - 2.0..=r.center().x + 6.0, y, Stroke::new(1.4, c));
            }
        }
        Tool::Check => {
            let b = Rect::from_center_size(r.center(), vec2(12.0, 12.0));
            p.rect_stroke(b, CornerRadius::same(3), Stroke::new(1.4, c), egui::StrokeKind::Inside);
            parts::icon(p, r.center(), Icon::Check, 10.0, c);
        }
    }
    resp.clicked()
}

/// What the link popover did this frame.
pub enum LinkEvent {
    Apply(String),
    Cancel,
}

/// The url field under `anchor`. Enter applies, Esc or a click elsewhere cancels.
pub fn link_prompt(ctx: &Context, t: &Theme, id: Id, anchor: Pos2, url: &mut String, first: bool) -> Option<LinkEvent> {
    let mut ev = None;
    let fid = id.with("link-url");
    Area::new(id.with("link"))
        .order(Order::Foreground)
        .fade_in(false)
        .pivot(Align2::LEFT_TOP)
        .fixed_pos(anchor + vec2(0.0, 6.0))
        .show(ctx, |ui| {
            popover(t).inner_margin(Margin { left: 10, right: 6, top: 6, bottom: 6 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    parts::icon(ui.painter(), ui.cursor().min + vec2(7.0, 14.0), Icon::Link, 14.0, t.subtle);
                    ui.add_space(20.0);
                    let hint = egui::RichText::new("Paste or type a link").font(font(W::Regular, 13.0)).color(t.muted);
                    let resp = ui.add(
                        TextEdit::singleline(url)
                            .id(fid)
                            .frame(Frame::NONE)
                            .margin(Margin::symmetric(0, 6))
                            .desired_width(260.0)
                            .font(font(W::Regular, 13.0))
                            .text_color(t.text)
                            .hint_text(hint),
                    );
                    if first {
                        resp.request_focus();
                    }
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    let esc = ui.input(|i| i.key_pressed(Key::Escape));
                    if esc {
                        ev = Some(LinkEvent::Cancel);
                    } else if resp.lost_focus() && enter {
                        let u = url.trim().to_string();
                        ev = Some(if u.is_empty() { LinkEvent::Cancel } else { LinkEvent::Apply(u) });
                    } else if !first && !resp.has_focus() {
                        ev = Some(LinkEvent::Cancel);
                    }
                    let k = parts::kbd_size(ui.painter(), "Enter");
                    let (r, _) = ui.allocate_exact_size(k + vec2(4.0, 0.0), Sense::hover());
                    parts::kbd(ui.painter(), t, r.left_center(), "Enter");
                });
            });
        });
    ev
}
