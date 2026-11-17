use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{DateTime, TimeZone, Utc};
use model::*;

use crate::*;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A database file in the temp folder. Drop removes it and its WAL files.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new() -> TempDb {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = Utc::now().timestamp_nanos_opt().unwrap();
        let path = std::env::temp_dir()
            .join(format!("kanbanana-store-test-{}-{n}-{nanos}", std::process::id()))
            .join("cache.sqlite3");
        TempDb { path }
    }

    fn open(&self) -> Db {
        open_at(&self.path).unwrap()
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn t(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_760_000_000 + secs, 0).unwrap()
}

fn workspace(slug: &str) -> Workspace {
    Workspace {
        slug: slug.into(),
        name: format!("{slug} name"),
        shortcut: 1,
        accent: Accent::Foam,
        viewer_id: None,
        position: 0,
    }
}

fn issue(ws: &str, id: &str, identifier: &str) -> Issue {
    Issue {
        id: id.into(),
        source: Source::Linear,
        workspace: ws.into(),
        identifier: identifier.into(),
        title: format!("Title {identifier}"),
        description: "Some *markdown*".into(),
        team_id: "team-1".into(),
        state_id: "state-todo".into(),
        project_id: Some("proj-1".into()),
        assignee_id: None,
        priority: Priority::High,
        label_ids: vec!["l1".into(), "l2".into()],
        sort_order: 10.5,
        comment_count: 0,
        url: Some(format!("https://linear.app/x/issue/{identifier}")),
        created_at: t(0),
        updated_at: t(10),
    }
}

fn catalog(ws: &str, suffix: &str) -> Catalog {
    Catalog {
        teams: vec![Team { id: format!("team-{suffix}"), workspace: ws.into(), key: "WEB".into(), name: "Webshop".into() }],
        states: vec![
            WorkflowState {
                id: format!("todo-{suffix}"),
                team_id: format!("team-{suffix}"),
                name: "Todo".into(),
                kind: StateType::Unstarted,
                position: 1.0,
                color: "#aaaaaa".into(),
            },
            WorkflowState {
                id: format!("done-{suffix}"),
                team_id: format!("team-{suffix}"),
                name: "Done".into(),
                kind: StateType::Completed,
                position: 2.0,
                color: "#00ff00".into(),
            },
        ],
        projects: vec![Project { id: format!("proj-{suffix}"), workspace: ws.into(), name: "Shop".into() }],
        users: vec![User {
            id: format!("user-{suffix}"),
            workspace: ws.into(),
            name: "Kaarel".into(),
            display_name: "kaarel".into(),
            avatar_url: None,
            active: true,
        }],
        labels: vec![Label {
            id: format!("label-{suffix}"),
            workspace: ws.into(),
            name: "Bug".into(),
            color: "#ff0000".into(),
            team_id: None,
        }],
    }
}

fn dev(ws: &str, identifier: &str, git_ref: Option<&str>, path: Option<&str>, at: i64) -> DevState {
    DevState {
        workspace: ws.into(),
        identifier: identifier.into(),
        stage: DevStage::Mr,
        git_ref: git_ref.map(Into::into),
        path: path.map(Into::into),
        url: None,
        updated_at: t(at),
    }
}

fn comment(id: &str, issue_id: &str, at: i64) -> Comment {
    Comment {
        id: id.into(),
        issue_id: issue_id.into(),
        parent_id: None,
        user_id: Some("user-1".into()),
        body: format!("body {id}"),
        created_at: t(at),
        updated_at: t(at),
        edited: false,
    }
}

#[test]
fn migrations_are_idempotent() {
    let tmp = TempDb::new();
    let db = tmp.open();
    let version: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(version, crate::migrate::STEPS.len() as i64);
    save_workspace(&db, &workspace("acme")).unwrap();
    drop(db);

    let db = tmp.open();
    crate::migrate::run(&db).unwrap();
    let version: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(version, crate::migrate::STEPS.len() as i64);
    assert_eq!(list_workspaces(&db).unwrap().len(), 1);
    let mode: String = db.conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode, "wal");
    let fk: i64 = db.conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn workspaces_save_list_and_cursor() {
    let tmp = TempDb::new();
    let db = tmp.open();
    let mut b = workspace("b");
    b.position = 0;
    let mut a = workspace("a");
    a.position = 1;
    a.accent = Accent::Love;
    save_workspace(&db, &a).unwrap();
    save_workspace(&db, &b).unwrap();
    assert_eq!(list_workspaces(&db).unwrap(), vec![b.clone(), a.clone()]);

    assert_eq!(sync_cursor(&db, "a").unwrap(), None);
    assert_eq!(sync_cursor(&db, "missing").unwrap(), None);
    set_sync_cursor(&db, "a", t(5)).unwrap();
    set_last_sync(&db, "a", t(6)).unwrap();
    set_viewer(&db, "a", "viewer-1").unwrap();
    assert_eq!(sync_cursor(&db, "a").unwrap(), Some(t(5)));
    assert!(matches!(set_viewer(&db, "missing", "x"), Err(Error::NotFound(_))));

    // Saving again keeps the cursor and the last sync time.
    a.name = "Renamed".into();
    a.viewer_id = Some("viewer-1".into());
    save_workspace(&db, &a).unwrap();
    assert_eq!(sync_cursor(&db, "a").unwrap(), Some(t(5)));
    let board = load_board(&db, "a").unwrap();
    assert_eq!(board.viewer_id.as_deref(), Some("viewer-1"));
    assert_eq!(board.last_sync, Some(t(6)));
    assert_eq!(list_workspaces(&db).unwrap()[1].name, "Renamed");
}

#[test]
fn replace_catalog_swaps_only_that_workspace() {
    let tmp = TempDb::new();
    let db = tmp.open();
    replace_catalog(&db, "a", &catalog("a", "a1")).unwrap();
    replace_catalog(&db, "b", &catalog("b", "b1")).unwrap();
    assert_eq!(load_catalog(&db, "a").unwrap(), catalog("a", "a1"));

    replace_catalog(&db, "a", &catalog("a", "a2")).unwrap();
    let a = load_catalog(&db, "a").unwrap();
    assert_eq!(a, catalog("a", "a2"));
    assert_eq!(load_catalog(&db, "b").unwrap(), catalog("b", "b1"));

    replace_catalog(&db, "a", &Catalog::default()).unwrap();
    assert_eq!(load_catalog(&db, "a").unwrap(), Catalog::default());
}

#[test]
fn issues_upsert_find_and_change() {
    let tmp = TempDb::new();
    let db = tmp.open();
    let mut one = issue("acme", "uuid-1", "WEB-1");
    let local = Issue { source: Source::Local, url: None, project_id: None, ..issue("acme", "local:1", "LOC-1") };
    let other = issue("nw", "uuid-2", "WEB-1");
    upsert_issues(&db, &[one.clone(), local.clone(), other.clone()]).unwrap();

    assert_eq!(get_issue(&db, "uuid-1").unwrap(), Some(one.clone()));
    assert_eq!(get_issue(&db, "local:1").unwrap(), Some(local.clone()));
    assert_eq!(get_issue(&db, "nope").unwrap(), None);
    assert_eq!(find_issue(&db, Some("acme"), "web-1").unwrap(), Some(one.clone()));
    assert_eq!(find_issue(&db, Some("nw"), " WEB-1 ").unwrap(), Some(other.clone()));
    assert_eq!(find_issue(&db, None, "loc-1").unwrap(), Some(local.clone()));
    assert_eq!(find_issue(&db, Some("acme"), "WEB-9").unwrap(), None);

    one.title = "New title".into();
    upsert_issues(&db, &[one.clone()]).unwrap();
    assert_eq!(get_issue(&db, "uuid-1").unwrap().unwrap().title, "New title");

    let before = Utc::now() - chrono::Duration::seconds(1);
    apply_change(&db, "uuid-1", &IssueChange::State { state_id: "state-done".into(), sort_order: None }).unwrap();
    apply_change(&db, "uuid-1", &IssueChange::Priority(Priority::Urgent)).unwrap();
    apply_change(&db, "uuid-1", &IssueChange::Labels(vec!["l9".into()])).unwrap();
    apply_change(&db, "uuid-1", &IssueChange::Assignee(Some("user-1".into()))).unwrap();
    apply_change(&db, "uuid-1", &IssueChange::Description("new desc".into())).unwrap();
    let got = get_issue(&db, "uuid-1").unwrap().unwrap();
    assert_eq!(got.state_id, "state-done");
    assert_eq!(got.sort_order, 10.5);
    assert_eq!(got.priority, Priority::Urgent);
    assert_eq!(got.label_ids, vec!["l9".to_string()]);
    assert_eq!(got.assignee_id.as_deref(), Some("user-1"));
    assert_eq!(got.description, "new desc");
    assert!(got.updated_at > before);

    apply_change(&db, "uuid-1", &IssueChange::State { state_id: "state-todo".into(), sort_order: Some(-3.0) }).unwrap();
    apply_change(&db, "uuid-1", &IssueChange::Assignee(None)).unwrap();
    let got = get_issue(&db, "uuid-1").unwrap().unwrap();
    assert_eq!(got.sort_order, -3.0);
    assert_eq!(got.assignee_id, None);
    assert!(matches!(apply_change(&db, "nope", &IssueChange::Title("x".into())), Err(Error::NotFound(_))));

    let board = load_board(&db, "acme").unwrap();
    assert_eq!(board.issues.len(), 2);
    assert_eq!(board.issues[0].id, "uuid-1");

    upsert_comment(&db, &comment("c1", "uuid-2", 1)).unwrap();
    delete_issues(&db, &["uuid-2".to_string(), "unknown".to_string()]).unwrap();
    assert_eq!(get_issue(&db, "uuid-2").unwrap(), None);
    assert!(list_comments(&db, "uuid-2").unwrap().is_empty());
}

#[test]
fn comments_replace_list_upsert() {
    let tmp = TempDb::new();
    let db = tmp.open();
    upsert_issues(&db, &[issue("acme", "i1", "WEB-1")]).unwrap();
    let mut reply = comment("c2", "i1", 20);
    reply.parent_id = Some("c1".into());
    replace_comments(&db, "i1", &[reply.clone(), comment("c1", "i1", 10)]).unwrap();
    let list = list_comments(&db, "i1").unwrap();
    assert_eq!(list.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["c1", "c2"]);
    assert_eq!(list[1], reply);
    assert_eq!(get_issue(&db, "i1").unwrap().unwrap().comment_count, 2);

    let mut edited = comment("c1", "i1", 10);
    edited.body = "edited".into();
    edited.edited = true;
    upsert_comment(&db, &edited).unwrap();
    assert_eq!(get_issue(&db, "i1").unwrap().unwrap().comment_count, 2);
    upsert_comment(&db, &comment("c3", "i1", 30)).unwrap();
    assert_eq!(get_issue(&db, "i1").unwrap().unwrap().comment_count, 3);
    let list = list_comments(&db, "i1").unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0], edited);

    replace_comments(&db, "i1", &[comment("c9", "i1", 1)]).unwrap();
    let list = list_comments(&db, "i1").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "c9");
}

#[test]
fn upsert_keeps_comment_count_when_incoming_is_zero() {
    let tmp = TempDb::new();
    let db = tmp.open();
    let mut one = issue("acme", "i1", "WEB-1");
    upsert_issues(&db, &[one.clone()]).unwrap();
    replace_comments(&db, "i1", &[comment("c1", "i1", 1), comment("c2", "i1", 2)]).unwrap();

    one.title = "Synced title".into();
    upsert_issues(&db, &[one.clone()]).unwrap();
    let got = get_issue(&db, "i1").unwrap().unwrap();
    assert_eq!(got.title, "Synced title");
    assert_eq!(got.comment_count, 2);

    one.comment_count = 5;
    upsert_issues(&db, &[one]).unwrap();
    assert_eq!(get_issue(&db, "i1").unwrap().unwrap().comment_count, 5);
}

#[test]
fn dev_state_set_get_clear() {
    let tmp = TempDb::new();
    let db = tmp.open();
    let d = dev("acme", "WEB-1", Some("web-1-fix"), Some("/home/k/Code/x/.claude/worktrees/web-1"), 1);
    set_dev(&db, &d).unwrap();
    assert_eq!(get_dev(&db, "acme", "WEB-1").unwrap(), Some(d.clone()));
    assert_eq!(get_dev(&db, "acme", "web-1").unwrap(), Some(d.clone()));
    assert_eq!(get_dev(&db, "nw", "WEB-1").unwrap(), None);

    let mut later = d.clone();
    later.stage = DevStage::Merged;
    later.url = Some("https://gitlab/x/-/merge_requests/1".into());
    later.updated_at = t(50);
    set_dev(&db, &later).unwrap();
    assert_eq!(get_dev(&db, "acme", "WEB-1").unwrap(), Some(later.clone()));
    assert_eq!(load_board(&db, "acme").unwrap().dev, vec![later]);

    clear_dev(&db, "acme", "web-1").unwrap();
    assert_eq!(get_dev(&db, "acme", "WEB-1").unwrap(), None);
}

#[test]
fn find_dev_matches_ref_and_path() {
    let tmp = TempDb::new();
    let db = tmp.open();
    let wt = dev("acme", "WEB-1", Some("web-1"), Some("/code/app/.claude/worktrees/web-1/"), 1);
    let main = dev("acme", "WEB-2", Some("web-2"), Some("/code/app"), 2);
    let no_path = dev("nw", "NW-7", Some("nw-7"), None, 3);
    for d in [&wt, &main, &no_path] {
        set_dev(&db, d).unwrap();
    }
    let ids = |v: Vec<DevState>| v.into_iter().map(|d| d.identifier).collect::<Vec<_>>();

    assert!(find_dev(&db, None, None).unwrap().is_empty());
    assert_eq!(ids(find_dev(&db, Some("nw-7"), None).unwrap()), ["NW-7"]);
    assert_eq!(ids(find_dev(&db, Some("nope"), None).unwrap()), Vec::<String>::new());

    // Exact path, trailing slash ignored.
    assert_eq!(ids(find_dev(&db, None, Some("/code/app/.claude/worktrees/web-1")).unwrap()), ["WEB-1", "WEB-2"]);
    // Inside the stored path. The deeper stored path wins.
    assert_eq!(ids(find_dev(&db, None, Some("/code/app/.claude/worktrees/web-1/src/x.rs")).unwrap()), ["WEB-1", "WEB-2"]);
    assert_eq!(ids(find_dev(&db, None, Some("/code/app//src/")).unwrap()), ["WEB-2"]);
    // A sibling with a shared prefix is not inside.
    assert!(find_dev(&db, None, Some("/code/application")).unwrap().is_empty());
    assert!(find_dev(&db, None, Some("/code")).unwrap().is_empty());
    // Ref and path both matching rank first.
    assert_eq!(ids(find_dev(&db, Some("web-2"), Some("/code/app/.claude/worktrees/web-1")).unwrap()), ["WEB-2", "WEB-1"]);
}

#[test]
fn delete_workspace_removes_all_rows() {
    let tmp = TempDb::new();
    let db = tmp.open();
    for ws in ["a", "b"] {
        save_workspace(&db, &workspace(ws)).unwrap();
        replace_catalog(&db, ws, &catalog(ws, ws)).unwrap();
        upsert_issues(&db, &[issue(ws, &format!("{ws}-i1"), "WEB-1")]).unwrap();
        upsert_comment(&db, &comment(&format!("{ws}-c1"), &format!("{ws}-i1"), 1)).unwrap();
        set_dev(&db, &dev(ws, "WEB-1", Some("x"), None, 1)).unwrap();
    }
    crate::workspaces::delete_workspace_rows(&db, "a").unwrap();

    for table in ["workspaces", "teams", "states", "projects", "users", "labels", "issues", "comments", "dev_state"] {
        let n: i64 = db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap();
        assert!(n >= 1, "{table} lost the other workspace's rows");
        let col = match table {
            "workspaces" => "slug",
            "comments" => "issue_id",
            _ => "workspace",
        };
        let left: i64 = db
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM {table} WHERE {col} LIKE 'a%'"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0, "{table} still has rows of workspace a");
    }
    assert_eq!(list_workspaces(&db).unwrap().len(), 1);
    assert_eq!(load_board(&db, "b").unwrap().issues.len(), 1);
}

#[test]
fn two_handles_see_each_others_writes() {
    let tmp = TempDb::new();
    let gui = tmp.open();
    let mcp = tmp.open();
    upsert_issues(&gui, &[issue("acme", "i1", "WEB-1")]).unwrap();

    let v1 = data_version(&gui).unwrap();
    assert_eq!(data_version(&gui).unwrap(), v1);
    set_dev(&mcp, &dev("acme", "WEB-1", Some("web-1"), None, 1)).unwrap();
    let v2 = data_version(&gui).unwrap();
    assert_ne!(v1, v2);
    assert!(get_dev(&gui, "acme", "WEB-1").unwrap().is_some());

    // A write on the same handle does not change its own data_version.
    apply_change(&gui, "i1", &IssueChange::Title("from gui".into())).unwrap();
    assert_eq!(data_version(&gui).unwrap(), v2);
    assert_eq!(get_issue(&mcp, "i1").unwrap().unwrap().title, "from gui");
}

#[test]
fn open_creates_parent_dirs() {
    let tmp = TempDb::new();
    let nested = tmp.path.parent().unwrap().join("deep/er/db.sqlite3");
    let db = open_at(&nested).unwrap();
    assert!(nested.exists());
    drop(db);
}
