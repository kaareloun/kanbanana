//! Issue mutations.

use model::{Issue, IssueChange};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::parse::{ISSUE_FRAGMENT, IssueNode, field};
use crate::{Client, Error, Result};

const UPDATE_ISSUE: &str = "mutation($id: String!, $input: IssueUpdateInput!) {
  issueUpdate(id: $id, input: $input) { success issue { ...IssueFields } }
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
        let payload: IssuePayload = field(data, "issueUpdate")?;
        match payload.issue {
            Some(node) if payload.success => Ok(node.into_issue(ws)),
            _ => Err(Error::GraphQl("issueUpdate did not succeed".into())),
        }
    }
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
    use model::Priority;

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
}
