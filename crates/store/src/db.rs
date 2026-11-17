use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, Params, Row};

use crate::error::{Error, Result};
use crate::migrate;

/// One connection to the cache file.
pub struct Db {
    pub(crate) conn: Connection,
}

/// `$KANBANANA_DB`, else `<data_dir>/kanbanana/cache.sqlite3`.
pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("KANBANANA_DB").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    crate::move_old_folders();
    let dir = dirs::data_dir().ok_or(Error::NoDataDir)?;
    Ok(dir.join("kanbanana").join("cache.sqlite3"))
}

pub fn open_default() -> Result<Db> {
    open_at(default_path()?)
}

/// Opens the file, creates it and its parent folders when missing, and runs migrations.
pub fn open_at(path: impl AsRef<Path>) -> Result<Db> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    // Set the timeout first. The switch to WAL needs a lock that the other process can hold.
    conn.busy_timeout(Duration::from_secs(5))?;
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    let db = Db { conn };
    migrate::run(&db)?;
    Ok(db)
}

/// Changes when another connection commits a write. The GUI polls it to see MCP writes.
pub fn data_version(db: &Db) -> Result<i64> {
    Ok(db.conn.query_row("PRAGMA data_version", [], |r| r.get(0))?)
}

pub(crate) fn time_to_text(t: &DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub(crate) fn text_to_time(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| Error::Corrupt(format!("bad timestamp {s:?}: {e}")))
}

pub(crate) fn opt_time(s: Option<String>) -> Result<Option<DateTime<Utc>>> {
    s.as_deref().map(text_to_time).transpose()
}

pub(crate) fn query_all<T>(
    db: &Db,
    sql: &str,
    params: impl Params,
    map: impl Fn(&Row) -> Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = db.conn.prepare_cached(sql)?;
    let mut rows = stmt.query(params)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(map(row)?);
    }
    Ok(out)
}

pub(crate) fn query_one<T>(
    db: &Db,
    sql: &str,
    params: impl Params,
    map: impl Fn(&Row) -> Result<T>,
) -> Result<Option<T>> {
    let mut stmt = db.conn.prepare_cached(sql)?;
    let mut rows = stmt.query(params)?;
    match rows.next()? {
        Some(row) => Ok(Some(map(row)?)),
        None => Ok(None),
    }
}
