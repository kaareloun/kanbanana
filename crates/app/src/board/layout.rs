//! Turns `BoardData` into group tabs and the columns of the active tab. No painting here.

use std::collections::HashSet;

use std::cmp::Ordering;

use model::{BoardData, Issue, Priority, StateType};

use crate::theme::parts::StatusKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum GroupBy {
    #[default]
    Team,
    Project,
}

/// Key of the tab that shows every issue.
pub const ALL: &str = "all";
const NO_PROJECT: &str = "project:none";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabKind {
    All,
    /// Index of the team or project in `BoardData`. The tab color comes from it.
    Team(usize),
    Project(usize),
    NoProject,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    /// "all", "team:<id>", "project:<id>" or "project:none".
    pub key: String,
    pub name: String,
    pub kind: TabKind,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub kind: StatusKind,
    /// Issue indexes into `BoardData.issues`, most urgent first. See `urgency_cmp`.
    pub cards: Vec<usize>,
}

pub fn status_kind(kind: StateType, name: &str) -> StatusKind {
    match kind {
        StateType::Triage | StateType::Backlog => StatusKind::Backlog,
        StateType::Unstarted => StatusKind::Todo,
        // Linear has no own type for review states. They are "started" too.
        StateType::Started if name.to_lowercase().contains("review") => StatusKind::Review,
        StateType::Started => StatusKind::Progress,
        StateType::Completed => StatusKind::Done,
        StateType::Canceled => StatusKind::Canceled,
    }
}

pub fn is_visible(data: &BoardData, issue: &Issue, mine: bool) -> bool {
    !mine || (issue.assignee_id.is_some() && issue.assignee_id == data.viewer_id)
}

/// True when the issue has a known state that is not canceled.
fn on_board(data: &BoardData, issue: &Issue) -> bool {
    data.states.iter().any(|s| s.id == issue.state_id && s.kind != StateType::Canceled)
}

fn project_key(data: &BoardData, issue: &Issue) -> String {
    match &issue.project_id {
        Some(p) if data.projects.iter().any(|x| &x.id == p) => format!("project:{p}"),
        _ => NO_PROJECT.into(),
    }
}

pub fn in_tab(data: &BoardData, issue: &Issue, group: GroupBy, key: &str) -> bool {
    if key == ALL {
        return true;
    }
    match group {
        GroupBy::Team => key.strip_prefix("team:") == Some(issue.team_id.as_str()),
        GroupBy::Project => project_key(data, issue) == key,
    }
}

/// "All teams" or "All projects" first, then one tab per team or project.
/// "No project" shows only when some issues have no project.
pub fn tabs(data: &BoardData, group: GroupBy, mine: bool) -> Vec<Tab> {
    let shown: Vec<&Issue> =
        data.issues.iter().filter(|i| on_board(data, i) && is_visible(data, i, mine)).collect();
    let count = |key: &str| shown.iter().filter(|i| in_tab(data, i, group, key)).count();
    let all = match group {
        GroupBy::Team => "All teams",
        GroupBy::Project => "All projects",
    };
    let mut out = vec![Tab { key: ALL.into(), name: all.into(), kind: TabKind::All, count: shown.len() }];
    match group {
        GroupBy::Team => {
            for (i, team) in data.teams.iter().enumerate() {
                let key = format!("team:{}", team.id);
                out.push(Tab { count: count(&key), key, name: team.name.clone(), kind: TabKind::Team(i) });
            }
        }
        GroupBy::Project => {
            for (i, p) in data.projects.iter().enumerate() {
                let key = format!("project:{}", p.id);
                out.push(Tab { count: count(&key), key, name: p.name.clone(), kind: TabKind::Project(i) });
            }
            let none = count(NO_PROJECT);
            if none > 0 {
                out.push(Tab { key: NO_PROJECT.into(), name: "No project".into(), kind: TabKind::NoProject, count: none });
            }
        }
    }
    out
}

/// Columns of one tab: the union of state names over its teams, by state type, then position.
/// Canceled states stay hidden.
pub fn columns(data: &BoardData, group: GroupBy, tab: &str, mine: bool) -> Vec<Column> {
    let issues: Vec<usize> = (0..data.issues.len())
        .filter(|&i| {
            let issue = &data.issues[i];
            on_board(data, issue) && is_visible(data, issue, mine) && in_tab(data, issue, group, tab)
        })
        .collect();
    // A team tab shows its own states. Other tabs show the states of the teams they contain.
    let teams: HashSet<&str> = match (group, tab.strip_prefix("team:")) {
        (GroupBy::Team, Some(team)) => HashSet::from([team]),
        _ => issues.iter().map(|&i| data.issues[i].team_id.as_str()).collect(),
    };
    let use_state = |team: &str| teams.is_empty() || teams.contains(team);

    let mut best: Vec<(StateType, f64, &str)> = Vec::new();
    for s in data.states.iter().filter(|s| s.kind != StateType::Canceled && use_state(&s.team_id)) {
        match best.iter_mut().find(|b| b.2 == s.name) {
            Some(b) => {
                if (s.kind, s.position) < (b.0, b.1) {
                    b.0 = s.kind;
                    b.1 = s.position;
                }
            }
            None => best.push((s.kind, s.position, &s.name)),
        }
    }
    best.sort_by(|a, b| (a.0, a.1).partial_cmp(&(b.0, b.1)).unwrap_or(std::cmp::Ordering::Equal));
    let mut cols: Vec<Column> = best
        .into_iter()
        .map(|(kind, _, name)| Column { name: name.to_string(), kind: status_kind(kind, name), cards: Vec::new() })
        .collect();

    for i in issues {
        let state = data.states.iter().find(|s| s.id == data.issues[i].state_id);
        if let Some(c) = state.and_then(|s| cols.iter_mut().find(|c| c.name == s.name)) {
            c.cards.push(i);
        }
    }
    for c in &mut cols {
        c.cards.sort_by(|a, b| urgency_cmp(&data.issues[*a], &data.issues[*b]));
    }
    cols
}

/// Urgent, High, Medium, Low, then No priority.
fn urgency(p: Priority) -> u8 {
    match p {
        Priority::Urgent => 0,
        Priority::High => 1,
        Priority::Medium => 2,
        Priority::Low => 3,
        Priority::None => 4,
    }
}

/// Card order inside a column: by priority, then by Linear's sort order.
pub fn urgency_cmp(a: &Issue, b: &Issue) -> Ordering {
    urgency(a.priority)
        .cmp(&urgency(b.priority))
        .then(a.sort_order.partial_cmp(&b.sort_order).unwrap_or(Ordering::Equal))
}

/// Index among the other cards of `col` where `issue` lands when it moves there.
pub fn landing(data: &BoardData, col: &Column, issue: usize) -> usize {
    let me = &data.issues[issue];
    col.cards
        .iter()
        .filter(|&&i| i != issue && urgency_cmp(&data.issues[i], me) != Ordering::Greater)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_merge_names_and_hide_canceled() {
        let data = crate::mock::board("acme");
        let cols = columns(&data, GroupBy::Team, ALL, false);
        let names: Vec<_> = cols.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Backlog", "Todo", "In Progress", "In Review", "Done"]);
        assert_eq!(cols[3].kind, StatusKind::Review);
    }

    #[test]
    fn team_tabs_count_their_issues() {
        let data = crate::mock::board("acme");
        let tabs = tabs(&data, GroupBy::Team, false);
        let got: Vec<_> = tabs.iter().map(|t| (t.name.as_str(), t.count)).collect();
        assert_eq!(got, [("All teams", 26), ("Webshop", 13), ("Platform", 6), ("Design", 7)]);
        let todo = &columns(&data, GroupBy::Team, &tabs[1].key, false)[1];
        assert_eq!(todo.cards.len(), 6);
    }

    #[test]
    fn cards_sort_by_priority_then_sort_order() {
        let data = crate::mock::board("acme");
        let todo = &columns(&data, GroupBy::Team, "team:acme-t-web", false)[1];
        let ids: Vec<_> = todo.cards.iter().map(|&i| data.issues[i].identifier.as_str()).collect();
        assert_eq!(ids, ["WEB-226", "WEB-219", "WEB-237", "WEB-217", "WEB-236", "WEB-238"]);
    }

    #[test]
    fn landing_follows_priority() {
        let data = crate::mock::board("acme");
        let cols = columns(&data, GroupBy::Team, "team:acme-t-web", false);
        let urgent = data.issues.iter().position(|i| i.identifier == "WEB-214").unwrap();
        let low = data.issues.iter().position(|i| i.identifier == "WEB-231").unwrap();
        let todo = &cols[1];
        assert_eq!(landing(&data, todo, urgent), 0);
        // Low with sort order 1000 goes before the other Low cards.
        assert_eq!(landing(&data, todo, low), 3);
        // In its own column a card lands where it is.
        let progress = &cols[2];
        assert_eq!(landing(&data, progress, urgent), 0);
    }

    #[test]
    fn project_tabs_put_no_project_last() {
        let data = crate::mock::board("acme");
        let tabs = tabs(&data, GroupBy::Project, false);
        let names: Vec<_> = tabs.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["All projects", "Webshop launch", "Infra", "No project"]);
    }
}
