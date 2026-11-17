//! Header row: status, identifier and team, copy buttons, open in Linear, close.

use eframe::egui::{Align, Layout, Margin, Sense, Ui, vec2};
use model::{BoardData, Issue};

use super::PanelAction;
use super::icons::Ic;
use super::widgets;
use crate::actions;
use crate::board::layout::status_kind;
use crate::theme::parts::{self, CMD, StatusKind};
use crate::theme::{Theme, W};

pub fn show(ui: &mut Ui, t: &Theme, issue: &Issue, data: &BoardData, out: &mut Vec<PanelAction>) {
    let team = data.teams.iter().find(|x| x.id == issue.team_id).map(|x| x.name.as_str());
    let kind = data
        .states
        .iter()
        .find(|s| s.id == issue.state_id)
        .map_or(StatusKind::Todo, |s| status_kind(s.kind, &s.name));
    let h = ui.max_rect().height();
    eframe::egui::Frame::NONE
        .inner_margin(Margin { left: 36, right: 18, top: 0, bottom: 0 })
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(vec2(ui.available_width(), h), Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                parts::status_icon(ui.painter(), t, r.center(), kind, 14.0);
                ui.add_space(2.0);
                widgets::text(ui, &issue.identifier, W::MonoMedium, 12.0, t.subtle);
                if let Some(team) = team {
                    widgets::text(ui, "in", W::Medium, 12.0, t.muted);
                    widgets::text(ui, team, W::Medium, 12.0, t.subtle);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, t, Ic::X, t.muted).on_hover_text("Close  Esc").clicked() {
                        out.push(PanelAction::Close);
                    }
                    if let Some(url) = &issue.url
                        && widgets::icon_button(ui, t, Ic::External, t.muted).on_hover_text("Open in Linear").clicked()
                    {
                        out.push(PanelAction::OpenUrl(url.clone()));
                    }
                    ui.add_space(2.0);
                    let tip = format!("Copy a prompt for Claude Code  {CMD} Shift C");
                    if widgets::head_button(ui, t, Ic::Asterisk, "Prompt").on_hover_text(tip).clicked() {
                        out.push(PanelAction::CopyText(actions::copy_prompt(issue)));
                    }
                    let tip = format!("Copy ID and title  {CMD} C");
                    if widgets::head_button(ui, t, Ic::Copy, "ID and title").on_hover_text(tip).clicked() {
                        out.push(PanelAction::CopyText(actions::copy_title(issue)));
                    }
                });
            });
        });
}
