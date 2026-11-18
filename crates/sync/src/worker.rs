//! The main worker. It owns its own `Db` and runs syncs and store writes in order.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use linear_api::Client;
use model::{IssueChange, IssueDraft, Source, Workspace};
use store::Db;

use crate::error::{Error, Result};
use crate::gate::timer_interval;
use crate::pool::lock_gate;
use crate::pull::{PullState, pull};
use crate::queue::{self, Job};
use crate::{Command, Event, Shared};

const POLL: Duration = Duration::from_secs(1);

struct Worker {
    db: Db,
    shared: Arc<Shared>,
    queue: VecDeque<Job>,
    active: Option<String>,
    pull: PullState,
    /// User syncs that wait for the rate limit gate to open.
    deferred: HashSet<String>,
    next_timer: Instant,
    next_poll: Instant,
    data_version: Option<i64>,
}

pub(crate) fn run(db_path: PathBuf, rx: Receiver<Command>, shared: Arc<Shared>) {
    let db = match store::open_at(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("sync: cannot open {}: {e}", db_path.display());
            return run_without_db(rx, &shared, &e.to_string());
        }
    };
    let now = Instant::now();
    let mut w = Worker {
        db,
        shared,
        queue: VecDeque::new(),
        active: None,
        pull: PullState::default(),
        deferred: HashSet::new(),
        next_timer: now + timer_interval(None),
        next_poll: now + POLL,
        data_version: None,
    };
    w.poll_version();
    loop {
        while let Ok(cmd) = rx.try_recv() {
            w.accept(cmd);
        }
        if let Some(job) = w.queue.pop_front() {
            if !w.run_job(job) {
                return;
            }
        } else {
            let wait = w.next_poll.min(w.next_timer).saturating_duration_since(Instant::now());
            match rx.recv_timeout(wait) {
                Ok(cmd) => w.accept(cmd),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        w.tick();
    }
}

/// Answers commands with failure events, so the GUI does not wait forever.
fn run_without_db(rx: Receiver<Command>, shared: &Shared, message: &str) {
    let message = format!("cannot open the database: {message}");
    while let Ok(cmd) = rx.recv() {
        let event = match cmd {
            Command::SyncNow { workspace } => Event::SyncFailed { workspace, message: message.clone() },
            Command::ChangeIssue { workspace, issue_id, .. } => {
                Event::ChangeFailed { workspace, issue_id, message: message.clone() }
            }
            Command::CreateIssue { workspace, request_id, .. } => {
                Event::CreateFailed { workspace, request_id, message: message.clone() }
            }
            Command::LoadComments { issue_id, .. } => {
                Event::CommentsFailed { issue_id, message: message.clone() }
            }
            Command::CreateComment { issue_id, .. } | Command::UpdateComment { issue_id, .. } => {
                Event::CommentFailed { issue_id, message: message.clone() }
            }
            Command::Shutdown => return,
            _ => continue,
        };
        shared.out.emit(event);
    }
}

impl Worker {
    fn accept(&mut self, cmd: Command) {
        // Cached comments go out before the queue runs, so the panel fills at once.
        if let Command::LoadComments { issue_id, .. } = &cmd {
            match store::list_comments(&self.db, issue_id) {
                Ok(list) if !list.is_empty() => {
                    self.emit(Event::Comments { issue_id: issue_id.clone(), comments: list })
                }
                Ok(_) => {}
                Err(e) => eprintln!("sync: cannot read cached comments of {issue_id}: {e}"),
            }
        }
        queue::push(&mut self.queue, cmd);
    }

    /// Returns false on Shutdown.
    fn run_job(&mut self, job: Job) -> bool {
        match job {
            Job::Sync { workspace, user } => self.sync(&workspace, user),
            Job::Cmd(cmd) => match cmd {
                Command::SetActive(ws) => {
                    self.active = ws;
                    self.next_timer = Instant::now() + self.interval();
                }
                Command::ChangeIssue { workspace, issue_id, change } => {
                    self.change(&workspace, &issue_id, change)
                }
                Command::CreateIssue { workspace, request_id, draft } => {
                    self.create(&workspace, request_id, &draft)
                }
                Command::LoadComments { workspace, issue_id } => {
                    self.load_comments(&workspace, &issue_id)
                }
                Command::CreateComment { workspace, issue_id, body, parent_id } => {
                    let r = self.check_comment_target(&issue_id).and_then(|()| {
                        self.call(&workspace, |c| c.create_comment(&issue_id, &body, parent_id.as_deref()))
                    });
                    self.comment_saved(&workspace, &issue_id, r, true);
                }
                Command::UpdateComment { workspace, issue_id, comment_id, body } => {
                    let r = self.check_comment_target(&issue_id).and_then(|()| {
                        self.call(&workspace, |c| c.update_comment(&comment_id, &body))
                    });
                    self.comment_saved(&workspace, &issue_id, r, false);
                }
                Command::AddWorkspace { workspace, api_key } => self.add_workspace(&workspace, &api_key),
                Command::RemoveWorkspace(slug) => self.remove_workspace(&slug),
                Command::Shutdown => return false,
                // Engine::send routes these to the pool. A sync job comes from queue::push.
                Command::SyncNow { .. }
                | Command::Upload { .. }
                | Command::FetchImage { .. }
                | Command::CheckKey { .. } => eprintln!("sync: worker got a pool command"),
            },
        }
        true
    }

    fn tick(&mut self) {
        let now = Instant::now();
        if now >= self.next_poll {
            self.poll_version();
            self.next_poll = now + POLL;
        }
        if now >= self.next_timer {
            if let Some(ws) = self.active.clone()
                && self.gated(&ws).is_none()
            {
                queue::push_sync(&mut self.queue, &ws, false);
            }
            self.next_timer = now + self.interval();
        }
        if !self.deferred.is_empty() {
            let open: Vec<String> =
                self.deferred.iter().filter(|ws| self.gated(ws).is_none()).cloned().collect();
            for ws in open {
                self.deferred.remove(&ws);
                queue::push_sync(&mut self.queue, &ws, true);
            }
        }
    }

    fn interval(&self) -> Duration {
        let remaining = self
            .active
            .as_deref()
            .and_then(|ws| self.shared.clients.cached(ws))
            .and_then(|c| c.rate_limit_remaining());
        timer_interval(remaining)
    }

    /// `data_version` changes only for commits by other connections, so the worker's
    /// own writes never trigger this event.
    fn poll_version(&mut self) {
        match store::data_version(&self.db) {
            Ok(v) => {
                if self.data_version.is_some_and(|old| old != v)
                    && let Some(ws) = self.active.clone()
                {
                    self.emit(Event::BoardChanged { workspace: ws });
                }
                self.data_version = Some(v);
            }
            Err(e) => eprintln!("sync: cannot read data_version: {e}"),
        }
    }

    fn sync(&mut self, ws: &str, user: bool) {
        if let Some(until) = self.gated(ws) {
            if user {
                self.deferred.insert(ws.to_string());
                self.emit(Event::RateLimited { workspace: ws.to_string(), until });
            }
            return;
        }
        self.emit(Event::SyncStarted { workspace: ws.to_string() });
        let result = self.shared.clients.get(ws).and_then(|c| pull(&self.db, &c, ws, &mut self.pull));
        match result {
            Ok(at) => self.emit(Event::Synced { workspace: ws.to_string(), at }),
            Err(e) => {
                eprintln!("sync: sync of {ws} failed: {e}");
                if self.note_rate_limit(ws, &e) && user {
                    self.deferred.insert(ws.to_string());
                }
                self.emit(Event::SyncFailed { workspace: ws.to_string(), message: e.to_string() });
            }
        }
    }

    fn change(&mut self, ws: &str, issue_id: &str, change: IssueChange) {
        let before = match store::get_issue(&self.db, issue_id) {
            Ok(Some(issue)) => issue,
            Ok(None) => return self.change_failed(ws, issue_id, Error::NoIssue(issue_id.to_string())),
            Err(e) => return self.change_failed(ws, issue_id, e.into()),
        };
        if let Err(e) = store::apply_change(&self.db, issue_id, &change) {
            return self.change_failed(ws, issue_id, e.into());
        }
        self.board_changed(ws);
        if before.source == Source::Local {
            return;
        }
        match self.call(ws, |c| c.update_issue(ws, issue_id, std::slice::from_ref(&change))) {
            Ok(saved) => {
                if let Err(e) = store::upsert_issues(&self.db, &[saved]) {
                    eprintln!("sync: cannot store saved issue {issue_id}: {e}");
                }
                self.board_changed(ws);
            }
            Err(e) => {
                self.change_failed(ws, issue_id, e);
                // Linear has the truth. The copy from before the edit is the fallback.
                let fresh = match self.gated(ws) {
                    Some(_) => None,
                    None => self
                        .call(ws, |c| c.issue(ws, issue_id))
                        .map_err(|e| eprintln!("sync: cannot reload issue {issue_id}: {e}"))
                        .ok(),
                };
                let restored = fresh.unwrap_or(before);
                if let Err(e) = store::upsert_issues(&self.db, &[restored]) {
                    eprintln!("sync: cannot restore issue {issue_id}: {e}");
                }
                self.board_changed(ws);
            }
        }
    }

    fn create(&mut self, ws: &str, request_id: u64, draft: &IssueDraft) {
        let result = self.call(ws, |c| c.create_issue(ws, draft)).and_then(|issue| {
            store::upsert_issues(&self.db, std::slice::from_ref(&issue))?;
            Ok(issue)
        });
        match result {
            Ok(issue) => {
                self.emit(Event::IssueCreated { workspace: ws.to_string(), request_id, issue });
                self.board_changed(ws);
            }
            Err(e) => {
                eprintln!("sync: create in {ws} failed: {e}");
                self.emit(Event::CreateFailed { workspace: ws.to_string(), request_id, message: e.to_string() });
            }
        }
    }

    fn change_failed(&self, ws: &str, issue_id: &str, e: Error) {
        eprintln!("sync: change of {issue_id} failed: {e}");
        self.emit(Event::ChangeFailed {
            workspace: ws.to_string(),
            issue_id: issue_id.to_string(),
            message: e.to_string(),
        });
    }

    fn load_comments(&mut self, ws: &str, issue_id: &str) {
        let cached = match store::get_issue(&self.db, issue_id) {
            Ok(issue) => issue,
            Err(e) => {
                eprintln!("sync: cannot read issue {issue_id}: {e}");
                None
            }
        };
        if cached.as_ref().is_some_and(|i| i.source == Source::Local) {
            // Local tasks have no Linear comments. The store has all there is.
            let result = store::list_comments(&self.db, issue_id).map_err(Error::from);
            return self.comments_result(issue_id, result);
        }
        let result = self.call(ws, |c| c.comments(issue_id)).and_then(|list| {
            store::replace_comments(&self.db, issue_id, &list)?;
            Ok(list)
        });
        let count_changed = match (&result, &cached) {
            (Ok(list), Some(issue)) => issue.comment_count != list.len() as i64,
            _ => false,
        };
        self.comments_result(issue_id, result);
        if count_changed {
            self.board_changed(ws);
        }
    }

    fn comments_result(&self, issue_id: &str, result: Result<Vec<model::Comment>>) {
        let issue_id = issue_id.to_string();
        match result {
            Ok(comments) => self.emit(Event::Comments { issue_id, comments }),
            Err(e) => {
                eprintln!("sync: comments of {issue_id} failed: {e}");
                self.emit(Event::CommentsFailed { issue_id, message: e.to_string() });
            }
        }
    }

    /// Comments go to Linear. Local tasks cannot take them yet.
    fn check_comment_target(&self, issue_id: &str) -> Result<()> {
        match store::get_issue(&self.db, issue_id)? {
            Some(issue) if issue.source == Source::Local => Err(Error::LocalComment),
            _ => Ok(()),
        }
    }

    fn comment_saved(&self, ws: &str, issue_id: &str, result: Result<model::Comment>, is_new: bool) {
        match result {
            Ok(mut comment) => {
                // commentUpdate can omit the issue. The command knows it.
                if comment.issue_id.is_empty() {
                    comment.issue_id = issue_id.to_string();
                }
                if let Err(e) = store::upsert_comment(&self.db, &comment) {
                    eprintln!("sync: cannot store comment {}: {e}", comment.id);
                }
                self.emit(Event::CommentSaved { issue_id: issue_id.to_string(), comment });
                if is_new {
                    self.board_changed(ws);
                }
            }
            Err(e) => {
                eprintln!("sync: comment on {issue_id} failed: {e}");
                self.emit(Event::CommentFailed { issue_id: issue_id.to_string(), message: e.to_string() });
            }
        }
    }

    fn add_workspace(&mut self, ws: &Workspace, api_key: &str) {
        let key = api_key.trim();
        let saved = store::secrets::set_api_key(&ws.slug, key)
            .and_then(|()| store::save_workspace(&self.db, ws));
        if let Err(e) = saved {
            eprintln!("sync: cannot add workspace {}: {e}", ws.slug);
            // The contract has no event for this. SyncFailed shows the error on that workspace.
            let message = format!("cannot add the workspace: {e}");
            return self.emit(Event::SyncFailed { workspace: ws.slug.clone(), message });
        }
        self.shared.clients.insert(&ws.slug, Client::new(key));
        lock_gate(&self.shared).remove(&ws.slug);
        self.pull.forget(&ws.slug);
        self.emit(Event::WorkspacesChanged);
        queue::push_sync(&mut self.queue, &ws.slug, true);
    }

    fn remove_workspace(&mut self, slug: &str) {
        if let Err(e) = store::delete_workspace(&self.db, slug) {
            eprintln!("sync: cannot remove workspace {slug}: {e}");
        }
        self.shared.clients.remove(slug);
        lock_gate(&self.shared).remove(slug);
        self.deferred.remove(slug);
        self.pull.forget(slug);
        if self.active.as_deref() == Some(slug) {
            self.active = None;
        }
        self.emit(Event::WorkspacesChanged);
    }

    /// Runs one API call through the rate limit gate.
    fn call<T>(&self, ws: &str, f: impl FnOnce(&Client) -> linear_api::Result<T>) -> Result<T> {
        if let Some(until) = self.gated(ws) {
            self.emit(Event::RateLimited { workspace: ws.to_string(), until });
            return Err(Error::Gated(until));
        }
        let client = self.shared.clients.get(ws)?;
        f(&client).map_err(|e| {
            let e = Error::from(e);
            self.note_rate_limit(ws, &e);
            e
        })
    }

    /// Closes the gate when Linear rate limited the call. Returns true then.
    fn note_rate_limit(&self, ws: &str, e: &Error) -> bool {
        let Some(secs) = e.retry_after_secs() else {
            return false;
        };
        let until = lock_gate(&self.shared).block(ws, Utc::now(), secs);
        self.emit(Event::RateLimited { workspace: ws.to_string(), until });
        true
    }

    fn gated(&self, ws: &str) -> Option<chrono::DateTime<Utc>> {
        lock_gate(&self.shared).blocked_until(ws, Utc::now())
    }

    fn board_changed(&self, ws: &str) {
        self.emit(Event::BoardChanged { workspace: ws.to_string() });
    }

    fn emit(&self, event: Event) {
        self.shared.out.emit(event);
    }
}
