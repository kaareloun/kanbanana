//! Finds Claude Code and adds the kanbanana MCP server to it. The `claude` CLI runs on
//! worker threads only. Results come back over a channel.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::Context;

/// The CLI can hang on a network call or a prompt. The app kills it after this time.
const TIMEOUT: Duration = Duration::from_secs(15);
const SERVER: &str = "kanbanana";
pub const INSTALL: &str = "npm install -g @anthropic-ai/claude-code";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// No check ran yet.
    Unknown,
    Checking,
    NotFound,
    NotConnected,
    Connected,
}

#[derive(Debug)]
enum Msg {
    Checked(Status),
    Added(Result<(), AddError>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AddError {
    NotFound,
    Failed(String),
}

/// State of the Claude Code link on the last onboarding step.
pub struct ClaudeLink {
    pub status: Status,
    pub adding: bool,
    /// Output of a failed `claude mcp add`.
    pub error: Option<String>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl Default for ClaudeLink {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        ClaudeLink { status: Status::Unknown, adding: false, error: None, tx, rx }
    }
}

impl ClaudeLink {
    pub fn with_status(status: Status) -> Self {
        ClaudeLink { status, ..Default::default() }
    }

    /// Starts a check on a worker thread.
    pub fn check(&mut self, ctx: &Context) {
        self.status = Status::Checking;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Checked(check()));
            ctx.request_repaint();
        });
    }

    /// Runs `claude mcp add` on a worker thread.
    pub fn add(&mut self, ctx: &Context, exe: PathBuf) {
        self.adding = true;
        self.error = None;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Added(add(&exe)));
            ctx.request_repaint();
        });
    }

    /// Takes the results of finished threads.
    pub fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Checked(s) => self.status = s,
                Msg::Added(r) => {
                    self.adding = false;
                    match r {
                        Ok(()) => self.status = Status::Connected,
                        Err(AddError::NotFound) => self.status = Status::NotFound,
                        Err(AddError::Failed(e)) => self.error = Some(e),
                    }
                }
            }
        }
    }
}

/// The command the user can run in a terminal.
pub fn add_command(exe: &Path) -> String {
    format!("claude mcp add --scope user {SERVER} -- {} mcp", exe.display())
}

fn add_args(exe: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["mcp", "add", "--scope", "user", SERVER, "--"].map(OsString::from).into();
    args.push(exe.as_os_str().to_owned());
    args.push("mcp".into());
    args
}

fn check() -> Status {
    let Ok(claude) = find_claude() else { return Status::NotFound };
    let args = |a: &[&str]| a.iter().map(OsString::from).collect::<Vec<_>>();
    match run(&claude, &args(&["mcp", "get", SERVER])) {
        Ok(out) if out.ok => {
            if listed_by_get(&out.stdout) { Status::Connected } else { Status::NotConnected }
        }
        Ok(out) if out.stdout.contains("No MCP server") || out.stderr.contains("No MCP server") => Status::NotConnected,
        // Older CLIs have no `mcp get`. The list has every server on its own line.
        _ => match run(&claude, &args(&["mcp", "list"])) {
            Ok(out) if listed(&out.stdout) => Status::Connected,
            _ => Status::NotConnected,
        },
    }
}

fn add(exe: &Path) -> Result<(), AddError> {
    let claude = find_claude().map_err(|_| AddError::NotFound)?;
    match run(&claude, &add_args(exe)) {
        Ok(out) if out.ok => Ok(()),
        Ok(out) => {
            let text = if out.stderr.trim().is_empty() { out.stdout } else { out.stderr };
            Err(AddError::Failed(text.trim().to_string()))
        }
        Err(RunError::NotFound) => Err(AddError::NotFound),
        Err(RunError::TimedOut) => Err(AddError::Failed("Claude Code did not answer in 15 s.".into())),
        Err(RunError::Io(e)) => Err(AddError::Failed(e)),
    }
}

/// `claude` from PATH that answers `--version`. An app started from the desktop menu can
/// have a short PATH, so the default install folders count too.
fn find_claude() -> Result<PathBuf, RunError> {
    let mut candidates = vec![PathBuf::from("claude")];
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local/bin/claude"));
        candidates.push(home.join(".claude/local/claude"));
    }
    let mut last = RunError::NotFound;
    for c in candidates {
        match run(&c, &[OsString::from("--version")]) {
            Ok(out) if out.ok => return Ok(c),
            Ok(_) => last = RunError::Io("claude --version failed".into()),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// `claude mcp get kanbanana` prints "kanbanana:" and then the server details.
fn listed_by_get(stdout: &str) -> bool {
    stdout.lines().map(str::trim).find(|l| !l.is_empty()) == Some("kanbanana:")
}

/// `claude mcp list` prints one "name: command - status" line per server.
fn listed(stdout: &str) -> bool {
    stdout.lines().any(|l| l.trim_start().starts_with("kanbanana:"))
}

struct Output {
    ok: bool,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
enum RunError {
    NotFound,
    TimedOut,
    Io(String),
}

/// Runs the program without a shell and kills it after `TIMEOUT`.
fn run(program: &Path, args: &[OsString]) -> Result<Output, RunError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => RunError::NotFound,
            _ => RunError::Io(e.to_string()),
        })?;
    let stdout = read_on_thread(child.stdout.take());
    let stderr = read_on_thread(child.stderr.take());
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::TimedOut);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(RunError::Io(e.to_string())),
        }
    };
    // A child of the CLI can keep the pipe open, so the read waits one second at most.
    let take = |rx: Receiver<String>| rx.recv_timeout(Duration::from_secs(1)).unwrap_or_default();
    Ok(Output { ok: status.success(), stdout: take(stdout), stderr: take(stderr) })
}

fn read_on_thread(pipe: Option<impl Read + Send + 'static>) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut p) = pipe {
            let mut bytes = Vec::new();
            let _ = p.read_to_end(&mut bytes);
            s = String::from_utf8_lossy(&bytes).into_owned();
        }
        let _ = tx.send(s);
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_output_shows_the_server() {
        let found = "kanbanana:\n  Scope: User config (available in all your projects)\n  Status: ✔ Connected\n  Type: stdio\n";
        assert!(listed_by_get(found));
        assert!(listed_by_get("\n  kanbanana:\n  Scope: Local config\n"));
        assert!(!listed_by_get("figcraft:\n  Scope: User config\n"));
        assert!(!listed_by_get("No MCP server named \"kanbanana\". Configured servers: figcraft"));
        assert!(!listed_by_get(""));
    }

    #[test]
    fn list_output_has_a_kanbanana_line() {
        let list = "Checking MCP server health…\n\nfigcraft: /opt/figcraft cli mcp - ✔ Connected\n\
            kanbanana: /home/me/bin/kanbanana mcp - ✔ Connected\n";
        assert!(listed(list));
        let other = "Checking MCP server health…\n\nfigcraft: /opt/figcraft cli mcp - ✔ Connected\n\
            kanbanana-old: /x mcp - ✗ Failed to connect\n";
        assert!(!listed(other));
        assert!(!listed("No MCP servers configured."));
    }

    #[test]
    fn add_args_have_no_shell_quoting() {
        let exe = Path::new("/opt/my apps/kanbanana");
        let args = add_args(exe);
        assert_eq!(args.len(), 8);
        assert_eq!(args[6], OsString::from("/opt/my apps/kanbanana"));
        assert_eq!(add_command(exe), "claude mcp add --scope user kanbanana -- /opt/my apps/kanbanana mcp");
    }

    #[test]
    fn missing_program_is_not_found() {
        let r = run(Path::new("kanbanana-no-such-program"), &[]);
        assert!(matches!(r, Err(RunError::NotFound)));
    }
}
