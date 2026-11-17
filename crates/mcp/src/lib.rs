//! MCP server for Claude Code, over stdio. `kanbanana mcp` runs it.
//! It shares the SQLite cache and the credentials file with the GUI.

mod protocol;
mod resolve;
mod tools;

use std::io::{BufRead, Write};

pub use protocol::INSTRUCTIONS;

/// Reads newline-delimited JSON-RPC from stdin and writes replies to stdout.
/// Logs go to stderr, because stdout carries the protocol.
pub fn run_stdio() -> anyhow::Result<()> {
    let db = store::open_default()?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = protocol::handle_line(&db, &line) {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
