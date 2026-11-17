//! Sync reads: viewer, catalog and issues.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::parse::{
    ISSUE_FRAGMENT, IssueNode, LabelNode, ProjectNode, StateNode, TeamNode, UserNode, field,
    split_archived,
};
use crate::{Catalog, Client, IssuePage, Result, Viewer};

const CATALOG_PAGE: i64 = 100;
/// Each issue has a nested labels connection, so issue pages stay small.
const ISSUE_PAGE: i64 = 50;

const VIEWER_QUERY: &str = "query { viewer { id name } organization { name } }";

const TEAMS_QUERY: &str = "query($first: Int!, $after: String) {
  teams(first: $first, after: $after) {
    nodes { id key name }
    pageInfo { hasNextPage endCursor }
  }
}";

const STATES_QUERY: &str = "query($first: Int!, $after: String) {
  workflowStates(first: $first, after: $after) {
    nodes { id name type position color team { id } }
    pageInfo { hasNextPage endCursor }
  }
}";

const PROJECTS_QUERY: &str = "query($first: Int!, $after: String) {
  projects(first: $first, after: $after) {
    nodes { id name }
    pageInfo { hasNextPage endCursor }
  }
}";

// Disabled users stay in the list. Old issues can still show them as assignees.
const USERS_QUERY: &str = "query($first: Int!, $after: String) {
  users(first: $first, after: $after, includeDisabled: true) {
    nodes { id name displayName avatarUrl active }
    pageInfo { hasNextPage endCursor }
  }
}";

const LABELS_QUERY: &str = "query($first: Int!, $after: String) {
  issueLabels(first: $first, after: $after) {
    nodes { id name color team { id } }
    pageInfo { hasNextPage endCursor }
  }
}";

const ISSUES_QUERY: &str = "query($first: Int!, $after: String, $filter: IssueFilter, $includeArchived: Boolean) {
  issues(first: $first, after: $after, filter: $filter, includeArchived: $includeArchived) {
    nodes { ...IssueFields }
    pageInfo { hasNextPage endCursor }
  }
}";

#[derive(Deserialize)]
struct NameNode {
    name: String,
}

#[derive(Deserialize)]
struct IdNameNode {
    id: String,
    name: String,
}

impl Client {
    pub fn viewer(&self) -> Result<Viewer> {
        let data = self.graphql(VIEWER_QUERY, json!({}))?;
        let viewer: IdNameNode = field(data.clone(), "viewer")?;
        let org: NameNode = field(data, "organization")?;
        Ok(Viewer { id: viewer.id, name: viewer.name, organization_name: org.name })
    }

    /// Reads all teams, states, projects, users and labels. `ws` fills each `workspace` field.
    pub fn catalog(&self, ws: &str) -> Result<Catalog> {
        let vars = json!({ "first": CATALOG_PAGE });
        let teams: Vec<TeamNode> = self.paginate(TEAMS_QUERY, vars.clone(), &["teams"])?;
        let states: Vec<StateNode> =
            self.paginate(STATES_QUERY, vars.clone(), &["workflowStates"])?;
        let projects: Vec<ProjectNode> =
            self.paginate(PROJECTS_QUERY, vars.clone(), &["projects"])?;
        let users: Vec<UserNode> = self.paginate(USERS_QUERY, vars.clone(), &["users"])?;
        let labels: Vec<LabelNode> = self.paginate(LABELS_QUERY, vars, &["issueLabels"])?;
        Ok(Catalog {
            teams: teams.into_iter().map(|t| t.into_team(ws)).collect(),
            states: states.into_iter().filter_map(StateNode::into_state).collect(),
            projects: projects.into_iter().map(|p| p.into_project(ws)).collect(),
            users: users.into_iter().map(|u| u.into_user(ws)).collect(),
            labels: labels.into_iter().map(|l| l.into_label(ws)).collect(),
        })
    }

    /// First sync (`since = None`): open issues plus issues completed in the last 30 days.
    /// Later syncs: every issue updated after `since`, archived ones as `archived_ids`.
    pub fn issues_updated_since(
        &self,
        ws: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<IssuePage> {
        let query = format!("{ISSUES_QUERY}{ISSUE_FRAGMENT}");
        let nodes: Vec<IssueNode> = self.paginate(&query, issue_vars(since), &["issues"])?;
        let mut page = IssuePage::default();
        split_archived(nodes, ws, &mut page);
        Ok(page)
    }
}

fn issue_vars(since: Option<DateTime<Utc>>) -> Value {
    match since {
        None => json!({
            "first": ISSUE_PAGE,
            "includeArchived": false,
            "filter": { "or": [
                { "state": { "type": { "nin": ["completed", "canceled"] } } },
                // Linear reads an ISO 8601 duration as relative to now.
                { "completedAt": { "gt": "-P30D" } }
            ] }
        }),
        Some(t) => json!({
            "first": ISSUE_PAGE,
            "includeArchived": true,
            "filter": { "updatedAt": { "gt": t.to_rfc3339_opts(SecondsFormat::Millis, true) } }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn incremental_vars_include_archived() {
        let t = Utc.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).unwrap();
        let v = issue_vars(Some(t));
        assert_eq!(v["includeArchived"], true);
        assert_eq!(v["filter"]["updatedAt"]["gt"], "2026-03-04T05:06:07.000Z");
    }

    #[test]
    fn first_sync_vars_skip_closed() {
        let v = issue_vars(None);
        assert_eq!(v["includeArchived"], false);
        assert_eq!(v["filter"]["or"][1]["completedAt"]["gt"], "-P30D");
    }
}
