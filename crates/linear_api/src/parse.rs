//! Linear JSON nodes and their conversion to `model` types.

use chrono::{DateTime, Utc};
use model::{
    Comment, Issue, Label, Priority, Project, Source, StateType, Team, User, WorkflowState,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{Error, IssuePage, Result};

/// Field selection for every issue the client reads or writes.
pub(crate) const ISSUE_FRAGMENT: &str = "
fragment IssueFields on Issue {
  id identifier title description priority sortOrder url
  createdAt updatedAt archivedAt
  team { id } state { id } project { id } assignee { id }
  labels(first: 20) { nodes { id } }
}";

pub(crate) const COMMENT_FRAGMENT: &str = "
fragment CommentFields on Comment {
  id body createdAt updatedAt editedAt
  issue { id } parent { id } user { id }
}";

/// Takes `data[field]` and decodes it.
pub(crate) fn field<T: DeserializeOwned>(mut data: Value, field: &str) -> Result<T> {
    let v = data
        .get_mut(field)
        .map(Value::take)
        .ok_or_else(|| Error::Decode(format!("missing field `{field}`")))?;
    serde_json::from_value(v).map_err(|e| Error::Decode(format!("{field}: {e}")))
}

#[derive(Debug, Deserialize)]
pub(crate) struct IdRef {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Connection<T> {
    pub nodes: Vec<T>,
    pub page_info: Option<PageInfo>,
}

impl<T> Connection<T> {
    /// Cursor of the next page, or None on the last page.
    pub fn next_cursor(&self) -> Option<String> {
        self.page_info.as_ref().filter(|p| p.has_next_page).and_then(|p| p.end_cursor.clone())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IssueNode {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    /// Linear types this field as Float, but it holds 0 to 4.
    pub priority: f64,
    pub sort_order: f64,
    pub url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub team: IdRef,
    pub state: IdRef,
    pub project: Option<IdRef>,
    pub assignee: Option<IdRef>,
    pub labels: Connection<IdRef>,
}

impl IssueNode {
    pub fn into_issue(self, ws: &str) -> Issue {
        Issue {
            id: self.id,
            source: Source::Linear,
            workspace: ws.to_string(),
            identifier: self.identifier,
            title: self.title,
            description: self.description.unwrap_or_default(),
            team_id: self.team.id,
            state_id: self.state.id,
            project_id: self.project.map(|p| p.id),
            assignee_id: self.assignee.map(|a| a.id),
            priority: Priority::from_linear(self.priority.round() as i64),
            label_ids: self.labels.nodes.into_iter().map(|l| l.id).collect(),
            sort_order: self.sort_order,
            // Not fetched: a comments connection per issue costs too much complexity.
            comment_count: 0,
            url: self.url,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Splits issue nodes into live issues and ids of archived ones.
pub(crate) fn split_archived(nodes: Vec<IssueNode>, ws: &str, page: &mut IssuePage) {
    for node in nodes {
        if node.archived_at.is_some() {
            page.archived_ids.push(node.id);
        } else {
            page.issues.push(node.into_issue(ws));
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentNode {
    pub id: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub issue: Option<IdRef>,
    pub parent: Option<IdRef>,
    pub user: Option<IdRef>,
}

impl CommentNode {
    /// `fallback_issue_id` covers comments where Linear returns no issue.
    pub fn into_comment(self, fallback_issue_id: &str) -> Comment {
        Comment {
            id: self.id,
            issue_id: self.issue.map(|i| i.id).unwrap_or_else(|| fallback_issue_id.to_string()),
            parent_id: self.parent.map(|p| p.id),
            user_id: self.user.map(|u| u.id),
            body: self.body,
            created_at: self.created_at,
            updated_at: self.updated_at,
            edited: self.edited_at.is_some(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct TeamNode {
    pub id: String,
    pub key: String,
    pub name: String,
}

impl TeamNode {
    pub fn into_team(self, ws: &str) -> Team {
        Team { id: self.id, workspace: ws.to_string(), key: self.key, name: self.name }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct StateNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub position: f64,
    pub color: String,
    pub team: Option<IdRef>,
}

impl StateNode {
    /// Returns None for states without a team. The board groups states by team.
    pub fn into_state(self) -> Option<WorkflowState> {
        Some(WorkflowState {
            id: self.id,
            team_id: self.team?.id,
            name: self.name,
            kind: StateType::from_linear(&self.kind),
            position: self.position,
            color: self.color,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectNode {
    pub id: String,
    pub name: String,
}

impl ProjectNode {
    pub fn into_project(self, ws: &str) -> Project {
        Project { id: self.id, workspace: ws.to_string(), name: self.name }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserNode {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub active: bool,
}

impl UserNode {
    pub fn into_user(self, ws: &str) -> User {
        User {
            id: self.id,
            workspace: ws.to_string(),
            name: self.name,
            display_name: self.display_name,
            avatar_url: self.avatar_url,
            active: self.active,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct LabelNode {
    pub id: String,
    pub name: String,
    pub color: String,
    pub team: Option<IdRef>,
}

impl LabelNode {
    pub fn into_label(self, ws: &str) -> Label {
        Label {
            id: self.id,
            workspace: ws.to_string(),
            name: self.name,
            color: self.color,
            team_id: self.team.map(|t| t.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn issue_json(id: &str, archived_at: Value) -> Value {
        json!({
            "id": id,
            "identifier": "WEB-214",
            "title": "Fix login",
            "description": null,
            "priority": 2,
            "sortOrder": -12.5,
            "url": "https://linear.app/x/issue/WEB-214",
            "createdAt": "2026-01-02T03:04:05.000Z",
            "updatedAt": "2026-01-03T03:04:05.000Z",
            "archivedAt": archived_at,
            "team": { "id": "t1" },
            "state": { "id": "s1" },
            "project": null,
            "assignee": null,
            "labels": { "nodes": [{ "id": "l1" }, { "id": "l2" }] }
        })
    }

    #[test]
    fn issue_with_nulls() {
        let node: IssueNode = serde_json::from_value(issue_json("i1", Value::Null)).unwrap();
        let issue = node.into_issue("acme");
        assert_eq!(issue.workspace, "acme");
        assert_eq!(issue.source, Source::Linear);
        assert_eq!(issue.description, "");
        assert_eq!(issue.project_id, None);
        assert_eq!(issue.assignee_id, None);
        assert_eq!(issue.priority, Priority::High);
        assert_eq!(issue.label_ids, vec!["l1", "l2"]);
        assert_eq!(issue.sort_order, -12.5);
        assert_eq!(issue.created_at.to_rfc3339(), "2026-01-02T03:04:05+00:00");
    }

    #[test]
    fn archived_issues_split_off() {
        let data = json!({ "issues": {
            "nodes": [
                issue_json("live", Value::Null),
                issue_json("gone", json!("2026-02-01T00:00:00.000Z"))
            ],
            "pageInfo": { "hasNextPage": true, "endCursor": "c2" }
        }});
        let conn: Connection<IssueNode> = field(data, "issues").unwrap();
        assert_eq!(conn.next_cursor().as_deref(), Some("c2"));
        let mut page = IssuePage::default();
        split_archived(conn.nodes, "acme", &mut page);
        assert_eq!(page.issues.len(), 1);
        assert_eq!(page.issues[0].id, "live");
        assert_eq!(page.archived_ids, vec!["gone"]);
    }

    #[test]
    fn comment_with_parent() {
        let node: CommentNode = serde_json::from_value(json!({
            "id": "c2",
            "body": "Reply",
            "createdAt": "2026-01-02T03:04:05.000Z",
            "updatedAt": "2026-01-02T04:04:05.000Z",
            "editedAt": "2026-01-02T04:04:05.000Z",
            "issue": null,
            "parent": { "id": "c1" },
            "user": { "id": "u1" }
        }))
        .unwrap();
        let c = node.into_comment("i1");
        assert_eq!(c.issue_id, "i1");
        assert_eq!(c.parent_id.as_deref(), Some("c1"));
        assert_eq!(c.user_id.as_deref(), Some("u1"));
        assert!(c.edited);
    }

    #[test]
    fn catalog_nodes() {
        let state: StateNode = serde_json::from_value(json!({
            "id": "s1", "name": "In Progress", "type": "started",
            "position": 2, "color": "#f2c94c", "team": { "id": "t1" }
        }))
        .unwrap();
        let state = state.into_state().unwrap();
        assert_eq!(state.kind, StateType::Started);
        assert_eq!(state.team_id, "t1");

        let label: LabelNode = serde_json::from_value(json!({
            "id": "l1", "name": "Bug", "color": "#eb5757", "team": null
        }))
        .unwrap();
        assert_eq!(label.into_label("acme").team_id, None);

        let user: UserNode = serde_json::from_value(json!({
            "id": "u1", "name": "Ann Smith", "displayName": "ann",
            "avatarUrl": null, "active": false
        }))
        .unwrap();
        let user = user.into_user("acme");
        assert_eq!(user.display_name, "ann");
        assert!(!user.active);
    }

    #[test]
    fn missing_field_is_decode_error() {
        let err = field::<Connection<IdRef>>(json!({}), "teams").unwrap_err();
        assert!(matches!(err, Error::Decode(_)));
    }
}
