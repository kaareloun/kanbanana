//! Issue mutations.

use model::{Issue, IssueChange, IssueDraft, Priority};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::parse::{ISSUE_FRAGMENT, IssueNode, field};
use crate::{Client, Error, Result};

const UPDATE_ISSUE: &str = "mutation($id: String!, $input: IssueUpdateInput!) {
  issueUpdate(id: $id, input: $input) { success issue { ...IssueFields } }
}";

const CREATE_ISSUE: &str = "mutation($input: IssueCreateInput!) {
  issueCreate(input: $input) { success issue { ...IssueFields } }
}";

const ISSUE_QUERY: &str = "query($id: String!) { issue(id: $id) { ...IssueFields } }";

#[derive(Deserialize)]
struct IssuePayload {
    success: bool,
    issue: Option<IssueNode>,
}

impl Client {
    /// Reads one issue by uuid or identifier. `ws` fills the `workspace` field of the result.
    pub fn issue(&self, ws: &str, id: &str) -> Result<Issue> {
        let query = format!("{ISSUE_QUERY}{ISSUE_FRAGMENT}");
        let data = self.graphql(&query, json!({ "id": id }))?;
        let node: IssueNode = field(data, "issue")?;
        Ok(node.into_issue(ws))
    }

    /// Sends all `changes` in one `issueUpdate` call and returns the issue as Linear saved it.
    /// `ws` fills the `workspace` field of the result.
    pub fn update_issue(&self, ws: &str, id: &str, changes: &[IssueChange]) -> Result<Issue> {
        let query = format!("{UPDATE_ISSUE}{ISSUE_FRAGMENT}");
        let data = self.graphql(&query, json!({ "id": id, "input": update_input(changes) }))?;
        saved_issue(data, "issueUpdate", ws)
    }

    /// Creates an issue with `issueCreate` and returns it as Linear saved it.
    /// `ws` fills the `workspace` field of the result.
    pub fn create_issue(&self, ws: &str, draft: &IssueDraft) -> Result<Issue> {
        let query = format!("{CREATE_ISSUE}{ISSUE_FRAGMENT}");
        let data = self.graphql(&query, json!({ "input": create_input(draft) }))?;
        saved_issue(data, "issueCreate", ws)
    }
}

/// Reads the issue of an `IssuePayload` in `data[mutation]`.
pub(crate) fn saved_issue(data: Value, mutation: &str, ws: &str) -> Result<Issue> {
    let payload: IssuePayload = field(data, mutation)?;
    match payload.issue {
        Some(node) if payload.success => Ok(node.into_issue(ws)),
        _ => Err(Error::GraphQl(format!("{mutation} did not succeed"))),
    }
}

/// Builds `IssueCreateInput`. Empty fields stay out, so Linear uses the team defaults.
pub(crate) fn create_input(draft: &IssueDraft) -> Value {
    let mut input = Map::new();
    input.insert("teamId".into(), json!(draft.team_id));
    input.insert("title".into(), json!(draft.title.trim()));
    if !draft.description.trim().is_empty() {
        input.insert("description".into(), json!(draft.description));
    }
    if let Some(s) = &draft.state_id {
        input.insert("stateId".into(), json!(s));
    }
    if draft.priority != Priority::None {
        input.insert("priority".into(), json!(draft.priority.to_linear()));
    }
    if let Some(a) = &draft.assignee_id {
        input.insert("assigneeId".into(), json!(a));
    }
    if !draft.label_ids.is_empty() {
        input.insert("labelIds".into(), json!(draft.label_ids));
    }
    if let Some(p) = &draft.project_id {
        input.insert("projectId".into(), json!(p));
    }
    Value::Object(input)
}

/// Builds `IssueUpdateInput`. A later change to the same field wins.
pub(crate) fn update_input(changes: &[IssueChange]) -> Value {
    let mut input = Map::new();
    for change in changes {
        match change {
            IssueChange::Title(t) => {
                input.insert("title".into(), json!(t));
            }
            IssueChange::Description(d) => {
                input.insert("description".into(), json!(d));
            }
            IssueChange::State { state_id, sort_order } => {
                input.insert("stateId".into(), json!(state_id));
                if let Some(s) = sort_order {
                    input.insert("sortOrder".into(), json!(s));
                }
            }
            IssueChange::SortOrder(s) => {
                input.insert("sortOrder".into(), json!(s));
            }
            IssueChange::Assignee(a) => {
                // JSON null unassigns the issue.
                input.insert("assigneeId".into(), json!(a));
            }
            IssueChange::Priority(p) => {
                input.insert("priority".into(), json!(p.to_linear()));
            }
            IssueChange::Labels(ids) => {
                input.insert("labelIds".into(), json!(ids));
            }
        }
    }
    Value::Object(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_from_changes() {
        let v = update_input(&[
            IssueChange::Title("New".into()),
            IssueChange::State { state_id: "s2".into(), sort_order: Some(3.5) },
            IssueChange::Assignee(None),
            IssueChange::Priority(Priority::Urgent),
            IssueChange::Labels(vec!["l1".into()]),
        ]);
        assert_eq!(
            v,
            json!({
                "title": "New",
                "stateId": "s2",
                "sortOrder": 3.5,
                "assigneeId": null,
                "priority": 1,
                "labelIds": ["l1"]
            })
        );
    }

    #[test]
    fn state_without_sort_order() {
        let v = update_input(&[IssueChange::State { state_id: "s2".into(), sort_order: None }]);
        assert_eq!(v, json!({ "stateId": "s2" }));
    }

    #[test]
    fn create_input_with_all_fields() {
        let draft = IssueDraft {
            team_id: "t1".into(),
            title: "  New issue ".into(),
            description: "Some **text**".into(),
            state_id: Some("s1".into()),
            priority: Priority::High,
            assignee_id: Some("u1".into()),
            label_ids: vec!["l1".into(), "l2".into()],
            project_id: Some("p1".into()),
        };
        assert_eq!(
            create_input(&draft),
            json!({
                "teamId": "t1",
                "title": "New issue",
                "description": "Some **text**",
                "stateId": "s1",
                "priority": 2,
                "assigneeId": "u1",
                "labelIds": ["l1", "l2"],
                "projectId": "p1"
            })
        );
    }

    #[test]
    fn create_input_leaves_out_defaults() {
        let draft = IssueDraft { team_id: "t1".into(), title: "Hi".into(), description: " \n".into(), ..Default::default() };
        assert_eq!(create_input(&draft), json!({ "teamId": "t1", "title": "Hi" }));
    }

    fn created(success: bool) -> Value {
        json!({ "issueCreate": { "success": success, "issue": {
            "id": "uuid-1", "identifier": "WEB-300", "title": "New issue", "description": null,
            "priority": 2.0, "sortOrder": -12.5, "url": "https://linear.app/acme/issue/WEB-300",
            "createdAt": "2026-11-18T10:00:00.000Z", "updatedAt": "2026-11-18T10:00:00.000Z",
            "archivedAt": null, "team": { "id": "t1" }, "state": { "id": "s1" },
            "project": null, "assignee": { "id": "u1" },
            "labels": { "nodes": [{ "id": "l1" }] }
        } } })
    }

    #[test]
    fn create_response_parses() {
        let issue = saved_issue(created(true), "issueCreate", "acme").unwrap();
        assert_eq!(issue.id, "uuid-1");
        assert_eq!(issue.workspace, "acme");
        assert_eq!(issue.identifier, "WEB-300");
        assert_eq!(issue.description, "");
        assert_eq!(issue.priority, Priority::High);
        assert_eq!(issue.state_id, "s1");
        assert_eq!(issue.assignee_id.as_deref(), Some("u1"));
        assert_eq!(issue.label_ids, vec!["l1".to_string()]);
        assert_eq!(issue.project_id, None);
    }

    #[test]
    fn create_without_success_is_an_error() {
        let e = saved_issue(created(false), "issueCreate", "acme").unwrap_err();
        assert!(matches!(e, Error::GraphQl(m) if m.contains("issueCreate")));
    }
}
