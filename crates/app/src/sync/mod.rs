//! Link to the background engine (crate `sync`) and a read handle on the store.
//! The engine does all network work and all writes. The GUI only reads the store.

use eframe::egui;
use model::{BoardData, Comment, Workspace};

pub use ::sync::{Command, Event};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SyncStatus {
    #[default]
    Idle,
    Syncing,
    Error(String),
}

#[derive(Default)]
pub struct Sync {
    pub status: SyncStatus,
    engine: Option<::sync::Engine>,
    db: Option<store::Db>,
}

impl Sync {
    /// Opens the store and starts the engine. On failure the app still runs and shows the error.
    pub fn start(ctx: &egui::Context) -> Sync {
        let path = match store::default_path() {
            Ok(p) => p,
            Err(e) => return Sync::failed(format!("No data folder: {e}")),
        };
        let db = match store::open_at(&path) {
            Ok(db) => db,
            Err(e) => return Sync::failed(format!("Cannot open the cache: {e}")),
        };
        let ctx = ctx.clone();
        let engine = ::sync::start(path, Box::new(move || ctx.request_repaint()));
        Sync { status: SyncStatus::Idle, engine: Some(engine), db: Some(db) }
    }

    fn failed(message: String) -> Sync {
        Sync { status: SyncStatus::Error(message), engine: None, db: None }
    }

    pub fn send(&self, cmd: Command) {
        if let Some(e) = &self.engine {
            e.send(cmd);
        }
    }

    pub fn events(&self) -> Vec<Event> {
        let mut out = Vec::new();
        if let Some(e) = &self.engine {
            while let Some(ev) = e.try_recv() {
                out.push(ev);
            }
        }
        out
    }

    pub fn workspaces(&self) -> Vec<Workspace> {
        self.db.as_ref().and_then(|db| store::list_workspaces(db).ok()).unwrap_or_default()
    }

    pub fn board(&self, ws: &str) -> Option<BoardData> {
        let db = self.db.as_ref()?;
        match store::load_board(db, ws) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("kanbanana: load board {ws}: {e}");
                None
            }
        }
    }

    pub fn cached_comments(&self, issue_id: &str) -> Vec<Comment> {
        self.db.as_ref().and_then(|db| store::list_comments(db, issue_id).ok()).unwrap_or_default()
    }
}
