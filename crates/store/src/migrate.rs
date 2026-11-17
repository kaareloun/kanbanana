use rusqlite::{Transaction, TransactionBehavior};

use crate::db::Db;
use crate::error::Result;

/// Schema steps. Step N moves `user_version` from N to N + 1. Only append new steps.
/// Identifiers use NOCASE, so "web-214" finds "WEB-214".
pub(crate) const STEPS: &[&str] = &[r#"
CREATE TABLE workspaces (
    slug        TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    shortcut    INTEGER NOT NULL,
    accent      TEXT NOT NULL,
    viewer_id   TEXT,
    position    INTEGER NOT NULL,
    sync_cursor TEXT,
    last_sync   TEXT
);

CREATE TABLE teams (
    id        TEXT PRIMARY KEY NOT NULL,
    workspace TEXT NOT NULL,
    key       TEXT NOT NULL,
    name      TEXT NOT NULL
);
CREATE INDEX teams_workspace ON teams (workspace);

CREATE TABLE states (
    id        TEXT PRIMARY KEY NOT NULL,
    workspace TEXT NOT NULL,
    team_id   TEXT NOT NULL,
    name      TEXT NOT NULL,
    kind      TEXT NOT NULL,
    position  REAL NOT NULL,
    color     TEXT NOT NULL
);
CREATE INDEX states_workspace ON states (workspace);

CREATE TABLE projects (
    id        TEXT PRIMARY KEY NOT NULL,
    workspace TEXT NOT NULL,
    name      TEXT NOT NULL
);
CREATE INDEX projects_workspace ON projects (workspace);

CREATE TABLE users (
    id           TEXT PRIMARY KEY NOT NULL,
    workspace    TEXT NOT NULL,
    name         TEXT NOT NULL,
    display_name TEXT NOT NULL,
    avatar_url   TEXT,
    active       INTEGER NOT NULL
);
CREATE INDEX users_workspace ON users (workspace);

CREATE TABLE labels (
    id        TEXT PRIMARY KEY NOT NULL,
    workspace TEXT NOT NULL,
    name      TEXT NOT NULL,
    color     TEXT NOT NULL,
    team_id   TEXT
);
CREATE INDEX labels_workspace ON labels (workspace);

CREATE TABLE issues (
    id            TEXT PRIMARY KEY NOT NULL,
    source        TEXT NOT NULL,
    workspace     TEXT NOT NULL,
    identifier    TEXT NOT NULL COLLATE NOCASE,
    title         TEXT NOT NULL,
    description   TEXT NOT NULL,
    team_id       TEXT NOT NULL,
    state_id      TEXT NOT NULL,
    project_id    TEXT,
    assignee_id   TEXT,
    priority      INTEGER NOT NULL,
    label_ids     TEXT NOT NULL,
    sort_order    REAL NOT NULL,
    comment_count INTEGER NOT NULL,
    url           TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX issues_workspace_identifier ON issues (workspace, identifier);
CREATE INDEX issues_identifier ON issues (identifier);

CREATE TABLE comments (
    id         TEXT PRIMARY KEY NOT NULL,
    issue_id   TEXT NOT NULL,
    parent_id  TEXT,
    user_id    TEXT,
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    edited     INTEGER NOT NULL
);
CREATE INDEX comments_issue ON comments (issue_id);

CREATE TABLE dev_state (
    workspace  TEXT NOT NULL,
    identifier TEXT NOT NULL COLLATE NOCASE,
    stage      TEXT NOT NULL,
    git_ref    TEXT,
    path       TEXT,
    url        TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (workspace, identifier)
);
"#];

fn user_version(conn: &rusqlite::Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Brings the schema up to date. Two processes can start at once, so the
/// version is read again inside a write transaction.
pub(crate) fn run(db: &Db) -> Result<()> {
    let target = STEPS.len() as i64;
    if user_version(&db.conn)? >= target {
        return Ok(());
    }
    let tx = Transaction::new_unchecked(&db.conn, TransactionBehavior::Immediate)?;
    let current = user_version(&tx)?;
    for (n, sql) in STEPS.iter().enumerate().skip(current.max(0) as usize) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", n as i64 + 1)?;
    }
    tx.commit()?;
    Ok(())
}
