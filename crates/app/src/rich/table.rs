//! The read-only table: real cells that wrap inside the panel width, and a horizontal
//! scroll when even narrow columns do not fit. The editor aligns tables in its own
//! layout instead, with the same padding and borders.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use eframe::egui::{CornerRadius, Galley, Id, Rect, ScrollArea, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use super::Look;
use super::commands::char_to_byte;
use super::layout::{self, CELL_PAD_X, CELL_PAD_Y};
use super::overlay;
use super::parse::{self, Block, ColAlign, Line};
use crate::theme::Theme;

/// A column is never narrower than this while the table wraps its cells.
const MIN_COL: f32 = 72.0;

#[derive(Default)]
pub struct TableResponse {
    pub open_url: Option<String>,
    /// Byte offset of a click on a cell, when the table senses clicks.
    pub clicked_at: Option<usize>,
}

/// Paints the table of the lines `group` as wide as the ui allows.
#[allow(clippy::too_many_arguments)]
pub fn show(ui: &mut Ui, t: &Theme, id: Id, text: &str, lines: &[Line], group: Range<usize>, look: &Look, sense: Sense) -> TableResponse {
    let rows: Vec<&Line> = lines[group.clone()].iter().filter(|l| matches!(l.block, Block::TableRow(_))).collect();
    let aligns = lines[group]
        .iter()
        .find(|l| l.block == Block::TableSep)
        .map(|l| parse::table_align(&text[l.range.clone()]))
        .unwrap_or_default();
    let ncols = rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    if ncols == 0 {
        return TableResponse::default();
    }
    let mut natural = vec![0.0_f32; ncols];
    for r in &rows {
        for (k, w) in natural.iter_mut().enumerate().take(r.cells.len()) {
            let g = ui.fonts_mut(|f| f.layout_job(layout::cell_job(text, r, k, t, look, f32::INFINITY)));
            *w = w.max(g.size().x.ceil());
        }
    }
    let width = ui.available_width();
    let pads = ncols as f32 * 2.0 * CELL_PAD_X;
    let widths = fit(&natural, width - pads - 1.0);
    let table = Table { t, id, text, rows: &rows, aligns: &aligns, widths: &widths, look, sense };
    if widths.iter().sum::<f32>() + pads > width {
        ScrollArea::horizontal()
            .id_salt(id.with("scroll"))
            .auto_shrink([false, true])
            .show(ui, |ui| paint(ui, &table))
            .inner
    } else {
        paint(ui, &table)
    }
}

struct Table<'a> {
    t: &'a Theme,
    id: Id,
    text: &'a str,
    rows: &'a [&'a Line],
    aligns: &'a [ColAlign],
    widths: &'a [f32],
    look: &'a Look,
    sense: Sense,
}

/// Column widths for `avail`. Narrow columns keep their width and the wide ones share
/// the rest. The sum is larger than `avail` only when the minimum widths do not fit.
fn fit(natural: &[f32], avail: f32) -> Vec<f32> {
    if natural.iter().sum::<f32>() <= avail {
        return natural.to_vec();
    }
    let mut cap = avail / natural.len() as f32;
    for _ in 0..natural.len() {
        let (fixed, wide) = natural.iter().fold((0.0, 0), |(s, n), w| if *w <= cap { (s + w, n) } else { (s, n + 1) });
        if wide == 0 {
            break;
        }
        let next = (avail - fixed) / wide as f32;
        if (next - cap).abs() < 0.5 {
            break;
        }
        cap = next;
    }
    natural.iter().map(|w| w.min(cap).max(w.min(MIN_COL))).collect()
}

fn paint(ui: &mut Ui, tb: &Table) -> TableResponse {
    let t = tb.t;
    let mut bounds = vec![0.0];
    for w in tb.widths {
        bounds.push(bounds[bounds.len() - 1] + w + 2.0 * CELL_PAD_X);
    }
    let mut cells: Vec<Vec<Option<Arc<Galley>>>> = Vec::new();
    let mut heights = Vec::new();
    for r in tb.rows {
        let row: Vec<Option<Arc<Galley>>> = (0..tb.widths.len())
            .map(|k| {
                (k < r.cells.len())
                    .then(|| ui.fonts_mut(|f| f.layout_job(layout::cell_job(tb.text, r, k, t, tb.look, tb.widths[k]))))
            })
            .collect();
        let h = row.iter().flatten().map(|g| g.size().y).fold(tb.look.line, f32::max);
        heights.push(h + 2.0 * CELL_PAD_Y);
        cells.push(row);
    }
    let size = vec2(bounds[bounds.len() - 1], heights.iter().sum());
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let resp = ui.interact(rect, tb.id.with("cells"), tb.sense);
    let mut out = TableResponse::default();
    let click = resp.clicked().then(|| resp.interact_pointer_pos()).flatten();
    let no_pics = HashMap::new();
    let mut y = rect.top();
    for (ri, r) in tb.rows.iter().enumerate() {
        for (k, g) in cells[ri].iter().enumerate() {
            let Some(g) = g else { continue };
            let w = tb.widths[k];
            let dx = match tb.aligns.get(k).copied().unwrap_or(ColAlign::Left) {
                ColAlign::Left => 0.0,
                ColAlign::Center => (w - g.size().x) / 2.0,
                ColAlign::Right => w - g.size().x,
            };
            let origin = pos2(rect.left() + bounds[k] + CELL_PAD_X + dx.max(0.0), y + CELL_PAD_Y);
            let bg = ui.painter().add(Shape::Noop);
            ui.painter().galley(origin, g.clone(), t.text);
            let cell = r.cells[k].clone();
            let line = Line {
                range: cell.clone(),
                block: Block::Para,
                depth: 0,
                marker: cell.start..cell.start,
                spans: r.spans.iter().filter(|s| s.range.start >= cell.start && s.range.end <= cell.end).cloned().collect(),
                image: None,
                cells: Vec::new(),
            };
            let c = overlay::Ctx {
                t,
                look: tb.look,
                id: tb.id.with((ri, k)),
                text: tb.text,
                base: cell.start,
                lines: std::slice::from_ref(&line),
                galley: g,
                origin,
                width: w,
                pics: &no_pics,
                uploads: &[],
                links_live: true,
            };
            if let Some(url) = overlay::paint(ui, &c, bg).open_url {
                out.open_url = Some(url);
            }
            let area = Rect::from_min_max(pos2(rect.left() + bounds[k], y), pos2(rect.left() + bounds[k + 1], y + heights[ri]));
            if let Some(p) = click.filter(|p| area.contains(*p)) {
                let at = g.cursor_from_pos(p - origin).index.0;
                out.clicked_at = Some(cell.start + char_to_byte(&tb.text[cell.clone()], at));
            }
        }
        y += heights[ri];
    }

    let stroke = Stroke::new(1.0, t.hl_med);
    let p = ui.painter();
    p.rect_stroke(rect, CornerRadius::same(6), stroke, StrokeKind::Inside);
    for x in &bounds[1..bounds.len() - 1] {
        p.vline(rect.left() + x, rect.y_range(), stroke);
    }
    let mut y = rect.top();
    for h in &heights[..heights.len() - 1] {
        y += h;
        p.hline(rect.x_range(), y, stroke);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_narrow_columns() {
        assert_eq!(fit(&[50.0, 100.0], 400.0), vec![50.0, 100.0]);
        // 300 for three columns: the narrow one keeps 40, the wide ones share 260.
        assert_eq!(fit(&[40.0, 500.0, 300.0], 300.0), vec![40.0, 130.0, 130.0]);
        // Too narrow for the minimum: the table scrolls.
        let w = fit(&[200.0, 200.0, 200.0], 120.0);
        assert_eq!(w, vec![MIN_COL; 3]);
    }
}
