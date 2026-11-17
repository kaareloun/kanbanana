//! Development row: the stage and branch that Claude reported over MCP.

use eframe::egui::{Align2, CornerRadius, CursorIcon, Rect, Sense, Ui, pos2, vec2};
use model::{BoardData, Issue};

use super::icons::{self, Ic};
use super::{PanelAction, id, props, time};
use crate::theme::parts;
use crate::theme::{Theme, W};

/// The row hides when Claude reported nothing for this issue.
pub fn row(ui: &mut Ui, t: &Theme, issue: &Issue, data: &BoardData, label_w: f32, out: &mut Vec<PanelAction>) {
    let Some(d) = data
        .dev
        .iter()
        .find(|d| d.identifier == issue.identifier && d.workspace == issue.workspace)
    else {
        return;
    };
    let r = props::row(ui, t, "Development");
    let p = ui.painter().clone();
    let c = t.stage(d.stage);
    let tc = t.tag_text(c);
    let git_ref = d.git_ref.as_deref().or(d.url.as_deref()).unwrap_or("");
    let mr = git_ref.starts_with('!') || git_ref.starts_with('#');
    let stage = d.stage.label();
    let stage_w = parts::text_size(&p, stage, W::SemiBold, 11.0).x;
    let ref_w = parts::text_size(&p, git_ref, W::SemiBold, 11.0).x;
    let w = 8.0 + 12.0 + 5.0 + stage_w + if git_ref.is_empty() { 0.0 } else { 6.0 + ref_w } + 8.0;
    let tag = Rect::from_min_size(pos2(r.left() + label_w, r.center().y - 10.0), vec2(w, 20.0));

    let resp = ui.interact(tag, id("lb-panel-dev"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let fill = if resp.hovered() { 0.26 } else { 0.16 };
    p.rect_filled(tag, CornerRadius::same(5), Theme::tint(c, fill));
    let cy = tag.center().y;
    icons::paint(&p, pos2(tag.left() + 14.0, cy), if mr { Ic::Mr } else { Ic::Branch }, 12.0, tc);
    let x = tag.left() + 25.0;
    parts::text(&p, pos2(x, cy), Align2::LEFT_CENTER, stage, W::SemiBold, 11.0, tc);
    parts::text(&p, pos2(x + stage_w + 6.0, cy), Align2::LEFT_CENTER, git_ref, W::SemiBold, 11.0, tc);

    let s = format!("set by Claude, {}", time::relative(d.updated_at, super::now()));
    parts::text(&p, pos2(tag.right() + 8.0, cy), Align2::LEFT_CENTER, &s, W::Medium, 12.0, t.muted);

    // The tag opens the MR when there is a link. Else it copies the worktree path or branch.
    let copy = d.path.clone().or_else(|| d.git_ref.clone());
    let tip = match (&d.url, &d.path) {
        (Some(url), _) => url.clone(),
        (None, Some(path)) => format!("Copy {path}"),
        (None, None) => "Copy branch".into(),
    };
    if resp.on_hover_text(tip).clicked() {
        if let Some(url) = &d.url {
            out.push(PanelAction::OpenUrl(url.clone()));
        } else if let Some(copy) = copy {
            out.push(PanelAction::CopyText(copy));
        }
    }
}
