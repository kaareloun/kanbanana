use model::{Label, Project, StateType, Team, User, WorkflowState};
use rusqlite::params;

use crate::db::{Db, query_all};
use crate::error::Result;

/// Teams, workflow states, projects, users and labels of one workspace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    pub teams: Vec<Team>,
    pub states: Vec<WorkflowState>,
    pub projects: Vec<Project>,
    pub users: Vec<User>,
    pub labels: Vec<Label>,
}

fn kind_to_text(k: StateType) -> &'static str {
    match k {
        StateType::Triage => "triage",
        StateType::Backlog => "backlog",
        StateType::Unstarted => "unstarted",
        StateType::Started => "started",
        StateType::Completed => "completed",
        StateType::Canceled => "canceled",
    }
}

/// Swaps the workspace's catalog for a new one in one transaction.
/// `WorkflowState` has no workspace field, so its rows take `ws`.
pub fn replace_catalog(db: &Db, ws: &str, catalog: &Catalog) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    for table in ["teams", "states", "projects", "users", "labels"] {
        tx.execute(&format!("DELETE FROM {table} WHERE workspace = ?1"), [ws])?;
    }
    // INSERT OR REPLACE: one Linear org added under two slugs must not fail the sync.
    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO teams (id, workspace, key, name) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for t in &catalog.teams {
            stmt.execute(params![t.id, ws, t.key, t.name])?;
        }
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO states (id, workspace, team_id, name, kind, position, color)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for s in &catalog.states {
            stmt.execute(params![s.id, ws, s.team_id, s.name, kind_to_text(s.kind), s.position, s.color])?;
        }
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO projects (id, workspace, name) VALUES (?1, ?2, ?3)",
        )?;
        for p in &catalog.projects {
            stmt.execute(params![p.id, ws, p.name])?;
        }
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO users (id, workspace, name, display_name, avatar_url, active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for u in &catalog.users {
            stmt.execute(params![u.id, ws, u.name, u.display_name, u.avatar_url, u.active])?;
        }
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO labels (id, workspace, name, color, team_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for l in &catalog.labels {
            stmt.execute(params![l.id, ws, l.name, l.color, l.team_id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Reads the catalog of one workspace. Rows keep the workspace they were stored under.
pub fn load_catalog(db: &Db, ws: &str) -> Result<Catalog> {
    let teams = query_all(
        db,
        "SELECT id, workspace, key, name FROM teams WHERE workspace = ?1 ORDER BY key",
        [ws],
        |r| {
            Ok(Team {
                id: r.get(0)?,
                workspace: r.get(1)?,
                key: r.get(2)?,
                name: r.get(3)?,
            })
        },
    )?;
    let states = query_all(
        db,
        "SELECT id, team_id, name, kind, position, color FROM states WHERE workspace = ?1
         ORDER BY team_id, position",
        [ws],
        |r| {
            let kind: String = r.get(3)?;
            Ok(WorkflowState {
                id: r.get(0)?,
                team_id: r.get(1)?,
                name: r.get(2)?,
                kind: StateType::from_linear(&kind),
                position: r.get(4)?,
                color: r.get(5)?,
            })
        },
    )?;
    let projects = query_all(
        db,
        "SELECT id, workspace, name FROM projects WHERE workspace = ?1 ORDER BY name",
        [ws],
        |r| {
            Ok(Project {
                id: r.get(0)?,
                workspace: r.get(1)?,
                name: r.get(2)?,
            })
        },
    )?;
    let users = query_all(
        db,
        "SELECT id, workspace, name, display_name, avatar_url, active FROM users
         WHERE workspace = ?1 ORDER BY name",
        [ws],
        |r| {
            Ok(User {
                id: r.get(0)?,
                workspace: r.get(1)?,
                name: r.get(2)?,
                display_name: r.get(3)?,
                avatar_url: r.get(4)?,
                active: r.get(5)?,
            })
        },
    )?;
    let labels = query_all(
        db,
        "SELECT id, workspace, name, color, team_id FROM labels WHERE workspace = ?1 ORDER BY name",
        [ws],
        |r| {
            Ok(Label {
                id: r.get(0)?,
                workspace: r.get(1)?,
                name: r.get(2)?,
                color: r.get(3)?,
                team_id: r.get(4)?,
            })
        },
    )?;
    Ok(Catalog { teams, states, projects, users, labels })
}
