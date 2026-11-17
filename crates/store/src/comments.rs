use model::Comment;
use rusqlite::{Connection, Row, params};

use crate::db::{Db, query_all, text_to_time, time_to_text};
use crate::error::Result;

const UPSERT: &str = "INSERT INTO comments
    (id, issue_id, parent_id, user_id, body, created_at, updated_at, edited)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
    ON CONFLICT (id) DO UPDATE SET
        issue_id = excluded.issue_id, parent_id = excluded.parent_id,
        user_id = excluded.user_id, body = excluded.body, created_at = excluded.created_at,
        updated_at = excluded.updated_at, edited = excluded.edited";

fn write_comment(conn: &Connection, c: &Comment) -> Result<()> {
    conn.prepare_cached(UPSERT)?.execute(params![
        c.id,
        c.issue_id,
        c.parent_id,
        c.user_id,
        c.body,
        time_to_text(&c.created_at),
        time_to_text(&c.updated_at),
        c.edited,
    ])?;
    Ok(())
}

fn comment_from_row(r: &Row) -> Result<Comment> {
    let created_at: String = r.get(5)?;
    let updated_at: String = r.get(6)?;
    Ok(Comment {
        id: r.get(0)?,
        issue_id: r.get(1)?,
        parent_id: r.get(2)?,
        user_id: r.get(3)?,
        body: r.get(4)?,
        created_at: text_to_time(&created_at)?,
        updated_at: text_to_time(&updated_at)?,
        edited: r.get(7)?,
    })
}

/// Swaps the cached comments of an issue for a fresh list.
/// Also sets the issue's comment count to the list length, so the card matches the panel.
pub fn replace_comments(db: &Db, issue_id: &str, comments: &[Comment]) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    tx.execute("DELETE FROM comments WHERE issue_id = ?1", [issue_id])?;
    for c in comments {
        write_comment(&tx, c)?;
    }
    tx.execute(
        "UPDATE issues SET comment_count = ?1 WHERE id = ?2",
        params![comments.len() as i64, issue_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// Oldest first, so replies follow their parent.
pub fn list_comments(db: &Db, issue_id: &str) -> Result<Vec<Comment>> {
    query_all(
        db,
        "SELECT id, issue_id, parent_id, user_id, body, created_at, updated_at, edited
         FROM comments WHERE issue_id = ?1 ORDER BY created_at, id",
        [issue_id],
        comment_from_row,
    )
}

/// Inserts or updates one comment. A new comment adds 1 to the issue's comment count.
pub fn upsert_comment(db: &Db, comment: &Comment) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    let known: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM comments WHERE id = ?1)",
        [&comment.id],
        |r| r.get(0),
    )?;
    write_comment(&tx, comment)?;
    if !known {
        tx.execute(
            "UPDATE issues SET comment_count = comment_count + 1 WHERE id = ?1",
            [&comment.issue_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}
