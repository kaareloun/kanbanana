use chrono::{DateTime, Utc};
use model::{Accent, Workspace};
use rusqlite::{Row, params};

use crate::db::{Db, opt_time, query_all, query_one, time_to_text};
use crate::error::{Error, Result};
use crate::secrets;

fn accent_to_text(a: Accent) -> &'static str {
    match a {
        Accent::Iris => "iris",
        Accent::Foam => "foam",
        Accent::Gold => "gold",
        Accent::Rose => "rose",
        Accent::Pine => "pine",
        Accent::Love => "love",
    }
}

fn text_to_accent(s: &str) -> Result<Accent> {
    Ok(match s {
        "iris" => Accent::Iris,
        "foam" => Accent::Foam,
        "gold" => Accent::Gold,
        "rose" => Accent::Rose,
        "pine" => Accent::Pine,
        "love" => Accent::Love,
        other => return Err(Error::Corrupt(format!("unknown accent {other:?}"))),
    })
}

fn workspace_from_row(row: &Row) -> Result<Workspace> {
    let accent: String = row.get("accent")?;
    Ok(Workspace {
        slug: row.get("slug")?,
        name: row.get("name")?,
        shortcut: row.get("shortcut")?,
        accent: text_to_accent(&accent)?,
        viewer_id: row.get("viewer_id")?,
        position: row.get("position")?,
    })
}

/// All workspaces in switcher order.
pub fn list_workspaces(db: &Db) -> Result<Vec<Workspace>> {
    query_all(
        db,
        "SELECT slug, name, shortcut, accent, viewer_id, position FROM workspaces
         ORDER BY position, slug",
        [],
        workspace_from_row,
    )
}

/// Inserts or updates a workspace. Keeps its sync cursor and last sync time.
pub fn save_workspace(db: &Db, ws: &Workspace) -> Result<()> {
    db.conn.execute(
        "INSERT INTO workspaces (slug, name, shortcut, accent, viewer_id, position)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (slug) DO UPDATE SET
            name = excluded.name, shortcut = excluded.shortcut, accent = excluded.accent,
            viewer_id = excluded.viewer_id, position = excluded.position",
        params![ws.slug, ws.name, ws.shortcut, accent_to_text(ws.accent), ws.viewer_id, ws.position],
    )?;
    Ok(())
}

fn update_workspace_column(db: &Db, slug: &str, sql: &str, value: &dyn rusqlite::ToSql) -> Result<()> {
    let n = db.conn.execute(sql, params![value, slug])?;
    if n == 0 {
        return Err(Error::NotFound(format!("workspace {slug}")));
    }
    Ok(())
}

pub fn set_viewer(db: &Db, slug: &str, viewer_id: &str) -> Result<()> {
    update_workspace_column(db, slug, "UPDATE workspaces SET viewer_id = ?1 WHERE slug = ?2", &viewer_id)
}

/// The `updatedAt` of the newest synced issue, or None before the first sync.
pub fn sync_cursor(db: &Db, slug: &str) -> Result<Option<DateTime<Utc>>> {
    let text = query_one(db, "SELECT sync_cursor FROM workspaces WHERE slug = ?1", [slug], |r| {
        Ok(r.get::<_, Option<String>>(0)?)
    })?;
    opt_time(text.flatten())
}

pub fn set_sync_cursor(db: &Db, slug: &str, cursor: DateTime<Utc>) -> Result<()> {
    update_workspace_column(
        db,
        slug,
        "UPDATE workspaces SET sync_cursor = ?1 WHERE slug = ?2",
        &time_to_text(&cursor),
    )
}

pub fn set_last_sync(db: &Db, slug: &str, at: DateTime<Utc>) -> Result<()> {
    update_workspace_column(
        db,
        slug,
        "UPDATE workspaces SET last_sync = ?1 WHERE slug = ?2",
        &time_to_text(&at),
    )
}

/// Removes the workspace row and every cached row of that workspace, in one transaction.
pub(crate) fn delete_workspace_rows(db: &Db, slug: &str) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM comments WHERE issue_id IN (SELECT id FROM issues WHERE workspace = ?1)",
        [slug],
    )?;
    for table in ["issues", "teams", "states", "projects", "users", "labels", "dev_state"] {
        tx.execute(&format!("DELETE FROM {table} WHERE workspace = ?1"), [slug])?;
    }
    tx.execute("DELETE FROM workspaces WHERE slug = ?1", [slug])?;
    tx.commit()?;
    Ok(())
}

/// Removes the workspace, its cached rows and its API key.
/// The rows go first, so a failure on the key file leaves only the key behind.
pub fn delete_workspace(db: &Db, slug: &str) -> Result<()> {
    delete_workspace_rows(db, slug)?;
    secrets::delete_api_key(slug)
}
