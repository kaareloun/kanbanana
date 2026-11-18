//! Read-only markdown body with the rich editor's layout and overlay, so comments look
//! the same as the description and never show the syntax. Tables get real cells.

use std::collections::HashMap;
use std::ops::Range;

use eframe::egui::{CursorIcon, FontId, Id, Sense, Shape, Ui, vec2};

use crate::rich::commands::{char_to_byte, toggle_check};
use crate::rich::{self, Look, layout, overlay, parse, table};
use crate::theme::Theme;

#[derive(Default)]
pub struct View {
    /// A link the user clicked.
    pub open_url: Option<String>,
    /// Byte offset of a click on the text. Only when the view senses clicks.
    pub clicked_at: Option<usize>,
    /// The text with a checklist item toggled. Only when the view senses clicks.
    pub toggled: Option<String>,
}

/// Paints `text` as wide as the ui. `click` makes the text a click target, for a
/// comment that opens its editor on click.
pub fn show(ui: &mut Ui, t: &Theme, id: Id, text: &str, look: Look, click: bool) -> View {
    let ctx = ui.ctx().clone();
    let lines = parse::parse(text, true);
    let mut pics = HashMap::new();
    for l in &lines {
        if let Some(img) = &l.image {
            pics.entry(img.url.clone()).or_insert_with(|| overlay::pic(&ctx, img));
        }
    }
    let width = ui.available_width();
    let sense = if click { Sense::click() } else { Sense::hover() };
    let mut view = View::default();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (k, (part, is_table)) in parts(&lines).into_iter().enumerate() {
            if is_table {
                let r = table::show(ui, t, id.with(("table", k)), text, &lines, part, &look, sense);
                view.open_url = view.open_url.take().or(r.open_url);
                view.clicked_at = view.clicked_at.or(r.clicked_at);
                continue;
            }
            let job = {
                let measure = |s: &str, f: &FontId| rich::measure(ui, s, f);
                let image_h = |img: &parse::Image| overlay::image_line_h(&pics, img, width);
                layout::job_lines(text, &lines, part.clone(), t, &look, width, &image_h, &measure)
            };
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            let bg = ui.painter().add(Shape::Noop);
            // An explicit id, so two comments never share an auto id.
            let (rect, _) = ui.allocate_exact_size(vec2(width, galley.size().y), Sense::hover());
            let resp = ui.interact(rect, id.with(("part", k)), sense);
            let origin = rect.min - vec2(galley.rect.left(), 0.0);
            ui.painter().galley(origin, galley.clone(), t.text);
            let base = lines[part.start].range.start;
            let c = overlay::Ctx {
                t,
                look: &look,
                id: id.with(k),
                text,
                base,
                lines: &lines[part.clone()],
                galley: &galley,
                origin,
                width,
                pics: &pics,
                uploads: &[],
                links_live: true,
            };
            let clicks = overlay::paint(ui, &c, bg);
            view.open_url = view.open_url.take().or(clicks.open_url);
            if !click {
                continue;
            }
            if let Some(li) = clicks.toggle {
                view.toggled = toggle_check(text, &lines[part.start + li]);
            }
            let resp = resp.on_hover_cursor(CursorIcon::Text);
            if resp.clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let at = galley.cursor_from_pos(p - origin).index.0;
                view.clicked_at = Some(base + char_to_byte(&text[base..], at));
            }
        }
    });
    view
}

/// Line index ranges to lay out one by one: runs of text lines, and each table alone.
fn parts(lines: &[parse::Line]) -> Vec<(Range<usize>, bool)> {
    let mut out = Vec::new();
    let mut at = 0;
    for g in parse::tables(lines) {
        if at < g.start {
            out.push((at..g.start, false));
        }
        at = g.end;
        out.push((g, true));
    }
    if at < lines.len() {
        out.push((at..lines.len(), false));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_their_own_parts() {
        let lines = parse::parse("a\n\n| x |\n| - |\n| 1 |\n\nb", true);
        assert_eq!(parts(&lines), vec![(0..2, false), (2..5, true), (5..7, false)]);
        assert_eq!(parts(&parse::parse("| x |\n| - |", true)), vec![(0..2, true)]);
    }
}
