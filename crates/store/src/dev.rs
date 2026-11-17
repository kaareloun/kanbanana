use model::{DevStage, DevState};
use rusqlite::{Row, params};

use crate::db::{Db, query_all, query_one, text_to_time, time_to_text};
use crate::error::{Error, Result};

const DEV_COLS: &str = "workspace, identifier, stage, git_ref, path, url, updated_at";

fn stage_to_text(s: DevStage) -> &'static str {
    match s {
        DevStage::Local => "local",
        DevStage::Mr => "mr",
        DevStage::Merged => "merged",
        DevStage::Staging => "staging",
        DevStage::Prod => "prod",
    }
}

fn dev_from_row(r: &Row) -> Result<DevState> {
    let stage: String = r.get(2)?;
    let updated_at: String = r.get(6)?;
    Ok(DevState {
        workspace: r.get(0)?,
        identifier: r.get(1)?,
        stage: DevStage::parse(&stage)
            .ok_or_else(|| Error::Corrupt(format!("unknown dev stage {stage:?}")))?,
        git_ref: r.get(3)?,
        path: r.get(4)?,
        url: r.get(5)?,
        updated_at: text_to_time(&updated_at)?,
    })
}

/// Inserts or replaces the dev state of one issue.
pub fn set_dev(db: &Db, dev: &DevState) -> Result<()> {
    db.conn.execute(
        &format!(
            "INSERT INTO dev_state ({DEV_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (workspace, identifier) DO UPDATE SET
                stage = excluded.stage, git_ref = excluded.git_ref, path = excluded.path,
                url = excluded.url, updated_at = excluded.updated_at"
        ),
        params![
            dev.workspace,
            dev.identifier,
            stage_to_text(dev.stage),
            dev.git_ref,
            dev.path,
            dev.url,
            time_to_text(&dev.updated_at),
        ],
    )?;
    Ok(())
}

/// The identifier match ignores case.
pub fn get_dev(db: &Db, ws: &str, identifier: &str) -> Result<Option<DevState>> {
    query_one(
        db,
        &format!("SELECT {DEV_COLS} FROM dev_state WHERE workspace = ?1 AND identifier = ?2"),
        [ws, identifier],
        dev_from_row,
    )
}

pub fn clear_dev(db: &Db, ws: &str, identifier: &str) -> Result<()> {
    db.conn.execute(
        "DELETE FROM dev_state WHERE workspace = ?1 AND identifier = ?2",
        [ws, identifier],
    )?;
    Ok(())
}

/// All dev rows of one workspace.
pub fn list_dev(db: &Db, ws: &str) -> Result<Vec<DevState>> {
    query_all(
        db,
        &format!("SELECT {DEV_COLS} FROM dev_state WHERE workspace = ?1 ORDER BY identifier"),
        [ws],
        dev_from_row,
    )
}

/// Finds dev rows by branch or path. A row matches when its ref equals `git_ref`,
/// or when `path` is its stored path or inside it. Best match first: both match,
/// then the deepest stored path, then the newest. Empty when both are None.
pub fn find_dev(db: &Db, git_ref: Option<&str>, path: Option<&str>) -> Result<Vec<DevState>> {
    let git_ref = git_ref.map(str::trim).filter(|r| !r.is_empty());
    let path = path.map(str::trim).filter(|p| !p.is_empty()).map(path_parts);
    if git_ref.is_none() && path.is_none() {
        return Ok(Vec::new());
    }
    let rows = query_all(
        db,
        &format!(
            "SELECT {DEV_COLS} FROM dev_state WHERE git_ref = ?1 OR path IS NOT NULL
             ORDER BY updated_at DESC"
        ),
        [git_ref],
        dev_from_row,
    )?;
    let mut scored: Vec<(bool, usize, DevState)> = rows
        .into_iter()
        .filter_map(|d| {
            let ref_hit = git_ref.is_some() && d.git_ref.as_deref() == git_ref;
            let path_depth = match (&path, d.path.as_deref()) {
                (Some(given), Some(stored)) => {
                    let stored = path_parts(stored);
                    given.starts_with(&stored).then_some(stored.len())
                }
                _ => None,
            };
            let both = ref_hit && path_depth.is_some();
            (ref_hit || path_depth.is_some()).then(|| (both, path_depth.unwrap_or(0), d))
        })
        .collect();
    // Stable sort keeps the newest-first order for ties.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    Ok(scored.into_iter().map(|(_, _, d)| d).collect())
}

/// Splits a path into normal components. Drops empty parts and "." and resolves "..",
/// so "/a//b/./c/" and "/a/b/c" compare equal. An absolute path starts with a "/" part,
/// so it never matches a relative one.
fn path_parts(path: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    if path.starts_with('/') {
        parts.push("/".to_string());
    }
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => match parts.last().map(String::as_str) {
                Some("/") => {}
                Some("..") | None => parts.push("..".to_string()),
                Some(_) => {
                    parts.pop();
                }
            },
            p => parts.push(p.to_string()),
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::path_parts;

    #[test]
    fn normalizes_paths() {
        assert_eq!(path_parts("/a//b/./c/"), path_parts("/a/b/c"));
        assert_eq!(path_parts("/a/b/../c"), path_parts("/a/c"));
        assert_eq!(path_parts("/.."), path_parts("/"));
        assert_ne!(path_parts("a/b"), path_parts("/a/b"));
        assert_eq!(path_parts("/"), vec!["/".to_string()]);
    }
}
