//! Shared data types. Every other crate uses these.
//! The board and the issue panel work on `Issue` and never check where it came from,
//! except to decide which backend saves a change (see `Source`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A Linear workspace that the user added with a personal API key.
/// The key itself lives in the private credentials file, never here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    /// Short local slug, also the key name in the credentials file. Example: "acme".
    pub slug: String,
    pub name: String,
    /// Alt+N switches to this workspace. 1 to 9.
    pub shortcut: u8,
    pub accent: Accent,
    /// Linear user id of the key owner. Filled after the first sync.
    pub viewer_id: Option<String>,
    pub position: i64,
}

/// Theme color names a workspace can use as its accent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    Iris,
    Foam,
    Gold,
    Rose,
    Pine,
    Love,
}

/// Where an issue lives. Linear issues are a cached copy. Local tasks live only in SQLite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Linear,
    Local,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub workspace: String,
    pub key: String,
    pub name: String,
}

/// Linear's state types. Columns sort by type first, then by position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StateType {
    Triage,
    Backlog,
    Unstarted,
    Started,
    Completed,
    Canceled,
}

impl StateType {
    /// Parses Linear's `WorkflowState.type` string.
    pub fn from_linear(s: &str) -> StateType {
        match s {
            "triage" => StateType::Triage,
            "backlog" => StateType::Backlog,
            "unstarted" => StateType::Unstarted,
            "started" => StateType::Started,
            "completed" => StateType::Completed,
            _ => StateType::Canceled,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowState {
    pub id: String,
    pub team_id: String,
    pub name: String,
    pub kind: StateType,
    pub position: f64,
    /// Hex color from Linear, for example "#f2c94c". The UI may ignore it and use theme colors.
    pub color: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub workspace: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub workspace: String,
    pub name: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub workspace: String,
    pub name: String,
    pub color: String,
    /// None for workspace-wide labels.
    pub team_id: Option<String>,
}

/// Linear's priority numbers: 0 none, 1 urgent, 2 high, 3 medium, 4 low.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    None = 0,
    Urgent = 1,
    High = 2,
    Medium = 3,
    Low = 4,
}

impl Priority {
    pub fn from_linear(n: i64) -> Priority {
        match n {
            1 => Priority::Urgent,
            2 => Priority::High,
            3 => Priority::Medium,
            4 => Priority::Low,
            _ => Priority::None,
        }
    }

    pub fn to_linear(self) -> i64 {
        self as i64
    }

    pub fn label(self) -> &'static str {
        match self {
            Priority::None => "No priority",
            Priority::Urgent => "Urgent",
            Priority::High => "High",
            Priority::Medium => "Medium",
            Priority::Low => "Low",
        }
    }

    pub const ALL: [Priority; 5] =
        [Priority::Urgent, Priority::High, Priority::Medium, Priority::Low, Priority::None];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    /// Linear issue uuid, or "local:<n>" for local tasks.
    pub id: String,
    pub source: Source,
    pub workspace: String,
    /// Human id, for example "WEB-214" or "LOC-12".
    pub identifier: String,
    pub title: String,
    /// Markdown.
    pub description: String,
    pub team_id: String,
    pub state_id: String,
    pub project_id: Option<String>,
    pub assignee_id: Option<String>,
    pub priority: Priority,
    pub label_ids: Vec<String>,
    /// Linear's sortOrder. Lower is higher in the column.
    pub sort_order: f64,
    pub comment_count: i64,
    pub url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub issue_id: String,
    /// Set for replies. Linear threads are one level deep.
    pub parent_id: Option<String>,
    pub user_id: Option<String>,
    /// Markdown.
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub edited: bool,
}

/// How far the work on an issue got. Claude reports it over MCP. The app does not scan git.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DevStage {
    Local,
    Mr,
    Merged,
    Staging,
    Prod,
}

impl DevStage {
    pub fn label(self) -> &'static str {
        match self {
            DevStage::Local => "Local",
            DevStage::Mr => "MR",
            DevStage::Merged => "Merged",
            DevStage::Staging => "Staging",
            DevStage::Prod => "Prod",
        }
    }

    pub fn parse(s: &str) -> Option<DevStage> {
        match s.to_ascii_lowercase().as_str() {
            "local" => Some(DevStage::Local),
            "mr" | "pr" => Some(DevStage::Mr),
            "merged" => Some(DevStage::Merged),
            "staging" => Some(DevStage::Staging),
            "prod" | "production" => Some(DevStage::Prod),
            _ => None,
        }
    }
}

/// Dev state of one issue, keyed by workspace and identifier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DevState {
    pub workspace: String,
    pub identifier: String,
    pub stage: DevStage,
    /// Branch name, or MR number like "!412".
    pub git_ref: Option<String>,
    /// Absolute worktree or repo path.
    pub path: Option<String>,
    /// MR or deploy URL.
    pub url: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Everything the board needs for one workspace, read from the store in one go.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardData {
    pub teams: Vec<Team>,
    pub states: Vec<WorkflowState>,
    pub projects: Vec<Project>,
    pub users: Vec<User>,
    pub labels: Vec<Label>,
    pub issues: Vec<Issue>,
    pub dev: Vec<DevState>,
    /// Linear user id of the key owner, for the "Assigned to me" filter.
    pub viewer_id: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
}

/// A change to an issue. The app applies it to the store at once, then sends it to the
/// right backend. The same enum serves the GUI and the MCP server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IssueChange {
    Title(String),
    Description(String),
    /// New state, and the sort order inside the new column.
    State { state_id: String, sort_order: Option<f64> },
    SortOrder(f64),
    Assignee(Option<String>),
    Priority(Priority),
    Labels(Vec<String>),
}
