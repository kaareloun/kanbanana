//! Read-only markdown body with the rich editor's layout and overlay, so comments look
//! the same as the description and never show the syntax.

use std::collections::HashMap;

use eframe::egui::{Id, Sense, Shape, Ui, vec2};

use crate::rich::{Look, layout, overlay, parse};
use crate::theme::Theme;

/// Paints `text` as wide as the ui. Returns a link the user clicked.
pub fn show(ui: &mut Ui, t: &Theme, id: Id, text: &str, look: Look) -> Option<String> {
    let ctx = ui.ctx().clone();
    let lines = parse::parse(text, look.headings);
    let mut pics = HashMap::new();
    for l in &lines {
        if let Some(img) = &l.image {
            pics.entry(img.url.clone()).or_insert_with(|| overlay::pic(&ctx, img));
        }
    }
    let width = ui.available_width();
    let job = layout::job(text, &lines, t, &look, width, &|img| overlay::image_line_h(&pics, img, width));
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let bg = ui.painter().add(Shape::Noop);
    let (rect, _) = ui.allocate_exact_size(vec2(width, galley.size().y), Sense::hover());
    let origin = rect.min - vec2(galley.rect.left(), 0.0);
    ui.painter().galley(origin, galley.clone(), t.text);
    let c = overlay::Ctx {
        t,
        look: &look,
        id,
        text,
        lines: &lines,
        galley: &galley,
        origin,
        width,
        pics: &pics,
        uploads: &[],
        links_live: true,
    };
    // Checkbox clicks are ignored: a comment changes only through its edit editor.
    overlay::paint(ui, &c, bg).open_url
}
