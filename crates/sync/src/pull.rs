//! One sync run of a workspace: catalog, viewer, then issues since the cursor.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use linear_api::Client;
use model::Issue;
use store::Db;

use crate::error::Result;

const CATALOG_MAX_AGE: Duration = Duration::from_secs(3600);

/// What the worker remembers between syncs, for this run of the app only.
#[derive(Default)]
pub(crate) struct PullState {
    catalog_at: HashMap<String, Instant>,
    viewer_done: HashSet<String>,
}

impl PullState {
    pub fn forget(&mut self, ws: &str) {
        self.catalog_at.remove(ws);
        self.viewer_done.remove(ws);
    }
}

/// Runs the sync and returns its finish time.
pub(crate) fn pull(db: &Db, client: &Client, ws: &str, state: &mut PullState) -> Result<DateTime<Utc>> {
    let cursor = store::sync_cursor(db, ws)?;
    if needs_catalog(state.catalog_at.get(ws).copied(), Instant::now()) {
        let c = client.catalog(ws)?;
        let catalog = store::Catalog {
            teams: c.teams,
            states: c.states,
            projects: c.projects,
            users: c.users,
            labels: c.labels,
        };
        store::replace_catalog(db, ws, &catalog)?;
        state.catalog_at.insert(ws.to_string(), Instant::now());
    }
    if !state.viewer_done.contains(ws) {
        let viewer = client.viewer()?;
        store::set_viewer(db, ws, &viewer.id)?;
        state.viewer_done.insert(ws.to_string());
    }
    let page = client.issues_updated_since(ws, cursor)?;
    store::upsert_issues(db, &page.issues)?;
    store::delete_issues(db, &page.archived_ids)?;
    if let Some(next) = max_updated(cursor, &page.issues)
        && Some(next) != cursor
    {
        store::set_sync_cursor(db, ws, next)?;
    }
    let now = Utc::now();
    store::set_last_sync(db, ws, now)?;
    Ok(now)
}

/// The first sync of each app run reads the catalog, and later syncs keep it at most 1 h old.
/// This covers a workspace that never synced. The cursor is not used, because a workspace
/// without issues keeps no cursor and would read the catalog on every sync.
pub(crate) fn needs_catalog(fetched_at: Option<Instant>, now: Instant) -> bool {
    fetched_at.is_none_or(|at| now.saturating_duration_since(at) >= CATALOG_MAX_AGE)
}

/// The newest `updated_at` seen. Linear's clock sets the cursor, not ours, so no edit is missed.
pub(crate) fn max_updated(cursor: Option<DateTime<Utc>>, issues: &[Issue]) -> Option<DateTime<Utc>> {
    issues.iter().map(|i| i.updated_at).chain(cursor).max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use model::{Priority, Source};

    fn t(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_760_000_000 + secs, 0).unwrap()
    }

    fn issue(updated: i64) -> Issue {
        Issue {
            id: format!("i{updated}"),
            source: Source::Linear,
            workspace: "a".into(),
            identifier: format!("A-{updated}"),
            title: String::new(),
            description: String::new(),
            team_id: "t".into(),
            state_id: "s".into(),
            project_id: None,
            assignee_id: None,
            priority: Priority::None,
            label_ids: vec![],
            sort_order: 0.0,
            comment_count: 0,
            url: None,
            created_at: t(0),
            updated_at: t(updated),
        }
    }

    #[test]
    fn cursor_is_max_updated_at() {
        assert_eq!(max_updated(None, &[]), None);
        assert_eq!(max_updated(Some(t(5)), &[]), Some(t(5)));
        assert_eq!(max_updated(None, &[issue(3), issue(9), issue(1)]), Some(t(9)));
        assert_eq!(max_updated(Some(t(20)), &[issue(3)]), Some(t(20)));
        assert_eq!(max_updated(Some(t(2)), &[issue(3)]), Some(t(3)));
    }

    #[test]
    fn catalog_refresh_rules() {
        let now = Instant::now() + Duration::from_secs(10_000);
        assert!(needs_catalog(None, now));
        assert!(!needs_catalog(Some(now - Duration::from_secs(3599)), now));
        assert!(needs_catalog(Some(now - Duration::from_secs(3600)), now));
    }
}
