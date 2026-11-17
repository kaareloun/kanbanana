# kanbanana: rules for agents

Read `NOTES.md` (scope) and look at `design/png/*.png` (the approved design) first.

## Build rule (important)

The machine has little free RAM. Run every cargo command through the lock wrapper:

```
scripts/cargo.sh check -p <crate>
scripts/cargo.sh test -p <crate>
```

It queues builds, so only one cargo process runs at a time. Never call `cargo` directly,
never pass `-j` above 4, never run `cargo build --release`. Prefer `check` over `build`.
Waiting on the lock is normal. Another agent is building.

## Layout and ownership

Cargo workspace. One owner per crate. Do not edit files outside the crate you own,
except where your task says so. If you need a change in another crate, say so in your report.

| Crate | Path | Job |
|---|---|---|
| `model` | `crates/model` | Shared types. Owned by the lead. Ask before changing. |
| `linear_api` | `crates/linear_api` | Linear GraphQL client: sync reads, mutations, uploads, image fetch. |
| `store` | `crates/store` | SQLite cache, workspaces, dev state, API keys in a private credentials file. |
| `sync` | `crates/sync` | Background engine: sync loop, mutations, comments, uploads, images. The GUI talks to it over channels. |
| `mcp` | `crates/mcp` | MCP server over stdio for Claude Code (`kanbanana mcp`). |
| `kanbanana` | `crates/app` | eframe GUI and the binary. Feature folders under `src/`. |

`Cargo.lock` is shared. Add dependencies only to your own crate's `Cargo.toml`,
with `scripts/cargo.sh add -p <crate> <dep>`.

## Code style

- Feature folders, plain functions where possible. No trait hierarchies, no builder
  frameworks. Small structs for state are fine.
- Errors: `thiserror` enums in library crates, `anyhow` in `app` and `mcp`.
- No `unwrap()` on I/O, network or user input. Tests may unwrap.
- Comments only where a reviewer would ask "why". Simplified Technical English:
  short sentences, present tense, 1 to 3 lines. No change history in comments.
- UI text: no em or en dashes, no italics. Sentence case. Use the theme tokens in
  `crates/app/src/theme/`, never raw hex colors in views.
- Fonts: Montserrat for UI, JetBrains Mono for ids, branches and code
  (`assets/fonts/`, embedded with `include_bytes!`).
- Unit tests for parsing and SQL. No tests that call the real Linear API.

## Contracts

### store (crate `store`)

`pub struct Db` wraps one `rusqlite::Connection`. Functions are free functions that take `&Db`.
The GUI and the MCP server are two processes on the same file: WAL mode, `busy_timeout` 5 s.

- `open_default() -> Result<Db>`: `$KANBANANA_DB` or `<data_dir>/kanbanana/cache.sqlite3`.
  `open_at(path)`. Runs migrations.
- `data_version(&Db) -> Result<i64>`: `PRAGMA data_version`. The GUI polls it to see writes
  from the MCP process.
- Workspaces: `list_workspaces`, `save_workspace(&Workspace)`, `delete_workspace(slug)` (also its
  cached rows and its key), `set_viewer(slug, viewer_id)`.
- Keys (module `secrets`, file `<config_dir>/kanbanana/credentials.json`, mode 0600, key = slug):
  `set_api_key(slug, key)`, `get_api_key(slug) -> Result<Option<String>>`, `delete_api_key(slug)`.
- Sync writes: `replace_catalog(ws, &Catalog)` (teams, states, projects, users, labels in one
  transaction), `upsert_issues(&[Issue])`, `delete_issues(&[id])`, `sync_cursor(ws)` and
  `set_sync_cursor(ws, DateTime)`, `set_last_sync(ws, DateTime)`.
- Reads: `load_board(ws) -> Result<BoardData>`, `get_issue(id)`,
  `find_issue(ws: Option<&str>, identifier) -> Result<Option<Issue>>`.
- Local edits: `apply_change(issue_id, &IssueChange)`.
- Comments: `replace_comments(issue_id, &[Comment])`, `list_comments(issue_id)`, `upsert_comment`.
- Dev state: `set_dev(&DevState)`, `get_dev(ws, identifier)`, `clear_dev(ws, identifier)`,
  `find_dev(git_ref: Option<&str>, path: Option<&str>) -> Result<Vec<DevState>>`
  (path matches when the given path is the stored path or inside it).

`Catalog` is defined in `store` as `{ teams, states, projects, users, labels }`
(vectors of the `model` types). `linear_api` returns the same parts; the caller copies them over.

### linear_api (crate `linear_api`)

`pub struct Client` holds the key and a `ureq::Agent`. Personal keys go in the
`Authorization` header as is, without `Bearer`. Endpoint `https://api.linear.app/graphql`.
All calls are blocking. Callers run them on a worker thread.

- `Client::new(key)`.
- `viewer() -> Viewer { id, name, organization_name }`.
- `catalog(ws) -> Catalog { teams, states, projects, users, labels }` (own struct, same fields
  as the store one), paginated.
- `issues_updated_since(ws, since: Option<DateTime<Utc>>) -> IssuePage`: paginated.
  First sync (`since = None`): all issues that are not completed or canceled, plus those
  completed in the last 30 days. Later syncs: everything with `updatedAt > since`,
  including archived ones, which come back in `IssuePage.archived_ids` so the caller deletes them.
- `update_issue(id, &[IssueChange]) -> Issue` (`issueUpdate`).
- `comments(issue_id) -> Vec<Comment>`, `create_comment(issue_id, body, parent_id) -> Comment`,
  `update_comment(id, body) -> Comment`.
- `upload(filename, content_type, bytes) -> String` asset URL: `fileUpload` mutation, then PUT the
  bytes to `uploadUrl` with the returned headers.
- `fetch_image(url) -> Vec<u8>`: GET with the `Authorization` header (Linear upload URLs need it).
- Errors: `Error::{Auth, RateLimited { retry_after_secs }, GraphQl(String), Http(String), Decode(String)}`.
  Read the rate limit headers and map HTTP 429 and the `RATELIMITED` GraphQL code to `RateLimited`.

### mcp (crate `mcp`)

`pub fn run_stdio() -> anyhow::Result<()>`: MCP over stdio, newline-delimited JSON-RPC 2.0.
The binary runs it for `kanbanana mcp`. It opens the store and reads keys from the credentials file.

### app (crate `kanbanana`)

`src/main.rs` dispatches `mcp` to `mcp::run_stdio()`, else starts the GUI.
Feature folders: `theme/`, `board/`, `issue_panel/`, `workspace/`, `sync/`, `images/`.
Views never call the network. They read `BoardData` and push `Action`s; the `sync/` worker
thread runs them and writes to the store.
