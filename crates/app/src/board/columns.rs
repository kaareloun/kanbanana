//! Column headers, the folded Backlog strip and the wells. Each well scrolls on its own.
//! Drag and drop runs here too.

use std::collections::{HashMap, HashSet};

use eframe::egui::{
    Align2, Color32, Context, CornerRadius, CursorIcon, Direction, Id, Painter, Pos2, Rect, ScrollArea,
    Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder,
    containers::scroll_area::{DragScroll, ScrollSource},
    epaint::TextShape,
    pos2, style::ScrollStyle, vec2,
};
use model::{BoardData, IssueChange};

use super::card::{self, Fx, Resolver};
use super::layout::{self, Column};
use super::{BoardProps, BoardView, Drag, Settle, drag};
use crate::actions::Action;
use crate::theme::motion::{self, SNAP, lerp};
use crate::theme::parts::{self, Icon, StatusKind};
use crate::theme::{Theme, W, font};

const PAD: f32 = 24.0;
const GAP: f32 = 16.0;
const STRIP_W: f32 = 44.0;
const HEAD_H: f32 = 44.0;
const HEAD_GAP: f32 = 16.0;
const BOTTOM: f32 = 20.0;
const WELL_PAD: f32 = 10.0;
const WELL_R: u8 = 14;
const CARD_GAP: f32 = 10.0;
const MIN_COL_W: f32 = 260.0;
/// Widest column: a 324 px card plus the well padding.
const MAX_COL_W: f32 = 344.0;
const FADE_H: f32 = 64.0;
const DRAG_ANGLE_DEG: f32 = 2.0;
const DRAG_SCALE: f32 = 1.02;

/// What the board needs from the frame, shared by all wells.
struct Env<'a> {
    t: &'a Theme,
    data: &'a BoardData,
    res: Resolver<'a>,
    cols: &'a [Column],
    selected: Option<&'a str>,
    menu: Option<String>,
    /// Scroll ids stay apart per workspace and tab.
    salt: String,
}

/// The dragged card, seen from the column it left.
#[derive(Clone, Copy)]
struct Src {
    issue: usize,
    col: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Card(usize),
    Placeholder,
    Drop,
}

/// Rects the board collects while it paints, for clicks that egui does not route to it.
#[derive(Default)]
struct Hits {
    /// Visible card rects with the issue id.
    cards: Vec<(Rect, String)>,
    /// The Backlog strip and the Backlog header. They have their own click.
    own: Vec<Rect>,
}

pub fn show(ui: &mut Ui, t: &Theme, props: &BoardProps, tab: &str, cols: &[Column], view: &mut BoardView, actions: &mut Vec<Action>) {
    let data = props.data;
    let area = ui.max_rect();
    if cols.is_empty() {
        let msg = if data.states.is_empty() { "Nothing synced yet." } else { "No issues here." };
        parts::text(ui.painter(), area.center(), Align2::CENTER_CENTER, msg, W::Medium, 13.0, t.muted);
        view.cards.clear();
        return;
    }
    let env = Env {
        t,
        data,
        res: Resolver::new(data, t),
        cols,
        selected: props.selected,
        menu: view.menu.as_ref().map(|m| m.issue_id.clone()),
        salt: format!("{}/{tab}", props.workspace.slug),
    };
    let ctx = ui.ctx().clone();
    let menu_open = view.menu.is_some();
    track_moves(&ctx, &env, view);

    let backlog: Vec<usize> = (0..cols.len()).filter(|&i| cols[i].kind == StatusKind::Backlog).collect();
    let open = if backlog.is_empty() { 1.0 } else { motion::toggle(&ctx, Id::unique("lb-backlog-open"), view.backlog_open) };
    let as_strip = !backlog.is_empty() && open < 0.5;
    let (geo, content_w) = geometry(area.width(), cols, &backlog, open);

    // A drag ends when its issue left the tab, e.g. after a filter change.
    let src = view.drag.as_ref().and_then(|d| {
        let issue = data.issues.iter().position(|i| i.id == d.issue_id)?;
        let col = cols.iter().position(|c| c.cards.contains(&issue))?;
        (!(as_strip && backlog.contains(&col))).then_some(Src { issue, col })
    });
    if src.is_none() {
        view.drag = None;
    }
    let pick = motion::toggle(&ctx, Id::unique("lb-drag-pickup"), view.drag.is_some());
    let pointer = view.drag.as_ref().and_then(|d| d.demo_pointer).or_else(|| ctx.pointer_interact_pos());
    let mut hits = Hits::default();

    ui.scope(|ui| {
        ui.spacing_mut().scroll.fade.strength = 0.0;
        ScrollArea::horizontal()
            .id_salt("lb-board-x")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let o = ui.cursor().min;
                let h = area.height();
                ui.allocate_rect(Rect::from_min_size(o, vec2(content_w, h)), Sense::hover());
                let well_y = o.y + HEAD_H + HEAD_GAP..=o.y + h - BOTTOM;
                let well = |ci: usize| {
                    let (x, w) = geo[ci];
                    Rect::from_x_y_ranges(o.x + x..=o.x + x + w, well_y.clone())
                };

                // Slots left to right: the strip (None) and the shown columns.
                let mut slots: Vec<(Option<usize>, Rect)> = Vec::new();
                for ci in 0..cols.len() {
                    if !as_strip || !backlog.contains(&ci) {
                        slots.push((Some(ci), well(ci)));
                    } else if backlog.first() == Some(&ci) {
                        slots.push((None, well(ci)));
                    }
                }
                let first_backlog = backlog.first().copied();
                let target = match (src, pointer) {
                    (Some(s), Some(pt)) if pt.y >= o.y && pt.y <= o.y + h => slots
                        .iter()
                        .find(|(_, r)| r.x_range().expand(GAP / 2.0).contains(pt.x))
                        .and_then(|(ci, _)| ci.or(first_backlog))
                        .filter(|&ci| target_state(data, s.issue, &cols[ci]).is_some()),
                    _ => None,
                };

                let mut drop = None;
                for &(ci, rect) in &slots {
                    match ci {
                        None => {
                            let hit = target.is_some() && target == first_backlog;
                            hits.own.push(rect);
                            if strip_well(ui, t, rect, cols, hit) {
                                view.backlog_open = true;
                            }
                            if hit && let (Some(s), Some(b)) = (src, first_backlog) {
                                drop = drop_change(data, &cols[b], s.issue);
                            }
                        }
                        Some(ci) => {
                            let head = Rect::from_min_size(pos2(rect.min.x, o.y), vec2(rect.width(), HEAD_H));
                            if cols[ci].kind == StatusKind::Backlog {
                                hits.own.push(head);
                            }
                            if header(ui, t, head, &cols[ci]) {
                                view.backlog_open = false;
                            }
                            let hit = target == Some(ci);
                            if hit && let Some(s) = src {
                                drop = drop_change(data, &cols[ci], s.issue);
                            }
                            let pt = hit.then_some(pointer).flatten();
                            well_column(ui, &env, rect, ci, src, pt, view, actions, &mut hits);
                        }
                    }
                }

                // The wells take every scroll target, so the board scrolls to the focused
                // well after they drew.
                if let Some(r) = view.focus_well.take() {
                    ui.scroll_to_rect(r, None);
                }
                if let (Some(d), Some(s), Some(pt)) = (view.drag.clone(), src, pointer) {
                    let card_w = geo[s.col].1 - 2.0 * WELL_PAD;
                    finish_drag(&ctx, &env, d, s, pt, card_w, pick, drop, view, actions);
                }
            });
    });

    panel_click(&ctx, area, props.selected.is_some() && !menu_open && !props.blocked, &hits, actions);
    view.cards = hits.cards;
    if view.settle.as_ref().is_some_and(|s| motion::now(&ctx) - s.start >= f64::from(SNAP)) {
        view.settle = None;
    }
}

/// Column x and width inside the board, and the board width. `open` runs from 0 (Backlog folded
/// into the strip) to 1 (Backlog as a full column), so the columns slide while it changes.
fn geometry(width: f32, cols: &[Column], backlog: &[usize], open: f32) -> (Vec<(f32, f32)>, f32) {
    let n = cols.len() as f32;
    let open_w = ((width - 2.0 * PAD - (n - 1.0) * GAP) / n).clamp(MIN_COL_W, MAX_COL_W).floor();
    let wide: Vec<(f32, f32)> = (0..cols.len()).map(|i| (PAD + i as f32 * (open_w + GAP), open_w)).collect();
    let wide_total = 2.0 * PAD + n * open_w + (n - 1.0) * GAP;
    if backlog.is_empty() || open >= 1.0 {
        return (wide, wide_total);
    }
    let rest = cols.len() - backlog.len();
    let m = rest.max(1) as f32;
    let fold_w = ((width - 2.0 * PAD - STRIP_W - GAP - (m - 1.0) * GAP) / m).clamp(MIN_COL_W, MAX_COL_W).floor();
    let mut j = 0.0;
    let folded: Vec<(f32, f32)> = (0..cols.len())
        .map(|i| {
            if backlog.first() == Some(&i) {
                (PAD, STRIP_W)
            } else if backlog.contains(&i) {
                (PAD + STRIP_W, 0.0)
            } else {
                j += 1.0;
                (PAD + STRIP_W + GAP + (j - 1.0) * (fold_w + GAP), fold_w)
            }
        })
        .collect();
    let fold_total = if rest == 0 { 2.0 * PAD + STRIP_W } else { 2.0 * PAD + STRIP_W + GAP + m * fold_w + (m - 1.0) * GAP };
    let geo = folded
        .iter()
        .zip(&wide)
        .map(|(f, w)| (lerp(f.0, w.0, open), lerp(f.1, w.1, open)))
        .collect();
    (geo, lerp(fold_total, wide_total, open))
}

/// Starts a short fade for cards that were not in their column last frame, e.g. after a sync.
fn track_moves(ctx: &Context, env: &Env, view: &mut BoardView) {
    let now = motion::now(ctx);
    let settling = view.settle.as_ref().map(|s| s.issue_id.clone());
    let mut seen = HashMap::with_capacity(env.cols.len());
    for c in env.cols {
        let key = format!("{}/{}", env.salt, c.name);
        let ids: HashSet<String> = c.cards.iter().map(|&i| env.data.issues[i].id.clone()).collect();
        if let Some(prev) = view.seen.get(&key) {
            for id in ids.difference(prev).filter(|id| Some(*id) != settling.as_ref()) {
                view.fades.insert(id.clone(), now);
            }
        }
        seen.insert(key, ids);
    }
    view.seen = seen;
    view.fades.retain(|_, start| now - *start < f64::from(SNAP));
}

/// With the issue panel open, the panel's scrim covers the board. A click on a card there opens
/// that card at once. A click on empty board space closes the panel. The panel itself keeps its clicks.
fn panel_click(ctx: &Context, area: Rect, panel_open: bool, hits: &Hits, actions: &mut Vec<Action>) {
    if !panel_open || actions.iter().any(|a| matches!(a, Action::SelectIssue(_))) {
        return;
    }
    let click = ctx.input(|i| if i.pointer.primary_clicked() { i.pointer.interact_pos() } else { None });
    let Some(pt) = click else { return };
    let panel_left = ctx.content_rect().right() - crate::issue_panel::WIDTH;
    if pt.x >= panel_left || !area.contains(pt) || hits.own.iter().any(|r| r.contains(pt)) {
        return;
    }
    let card = hits.cards.iter().find(|(r, _)| r.contains(pt)).map(|(_, id)| id.clone());
    actions.push(Action::SelectIssue(card));
}

/// Paints the floating card. On release it applies the drop, ends the drag and lets the card settle.
#[allow(clippy::too_many_arguments)]
fn finish_drag(
    ctx: &Context,
    env: &Env,
    d: Drag,
    s: Src,
    pt: Pos2,
    card_w: f32,
    pick: f32,
    drop: Option<IssueChange>,
    view: &mut BoardView,
    actions: &mut Vec<Action>,
) {
    let rect = Rect::from_min_size(pt - d.grab, vec2(card_w, card::HEIGHT));
    let cd = env.res.card(&env.data.issues[s.issue]);
    let angle = DRAG_ANGLE_DEG.to_radians() * pick;
    let scale = lerp(1.0, DRAG_SCALE, pick);
    drag::paint_floating(ctx, env.t, rect, &cd, angle, scale, pick, Fx::FLOATING);
    if d.demo_pointer.is_some() {
        return;
    }
    ctx.set_cursor_icon(CursorIcon::Grabbing);
    if ctx.input(|i| !i.pointer.primary_down()) {
        if let Some(change) = drop {
            actions.push(Action::ChangeIssue { issue_id: d.issue_id.clone(), change });
        }
        view.settle = Some(Settle { issue_id: d.issue_id.clone(), from: rect, angle, scale, start: motion::now(ctx) });
        view.drag = None;
    }
}

/// Status icon, name and count over a 3 px rule in the state color.
/// Returns true when the header of an open Backlog column is clicked.
fn header(ui: &mut Ui, t: &Theme, rect: Rect, col: &Column) -> bool {
    let p = ui.painter();
    let rule = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 3.0), rect.max);
    p.rect_filled(rule, CornerRadius::same(2), rule_color(t, col.kind));
    let mid = rule.min.y - 10.0 - 10.0;
    parts::status_icon(p, t, pos2(rect.min.x + 6.0 + 8.0, mid), col.kind, 16.0);
    let r = parts::text(p, pos2(rect.min.x + 31.0, mid), Align2::LEFT_CENTER, &col.name, W::SemiBold, 14.0, t.text);
    let n = col.cards.len().to_string();
    parts::text(p, pos2(r.max.x + 9.0, mid + 0.5), Align2::LEFT_CENTER, &n, W::MonoMedium, 12.0, t.muted);
    if col.kind != StatusKind::Backlog {
        return false;
    }
    let resp = ui
        .interact(rect, Id::unique(("lb-col-head", &col.name)), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Fold Backlog");
    resp.clicked()
}

fn rule_color(t: &Theme, kind: StatusKind) -> Color32 {
    match kind {
        StatusKind::Backlog | StatusKind::Canceled => t.muted,
        StatusKind::Todo => t.subtle,
        StatusKind::Progress => t.gold,
        StatusKind::Review => t.foam,
        StatusKind::Done => t.iris,
    }
}

/// The folded Backlog: icon, count, the word "Backlog" turned 90 degrees and a chevron.
/// Returns true when clicked.
fn strip_well(ui: &mut Ui, t: &Theme, rect: Rect, cols: &[Column], drop_here: bool) -> bool {
    let resp = ui
        .interact(rect, Id::unique("lb-backlog-strip"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Show Backlog");
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(WELL_R), t.surface);
    if drop_here {
        p.rect_filled(rect, CornerRadius::same(WELL_R), Theme::tint(t.accent_color, 0.08));
        p.rect_stroke(rect, CornerRadius::same(WELL_R), Stroke::new(1.5, t.accent_color), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_stroke(rect, CornerRadius::same(WELL_R), Stroke::new(1.0, t.hl_med), StrokeKind::Inside);
    }
    let backlog: Vec<&Column> = cols.iter().filter(|c| c.kind == StatusKind::Backlog).collect();
    let count: usize = backlog.iter().map(|c| c.cards.len()).sum();
    let name = if backlog.len() == 1 { backlog[0].name.as_str() } else { "Backlog" };
    let cx = rect.center().x;
    let mut y = rect.min.y + 14.0;
    parts::status_icon(p, t, pos2(cx, y + 7.5), StatusKind::Backlog, 15.0);
    y += 15.0 + 8.0;
    let r = parts::text(p, pos2(cx, y), Align2::CENTER_TOP, &count.to_string(), W::MonoMedium, 12.0, t.muted);
    y = r.max.y + 14.0;
    // The word reads top to bottom: turned 90 degrees clockwise around its top left corner.
    let g = p.layout_no_wrap(name.to_string(), font(W::SemiBold, 12.0), t.subtle);
    let at = pos2(cx + g.size().y / 2.0, y);
    p.add(Shape::Text(TextShape::new(at, g, t.subtle).with_angle(std::f32::consts::FRAC_PI_2)));
    let c = if resp.hovered() { t.text } else { t.muted };
    parts::icon(p, pos2(cx, rect.max.y - 14.0 - 7.0), Icon::ChevronRight, 14.0, c);
    resp.clicked()
}

/// One column well with its own scroll area. `pointer` is set when the drag targets this column.
#[allow(clippy::too_many_arguments)]
fn well_column(
    ui: &mut Ui,
    env: &Env,
    rect: Rect,
    ci: usize,
    src: Option<Src>,
    pointer: Option<Pos2>,
    view: &mut BoardView,
    actions: &mut Vec<Action>,
    hits: &mut Hits,
) {
    let t = env.t;
    let col = &env.cols[ci];
    let radius = CornerRadius::same(WELL_R);
    ui.painter().rect_filled(rect, radius, t.surface);
    if pointer.is_some() {
        ui.painter().rect_filled(rect, radius, Theme::tint(t.accent_color, 0.05));
    }
    let card_w = rect.width() - 2.0 * WELL_PAD;
    if card_w < 40.0 {
        return;
    }
    let src_here = src.filter(|s| s.col == ci);

    let out = ui.scope_builder(UiBuilder::new().id_salt(("lb-well", &env.salt, &col.name)).max_rect(rect), |ui| {
        well_style(ui, t);
        ScrollArea::vertical()
            .id_salt("scroll")
            .auto_shrink([false, false])
            .scroll_source(ScrollSource { scroll_bar: true, drag: DragScroll::Never, mouse_wheel: true })
            .show(ui, |ui| {
                let top = ui.cursor().min.y + WELL_PAD;
                let x = rect.min.x + WELL_PAD;
                let p = ui.painter().clone();
                // The slot shows where the card lands: its place by priority, not the pointer.
                let k = pointer.zip(src).map(|(_, s)| layout::landing(env.data, col, s.issue));
                let placed = place(col, src_here.or(k.and(src)), k, top);
                let mut alive = false;
                let mut bottom = top;
                let clip = ui.clip_rect();
                for (what, y) in placed {
                    let r = Rect::from_min_size(pos2(x, y), vec2(card_w, card::HEIGHT));
                    bottom = r.max.y;
                    match what {
                        Slot::Card(i) => {
                            let seen = r.intersect(clip);
                            if seen.is_positive() {
                                hits.cards.push((seen, env.data.issues[i].id.clone()));
                            }
                            card_widget(ui, env, r, i, view, actions);
                            if view.focus_scroll && view.focus.as_deref() == Some(env.data.issues[i].id.as_str()) {
                                ui.scroll_to_rect(r.expand(WELL_PAD), None);
                                view.focus_scroll = false;
                                view.focus_well = Some(rect);
                            }
                        }
                        Slot::Placeholder | Slot::Drop => {
                            if what == Slot::Placeholder {
                                card::paint_placeholder(&p, t, r);
                            } else {
                                card::paint_drop_slot(&p, t, r);
                            }
                            // The dragged card keeps one widget, so egui keeps the drag alive.
                            if let (Some(s), false) = (src_here, alive) {
                                ui.interact(r, card_id(&env.data.issues[s.issue].id), Sense::click_and_drag());
                                alive = true;
                            }
                        }
                    }
                }
                ui.allocate_rect(Rect::from_min_max(pos2(x, top - WELL_PAD), pos2(x + card_w, bottom + WELL_PAD)), Sense::hover());
                // Scroll the column while a dragged card hangs near its top or bottom edge.
                let live = view.drag.as_ref().is_some_and(|d| d.demo_pointer.is_none());
                if let Some(pt) = pointer.filter(|_| live) {
                    let step = if pt.y > rect.max.y - 48.0 { -8.0 } else if pt.y < rect.min.y + 48.0 { 8.0 } else { 0.0 };
                    if step != 0.0 {
                        ui.scroll_with_delta(vec2(0.0, step));
                        ui.ctx().request_repaint();
                    }
                }
            })
    });
    let out = out.inner;

    let hidden = out.content_size.y - out.inner_rect.height() - out.state.clamped_offset().y;
    if hidden > 1.0 {
        paint_fade(ui.painter(), t, rect);
    }
    // A drop slot replaces the empty state while a card hangs over the column.
    let empty = motion::toggle(ui.ctx(), Id::unique(("lb-well-empty", &env.salt, &col.name)), col.cards.is_empty() && pointer.is_none());
    if empty > 0.0 {
        paint_empty(ui.painter(), t, rect, empty);
    }
}

/// A faded banana and one muted line in the middle of an empty well. Paint only, so drops still land.
fn paint_empty(p: &Painter, t: &Theme, rect: Rect, alpha: f32) {
    let c = rect.center() - vec2(0.0, 14.0);
    crate::brand::banana(p, Rect::from_center_size(c, vec2(44.0, 44.0)), 0.0, 0.7 * alpha);
    let at = pos2(c.x, c.y + 26.0);
    parts::text(p, at, Align2::CENTER_TOP, "Nothing here. Have a banana.", W::Medium, 12.0, t.muted.gamma_multiply(alpha));
}

fn well_style(ui: &mut Ui, t: &Theme) {
    let mut s = ScrollStyle::thin();
    s.bar_width = 6.0;
    s.floating_width = 4.0;
    s.floating_allocated_width = 0.0;
    s.bar_outer_margin = 3.0;
    s.bar_inner_margin = 0.0;
    s.dormant_background_opacity = 0.0;
    s.active_background_opacity = 0.0;
    s.interact_background_opacity = 0.0;
    s.dormant_handle_opacity = 0.7;
    s.fade.strength = 0.0;
    ui.spacing_mut().scroll = s;
    let w = &mut ui.visuals_mut().widgets;
    w.inactive.bg_fill = t.hl_high;
    w.hovered.bg_fill = t.hl_high;
    w.active.bg_fill = t.muted;
}

/// Fades the cards into the well color at the bottom, to show more content below.
fn paint_fade(p: &Painter, t: &Theme, rect: Rect) {
    let r = f32::from(WELL_R);
    let grad = Rect::from_min_max(pos2(rect.min.x, rect.max.y - FADE_H), pos2(rect.max.x, rect.max.y - r));
    p.add(Shape::gradient_rect(grad, Direction::TopDown, [Color32::TRANSPARENT, t.surface]));
    let foot = Rect::from_min_max(pos2(rect.min.x, rect.max.y - r), rect.max);
    let round = CornerRadius { nw: 0, ne: 0, sw: WELL_R, se: WELL_R };
    p.rect_filled(foot, round, t.surface);
}

fn is_marked(env: &Env, i: usize) -> bool {
    let id = env.data.issues[i].id.as_str();
    env.selected == Some(id) || env.menu.as_deref() == Some(id)
}

/// The focus ring hides on the open issue, since its selection ring marks it already.
fn is_focused(env: &Env, view: &BoardView, i: usize) -> bool {
    let id = env.data.issues[i].id.as_str();
    view.focus.as_deref() == Some(id) && env.selected != Some(id)
}

fn card_id(issue_id: &str) -> Id {
    Id::unique(("lb-card", issue_id))
}

/// Paints one card and handles click, right click, hover and the start of a drag.
fn card_widget(ui: &mut Ui, env: &Env, rect: Rect, i: usize, view: &mut BoardView, actions: &mut Vec<Action>) {
    let issue = &env.data.issues[i];
    let resp = ui.interact(rect, card_id(&issue.id), Sense::click_and_drag());
    let free = view.drag.is_none();
    let ctx = ui.ctx().clone();
    let hovered = resp.hovered() && free;
    let fx = Fx {
        hover: motion::toggle(&ctx, Id::unique(("lb-card-hover", &issue.id)), hovered),
        select: motion::toggle(&ctx, Id::unique(("lb-card-select", &issue.id)), is_marked(env, i)),
        focus: motion::toggle(&ctx, Id::unique(("lb-card-focus", &issue.id)), is_focused(env, view, i)),
        floating: false,
    };
    let cd = env.res.card(issue);
    let settle = view.settle.as_ref().filter(|s| s.issue_id == issue.id);
    match settle.map(|s| (s, motion::since(&ctx, s.start))) {
        Some((s, p)) if p < 1.0 => {
            let r = Rect::from_min_max(s.from.min.lerp(rect.min, p), s.from.max.lerp(rect.max, p));
            let fx = Fx { select: lerp(1.0, fx.select, p), ..Fx::FLOATING };
            drag::paint_floating(&ctx, env.t, r, &cd, s.angle * (1.0 - p), lerp(s.scale, 1.0, p), 1.0 - p, fx);
        }
        _ => {
            let mut p = ui.painter().clone();
            if let Some(start) = view.fades.get(&issue.id) {
                p.multiply_opacity(motion::since(&ctx, *start));
            }
            card::paint(&p, env.t, rect, &cd, fx);
            if hovered && view.menu.is_none() {
                resp.clone().on_hover_ui(|ui| card::tooltip(ui, env.t, &cd));
            }
        }
    }
    if hovered {
        view.hovered = Some(issue.id.clone());
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() {
        actions.push(Action::SelectIssue(Some(issue.id.clone())));
    }
    if resp.secondary_clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(rect.center());
        view.menu = Some(super::Menu::new(issue.id.clone(), at));
    }
    if resp.drag_started() && free {
        let at = resp.interact_pointer_pos().unwrap_or(rect.min);
        view.drag = Some(Drag { issue_id: issue.id.clone(), grab: at - rect.min, demo_pointer: None });
        view.menu = None;
        view.settle = None;
    }
}

/// Places the cards of one column from `top` down. The dragged card shows as a placeholder.
/// With `k` set, a drop slot goes in before the k-th card that is not the dragged one.
fn place(col: &Column, src: Option<Src>, k: Option<usize>, top: f32) -> Vec<(Slot, f32)> {
    let mut out = Vec::with_capacity(col.cards.len() + 1);
    let mut y = top;
    let mut push = |what: Slot, y: &mut f32| {
        out.push((what, *y));
        *y += card::HEIGHT + CARD_GAP;
    };
    let mut n = 0;
    let mut slot_done = false;
    for &i in &col.cards {
        if src.is_some_and(|s| s.issue == i) {
            if k == Some(n) && !slot_done {
                push(Slot::Drop, &mut y);
                slot_done = true;
            } else {
                push(Slot::Placeholder, &mut y);
            }
            continue;
        }
        if src.is_some() && k == Some(n) && !slot_done {
            push(Slot::Drop, &mut y);
            slot_done = true;
        }
        push(Slot::Card(i), &mut y);
        n += 1;
    }
    if src.is_some() && k == Some(n) && !slot_done {
        push(Slot::Drop, &mut y);
    }
    out
}

/// The state with the column's name in the issue's own team.
fn target_state<'a>(data: &'a BoardData, issue: usize, col: &Column) -> Option<&'a str> {
    let team = &data.issues[issue].team_id;
    data.states
        .iter()
        .find(|s| &s.team_id == team && s.name == col.name)
        .map(|s| s.id.as_str())
}

/// The change a drop on `col` makes: a new state. The card keeps its sort order, since
/// priority sets its place. None when the card stays in its own column.
fn drop_change(data: &BoardData, col: &Column, issue: usize) -> Option<IssueChange> {
    let state_id = target_state(data, issue, col)?;
    if state_id == data.issues[issue].state_id {
        return None;
    }
    Some(IssueChange::State { state_id: state_id.to_string(), sort_order: None })
}
