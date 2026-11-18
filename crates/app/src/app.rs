//! The one App struct. It holds all state, runs the views and applies their actions.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use chrono::Utc;
use eframe::egui::{
    self, Align2, CentralPanel, ColorImage, Frame, Key, OpenUrl, Panel, ViewportCommand, pos2, vec2,
};
use model::{Accent, BoardData, Issue, IssueChange, StateType, Workspace};

use crate::actions::{self, Action};
use crate::board::{self, BoardProps, BoardView, Drag};
use crate::board::layout::GroupBy;
use crate::issue_panel::{self, PanelAction, PanelState};
use crate::mock;
use crate::new_issue::{self, NewIssueState};
use crate::shortcuts::{self, Shortcuts};
use crate::onboarding::{self, OnboardingState, Step, claude::Status, page::OnboardingProps};
use crate::settings::{self, SettingsState, page::SettingsProps};
use crate::sync::{Command, Event, Sync, SyncStatus};
use crate::theme::{self, Theme, W, parts};
use crate::workspace::{AddForm, KeyCheck, page, rail};
use crate::workspace::rail::RailMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartScreen {
    Board,
    BoardProject,
    /// Back office without a drag, so its empty In Review column shows.
    BoardEmpty,
    Workspaces,
    /// First run: no workspaces yet.
    Welcome,
    Panel,
    PanelComments,
    PanelCommentEdit,
    PanelEdit,
    Settings,
    /// Settings scrolled to the theme cards.
    SettingsAppearance,
    OnboardingTheme,
    /// The add form with a mock key that checked OK.
    OnboardingWorkspace,
    OnboardingClaude,
    /// The new issue form with a typed title, a status and a label.
    NewIssue,
    /// The keyboard shortcuts overlay.
    Shortcuts,
    /// The board with the keyboard focus ring on a card.
    BoardFocus,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub mock: bool,
    pub screenshot: Option<PathBuf>,
    pub screen: StartScreen,
    /// `--theme`. It wins over the theme in the settings file.
    pub theme: Option<Theme>,
}

impl Options {
    pub fn parse(args: &[String]) -> anyhow::Result<Options> {
        let mut o = Options { mock: false, screenshot: None, screen: StartScreen::Board, theme: None };
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--mock" => o.mock = true,
                "--screenshot" => o.screenshot = Some(PathBuf::from(it.next().context("--screenshot needs a path")?)),
                "--screen" => {
                    o.screen = match it.next().map(String::as_str) {
                        Some("board") => StartScreen::Board,
                        Some("board-project") => StartScreen::BoardProject,
                        Some("board-empty") => StartScreen::BoardEmpty,
                        Some("workspaces") => StartScreen::Workspaces,
                        Some("welcome") => StartScreen::Welcome,
                        Some("panel") => StartScreen::Panel,
                        Some("panel-comments") => StartScreen::PanelComments,
                        Some("panel-comment-edit") => StartScreen::PanelCommentEdit,
                        Some("panel-edit") => StartScreen::PanelEdit,
                        Some("settings") => StartScreen::Settings,
                        Some("settings-appearance") => StartScreen::SettingsAppearance,
                        Some("onboarding-theme") => StartScreen::OnboardingTheme,
                        Some("onboarding-workspace") => StartScreen::OnboardingWorkspace,
                        Some("onboarding-claude") => StartScreen::OnboardingClaude,
                        Some("new-issue") => StartScreen::NewIssue,
                        Some("shortcuts") => StartScreen::Shortcuts,
                        Some("board-focus") => StartScreen::BoardFocus,
                        other => bail!(
                            "unknown screen {other:?}. Use board, board-project, board-empty, board-focus, workspaces, welcome, panel, \
                             panel-comments, panel-comment-edit, panel-edit, settings, settings-appearance, onboarding-theme, onboarding-workspace, \
                             onboarding-claude, new-issue or shortcuts."
                        ),
                    }
                }
                "--theme" => {
                    let name = it.next().context("--theme needs a theme name, for example main or nord")?;
                    o.theme = Some(Theme::by_name(name).with_context(|| format!("unknown theme {name}"))?);
                }
                other => bail!("unknown argument {other}"),
            }
        }
        Ok(o)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Board,
    Workspaces,
    Settings,
    Onboarding,
}

struct Shot {
    path: PathBuf,
    frames: u32,
    requested: bool,
    image: Arc<Mutex<Option<Arc<ColorImage>>>>,
    started: Instant,
}

pub struct App {
    theme: Theme,
    mock: bool,
    workspaces: Vec<Workspace>,
    boards: HashMap<String, BoardData>,
    active: Option<String>,
    screen: Screen,
    board: BoardView,
    panel: PanelState,
    comments: issue_panel::CommentsView,
    form: AddForm,
    settings: SettingsState,
    onboarding: OnboardingState,
    /// Path of this binary, for the `claude mcp add` command.
    exe: Option<PathBuf>,
    sync: Sync,
    shot: Option<Shot>,
    /// Window focus in the last frame. Gaining focus starts a sync.
    focused: bool,
    /// Theme of the visuals set on the context, to update them only on a change.
    visuals_theme: Option<Theme>,
    /// Text of the last copy and when it happened, for the "Copied" toast.
    toast: Option<(String, Instant)>,
    new_issue: NewIssueState,
    shortcuts: Shortcuts,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: Options) -> App {
        // Mock screens ignore the settings file, so screenshots look the same on every machine.
        let base = match opts.theme {
            Some(t) => t,
            None if opts.mock => Theme::MAIN,
            None => settings::load_theme(),
        };
        theme::install(&cc.egui_ctx, &base);
        crate::images::install(&cc.egui_ctx);
        crate::brand::preload(&cc.egui_ctx);
        let mut app = App {
            theme: base,
            mock: opts.mock,
            workspaces: Vec::new(),
            boards: HashMap::new(),
            active: None,
            screen: Screen::Workspaces,
            board: BoardView::default(),
            panel: PanelState::default(),
            comments: Default::default(),
            form: AddForm::default(),
            settings: SettingsState::default(),
            onboarding: OnboardingState::default(),
            exe: if opts.mock { Some(PathBuf::from(MOCK_EXE)) } else { std::env::current_exe().ok() },
            sync: if opts.mock { Sync::default() } else { Sync::start(&cc.egui_ctx) },
            shot: opts.screenshot.map(|path| Shot {
                path,
                frames: 0,
                requested: false,
                image: Arc::new(Mutex::new(None)),
                started: Instant::now(),
            }),
            focused: true,
            visuals_theme: None,
            toast: None,
            new_issue: NewIssueState::default(),
            shortcuts: Shortcuts::default(),
        };
        if opts.mock {
            app.load_mock(opts.screen);
            app.load_panel_demo(opts.screen);
        } else {
            app.load_store();
            app.start_onboarding();
        }
        app.form = AddForm::fresh(&app.workspaces);
        if opts.screen == StartScreen::Workspaces && opts.mock {
            app.form = demo_form();
        }
        if opts.screen == StartScreen::OnboardingWorkspace && opts.mock {
            app.form = AddForm { name: "bluebird".into(), ..demo_form() };
            app.form.shortcut = 1;
            app.form.accent = Accent::Iris;
        }
        app
    }

    /// Opens the onboarding flow on a fresh install. Existing users only get the flag.
    fn start_onboarding(&mut self) {
        match onboarding::start(settings::load_onboarded(), !self.workspaces.is_empty()) {
            onboarding::Start::Show => self.screen = Screen::Onboarding,
            onboarding::Start::MarkDone => {
                if let Err(e) = settings::save_onboarded(true) {
                    eprintln!("kanbanana: save settings: {e:#}");
                }
            }
            onboarding::Start::Skip => {}
        }
    }

    /// Reads the cached workspaces and boards, so the board shows at once, then asks for a sync.
    fn load_store(&mut self) {
        self.workspaces = self.sync.workspaces();
        for ws in &self.workspaces {
            if let Some(b) = self.sync.board(&ws.slug) {
                self.boards.insert(ws.slug.clone(), b);
            }
        }
        self.active = self.workspaces.first().map(|w| w.slug.clone());
        self.screen = if self.active.is_some() { Screen::Board } else { Screen::Workspaces };
        self.activate();
    }

    /// Tells the engine which workspace is on screen and syncs it now.
    fn activate(&mut self) {
        self.sync.send(Command::SetActive(self.active.clone()));
        if let Some(ws) = &self.active {
            self.sync.send(Command::SyncNow { workspace: ws.clone() });
        }
    }

    fn reload_board(&mut self, ws: &str) {
        if let Some(b) = self.sync.board(ws) {
            self.boards.insert(ws.to_string(), b);
        }
    }

    fn load_mock(&mut self, screen: StartScreen) {
        self.workspaces = mock::workspaces();
        if screen == StartScreen::Workspaces {
            // Design 06 adds the third workspace, so the list shows two.
            self.workspaces.truncate(2);
        }
        let onboarding = match screen {
            StartScreen::OnboardingTheme => Some(OnboardingState::at(Step::Theme)),
            StartScreen::OnboardingWorkspace => Some(OnboardingState::at(Step::Workspace)),
            StartScreen::OnboardingClaude => {
                let mut s = OnboardingState::at(Step::Claude);
                s.linked = Some("bluebird".into());
                s.claude = onboarding::claude::ClaudeLink::with_status(Status::NotConnected);
                Some(s)
            }
            _ => None,
        };
        if screen == StartScreen::Welcome || onboarding.is_some() {
            self.workspaces.clear();
        }
        for ws in &self.workspaces {
            self.boards.insert(ws.slug.clone(), mock::board(&ws.slug));
        }
        self.active = if self.workspaces.is_empty() { None } else { Some("acme".into()) };
        if let Some(s) = onboarding {
            self.onboarding = s;
        }
        self.screen = match screen {
            StartScreen::Workspaces | StartScreen::Welcome => Screen::Workspaces,
            StartScreen::OnboardingTheme | StartScreen::OnboardingWorkspace | StartScreen::OnboardingClaude => {
                Screen::Onboarding
            }
            StartScreen::Settings | StartScreen::SettingsAppearance => {
                // Opened long ago, so the screenshot does not catch the fade in.
                self.settings = SettingsState::open(f64::NEG_INFINITY);
                mock_paths(&mut self.settings);
                self.settings.scroll_to_appearance = screen == StartScreen::SettingsAppearance;
                Screen::Settings
            }
            _ => Screen::Board,
        };
        self.board.set_tab("acme", GroupBy::Team, "team:acme-t-web");
        self.board.set_tab("northwind", GroupBy::Project, "project:northwind-p-erp");
        if screen == StartScreen::Board {
            // Design v3 01: the card menu on WEB-214 just after "Copy ID and title".
            self.board.menu = Some(board::Menu::demo("acme-i-web-214", pos2(625.0, 236.0), 1));
            let title = "WEB-214 Refund webhook marks the order paid twice";
            self.toast = Some((title.into(), Instant::now()));
        }
        if screen == StartScreen::BoardEmpty {
            self.active = Some("northwind".into());
            self.board.group = GroupBy::Project;
        }
        if screen == StartScreen::BoardFocus {
            self.board.focus = Some("acme-i-web-219".into());
        }
        if screen == StartScreen::Shortcuts {
            self.shortcuts.opened_at = Some(f64::NEG_INFINITY);
        }
        if screen == StartScreen::NewIssue
            && let Some(b) = self.boards.get("acme")
        {
            let title = "Gift card balance shows 0 after a partial refund";
            self.new_issue.demo(b, "acme-t-web", title, "In Progress", &["Bug"]);
        }
        if screen == StartScreen::BoardProject {
            self.active = Some("northwind".into());
            self.board.group = GroupBy::Project;
            self.board.mine = true;
            // Design v3 02: NW-420 hangs over In Review, slightly turned.
            let grab = vec2(140.0, 60.0);
            self.board.drag = Some(Drag {
                issue_id: "northwind-i-nw-420".into(),
                grab,
                demo_pointer: Some(pos2(816.0, 172.0) + grab),
            });
        }
    }

    fn global_keys(&self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if self.form.capturing || self.screen == Screen::Onboarding {
            return;
        }
        if self.screen != Screen::Settings && ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Comma)) {
            actions.push(Action::OpenSettings);
        }
        const KEYS: [Key; 9] =
            [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
        ctx.input(|i| {
            if !i.modifiers.alt {
                return;
            }
            for (n, k) in KEYS.iter().enumerate() {
                if i.key_pressed(*k)
                    && let Some(ws) = self.workspaces.iter().find(|w| w.shortcut as usize == n + 1)
                {
                    actions.push(Action::SwitchWorkspace(ws.slug.clone()));
                }
            }
        });
    }

    fn board_screen(&mut self, ui: &mut egui::Ui, t: &Theme, actions: &mut Vec<Action>) {
        let ws = self.active.as_ref().and_then(|s| self.workspaces.iter().find(|w| &w.slug == s));
        let Some(ws) = ws else {
            CentralPanel::default().frame(Frame::NONE.fill(t.base)).show(ui, |ui| {
                let c = ui.max_rect().center();
                let p = ui.painter();
                parts::text(p, c, Align2::CENTER_CENTER, "No workspace yet. Add one with the + button.", W::Medium, 13.0, t.muted);
            });
            return;
        };
        let empty = BoardData::default();
        let data = self.boards.get(&ws.slug).unwrap_or(&empty);
        let blocked = self.new_issue.is_open() || self.shortcuts.is_open();
        self.panel.keys_off = blocked;
        if let Some(issue) = self.panel.issue_id.as_ref().and_then(|id| data.issues.iter().find(|i| &i.id == id)) {
            let ctx = ui.ctx().clone();
            let mut out = Vec::new();
            issue_panel::show_panel(&ctx, t, &mut self.panel, issue, data, &self.comments, &mut out);
            let out = take_engine_actions(&issue.id, out, actions);
            issue_panel::to_app_actions(&ctx, &issue.id, out, actions);
        } else {
            let ctx = ui.ctx().clone();
            let mut out = Vec::new();
            if let Some(id) = issue_panel::show_closing(&ctx, t, &mut self.panel, data, &mut out) {
                issue_panel::to_app_actions(&ctx, &id, out, actions);
            }
        }
        let props = BoardProps {
            workspace: ws,
            data,
            sync: &self.sync.status,
            selected: self.panel.issue_id.as_deref(),
            blocked,
        };
        board::show(ui, t, &props, &mut self.board, actions);
        let ctx = ui.ctx().clone();
        new_issue::show(&ctx, t, &mut self.new_issue, data, actions);
        shortcuts::show(&ctx, t, &mut self.shortcuts);
    }

    fn apply(&mut self, ctx: &egui::Context, a: Action) {
        match a {
            Action::ChangeIssue { issue_id, change } => {
                if let Some(ws) = self.active.clone() {
                    if self.boards.get(&ws).is_some_and(|b| completes(b, &issue_id, &change)) {
                        self.pop_banana(ctx, &issue_id);
                    }
                    if let Some(b) = self.boards.get_mut(&ws) {
                        actions::apply_change(b, &issue_id, &change);
                    }
                    self.sync.send(Command::ChangeIssue { workspace: ws, issue_id, change });
                }
            }
            Action::SelectIssue(id) => self.select_issue(id),
            Action::OpenIssueWith { issue_id, picker } => {
                self.panel.open_with = Some(picker);
                self.select_issue(Some(issue_id));
            }
            Action::NewIssue => {
                if self.screen == Screen::Board
                    && let Some(ws) = self.active.clone()
                    && let Some(b) = self.boards.get(&ws)
                {
                    let (team, project) = self.board.tab_scope(&ws, b);
                    self.shortcuts.opened_at = None;
                    self.new_issue.open(theme::motion::now(ctx), b, team, project);
                }
            }
            Action::CreateIssue { request_id, draft } => {
                let Some(ws) = self.active.clone() else { return };
                if self.mock {
                    // Mock mode has no engine. The board takes the issue at once.
                    match self.boards.get_mut(&ws).and_then(|b| mock::create(b, &ws, &draft)) {
                        Some(issue) => self.issue_created(&ws, request_id, &issue),
                        None => self.new_issue.on_failed(request_id, "the mock board has no such team"),
                    }
                } else {
                    self.sync.send(Command::CreateIssue { workspace: ws, request_id, draft });
                }
            }
            Action::ShowShortcuts => {
                if !self.new_issue.is_open() {
                    self.shortcuts.toggle(theme::motion::now(ctx));
                }
            }
            Action::SwitchWorkspace(slug) => {
                if self.workspaces.iter().any(|w| w.slug == slug) {
                    let changed = self.active.as_deref() != Some(slug.as_str());
                    self.active = Some(slug);
                    self.screen = Screen::Board;
                    self.panel = PanelState::default();
                    self.comments = Default::default();
                    self.board.drag = None;
                    self.board.focus = None;
                    if changed {
                        self.new_issue.close();
                        self.activate();
                    }
                }
            }
            Action::OpenWorkspaces => {
                self.screen = Screen::Workspaces;
                self.board.drag = None;
            }
            Action::OpenSettings => {
                self.settings = SettingsState::open(theme::motion::now(ctx));
                if self.mock {
                    mock_paths(&mut self.settings);
                }
                self.screen = Screen::Settings;
                self.board.drag = None;
            }
            Action::CloseSettings => {
                self.settings.pending_remove = None;
                self.screen = if self.active.is_some() { Screen::Board } else { Screen::Workspaces };
            }
            Action::SetTheme(name) => {
                if let Some(t) = Theme::by_name(&name) {
                    self.theme = t;
                    if !self.mock
                        && let Err(e) = settings::save_theme(&name)
                    {
                        eprintln!("kanbanana: save settings: {e:#}");
                    }
                }
            }
            Action::OpenFolder(path) => settings::open_folder(&path),
            Action::RunOnboarding => {
                if !self.mock
                    && let Err(e) = settings::save_onboarded(false)
                {
                    eprintln!("kanbanana: save settings: {e:#}");
                }
                self.onboarding = OnboardingState::default();
                self.form = AddForm::fresh(&self.workspaces);
                self.screen = Screen::Onboarding;
                self.board.drag = None;
            }
            Action::FinishOnboarding => {
                if !self.mock
                    && let Err(e) = settings::save_onboarded(true)
                {
                    eprintln!("kanbanana: save settings: {e:#}");
                }
                self.screen = if self.active.is_some() { Screen::Board } else { Screen::Workspaces };
            }
            // Mock screens never run the claude CLI. They fake its answers.
            Action::CheckClaude if self.mock => self.onboarding.claude.status = Status::NotConnected,
            Action::CheckClaude => self.onboarding.claude.check(ctx),
            Action::AddToClaude if self.mock => self.onboarding.claude.status = Status::Connected,
            Action::AddToClaude => match self.exe.clone() {
                Some(exe) => self.onboarding.claude.add(ctx, exe),
                None => self.onboarding.claude.error = Some("Kanbanana could not find its own path.".into()),
            },
            Action::CloseWorkspaces => {
                if self.active.is_some() {
                    self.screen = Screen::Board;
                }
            }
            Action::Refresh => {
                if self.mock {
                    if let Some(b) = self.active.as_ref().and_then(|s| self.boards.get_mut(s)) {
                        b.last_sync = Some(Utc::now());
                    }
                } else if let Some(ws) = self.active.clone() {
                    self.sync.send(Command::SyncNow { workspace: ws });
                }
            }
            Action::CheckKey(ref key) => {
                if self.mock {
                    self.form.check = mock_check(key);
                    if let KeyCheck::Ok { workspace, .. } = &self.form.check
                        && self.form.name.trim().is_empty()
                    {
                        self.form.name = workspace.clone();
                    }
                } else {
                    self.form.check = KeyCheck::Checking;
                    self.sync.send(Command::CheckKey { key: key.clone() });
                }
            }
            Action::AddWorkspace { ref workspace, .. } => {
                let slug = workspace.slug.clone();
                self.workspaces.push(workspace.clone());
                if self.mock {
                    self.boards.insert(slug.clone(), BoardData::default());
                }
                self.active = Some(slug);
                if self.screen == Screen::Onboarding {
                    self.onboarding.linked = Some(workspace.name.clone());
                    self.onboarding.go(Step::Claude, theme::motion::now(ctx));
                } else {
                    self.screen = Screen::Board;
                }
                self.form = AddForm::fresh(&self.workspaces);
                if let Action::AddWorkspace { workspace, api_key } = a {
                    self.sync.send(Command::AddWorkspace { workspace, api_key });
                }
                self.sync.send(Command::SetActive(self.active.clone()));
            }
            Action::RemoveWorkspace(ref slug) => {
                self.workspaces.retain(|w| &w.slug != slug);
                self.boards.remove(slug);
                if self.active.as_ref() == Some(slug) {
                    self.active = self.workspaces.first().map(|w| w.slug.clone());
                    self.panel = PanelState::default();
                }
                self.sync.send(Command::RemoveWorkspace(slug.clone()));
                self.activate();
            }
            Action::OpenUrl(url) => ctx.open_url(OpenUrl::new_tab(url)),
            Action::Copy(text) => {
                // Mock screens push copies for the demo toast. They must not touch the clipboard.
                if !self.mock {
                    ctx.copy_text(text.clone());
                }
                self.toast = Some((text, Instant::now()));
            }
            Action::LoadComments { .. } if self.mock => self.comments = issue_panel::mock_comments(),
            Action::LoadComments { issue_id } => {
                self.comments = issue_panel::CommentsView {
                    loading: true,
                    error: None,
                    list: self.sync.cached_comments(&issue_id),
                };
                if let Some(ws) = self.active.clone() {
                    self.sync.send(Command::LoadComments { workspace: ws, issue_id });
                }
            }
            Action::CreateComment { issue_id, body, parent_id } => {
                if let Some(ws) = self.active.clone() {
                    self.sync.send(Command::CreateComment { workspace: ws, issue_id, body, parent_id });
                }
            }
            Action::UpdateComment { issue_id, comment_id, body } => {
                if let Some(ws) = self.active.clone() {
                    self.sync.send(Command::UpdateComment { workspace: ws, issue_id, comment_id, body });
                }
            }
            Action::Upload { request_id, filename, content_type, bytes } => {
                if self.mock {
                    let url = format!("https://uploads.linear.app/mock/{filename}");
                    issue_panel::on_uploaded(&mut self.panel, request_id, &url);
                } else if let Some(ws) = self.active.clone() {
                    self.sync.send(Command::Upload { workspace: ws, request_id, filename, content_type, bytes });
                }
            }
        }
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        let active = self.active.clone();
        let is_active = |ws: &str| active.as_deref() == Some(ws);
        for ev in self.sync.events() {
            match ev {
                Event::SyncStarted { workspace } if is_active(&workspace) => {
                    self.sync.status = SyncStatus::Syncing;
                }
                Event::Synced { workspace, .. } => {
                    self.reload_board(&workspace);
                    if is_active(&workspace) {
                        self.sync.status = SyncStatus::Idle;
                    }
                }
                Event::SyncFailed { workspace, message }
                    if !self.mock && !self.sync.workspaces().iter().any(|w| w.slug == workspace) =>
                {
                    // The engine could not save a new workspace. Take it off the rail and
                    // show why in the add form, where the user can try again.
                    self.workspaces.retain(|w| w.slug != workspace);
                    self.boards.remove(&workspace);
                    if self.active.as_deref() == Some(workspace.as_str()) {
                        self.active = self.workspaces.first().map(|w| w.slug.clone());
                    }
                    if self.screen == Screen::Onboarding {
                        self.onboarding.linked = None;
                        self.onboarding.go(Step::Workspace, theme::motion::now(ctx));
                    } else {
                        self.screen = Screen::Workspaces;
                    }
                    self.form.check = KeyCheck::Error(message);
                }
                Event::SyncFailed { workspace, message } => {
                    self.reload_board(&workspace);
                    if is_active(&workspace) {
                        self.sync.status = SyncStatus::Error(message);
                    }
                }
                Event::RateLimited { workspace, until } if is_active(&workspace) => {
                    let local = until.with_timezone(&chrono::Local).format("%H:%M");
                    self.sync.status = SyncStatus::Error(format!("Rate limit, retry {local}"));
                }
                Event::BoardChanged { workspace } => self.reload_board(&workspace),
                Event::IssueCreated { workspace, request_id, issue } => {
                    self.reload_board(&workspace);
                    self.issue_created(&workspace, request_id, &issue);
                }
                Event::CreateFailed { request_id, message, .. } => self.new_issue.on_failed(request_id, &message),
                Event::ChangeFailed { workspace, message, .. } => {
                    self.reload_board(&workspace);
                    self.sync.status = SyncStatus::Error(format!("Change not saved: {message}"));
                }
                Event::Comments { issue_id, comments } => {
                    if self.panel.issue_id.as_deref() == Some(issue_id.as_str()) {
                        self.comments.list = comments;
                        self.comments.loading = false;
                        self.comments.error = None;
                    }
                }
                Event::CommentsFailed { issue_id, message } | Event::CommentFailed { issue_id, message } => {
                    if self.panel.issue_id.as_deref() == Some(issue_id.as_str()) {
                        self.comments.loading = false;
                        self.comments.error = Some(message);
                    }
                }
                Event::CommentSaved { issue_id, comment } => {
                    if self.panel.issue_id.as_deref() == Some(issue_id.as_str()) {
                        match self.comments.list.iter_mut().find(|c| c.id == comment.id) {
                            Some(c) => *c = comment,
                            None => self.comments.list.push(comment),
                        }
                    }
                }
                Event::Uploaded { request_id, asset_url } => {
                    issue_panel::on_uploaded(&mut self.panel, request_id, &asset_url);
                }
                Event::UploadFailed { request_id, message } => {
                    issue_panel::on_upload_failed(&mut self.panel, request_id, &message);
                }
                Event::Image { url, bytes } => crate::images::insert(ctx, &url, bytes.to_vec()),
                Event::ImageFailed { url, message } => crate::images::failed(ctx, &url, &message),
                Event::KeyChecked(Ok(info)) => {
                    if self.form.name.trim().is_empty() {
                        self.form.name = info.suggested_slug.clone();
                    }
                    self.form.check = KeyCheck::Ok {
                        workspace: info.organization_name,
                        user: info.user_name,
                        teams: info.team_names,
                    };
                }
                Event::KeyChecked(Err(message)) => self.form.check = KeyCheck::Error(message),
                Event::WorkspacesChanged => {
                    self.workspaces = self.sync.workspaces();
                    let slugs: Vec<String> = self.workspaces.iter().map(|w| w.slug.clone()).collect();
                    self.boards.retain(|k, _| slugs.contains(k));
                    for slug in slugs {
                        self.reload_board(&slug);
                    }
                }
                _ => {}
            }
        }
    }

    /// `--screenshot`: render a few frames, grab the window, save it and quit.
    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        ctx.request_repaint();
        ctx.send_viewport_cmd(ViewportCommand::RequestPaintWhileHidden);
        shot.frames += 1;
        if !shot.requested && shot.frames >= 8 {
            let slot = shot.image.clone();
            ctx.request_screenshot(move |img| {
                if let Ok(mut s) = slot.lock() {
                    *s = Some(img);
                }
            });
            shot.requested = true;
        }
        let image = shot.image.lock().ok().and_then(|mut s| s.take());
        if let Some(img) = image {
            match save_png(&shot.path, &img) {
                Ok(()) => eprintln!("saved {}", shot.path.display()),
                Err(e) => eprintln!("screenshot failed: {e:#}"),
            }
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else if shot.started.elapsed() > Duration::from_secs(30) {
            eprintln!("screenshot timed out");
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let t = self.active_theme();
        if self.visuals_theme != Some(t) {
            ctx.set_visuals(t.visuals());
            self.visuals_theme = Some(t);
        }
        let mut actions = Vec::new();
        self.handle_events(&ctx);
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.focused && let Some(ws) = self.active.clone() {
            self.sync.send(Command::SyncNow { workspace: ws });
        }
        self.focused = focused;
        self.global_keys(&ctx, &mut actions);

        self.onboarding.claude.poll();
        // The onboarding flow has no rail, so the user sees only the flow.
        let mode = match self.screen {
            Screen::Board => Some(RailMode::Board),
            Screen::Workspaces => Some(RailMode::Adding),
            Screen::Settings => Some(RailMode::Settings),
            Screen::Onboarding => None,
        };
        if let Some(mode) = mode {
            let active = if mode == RailMode::Board { self.active.as_deref() } else { None };
            let me = self
                .active
                .as_ref()
                .and_then(|s| self.workspaces.iter().find(|w| &w.slug == s))
                .and_then(|ws| settings::viewer(ws, self.boards.get(&ws.slug)));
            Panel::left("lb-rail")
                .exact_size(rail::WIDTH)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::NONE.fill(t.base))
                .show(ui, |ui| rail::show(ui, &t, &self.workspaces, &self.boards, active, me, mode, &mut actions));
        }

        match self.screen {
            Screen::Onboarding => {
                let props = OnboardingProps {
                    workspaces: &self.workspaces,
                    theme: settings::theme_index(&self.theme),
                    exe: self.exe.as_deref(),
                };
                onboarding::page::show(ui, &t, &props, &mut self.onboarding, &mut self.form, &mut actions);
            }
            Screen::Workspaces => page::show(ui, &t, &self.workspaces, &self.boards, &mut self.form, &mut actions),
            Screen::Board => self.board_screen(ui, &t, &mut actions),
            Screen::Settings => {
                let props = SettingsProps {
                    workspaces: &self.workspaces,
                    boards: &self.boards,
                    active: self.active.as_deref(),
                    theme: settings::theme_index(&self.theme),
                };
                settings::page::show(ui, &t, &props, &mut self.settings, &mut actions);
            }
        }

        for a in actions {
            self.apply(&ctx, a);
        }
        for url in crate::images::take_requests(&ctx) {
            if self.mock {
                crate::images::insert(&ctx, &url, crate::images::mock_bytes(&url));
            } else if let Some(ws) = self.active.clone() {
                self.sync.send(Command::FetchImage { workspace: ws, url });
            }
        }
        self.show_toast(&ctx, &t);
        crate::brand::paint_pops(&ctx);
        self.screenshot(&ctx);
        // The "Synced 12 s ago" text needs a tick.
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.theme.base.to_normalized_gamma_f32()
    }
}

/// Binary path in mock mode, so screenshots do not show the paths of this machine.
const MOCK_EXE: &str = "/usr/bin/kanbanana";

fn mock_paths(s: &mut SettingsState) {
    s.exe = Some(PathBuf::from(MOCK_EXE));
    s.cache = Some(PathBuf::from("/home/me/.local/share/kanbanana/cache.sqlite3"));
    s.credentials = Some(PathBuf::from("/home/me/.config/kanbanana/credentials.json"));
}

fn mock_check(key: &str) -> KeyCheck {
    if key.starts_with("lin_api_") {
        KeyCheck::Ok {
            workspace: "bluebird".into(),
            user: "Kaarel".into(),
            teams: vec!["Shop".into(), "Service".into(), "IT".into()],
        }
    } else {
        KeyCheck::Error("Linear did not accept this key. Personal keys start with lin_api_.".into())
    }
}

/// The filled-in form from design 06.
fn demo_form() -> AddForm {
    AddForm {
        api_key: format!("lin_api_7Qm2{}Xk2Q", "x".repeat(32)),
        check: mock_check("lin_api_"),
        name: "bluebird".into(),
        shortcut: 3,
        accent: Accent::Gold,
        capturing: false,
        pending_remove: None,
    }
}

fn save_png(path: &std::path::Path, img: &ColorImage) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let [w, h] = img.size;
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, bytes).context("bad screenshot size")?;
    buf.save(path).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

impl App {
    /// `--screen panel*`: opens WEB-214 with the description and comments from the design.
    fn load_panel_demo(&mut self, screen: StartScreen) {
        let name = match screen {
            StartScreen::Panel => "panel",
            StartScreen::PanelComments => "panel-comments",
            StartScreen::PanelCommentEdit => "panel-comment-edit",
            StartScreen::PanelEdit => "panel-edit",
            _ => return,
        };
        let Some(board) = self.boards.get_mut("acme") else { return };
        let Some(issue) = board.issues.iter_mut().find(|i| i.identifier == "WEB-214") else { return };
        issue.description = issue_panel::MOCK_DESCRIPTION.into();
        let issue = issue.clone();
        issue_panel::demo(&mut self.panel, &issue, board, name);
        self.comments = issue_panel::mock_comments();
    }
}

/// True when the change moves the issue from an open state into a completed one.
fn completes(data: &BoardData, issue_id: &str, change: &IssueChange) -> bool {
    let IssueChange::State { state_id, .. } = change else { return false };
    let kind = |id: &str| data.states.iter().find(|s| s.id == id).map(|s| s.kind);
    let Some(issue) = data.issues.iter().find(|i| i.id == issue_id) else { return false };
    kind(state_id) == Some(StateType::Completed) && kind(&issue.state_id) != Some(StateType::Completed)
}

/// Comment and upload actions go to the engine. The rest go through the panel's own mapping.
fn take_engine_actions(issue_id: &str, panel: Vec<PanelAction>, actions: &mut Vec<Action>) -> Vec<PanelAction> {
    let mut rest = Vec::new();
    for a in panel {
        let issue_id = issue_id.to_string();
        match a {
            PanelAction::LoadComments => actions.push(Action::LoadComments { issue_id }),
            PanelAction::CreateComment { body, parent_id } => {
                actions.push(Action::CreateComment { issue_id, body, parent_id })
            }
            PanelAction::UpdateComment { comment_id, body } => {
                actions.push(Action::UpdateComment { issue_id, comment_id, body })
            }
            PanelAction::Upload { request_id, filename, content_type, bytes } => {
                actions.push(Action::Upload { request_id, filename, content_type, bytes })
            }
            other => rest.push(other),
        }
    }
    rest
}

impl App {
    fn select_issue(&mut self, id: Option<String>) {
        if id != self.panel.issue_id {
            self.comments = Default::default();
        }
        self.panel.issue_id = id;
    }

    /// The new card gets the focus ring and the board shows it. Without "Create more"
    /// the form closed, and the panel opens the issue.
    fn issue_created(&mut self, ws: &str, request_id: u64, issue: &Issue) {
        let Some(closed) = self.new_issue.on_created(request_id) else { return };
        if self.active.as_deref() != Some(ws) {
            return;
        }
        if let Some(b) = self.boards.get(ws) {
            self.board.reveal(ws, b, issue);
        }
        self.board.focus_on(&issue.id);
        if closed {
            self.select_issue(Some(issue.id.clone()));
        }
    }

    /// The theme with the accent of the workspace on screen.
    fn active_theme(&self) -> Theme {
        if self.screen == Screen::Workspaces {
            return self.theme.with_accent(self.form.accent);
        }
        if self.screen == Screen::Onboarding {
            let form = self.onboarding.step == Step::Workspace && self.onboarding.linked.is_none();
            return if form { self.theme.with_accent(self.form.accent) } else { self.theme };
        }
        let ws = self.active.as_ref().and_then(|s| self.workspaces.iter().find(|w| &w.slug == s));
        match ws {
            Some(ws) => self.theme.with_accent(ws.accent),
            None => self.theme,
        }
    }

    /// The done pop starts at the card. A card under the issue panel is hidden, so the pop
    /// starts at the pointer, which is on the status the user just picked.
    fn pop_banana(&self, ctx: &egui::Context, issue_id: &str) {
        let panel_left = ctx.content_rect().right() - issue_panel::WIDTH;
        let panel_open = self.panel.issue_id.is_some();
        let card = self.board.card_rect(issue_id).filter(|r| !panel_open || r.center().x < panel_left);
        let at = card.map(|r| r.center()).or_else(|| ctx.input(|i| i.pointer.latest_pos()));
        if let Some(at) = at {
            crate::brand::pop(ctx, at);
        }
    }

    /// "Copied" toast at the bottom center for 2.5 s after a copy.
    fn show_toast(&mut self, ctx: &egui::Context, t: &Theme) {
        let Some((text, at)) = &self.toast else { return };
        let age = at.elapsed().as_secs_f32();
        if age > 2.5 {
            self.toast = None;
            return;
        }
        // Smooth frames while it moves or fades, a slow tick while it only waits.
        if age < 0.12 || age > 2.2 {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        // Fades and slides up 8 px in 120 ms, then fades out in the last 300 ms.
        let enter = 1.0 - (1.0 - (age / 0.12).clamp(0.0, 1.0)).powi(3);
        let alpha = ((2.5 - age) / 0.3).clamp(0.0, 1.0) * enter;
        let lift = 8.0 * (1.0 - enter);
        // With the issue panel open, the toast sits over the panel, above its composer.
        let toast_offset = if self.panel.issue_id.is_some() && self.screen == Screen::Board {
            let w = ctx.content_rect().width();
            vec2(w / 2.0 - issue_panel::WIDTH / 2.0, -96.0)
        } else {
            vec2(0.0, -24.0)
        };
        egui::Area::new(egui::Id::NULL.with("lb-toast"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, toast_offset + vec2(0.0, lift))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                Frame::NONE
                    .fill(t.overlay)
                    .stroke(egui::Stroke::new(1.0, t.hl_med))
                    .corner_radius(10)
                    .shadow(t.shadow_sm())
                    .inner_margin(egui::Margin { left: 14, right: 16, top: 10, bottom: 10 })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let p = ui.painter().clone();
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                            parts::icon(&p, r.center(), parts::Icon::Check, 15.0, t.foam);
                            ui.label(egui::RichText::new("Copied").font(theme::font(W::SemiBold, 13.0)).color(t.text));
                            ui.add(
                                egui::Label::new(egui::RichText::new(text).font(theme::font(W::MonoMedium, 12.0)).color(t.subtle))
                                    .truncate(),
                            )
                            .on_hover_text(text.as_str());
                        });
                    });
            });
    }
}
