//! The description editor: the rich editor plus image paste, drop and upload progress.

use std::ops::Range;

use eframe::egui::Ui;

use super::{DESC_ID, PanelAction, PanelState, Upload, id, paste};
use crate::rich::{self, RichResponse};
use crate::theme::Theme;

pub const PLACEHOLDER: &str = "Add a description...";

pub fn show(
    ui: &mut Ui,
    t: &Theme,
    state: &mut PanelState,
    text: &mut String,
    request_focus: bool,
    select: Option<Range<usize>>,
    out: &mut Vec<PanelAction>,
) -> RichResponse {
    let uploads: Vec<(String, Option<f32>)> =
        state.uploads.values().filter(|u| u.description).map(|u| (u.filename.clone(), u.progress)).collect();
    let opts = rich::Options {
        look: rich::DESCRIPTION,
        placeholder: PLACEHOLDER,
        min_rows: 1,
        uploads: &uploads,
        request_focus,
        select,
    };
    let mut r = rich::editor(ui, t, id(DESC_ID), text, &opts);
    let start = out.len();
    if paste::handle(ui, &r.response, text, &mut state.next_request_id, out) {
        r.changed = true;
    }
    for a in &out[start..] {
        if let PanelAction::Upload { request_id, filename, .. } = a {
            state.uploads.insert(*request_id, Upload::new(filename, true));
        }
    }
    if let Some(url) = r.open_url.take() {
        out.push(PanelAction::OpenUrl(url));
    }
    r
}
