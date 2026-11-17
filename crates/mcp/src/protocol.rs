//! JSON-RPC 2.0 framing and the MCP methods this server supports.

use serde_json::{Value, json};
use store::Db;

use crate::tools;

const SUPPORTED_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Claude Code shows this to the model when it connects. It is how Claude learns,
/// without any user setup, to keep the board's dev state current while it works.
pub const INSTRUCTIONS: &str = "\
Kanbanana is the user's local board for their Linear issues. Each issue on the board \
shows how far the code work got (Local, MR, Merged, Staging, Prod) and on which branch. \
The app does not look at git. You report it with set_dev. Keep it current without being asked:

- When you start work in a git repository or worktree, call current_issue with the branch \
and the path. It tells you which issue this work belongs to.
- If no issue is linked but the work is for a Linear issue (the user named an id like \
ABC-123, or the branch, commits or task text contain one), call set_dev with that \
identifier, stage \"local\", the branch and the worktree path.
- Call set_dev again when the stage changes: \"mr\" after you open a merge or pull request \
(pass its number as git_ref, for example \"!412\" or \"#88\", and its URL), \"merged\" after it \
merges, \"staging\" or \"prod\" when you know it is deployed there.
- set_dev only changes this local board. It never writes to Linear.
- Change Linear itself (update_issue, add_comment) only when the user asks for it.";

/// Handles one JSON-RPC message. Returns the reply line, or None for notifications.
pub fn handle_line(db: &Db, line: &str) -> Option<String> {
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(error_reply(Value::Null, -32700, &format!("Parse error: {e}"))),
    };
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);

    // Notifications have no id and get no reply.
    let id = id?;

    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools::list() })),
        "tools/call" => Ok(call_tool(db, &params)),
        _ => Err((-32601, format!("Method not found: {method}"))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
        Err((code, message)) => error_reply(id, code, &message),
    })
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
    let version =
        if SUPPORTED_VERSIONS.contains(&asked) { asked } else { SUPPORTED_VERSIONS[1] };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "kanbanana", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

/// Tool errors go back as a normal result with isError, so Claude sees the message.
fn call_tool(db: &Db, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let (text, is_error) = match tools::call(db, name, &args) {
        Ok(text) => (text, false),
        Err(e) => (format!("{e:#}"), true),
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn error_reply(id: Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        .to_string()
}
