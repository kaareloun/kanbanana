//! Workspace rail, workspace switching and the Workspaces page.

pub mod page;
pub mod rail;

use model::{Accent, Workspace};

/// Result of checking an API key against Linear.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum KeyCheck {
    #[default]
    Idle,
    Checking,
    Ok { workspace: String, user: String, teams: Vec<String> },
    Error(String),
}

/// State of the "Add a workspace" form.
#[derive(Clone, Debug, PartialEq)]
pub struct AddForm {
    pub api_key: String,
    pub check: KeyCheck,
    pub name: String,
    pub shortcut: u8,
    pub accent: Accent,
    /// True while the shortcut field waits for Alt and a number.
    pub capturing: bool,
    /// Slug whose Remove button waits for a second click.
    pub pending_remove: Option<String>,
}

impl Default for AddForm {
    fn default() -> Self {
        AddForm {
            api_key: String::new(),
            check: KeyCheck::Idle,
            name: String::new(),
            shortcut: 1,
            accent: Accent::Iris,
            capturing: false,
            pending_remove: None,
        }
    }
}

impl AddForm {
    /// A fresh form with the first free shortcut and an unused color.
    pub fn fresh(existing: &[Workspace]) -> Self {
        let shortcut = (1..=9).find(|n| existing.iter().all(|w| w.shortcut != *n)).unwrap_or(9);
        let accent = ACCENTS
            .into_iter()
            .find(|a| existing.iter().all(|w| w.accent != *a))
            .unwrap_or(Accent::Iris);
        AddForm { shortcut, accent, ..Default::default() }
    }
}

pub const ACCENTS: [Accent; 6] =
    [Accent::Iris, Accent::Foam, Accent::Gold, Accent::Rose, Accent::Pine, Accent::Love];

/// Local slug from a display name: lower case, letters, digits and dashes.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// First letter for the rail tile.
pub fn letter(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_names() {
        assert_eq!(slugify("Arvuti Tark OÜ"), "arvuti-tark-oü");
        assert_eq!(slugify("  acme "), "acme");
        assert_eq!(slugify("a--b!"), "a-b");
    }

    #[test]
    fn fresh_form_skips_taken_shortcuts() {
        let ws = crate::mock::workspaces();
        let f = AddForm::fresh(&ws[..2]);
        assert_eq!(f.shortcut, 3);
        assert_eq!(f.accent, Accent::Gold);
    }
}
