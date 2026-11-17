//! Tool schemas and handlers. Replies are short plain text for the model to read.

use anyhow::{Context, anyhow, bail};
use chrono::Utc;
use model::{BoardData, DevStage, DevState, Issue, IssueChange};
use serde_json::{Value, json};
use store::Db;

use crate::resolve;

pub fn list() -> Vec<Value> {
    let ws = json!({ "type": "string", "description": "Workspace slug, for example \"acme\". Optional when the identifier is unique." });
    vec![
        json!({
            "name": "current_issue",
            "description": "Finds the Linear issue that the current git work belongs to. Call it when you start work in a repository or worktree. Pass the branch and the absolute path of the worktree or repository.",
            "inputSchema": { "type": "object", "properties": {
                "branch": { "type": "string" },
                "path": { "type": "string", "description": "Absolute path of the worktree or repository." }
            }}
        }),
        json!({
            "name": "set_dev",
            "description": "Records how far the code work on an issue got, so the board shows it. Local board only, never writes to Linear. Call it when you create the branch or worktree (stage local), open a merge or pull request (mr), after merge (merged), and after deploy (staging, prod). Stage none removes the record.",
            "inputSchema": { "type": "object", "required": ["identifier", "stage"], "properties": {
                "identifier": { "type": "string", "description": "Issue id, for example WEB-214." },
                "stage": { "type": "string", "enum": ["local", "mr", "merged", "staging", "prod", "none"] },
                "git_ref": { "type": "string", "description": "Branch name, or the MR/PR number like \"!412\" or \"#88\"." },
                "path": { "type": "string", "description": "Absolute worktree or repository path." },
                "url": { "type": "string", "description": "MR/PR or deploy URL." },
                "workspace": ws
            }}
        }),
        json!({
            "name": "list_issues",
            "description": "Lists cached issues with status, priority and assignee.",
            "inputSchema": { "type": "object", "properties": {
                "workspace": ws,
                "team": { "type": "string", "description": "Team key or name." },
                "state": { "type": "string", "description": "State name, for example \"In Progress\"." },
                "assigned_to_me": { "type": "boolean" },
                "query": { "type": "string", "description": "Text to find in the title." },
                "limit": { "type": "integer", "default": 50 }
            }}
        }),
        json!({
            "name": "get_issue",
            "description": "Reads one issue: fields, description (markdown), dev state and comments.",
            "inputSchema": { "type": "object", "required": ["identifier"], "properties": {
                "identifier": { "type": "string" }, "workspace": ws
            }}
        }),
        json!({
            "name": "update_issue",
            "description": "Changes the issue in Linear. Only use it when the user asks for the change.",
            "inputSchema": { "type": "object", "required": ["identifier"], "properties": {
                "identifier": { "type": "string" },
                "workspace": ws,
                "state": { "type": "string", "description": "Target state name." },
                "assignee": { "type": "string", "description": "\"me\", a user name, or \"none\"." },
                "priority": { "type": "string", "enum": ["urgent", "high", "medium", "low", "none"] },
                "labels": { "type": "array", "items": { "type": "string" }, "description": "Replaces all labels." },
                "add_labels": { "type": "array", "items": { "type": "string" } },
                "remove_labels": { "type": "array", "items": { "type": "string" } },
                "title": { "type": "string" },
                "description": { "type": "string", "description": "Markdown. Replaces the description." }
            }}
        }),
        json!({
            "name": "add_comment",
            "description": "Writes a comment on the issue in Linear. Only use it when the user asks.",
            "inputSchema": { "type": "object", "required": ["identifier", "body"], "properties": {
                "identifier": { "type": "string" },
                "workspace": ws,
                "body": { "type": "string", "description": "Markdown." },
                "parent_comment_id": { "type": "string", "description": "Reply to this comment." }
            }}
        }),
        json!({
            "name": "list_workspaces",
            "description": "Lists the workspaces added in Kanbanana.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
    ]
}

pub fn call(db: &Db, name: &str, args: &Value) -> anyhow::Result<String> {
    match name {
        "current_issue" => current_issue(db, args),
        "set_dev" => set_dev(db, args),
        "list_issues" => list_issues(db, args),
        "get_issue" => get_issue(db, args),
        "update_issue" => update_issue(db, args),
        "add_comment" => add_comment(db, args),
        "list_workspaces" => list_workspaces(db),
        _ => bail!("Unknown tool {name}."),
    }
}

fn s<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str).filter(|v| !v.trim().is_empty())
}

fn strings(args: &Value, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(Value::as_array).map(|a| {
        a.iter().filter_map(Value::as_str).map(str::to_string).collect()
    })
}

fn required<'a>(args: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    s(args, key).ok_or_else(|| anyhow!("Missing argument {key}."))
}

fn current_issue(db: &Db, args: &Value) -> anyhow::Result<String> {
    let branch = s(args, "branch");
    let path = s(args, "path");
    if branch.is_none() && path.is_none() {
        bail!("Pass branch, path or both.");
    }
    let linked = store::find_dev(db, branch, path)?;
    if let Some(dev) = linked.first() {
        let issue = resolve::issue(db, Some(&dev.workspace), &dev.identifier)?;
        let board = resolve::board(db, &issue.workspace)?;
        return Ok(format!(
            "This work belongs to {}.\n{}",
            issue.identifier,
            summary(&board, &issue)
        ));
    }
    // Not linked yet: the branch name may still carry an issue id.
    for id in branch.map(resolve::identifiers_in).unwrap_or_default() {
        if let Ok(issue) = resolve::issue(db, None, &id) {
            let board = resolve::board(db, &issue.workspace)?;
            return Ok(format!(
                "The branch name points to {id}, but it is not linked yet. If this is right, call \
                 set_dev with identifier {id}, stage local, this branch and path.\n{}",
                summary(&board, &issue)
            ));
        }
    }
    Ok("No issue is linked to this branch or path. If the work is for a Linear issue, call \
        set_dev with its identifier, stage local, the branch and the path."
        .to_string())
}

fn set_dev(db: &Db, args: &Value) -> anyhow::Result<String> {
    let identifier = required(args, "identifier")?;
    let issue = resolve::issue(db, s(args, "workspace"), identifier)?;
    let stage_name = required(args, "stage")?;
    if stage_name.eq_ignore_ascii_case("none") {
        store::clear_dev(db, &issue.workspace, &issue.identifier)?;
        return Ok(format!("Removed the dev state of {}.", issue.identifier));
    }
    let stage = DevStage::parse(stage_name)
        .ok_or_else(|| anyhow!("Unknown stage \"{stage_name}\"."))?;
    // Keep earlier details that this call does not repeat, for example the path after an MR.
    let old = store::get_dev(db, &issue.workspace, &issue.identifier)?;
    let keep = |new: Option<&str>, old: Option<&String>| {
        new.map(str::to_string).or_else(|| old.cloned())
    };
    let dev = DevState {
        workspace: issue.workspace.clone(),
        identifier: issue.identifier.clone(),
        stage,
        git_ref: keep(s(args, "git_ref"), old.as_ref().and_then(|d| d.git_ref.as_ref())),
        path: keep(s(args, "path"), old.as_ref().and_then(|d| d.path.as_ref())),
        url: keep(s(args, "url"), old.as_ref().and_then(|d| d.url.as_ref())),
        updated_at: Utc::now(),
    };
    store::set_dev(db, &dev)?;
    Ok(format!("{} is now at stage {} on the board.", issue.identifier, stage.label()))
}

fn list_issues(db: &Db, args: &Value) -> anyhow::Result<String> {
    let workspaces: Vec<String> = match s(args, "workspace") {
        Some(ws) => vec![ws.to_string()],
        None => store::list_workspaces(db)?.into_iter().map(|w| w.slug).collect(),
    };
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;
    let query = s(args, "query").map(str::to_lowercase);
    let mut lines = Vec::new();
    for ws in workspaces {
        let board = resolve::board(db, &ws)?;
        for issue in &board.issues {
            if let Some(team) = s(args, "team") {
                let t = board.teams.iter().find(|t| t.id == issue.team_id);
                if !t.is_some_and(|t| t.key.eq_ignore_ascii_case(team) || t.name.eq_ignore_ascii_case(team)) {
                    continue;
                }
            }
            if let Some(state) = s(args, "state") {
                if !state_name(&board, issue).eq_ignore_ascii_case(state) {
                    continue;
                }
            }
            if args.get("assigned_to_me").and_then(Value::as_bool) == Some(true)
                && (board.viewer_id.is_none() || issue.assignee_id != board.viewer_id)
            {
                continue;
            }
            if let Some(q) = &query {
                if !issue.title.to_lowercase().contains(q) {
                    continue;
                }
            }
            lines.push(format!(
                "{} [{}] {} ({}, {})",
                issue.identifier,
                state_name(&board, issue),
                issue.title,
                issue.priority.label(),
                user_name(&board, issue.assignee_id.as_deref()),
            ));
            if lines.len() >= limit {
                break;
            }
        }
    }
    if lines.is_empty() {
        return Ok("No issues match.".to_string());
    }
    Ok(lines.join("\n"))
}

fn get_issue(db: &Db, args: &Value) -> anyhow::Result<String> {
    let issue = resolve::issue(db, s(args, "workspace"), required(args, "identifier")?)?;
    let board = resolve::board(db, &issue.workspace)?;
    // Comments are not part of the board sync. Fetch them now, fall back to the cache.
    let mut note = String::new();
    if issue.source == model::Source::Linear {
        match resolve::client(&issue.workspace).and_then(|c| Ok(c.comments(&issue.id)?)) {
            Ok(comments) => store::replace_comments(db, &issue.id, &comments)?,
            Err(e) => note = format!("\n(Showing cached comments: {e:#})"),
        }
    }
    let comments = store::list_comments(db, &issue.id)?;
    let mut out = summary(&board, &issue);
    out.push_str("\n\nDescription:\n");
    out.push_str(if issue.description.trim().is_empty() { "(empty)" } else { &issue.description });
    out.push_str(&format!("\n\nComments ({}):", comments.len()));
    for c in &comments {
        let indent = if c.parent_id.is_some() { "    " } else { "" };
        out.push_str(&format!(
            "\n{indent}- [{}] {}, {}: {}",
            c.id,
            user_name(&board, c.user_id.as_deref()),
            c.created_at.format("%Y-%m-%d %H:%M"),
            c.body.replace('\n', &format!("\n{indent}  "))
        ));
    }
    out.push_str(&note);
    Ok(out)
}

fn update_issue(db: &Db, args: &Value) -> anyhow::Result<String> {
    let issue = resolve::issue(db, s(args, "workspace"), required(args, "identifier")?)?;
    if issue.source != model::Source::Linear {
        bail!("{} is a local task. Local tasks are not supported yet.", issue.identifier);
    }
    let board = resolve::board(db, &issue.workspace)?;
    let mut changes = Vec::new();
    if let Some(title) = s(args, "title") {
        changes.push(IssueChange::Title(title.to_string()));
    }
    if let Some(desc) = args.get("description").and_then(Value::as_str) {
        changes.push(IssueChange::Description(desc.to_string()));
    }
    if let Some(state) = s(args, "state") {
        let state_id = resolve::state_id(&board, &issue.team_id, state)?;
        changes.push(IssueChange::State { state_id, sort_order: None });
    }
    if let Some(who) = args.get("assignee").and_then(Value::as_str) {
        changes.push(IssueChange::Assignee(resolve::user_id(&board, who)?));
    }
    if let Some(p) = s(args, "priority") {
        changes.push(IssueChange::Priority(resolve::priority(p)?));
    }
    let mut labels = issue.label_ids.clone();
    let mut labels_changed = false;
    if let Some(names) = strings(args, "labels") {
        labels = resolve::label_ids(&board, &issue.team_id, &names)?;
        labels_changed = true;
    }
    if let Some(names) = strings(args, "add_labels") {
        for id in resolve::label_ids(&board, &issue.team_id, &names)? {
            if !labels.contains(&id) {
                labels.push(id);
            }
        }
        labels_changed = true;
    }
    if let Some(names) = strings(args, "remove_labels") {
        let remove = resolve::label_ids(&board, &issue.team_id, &names)?;
        labels.retain(|id| !remove.contains(id));
        labels_changed = true;
    }
    if labels_changed {
        changes.push(IssueChange::Labels(labels));
    }
    if changes.is_empty() {
        bail!("Nothing to change. Pass at least one field.");
    }
    let client = resolve::client(&issue.workspace)?;
    let updated = client.update_issue(&issue.workspace, &issue.id, &changes).context("Linear rejected the change")?;
    store::upsert_issues(db, std::slice::from_ref(&updated))?;
    let board = resolve::board(db, &issue.workspace)?;
    Ok(format!("Updated {}.\n{}", updated.identifier, summary(&board, &updated)))
}

fn add_comment(db: &Db, args: &Value) -> anyhow::Result<String> {
    let issue = resolve::issue(db, s(args, "workspace"), required(args, "identifier")?)?;
    if issue.source != model::Source::Linear {
        bail!("{} is a local task. Local tasks are not supported yet.", issue.identifier);
    }
    let body = required(args, "body")?;
    let client = resolve::client(&issue.workspace)?;
    let comment = client.create_comment(&issue.id, body, s(args, "parent_comment_id"))?;
    store::upsert_comment(db, &comment)?;
    Ok(format!("Commented on {} (comment id {}).", issue.identifier, comment.id))
}

fn list_workspaces(db: &Db) -> anyhow::Result<String> {
    let list = store::list_workspaces(db)?;
    if list.is_empty() {
        return Ok("No workspaces yet. The user adds them in Kanbanana.".to_string());
    }
    let mut lines = Vec::new();
    for ws in list {
        let board = resolve::board(db, &ws.slug)?;
        let teams: Vec<_> = board.teams.iter().map(|t| format!("{} {}", t.key, t.name)).collect();
        lines.push(format!("{} ({}): {}", ws.slug, ws.name, teams.join(", ")));
    }
    Ok(lines.join("\n"))
}

fn summary(board: &BoardData, issue: &Issue) -> String {
    let labels: Vec<_> = issue
        .label_ids
        .iter()
        .filter_map(|id| board.labels.iter().find(|l| &l.id == id).map(|l| l.name.as_str()))
        .collect();
    let project = issue
        .project_id
        .as_ref()
        .and_then(|id| board.projects.iter().find(|p| &p.id == id))
        .map(|p| p.name.as_str())
        .unwrap_or("none");
    let mut out = format!(
        "{} {}\nState: {}. Priority: {}. Assignee: {}. Labels: {}. Project: {}.",
        issue.identifier,
        issue.title,
        state_name(board, issue),
        issue.priority.label(),
        user_name(board, issue.assignee_id.as_deref()),
        if labels.is_empty() { "none".to_string() } else { labels.join(", ") },
        project,
    );
    if let Some(dev) = board.dev.iter().find(|d| d.identifier == issue.identifier) {
        out.push_str(&format!("\nDev: {}", dev.stage.label()));
        for part in [&dev.git_ref, &dev.path, &dev.url].into_iter().flatten() {
            out.push_str(&format!(", {part}"));
        }
        out.push('.');
    }
    if let Some(url) = &issue.url {
        out.push_str(&format!("\nURL: {url}"));
    }
    out
}

fn state_name<'a>(board: &'a BoardData, issue: &Issue) -> &'a str {
    board
        .states
        .iter()
        .find(|s| s.id == issue.state_id)
        .map(|s| s.name.as_str())
        .unwrap_or("Unknown")
}

fn user_name<'a>(board: &'a BoardData, id: Option<&str>) -> &'a str {
    id.and_then(|id| board.users.iter().find(|u| u.id == id))
        .map(|u| u.name.as_str())
        .unwrap_or("Unassigned")
}
