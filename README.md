<p align="center">
  <img src="assets/brand/icon.png" width="128" alt="Kanbanana icon: a banana lying across three kanban columns">
</p>

<h1 align="center">Kanbanana</h1>

<p align="center">A fast desktop kanban board for <a href="https://linear.app">Linear</a>, with Claude Code built in.</p>

![Board](docs/screenshots/board.png)

## Install

```
yay -S kanbanana-bin                     # Arch
```

```
brew tap kaareloun/kanbanana https://github.com/kaareloun/kanbanana
brew install kanbanana                   # macOS
```

Other Linux: download an archive from the [releases page](https://github.com/kaareloun/kanbanana/releases).

## Set up

1. Create a personal API key in Linear: Settings, Security and access, Personal API keys.
2. Open Kanbanana and paste it. Add more workspaces with the **+** in the left rail.
3. Optional: connect Claude Code. The exact command is in Settings, Claude Code:

   ```
   claude mcp add --scope user kanbanana -- /path/to/kanbanana mcp
   ```

   Claude then shows which issue it works on and how far the code got.

## Keys

Press `?` in the app for the full list.

| Key | Action |
|---|---|
| C | New issue |
| J / K, arrows | Move between cards |
| Enter | Open issue |
| S, P, A, L | Status, priority, assignee, labels |
| I | Assign to me |
| Alt+1..9 | Switch workspace |

On macOS, Alt is Option.

## Build from source

```
git clone https://github.com/kaareloun/kanbanana
cd kanbanana
cargo run --release -p kanbanana
```

Needs Rust stable. Try it without a Linear key: `cargo run -p kanbanana -- --mock`.

## Data

API keys live in a file only you can read (`~/.config/kanbanana/credentials.json`). Issues are cached in `~/.local/share/kanbanana/`. On macOS both are in `~/Library/Application Support/kanbanana/`. The app talks only to Linear and the image hosts your issues link to.

Unofficial, not affiliated with Linear. MIT license.
