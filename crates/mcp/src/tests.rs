use chrono::Utc;
use model::{Accent, Issue, Priority, Source, Team, Workspace, WorkflowState, StateType};
use serde_json::{Value, json};

use crate::protocol::handle_line;

fn temp_db() -> store::Db {
    let path = std::env::temp_dir().join(format!(
        "lb-mcp-test-{}-{}.sqlite3",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let db = store::open_at(&path).unwrap();
    store::save_workspace(&db, &Workspace {
        slug: "acme".into(),
        name: "acme".into(),
        shortcut: 1,
        accent: Accent::Iris,
        viewer_id: Some("u1".into()),
        position: 0,
    })
    .unwrap();
    store::replace_catalog(&db, "acme", &store::Catalog {
        teams: vec![Team { id: "t1".into(), workspace: "acme".into(), key: "WEB".into(), name: "Webshop".into() }],
        states: vec![WorkflowState {
            id: "s1".into(),
            team_id: "t1".into(),
            name: "In Progress".into(),
            kind: StateType::Started,
            position: 1.0,
            color: "#f2c94c".into(),
        }],
        ..Default::default()
    })
    .unwrap();
    store::upsert_issues(&db, &[Issue {
        id: "i1".into(),
        source: Source::Linear,
        workspace: "acme".into(),
        identifier: "WEB-214".into(),
        title: "Refund webhook marks the order paid twice".into(),
        description: String::new(),
        team_id: "t1".into(),
        state_id: "s1".into(),
        project_id: None,
        assignee_id: Some("u1".into()),
        priority: Priority::Urgent,
        label_ids: vec![],
        sort_order: 1.0,
        comment_count: 0,
        url: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }])
    .unwrap();
    db
}

fn call(db: &store::Db, id: i64, method: &str, params: Value) -> Value {
    let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
    serde_json::from_str(&handle_line(db, &line).unwrap()).unwrap()
}

fn tool(db: &store::Db, name: &str, args: Value) -> (String, bool) {
    let r = call(db, 9, "tools/call", json!({ "name": name, "arguments": args }));
    let text = r["result"]["content"][0]["text"].as_str().unwrap().to_string();
    (text, r["result"]["isError"].as_bool().unwrap())
}

#[test]
fn handshake_and_tool_list() {
    let db = temp_db();
    let r = call(&db, 1, "initialize", json!({ "protocolVersion": "2025-06-18" }));
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert!(r["result"]["instructions"].as_str().unwrap().contains("set_dev"));
    assert!(handle_line(&db, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
    let tools = call(&db, 2, "tools/list", json!({}));
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"set_dev".to_string()));
    assert!(names.contains(&"current_issue".to_string()));
    let unknown = call(&db, 3, "nope", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
}

#[test]
fn set_dev_then_find_by_path_and_branch() {
    let db = temp_db();
    let (text, err) = tool(&db, "current_issue", json!({ "branch": "refund-webhook" }));
    assert!(!err);
    assert!(text.contains("No issue is linked"));

    let (text, err) = tool(&db, "set_dev", json!({
        "identifier": "web-214", "stage": "local",
        "git_ref": "refund-webhook",
        "path": "/home/me/Code/webshop/.claude/worktrees/refund-webhook"
    }));
    assert!(!err, "{text}");

    let (text, _) = tool(&db, "current_issue", json!({
        "path": "/home/me/Code/webshop/.claude/worktrees/refund-webhook/src"
    }));
    assert!(text.contains("belongs to WEB-214"), "{text}");
    assert!(text.contains("Dev: Local"), "{text}");

    // A later stage keeps the branch and path that the call does not repeat.
    tool(&db, "set_dev", json!({ "identifier": "WEB-214", "stage": "mr", "git_ref": "!412" }));
    let dev = store::get_dev(&db, "acme", "WEB-214").unwrap().unwrap();
    assert_eq!(dev.git_ref.as_deref(), Some("!412"));
    assert!(dev.path.unwrap().ends_with("refund-webhook"));

    let (_, err) = tool(&db, "set_dev", json!({ "identifier": "WEB-214", "stage": "none" }));
    assert!(!err);
    assert!(store::get_dev(&db, "acme", "WEB-214").unwrap().is_none());
}

#[test]
fn branch_name_hint_and_bad_input() {
    let db = temp_db();
    let (text, _) = tool(&db, "current_issue", json!({ "branch": "web-214-refund-state" }));
    assert!(text.contains("points to WEB-214"), "{text}");
    let (text, err) = tool(&db, "set_dev", json!({ "identifier": "XYZ-1", "stage": "local" }));
    assert!(err && text.contains("No issue XYZ-1"), "{text}");
    let (_, err) = tool(&db, "set_dev", json!({ "identifier": "WEB-214", "stage": "shipped" }));
    assert!(err);
    let (text, _) = tool(&db, "list_issues", json!({ "assigned_to_me": true }));
    assert!(text.contains("WEB-214 [In Progress]"), "{text}");
}
