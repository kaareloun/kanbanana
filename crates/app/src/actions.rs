//! What views ask the app to do. Views push these and never do I/O.

use std::path::PathBuf;

use model::{BoardData, Issue, IssueChange, IssueDraft, Workspace};

use crate::issue_panel::PickerKind;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Edit an issue. The app applies it to local state at once, then queues it for sync.
    ChangeIssue { issue_id: String, change: IssueChange },
    SelectIssue(Option<String>),
    /// Opens the issue in the panel with this picker open.
    OpenIssueWith { issue_id: String, picker: PickerKind },
    /// Opens the new issue form.
    NewIssue,
    /// Creates an issue in the active workspace. The result goes to the form by `request_id`.
    CreateIssue { request_id: u64, draft: IssueDraft },
    /// Opens or closes the keyboard shortcuts overlay.
    ShowShortcuts,
    SwitchWorkspace(String),
    OpenWorkspaces,
    CloseWorkspaces,
    Refresh,
    /// Check an API key against Linear. The result lands in the add form as a `KeyCheck`.
    CheckKey(String),
    AddWorkspace { workspace: Workspace, api_key: String },
    RemoveWorkspace(String),
    /// The issue panel uses it for issue and MR links.
    OpenUrl(String),
    LoadComments { issue_id: String },
    CreateComment { issue_id: String, body: String, parent_id: Option<String> },
    UpdateComment { issue_id: String, comment_id: String, body: String },
    Upload { request_id: u64, filename: String, content_type: String, bytes: Vec<u8> },
    /// Copies text to the clipboard and shows the "Copied" toast.
    Copy(String),
    OpenSettings,
    CloseSettings,
    /// Theme name from `settings::THEMES`. Applies at once and goes to the settings file.
    SetTheme(String),
    /// Opens the folder of this file in the system file manager.
    OpenFolder(PathBuf),
    /// Clears the onboarding flag and opens the onboarding flow.
    RunOnboarding,
    /// Writes the onboarding flag and goes to the board.
    FinishOnboarding,
    /// Looks for Claude Code and the kanbanana server in it, on a worker thread.
    CheckClaude,
    /// Runs `claude mcp add` for the kanbanana server, on a worker thread.
    AddToClaude,
}

/// "WEB-214 Refund webhook marks the order paid twice".
pub fn copy_title(issue: &Issue) -> String {
    format!("{} {}", issue.identifier, issue.title)
}

/// A prompt to paste into Claude Code. The MCP server finds the issue by its id.
pub fn copy_prompt(issue: &Issue) -> String {
    format!("start {} in {}", issue.identifier, issue.workspace)
}

/// Applies a change to the in-memory board, so the UI updates before the sync.
pub fn apply_change(data: &mut BoardData, issue_id: &str, change: &IssueChange) {
    let Some(issue) = data.issues.iter_mut().find(|i| i.id == issue_id) else {
        return;
    };
    match change {
        IssueChange::Title(s) => issue.title = s.clone(),
        IssueChange::Description(s) => issue.description = s.clone(),
        IssueChange::State { state_id, sort_order } => {
            issue.state_id = state_id.clone();
            if let Some(o) = sort_order {
                issue.sort_order = *o;
            }
        }
        IssueChange::SortOrder(o) => issue.sort_order = *o,
        IssueChange::Assignee(a) => issue.assignee_id = a.clone(),
        IssueChange::Priority(p) => issue.priority = *p,
        IssueChange::Labels(l) => issue.label_ids = l.clone(),
    }
}
