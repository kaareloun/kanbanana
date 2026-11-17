use chrono::Utc;
use model::{BoardData, Issue, IssueChange, Priority, Source};
use rusqlite::{Row, params};

use crate::catalog::load_catalog;
use crate::db::{Db, opt_time, query_all, query_one, text_to_time, time_to_text};
use crate::dev::list_dev;
use crate::error::{Error, Result};

const ISSUE_COLS: &str = "id, source, workspace, identifier, title, description, team_id, \
    state_id, project_id, assignee_id, priority, label_ids, sort_order, comment_count, url, \
    created_at, updated_at";

fn source_to_text(s: Source) -> &'static str {
    match s {
        Source::Linear => "linear",
        Source::Local => "local",
    }
}

fn text_to_source(s: &str) -> Result<Source> {
    match s {
        "linear" => Ok(Source::Linear),
        "local" => Ok(Source::Local),
        other => Err(Error::Corrupt(format!("unknown issue source {other:?}"))),
    }
}

fn labels_to_text(ids: &[String]) -> Result<String> {
    serde_json::to_string(ids).map_err(|e| Error::Corrupt(format!("label ids: {e}")))
}

fn issue_from_row(r: &Row) -> Result<Issue> {
    let source: String = r.get(1)?;
    let label_ids: String = r.get(11)?;
    let created_at: String = r.get(15)?;
    let updated_at: String = r.get(16)?;
    Ok(Issue {
        id: r.get(0)?,
        source: text_to_source(&source)?,
        workspace: r.get(2)?,
        identifier: r.get(3)?,
        title: r.get(4)?,
        description: r.get(5)?,
        team_id: r.get(6)?,
        state_id: r.get(7)?,
        project_id: r.get(8)?,
        assignee_id: r.get(9)?,
        priority: Priority::from_linear(r.get(10)?),
        label_ids: serde_json::from_str(&label_ids)
            .map_err(|e| Error::Corrupt(format!("label ids {label_ids:?}: {e}")))?,
        sort_order: r.get(12)?,
        comment_count: r.get(13)?,
        url: r.get(14)?,
        created_at: text_to_time(&created_at)?,
        updated_at: text_to_time(&updated_at)?,
    })
}

/// Inserts new issues and overwrites known ones, in one transaction.
/// An incoming `comment_count` of 0 keeps the stored count. Linear sync does not send counts.
pub fn upsert_issues(db: &Db, issues: &[Issue]) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(&format!(
            "INSERT INTO issues ({ISSUE_COLS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT (id) DO UPDATE SET
                source = excluded.source, workspace = excluded.workspace,
                identifier = excluded.identifier, title = excluded.title,
                description = excluded.description, team_id = excluded.team_id,
                state_id = excluded.state_id, project_id = excluded.project_id,
                assignee_id = excluded.assignee_id, priority = excluded.priority,
                label_ids = excluded.label_ids, sort_order = excluded.sort_order,
                comment_count = CASE WHEN excluded.comment_count = 0
                    THEN issues.comment_count ELSE excluded.comment_count END,
                url = excluded.url,
                created_at = excluded.created_at, updated_at = excluded.updated_at"
        ))?;
        for i in issues {
            stmt.execute(params![
                i.id,
                source_to_text(i.source),
                i.workspace,
                i.identifier,
                i.title,
                i.description,
                i.team_id,
                i.state_id,
                i.project_id,
                i.assignee_id,
                i.priority.to_linear(),
                labels_to_text(&i.label_ids)?,
                i.sort_order,
                i.comment_count,
                i.url,
                time_to_text(&i.created_at),
                time_to_text(&i.updated_at),
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Deletes issues and their cached comments. Unknown ids are skipped.
pub fn delete_issues(db: &Db, ids: &[String]) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    {
        let mut del_comments = tx.prepare("DELETE FROM comments WHERE issue_id = ?1")?;
        let mut del_issue = tx.prepare("DELETE FROM issues WHERE id = ?1")?;
        for id in ids {
            del_comments.execute([id])?;
            del_issue.execute([id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Everything the board shows for one workspace.
pub fn load_board(db: &Db, ws: &str) -> Result<BoardData> {
    let catalog = load_catalog(db, ws)?;
    let issues = query_all(
        db,
        &format!("SELECT {ISSUE_COLS} FROM issues WHERE workspace = ?1 ORDER BY sort_order, id"),
        [ws],
        issue_from_row,
    )?;
    let row = query_one(
        db,
        "SELECT viewer_id, last_sync FROM workspaces WHERE slug = ?1",
        [ws],
        |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
    )?;
    let (viewer_id, last_sync) = row.unwrap_or_default();
    Ok(BoardData {
        teams: catalog.teams,
        states: catalog.states,
        projects: catalog.projects,
        users: catalog.users,
        labels: catalog.labels,
        issues,
        dev: list_dev(db, ws)?,
        viewer_id,
        last_sync: opt_time(last_sync)?,
    })
}

pub fn get_issue(db: &Db, id: &str) -> Result<Option<Issue>> {
    query_one(db, &format!("SELECT {ISSUE_COLS} FROM issues WHERE id = ?1"), [id], issue_from_row)
}

/// Finds an issue by its human id, ignoring case. Without `ws` it searches all workspaces.
pub fn find_issue(db: &Db, ws: Option<&str>, identifier: &str) -> Result<Option<Issue>> {
    let identifier = identifier.trim();
    match ws {
        Some(ws) => query_one(
            db,
            &format!("SELECT {ISSUE_COLS} FROM issues WHERE workspace = ?1 AND identifier = ?2"),
            [ws, identifier],
            issue_from_row,
        ),
        None => query_one(
            db,
            &format!("SELECT {ISSUE_COLS} FROM issues WHERE identifier = ?1 ORDER BY workspace"),
            [identifier],
            issue_from_row,
        ),
    }
}

/// Applies a local edit to the cached row and sets `updated_at` to now.
pub fn apply_change(db: &Db, issue_id: &str, change: &IssueChange) -> Result<()> {
    let now = time_to_text(&Utc::now());
    let n = match change {
        IssueChange::Title(t) => db.conn.execute(
            "UPDATE issues SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![t, now, issue_id],
        )?,
        IssueChange::Description(d) => db.conn.execute(
            "UPDATE issues SET description = ?1, updated_at = ?2 WHERE id = ?3",
            params![d, now, issue_id],
        )?,
        IssueChange::State { state_id, sort_order } => db.conn.execute(
            "UPDATE issues SET state_id = ?1, sort_order = COALESCE(?2, sort_order), updated_at = ?3
             WHERE id = ?4",
            params![state_id, sort_order, now, issue_id],
        )?,
        IssueChange::SortOrder(s) => db.conn.execute(
            "UPDATE issues SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
            params![s, now, issue_id],
        )?,
        IssueChange::Assignee(a) => db.conn.execute(
            "UPDATE issues SET assignee_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![a, now, issue_id],
        )?,
        IssueChange::Priority(p) => db.conn.execute(
            "UPDATE issues SET priority = ?1, updated_at = ?2 WHERE id = ?3",
            params![p.to_linear(), now, issue_id],
        )?,
        IssueChange::Labels(ids) => db.conn.execute(
            "UPDATE issues SET label_ids = ?1, updated_at = ?2 WHERE id = ?3",
            params![labels_to_text(ids)?, now, issue_id],
        )?,
    };
    if n == 0 {
        return Err(Error::NotFound(format!("issue {issue_id}")));
    }
    Ok(())
}
