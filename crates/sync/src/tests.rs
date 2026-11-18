//! Engine tests on a temp database. No test calls Linear or reads the real key file.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use model::*;

use crate::*;

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = Utc::now().timestamp_nanos_opt().unwrap();
        TempDir(std::env::temp_dir().join(format!("kanbanana-sync-test-{}-{n}-{nanos}", std::process::id())))
    }

    fn db_path(&self) -> PathBuf {
        self.0.join("cache.sqlite3")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn local_task(id: &str) -> Issue {
    let t = Utc.timestamp_opt(1_760_000_000, 0).unwrap();
    Issue {
        id: id.into(),
        source: Source::Local,
        workspace: "acme".into(),
        identifier: "LOC-1".into(),
        title: "Local task".into(),
        description: String::new(),
        team_id: "team-1".into(),
        state_id: "state-todo".into(),
        project_id: None,
        assignee_id: None,
        priority: Priority::None,
        label_ids: vec![],
        sort_order: 1.0,
        comment_count: 0,
        url: None,
        created_at: t,
        updated_at: t,
    }
}

/// A database with workspace "acme" and one local task.
fn setup(dir: &TempDir) -> store::Db {
    let db = store::open_at(dir.db_path()).unwrap();
    store::save_workspace(
        &db,
        &Workspace {
            slug: "acme".into(),
            name: "Acme".into(),
            shortcut: 1,
            accent: Accent::Foam,
            viewer_id: None,
            position: 0,
        },
    )
    .unwrap();
    store::upsert_issues(&db, &[local_task("local:1")]).unwrap();
    db
}

fn wait_for(engine: &Engine, timeout: Duration, want: impl Fn(&Event) -> bool) -> Vec<Event> {
    let end = Instant::now() + timeout;
    let mut seen = Vec::new();
    while Instant::now() < end {
        match engine.try_recv() {
            Some(e) => {
                let done = want(&e);
                seen.push(e);
                if done {
                    return seen;
                }
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    panic!("event not seen in {timeout:?}, got {seen:?}");
}

fn collect(engine: &Engine, d: Duration) -> Vec<Event> {
    let end = Instant::now() + d;
    let mut seen = Vec::new();
    while Instant::now() < end {
        match engine.try_recv() {
            Some(e) => seen.push(e),
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    seen
}

fn board_changed(e: &Event) -> bool {
    matches!(e, Event::BoardChanged { workspace } if workspace == "acme")
}

fn change_title(engine: &Engine, title: &str) {
    engine.send(Command::ChangeIssue {
        workspace: "acme".into(),
        issue_id: "local:1".into(),
        change: IssueChange::Title(title.into()),
    });
}

#[test]
fn local_change_writes_store_once_without_network() {
    let dir = TempDir::new();
    let db = setup(&dir);
    let engine = start(dir.db_path(), Box::new(|| {}));
    engine.send(Command::SetActive(Some("acme".into())));
    change_title(&engine, "Renamed");
    wait_for(&engine, Duration::from_secs(5), board_changed);
    assert_eq!(store::get_issue(&db, "local:1").unwrap().unwrap().title, "Renamed");

    // The worker's own write must not come back as an external change.
    let later = collect(&engine, Duration::from_millis(1500));
    assert!(later.is_empty(), "{later:?}");
    engine.shutdown();
}

#[test]
fn change_of_unknown_issue_fails() {
    let dir = TempDir::new();
    let _db = setup(&dir);
    let engine = start(dir.db_path(), Box::new(|| {}));
    engine.send(Command::ChangeIssue {
        workspace: "acme".into(),
        issue_id: "nope".into(),
        change: IssueChange::Title("x".into()),
    });
    let seen = wait_for(&engine, Duration::from_secs(5), |e| matches!(e, Event::ChangeFailed { .. }));
    assert!(!seen.iter().any(board_changed), "{seen:?}");
    engine.shutdown();
}

#[test]
fn external_write_emits_board_changed_for_active_workspace() {
    let dir = TempDir::new();
    let db = setup(&dir);
    let engine = start(dir.db_path(), Box::new(|| {}));
    engine.send(Command::SetActive(Some("acme".into())));
    // A first round trip makes sure the worker runs and has read data_version.
    change_title(&engine, "Warm up");
    wait_for(&engine, Duration::from_secs(5), board_changed);
    assert!(collect(&engine, Duration::from_millis(300)).is_empty());

    // Another handle stands in for the MCP process.
    let mut task = store::get_issue(&db, "local:1").unwrap().unwrap();
    task.title = "Set by MCP".into();
    store::upsert_issues(&db, &[task]).unwrap();
    wait_for(&engine, Duration::from_secs(3), board_changed);
    let later = collect(&engine, Duration::from_millis(1500));
    assert!(later.is_empty(), "one event per external write, got {later:?}");
    engine.shutdown();
}

#[test]
fn local_comments_load_from_store() {
    let dir = TempDir::new();
    let _db = setup(&dir);
    let engine = start(dir.db_path(), Box::new(|| {}));
    engine.send(Command::LoadComments { workspace: "acme".into(), issue_id: "local:1".into() });
    let seen = wait_for(&engine, Duration::from_secs(5), |e| matches!(e, Event::Comments { .. }));
    assert_eq!(
        seen.last(),
        Some(&Event::Comments { issue_id: "local:1".into(), comments: vec![] })
    );
    engine.send(Command::CreateComment {
        workspace: "acme".into(),
        issue_id: "local:1".into(),
        body: "hi".into(),
        parent_id: None,
    });
    wait_for(&engine, Duration::from_secs(5), |e| matches!(e, Event::CommentFailed { .. }));
    engine.shutdown();
}

#[test]
fn wake_runs_after_each_event() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    let dir = TempDir::new();
    let _db = setup(&dir);
    let wakes = Arc::new(AtomicUsize::new(0));
    let w = wakes.clone();
    let engine = start(dir.db_path(), Box::new(move || {
        w.fetch_add(1, Ordering::SeqCst);
    }));
    change_title(&engine, "Woken");
    wait_for(&engine, Duration::from_secs(5), board_changed);
    assert!(wakes.load(Ordering::SeqCst) >= 1);
    engine.shutdown();
}

#[test]
fn create_without_database_fails_with_request_id() {
    let dir = TempDir::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    // A file where the database folder should be, so the store cannot open.
    let blocker = dir.0.join("file");
    std::fs::write(&blocker, b"x").unwrap();
    let engine = start(blocker.join("cache.sqlite3"), Box::new(|| {}));
    let draft = IssueDraft { team_id: "team-1".into(), title: "New".into(), ..Default::default() };
    engine.send(Command::CreateIssue { workspace: "acme".into(), request_id: 7, draft });
    let seen = wait_for(&engine, Duration::from_secs(5), |e| matches!(e, Event::CreateFailed { .. }));
    assert!(matches!(seen.last(), Some(Event::CreateFailed { request_id: 7, .. })), "{seen:?}");
    engine.shutdown();
}
