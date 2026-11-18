//! New issue form: a modal over the board. `C` or the top bar button opens it.
//! It fills an `IssueDraft` and pushes `Action::CreateIssue`. The engine does the rest.

mod view;

use eframe::egui::Rect;
use model::{BoardData, Issue, IssueDraft, Source, StateType};

use crate::issue_panel::Picker;

pub use view::show;

pub struct NewIssueState {
    open: bool,
    /// egui time of the open, for the fade in.
    opened_at: f64,
    pub draft: IssueDraft,
    picker: Option<Picker>,
    /// Chip rects of the last frame, by `PickerKind as usize`. The pickers open under them.
    chips: [Rect; 5],
    /// Keeps the form open after a create, for the next issue.
    pub create_more: bool,
    /// Esc on a filled form shows "Discard this issue?" first.
    confirm_discard: bool,
    error: Option<String>,
    /// The create request that waits for the engine.
    pending: Option<u64>,
    focus_title: bool,
    next_request: u64,
    /// Height of the form in the last frame, to place it before it draws.
    height: f32,
}

impl Default for NewIssueState {
    fn default() -> Self {
        NewIssueState {
            open: false,
            opened_at: f64::NEG_INFINITY,
            draft: IssueDraft::default(),
            picker: None,
            chips: [Rect::NOTHING; 5],
            create_more: false,
            confirm_discard: false,
            error: None,
            pending: None,
            focus_title: false,
            next_request: 1,
            height: 420.0,
        }
    }
}

impl NewIssueState {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens an empty form for `team`, else the first team. `project` comes from a project tab.
    pub fn open(&mut self, now: f64, data: &BoardData, team: Option<String>, project: Option<String>) {
        if self.open {
            return;
        }
        let team = team.filter(|id| data.teams.iter().any(|t| &t.id == id)).or_else(|| data.teams.first().map(|t| t.id.clone()));
        *self = NewIssueState {
            open: true,
            opened_at: now,
            create_more: self.create_more,
            next_request: self.next_request,
            height: self.height,
            focus_title: true,
            ..NewIssueState::default()
        };
        if let Some(team) = team {
            set_team(&mut self.draft, data, &team);
        }
        self.draft.project_id = project;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.picker = None;
        self.pending = None;
        self.confirm_discard = false;
    }

    /// The engine saved the request. Returns true when the form closed, false when it
    /// stays open for the next issue, None when the request is not this form's.
    pub fn on_created(&mut self, request_id: u64) -> Option<bool> {
        if !self.open || self.pending != Some(request_id) {
            return None;
        }
        self.pending = None;
        self.error = None;
        if self.create_more {
            self.draft.title.clear();
            self.draft.description.clear();
            self.focus_title = true;
            Some(false)
        } else {
            self.close();
            Some(true)
        }
    }

    pub fn on_failed(&mut self, request_id: u64, message: &str) {
        if self.pending == Some(request_id) {
            self.pending = None;
            self.error = Some(format!("Not created: {message}"));
        }
    }

    /// The form shows a typed title and the given status and labels, for `--screen new-issue`.
    pub fn demo(&mut self, data: &BoardData, team: &str, title: &str, state: &str, labels: &[&str]) {
        self.open(f64::NEG_INFINITY, data, Some(team.to_string()), None);
        self.draft.title = title.into();
        self.draft.state_id = data.states.iter().find(|s| s.team_id == team && s.name == state).map(|s| s.id.clone());
        self.draft.label_ids =
            labels.iter().filter_map(|n| data.labels.iter().find(|l| l.name == *n)).map(|l| l.id.clone()).collect();
    }

    fn has_text(&self) -> bool {
        !self.draft.title.trim().is_empty() || !self.draft.description.trim().is_empty()
    }
}

/// The state a new issue starts in: the team's first unstarted state ("Todo"), else its
/// first backlog state, else its first state.
pub fn default_state(data: &BoardData, team_id: &str) -> Option<String> {
    let mut states: Vec<_> = data.states.iter().filter(|s| s.team_id == team_id).collect();
    states.sort_by(|a, b| a.position.total_cmp(&b.position));
    [StateType::Unstarted, StateType::Backlog]
        .iter()
        .find_map(|k| states.iter().find(|s| s.kind == *k))
        .or(states.first())
        .map(|s| s.id.clone())
}

/// Moves the draft to another team. Status and team labels belong to a team, so the
/// status goes back to the team default and labels of other teams drop off.
fn set_team(draft: &mut IssueDraft, data: &BoardData, team_id: &str) {
    draft.team_id = team_id.to_string();
    let keeps = |id: &String| data.states.iter().any(|s| &s.id == id && s.team_id == team_id);
    if !draft.state_id.as_ref().is_some_and(keeps) {
        draft.state_id = default_state(data, team_id);
    }
    draft.label_ids.retain(|id| {
        data.labels.iter().any(|l| &l.id == id && l.team_id.as_ref().is_none_or(|t| t == team_id))
    });
}

/// The draft as an issue, so the issue panel pickers can list and apply its options.
fn as_issue(draft: &IssueDraft) -> Issue {
    let now = chrono::Utc::now();
    Issue {
        id: String::new(),
        source: Source::Linear,
        workspace: String::new(),
        identifier: String::new(),
        title: draft.title.clone(),
        description: String::new(),
        team_id: draft.team_id.clone(),
        state_id: draft.state_id.clone().unwrap_or_default(),
        project_id: draft.project_id.clone(),
        assignee_id: draft.assignee_id.clone(),
        priority: draft.priority,
        label_ids: draft.label_ids.clone(),
        sort_order: 0.0,
        comment_count: 0,
        url: None,
        created_at: now,
        updated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{Label, Team, WorkflowState};

    fn state(id: &str, team: &str, kind: StateType, position: f64) -> WorkflowState {
        WorkflowState { id: id.into(), team_id: team.into(), name: id.into(), kind, position, color: String::new() }
    }

    fn data() -> BoardData {
        let team = |id: &str| Team { id: id.into(), workspace: "w".into(), key: id.to_uppercase(), name: id.into() };
        let label = |id: &str, team: Option<&str>| Label {
            id: id.into(),
            workspace: "w".into(),
            name: id.into(),
            color: String::new(),
            team_id: team.map(Into::into),
        };
        BoardData {
            teams: vec![team("a"), team("b")],
            states: vec![
                state("a-backlog", "a", StateType::Backlog, 0.0),
                state("a-started", "a", StateType::Started, 2.0),
                state("a-todo", "a", StateType::Unstarted, 1.0),
                state("b-backlog", "b", StateType::Backlog, 0.0),
                state("b-done", "b", StateType::Completed, 1.0),
            ],
            labels: vec![label("all", None), label("only-a", Some("a"))],
            ..Default::default()
        }
    }

    #[test]
    fn default_state_prefers_todo_then_backlog() {
        let d = data();
        assert_eq!(default_state(&d, "a").as_deref(), Some("a-todo"));
        assert_eq!(default_state(&d, "b").as_deref(), Some("b-backlog"));
        assert_eq!(default_state(&d, "c"), None);
    }

    #[test]
    fn open_uses_the_tab_team_or_the_first_team() {
        let d = data();
        let mut s = NewIssueState::default();
        s.open(0.0, &d, Some("b".into()), None);
        assert_eq!(s.draft.team_id, "b");
        assert_eq!(s.draft.state_id.as_deref(), Some("b-backlog"));
        s.close();
        s.open(0.0, &d, Some("gone".into()), Some("p1".into()));
        assert_eq!(s.draft.team_id, "a");
        assert_eq!(s.draft.state_id.as_deref(), Some("a-todo"));
        assert_eq!(s.draft.project_id.as_deref(), Some("p1"));
    }

    #[test]
    fn team_change_resets_status_and_drops_team_labels() {
        let d = data();
        let mut draft = IssueDraft::default();
        set_team(&mut draft, &d, "a");
        draft.state_id = Some("a-started".into());
        draft.label_ids = vec!["all".into(), "only-a".into()];
        set_team(&mut draft, &d, "a");
        assert_eq!(draft.state_id.as_deref(), Some("a-started"));
        set_team(&mut draft, &d, "b");
        assert_eq!(draft.state_id.as_deref(), Some("b-backlog"));
        assert_eq!(draft.label_ids, vec!["all".to_string()]);
    }

    #[test]
    fn create_more_keeps_the_fields_and_clears_the_text() {
        let d = data();
        let mut s = NewIssueState { create_more: true, ..Default::default() };
        s.open(0.0, &d, None, None);
        s.draft.title = "One".into();
        s.draft.label_ids = vec!["all".into()];
        s.pending = Some(4);
        assert_eq!(s.on_created(3), None);
        assert_eq!(s.on_created(4), Some(false));
        assert!(s.is_open());
        assert_eq!(s.draft.title, "");
        assert_eq!(s.draft.label_ids, vec!["all".to_string()]);

        s.create_more = false;
        s.pending = Some(5);
        assert_eq!(s.on_created(5), Some(true));
        assert!(!s.is_open());
    }

    #[test]
    fn failure_shows_the_message_and_keeps_the_draft() {
        let d = data();
        let mut s = NewIssueState::default();
        s.open(0.0, &d, None, None);
        s.draft.title = "Keep me".into();
        s.pending = Some(1);
        s.on_failed(1, "rate limit");
        assert_eq!(s.error.as_deref(), Some("Not created: rate limit"));
        assert_eq!(s.draft.title, "Keep me");
        assert!(s.pending.is_none());
    }
}
