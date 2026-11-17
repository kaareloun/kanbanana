# kanbanana

Standalone Rust desktop app: a Linear board with a workspace switcher.
Status (2026-11-17): v1 built (crates/). All tests pass and mock screens match the design. Not yet tried against a real Linear API key.

## Scope (v1)

1. Board view. Columns are workflow states. Group by team or by project.
2. Workspace switcher (acme, northwind, bluebird), one API key each.
3. Basic issue work: open, edit title/description, move between columns,
   status, assignee, priority, labels, comments, show and paste images.
4. Dev tracking: Claude tells the app how far each issue got (Local, MR, Merged,
   Staging, Prod) and on which branch. The app shows it on the card and in the panel.
5. Claude integration: a local MCP server, so Claude Code can read and edit issues
   and set their dev state.

Out of scope: live updates, Linear's rich editor, notifications, cycles,
custom views, advanced filters, integrations.

## Dev tracking

Design: screens 01 (one line per card) and 03 (Development row in the panel).
The app scans nothing and has no settings for this.

- Claude reports the dev state over MCP with `set_dev`. The app stores it and shows it.
- Per issue: one stage (Local, MR, Merged, Staging, Prod), plus an optional ref
  (branch name or MR number), worktree path (copy button) and MR URL.
- The panel shows "set by Claude, 2 h ago". The stage is only a hint. The Linear
  status stays separate, and Claude changes it with `update_issue` if you ask.
- Model: `dev_state(issue_ref, stage, ref, path, url, updated_at)` in SQLite.
  `issue_ref` is source-agnostic, so local tasks get it too. It stays on this
  machine and is not sent to Linear.

## Claude MCP

Design: no screens. The only visible result is the dev line and Claude's edits.

- Same binary, second mode: `kanbanana mcp` speaks MCP over stdio.
  Setup once, in a terminal: `claude mcp add kanbanana -- kanbanana mcp`.
  It uses the same SQLite cache and credentials file, so it works when the GUI is closed.
  The GUI reloads when the database file changes.
- Tools: `current_issue` (finds the issue for a branch or path from stored dev state),
  `list_issues`, `get_issue`, `set_dev`, `update_issue`, `add_comment`,
  `create_task` (local tasks, later).
- No approval UI in the app. Claude Code asks before tool calls as usual, and its
  own allow rules decide what runs without asking.

## Future feature: local tasks next to Linear (do not build yet)

The app imports Linear issues and also keeps its own tasks.
Both show on the same board with the same fields.

- One issue model with `source: linear | local`. Local tasks get their own ids (e.g. `LOC-12`).
- Local tasks live in SQLite. Linear issues are a cached copy from the API.
- Local tasks use the same columns (the team's workflow states).
- Later: "push to Linear" turns a local task into a Linear issue.

v1 must keep the model and the board code source-agnostic, so this needs no rewrite.

## Designs

- The designs in `design/` are made with figcraft (`design/*.figcraft`, PNG exports in
  `design/png*/`). The Python scripts in `design/src/` generate them.
- Rebuild with `design/src/build.sh`, `build_v2.sh` or `build_v3.sh`. They need the
  figcraft CLI on `PATH`, or its path in `FIGCRAFT`.
- figcraft CLI: `figcraft cli run script.json --out design.figcraft`,
  `figcraft cli export --out <dir> design.figcraft` (PNG per top-level frame),
  `figcraft cli commands` (all commands and params).
- In `command.batch`, name a step with `"as": "x"` and refer to it with `{"$ref": "x"}`.
