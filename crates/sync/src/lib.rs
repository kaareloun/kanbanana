//! Background engine. The GUI sends `Command`s and drains `Event`s every frame.
//! The GUI never waits on the network or on the store writes it asks for.
//!
//! Threads: one worker owns its own `store::Db` and runs syncs and store writes in order.
//! A pool of 2 threads runs Db-free network work (images, uploads, key checks),
//! so that work never waits behind a sync.

mod clients;
mod error;
mod gate;
mod images;
mod keys;
mod pool;
mod pull;
mod queue;
mod worker;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use chrono::{DateTime, Utc};
use crossbeam_channel::{Receiver, Sender, unbounded};

pub use keys::suggest_slug;

use clients::Clients;
use gate::Gate;

const POOL_THREADS: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// The workspace on screen. The worker syncs it every 30 s. It does not sync at once.
    SetActive(Option<String>),
    /// Window focus, refresh key, workspace switch. Queued syncs of one workspace run once.
    SyncNow { workspace: String },
    ChangeIssue { workspace: String, issue_id: String, change: model::IssueChange },
    /// Creates an issue. `request_id` comes back in `IssueCreated` or `CreateFailed`.
    CreateIssue { workspace: String, request_id: u64, draft: model::IssueDraft },
    LoadComments { workspace: String, issue_id: String },
    CreateComment { workspace: String, issue_id: String, body: String, parent_id: Option<String> },
    UpdateComment { workspace: String, issue_id: String, comment_id: String, body: String },
    Upload { workspace: String, request_id: u64, filename: String, content_type: String, bytes: Vec<u8> },
    FetchImage { workspace: String, url: String },
    CheckKey { key: String },
    AddWorkspace { workspace: model::Workspace, api_key: String },
    RemoveWorkspace(String),
    Shutdown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    SyncStarted { workspace: String },
    /// Reload the board from the store.
    Synced { workspace: String, at: DateTime<Utc> },
    SyncFailed { workspace: String, message: String },
    RateLimited { workspace: String, until: DateTime<Utc> },
    /// A store write the GUI must reload for. Also sent for writes by other processes.
    BoardChanged { workspace: String },
    ChangeFailed { workspace: String, issue_id: String, message: String },
    /// The store has the new issue. A `BoardChanged` follows.
    IssueCreated { workspace: String, request_id: u64, issue: model::Issue },
    CreateFailed { workspace: String, request_id: u64, message: String },
    Comments { issue_id: String, comments: Vec<model::Comment> },
    CommentsFailed { issue_id: String, message: String },
    CommentSaved { issue_id: String, comment: model::Comment },
    CommentFailed { issue_id: String, message: String },
    Uploaded { request_id: u64, asset_url: String },
    UploadFailed { request_id: u64, message: String },
    Image { url: String, bytes: Arc<[u8]> },
    ImageFailed { url: String, message: String },
    KeyChecked(Result<KeyInfo, String>),
    WorkspacesChanged,
}

/// What a new API key gives access to. The add-workspace dialog shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyInfo {
    pub user_name: String,
    pub organization_name: String,
    pub team_names: Vec<String>,
    pub suggested_slug: String,
}

/// Sends events to the GUI and wakes it after each one.
#[derive(Clone)]
pub(crate) struct Emitter {
    tx: Sender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Emitter {
    pub(crate) fn emit(&self, event: Event) {
        // A send error means the GUI dropped the engine. Nothing is left to tell.
        let _ = self.tx.send(event);
        (self.wake)();
    }
}

/// State that the worker and the pool share.
pub(crate) struct Shared {
    pub clients: Clients,
    pub gate: Mutex<Gate>,
    pub out: Emitter,
}

pub struct Engine {
    worker_tx: Sender<Command>,
    pool_tx: Sender<pool::Job>,
    events: Receiver<Event>,
    handles: Vec<JoinHandle<()>>,
}

/// Starts the worker and the pool. `wake` is the GUI's `ctx.request_repaint`.
pub fn start(db_path: PathBuf, wake: Box<dyn Fn() + Send + Sync>) -> Engine {
    let (event_tx, events) = unbounded();
    let shared = Arc::new(Shared {
        clients: Clients::default(),
        gate: Mutex::new(Gate::default()),
        out: Emitter { tx: event_tx, wake: Arc::from(wake) },
    });
    let (worker_tx, worker_rx) = unbounded();
    let (pool_tx, pool_rx) = unbounded();
    let mut handles = Vec::new();
    let s = shared.clone();
    handles.push(spawn("sync-worker", move || worker::run(db_path, worker_rx, s)));
    let images = Arc::new(images::ImageState::default());
    for i in 0..POOL_THREADS {
        let (s, rx, images) = (shared.clone(), pool_rx.clone(), images.clone());
        handles.push(spawn(&format!("sync-pool-{i}"), move || pool::run(rx, s, images)));
    }
    Engine { worker_tx, pool_tx, events, handles }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    match std::thread::Builder::new().name(name.to_string()).spawn(f) {
        Ok(h) => h,
        // Without its threads the engine cannot work at all.
        Err(e) => panic!("sync: cannot start thread {name}: {e}"),
    }
}

impl Engine {
    /// Never blocks. Images, uploads and key checks go to the pool, the rest to the worker.
    pub fn send(&self, cmd: Command) {
        let sent = match cmd {
            Command::FetchImage { workspace, url } => {
                self.pool_tx.send(pool::Job::Image { workspace, url }).is_ok()
            }
            Command::Upload { workspace, request_id, filename, content_type, bytes } => self
                .pool_tx
                .send(pool::Job::Upload { workspace, request_id, filename, content_type, bytes })
                .is_ok(),
            Command::CheckKey { key } => self.pool_tx.send(pool::Job::CheckKey { key }).is_ok(),
            Command::Shutdown => {
                self.stop();
                true
            }
            other => self.worker_tx.send(other).is_ok(),
        };
        if !sent {
            eprintln!("sync: engine is stopped, command dropped");
        }
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Stops the threads and waits for them. A running network call can take up to 2 min.
    pub fn shutdown(mut self) {
        self.stop();
        for h in std::mem::take(&mut self.handles) {
            let _ = h.join();
        }
    }

    fn stop(&self) {
        let _ = self.worker_tx.send(Command::Shutdown);
        for _ in 0..POOL_THREADS {
            let _ = self.pool_tx.send(pool::Job::Stop);
        }
    }
}

impl Drop for Engine {
    /// Does not join. The process can exit during a network call, SQLite stays consistent.
    fn drop(&mut self) {
        self.stop();
    }
}
