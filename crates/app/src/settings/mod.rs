//! Settings: accounts, the Claude Code link, the theme and where the app keeps its files.

pub mod page;
mod prefs;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use model::{BoardData, User, Workspace};

pub use prefs::{load_onboarded, load_theme, save_onboarded, save_theme};

use crate::theme::Theme;

/// Theme names in the settings file and their labels in the theme picker.
pub const THEMES: [(&str, &str, Theme); 9] = [
    ("main", "Rosé Pine", Theme::MAIN),
    ("moon", "Rosé Pine Moon", Theme::MOON),
    ("dawn", "Rosé Pine Dawn", Theme::DAWN),
    ("catppuccin-mocha", "Catppuccin Mocha", Theme::CATPPUCCIN_MOCHA),
    ("catppuccin-latte", "Catppuccin Latte", Theme::CATPPUCCIN_LATTE),
    ("tokyo-night", "Tokyo Night", Theme::TOKYO_NIGHT),
    ("gruvbox-dark", "Gruvbox Dark", Theme::GRUVBOX_DARK),
    ("nord", "Nord", Theme::NORD),
    ("dracula", "Dracula", Theme::DRACULA),
];

/// Index of a base theme in `THEMES`. Main when it is none of them.
pub fn theme_index(t: &Theme) -> usize {
    THEMES.iter().position(|(_, _, x)| x == t).unwrap_or(0)
}

/// State of the Settings screen.
#[derive(Clone, Debug, Default)]
pub struct SettingsState {
    /// Slug whose Remove button waits for a second click.
    pub pending_remove: Option<String>,
    /// egui time when the screen opened, for the fade in.
    pub opened_at: f64,
    /// Scrolls to Appearance on the next frame. The mock `settings-appearance` screen sets it.
    pub scroll_to_appearance: bool,
    pub exe: Option<PathBuf>,
    pub cache: Option<PathBuf>,
    pub credentials: Option<PathBuf>,
}

impl SettingsState {
    pub fn open(opened_at: f64) -> SettingsState {
        SettingsState {
            pending_remove: None,
            opened_at,
            scroll_to_appearance: false,
            exe: std::env::current_exe().ok(),
            cache: store::default_path().ok(),
            credentials: store::secrets::credentials_path().ok(),
        }
    }
}

/// "12 s ago" for the top bar and the account cards.
pub fn ago(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let s = (now - at).num_seconds().max(0);
    match s {
        0..60 => format!("{s} s ago"),
        60..3600 => format!("{} min ago", s / 60),
        3600..86400 => format!("{} h ago", s / 3600),
        _ => format!("{} d ago", s / 86400),
    }
}

/// The key owner of a workspace, from the board's users.
pub fn viewer<'a>(ws: &Workspace, board: Option<&'a BoardData>) -> Option<&'a User> {
    let board = board?;
    let id = board.viewer_id.as_ref().or(ws.viewer_id.as_ref())?;
    board.users.iter().find(|u| &u.id == id)
}

/// Opens the folder that holds `file` in the system file manager.
pub fn open_folder(file: &Path) {
    let Some(dir) = file.parent() else { return };
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    match std::process::Command::new(opener).arg(dir).spawn() {
        // Wait on a thread, so the child does not stay as a zombie process.
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(e) => eprintln!("kanbanana: {opener} {}: {e}", dir.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::ago;
    use chrono::{Duration, Utc};

    #[test]
    fn ago_units() {
        let now = Utc::now();
        assert_eq!(ago(now - Duration::seconds(12), now), "12 s ago");
        assert_eq!(ago(now - Duration::minutes(5), now), "5 min ago");
        assert_eq!(ago(now - Duration::hours(2), now), "2 h ago");
        assert_eq!(ago(now + Duration::seconds(3), now), "0 s ago");
    }
}
