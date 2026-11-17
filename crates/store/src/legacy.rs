//! The app had other names before "kanbanana": first "linear-board", then "lineboard".
//! On the first start under the new name, this moves the old data and config folders
//! to the new name, so the cache, the API keys and the settings keep working.

use std::path::Path;
use std::sync::Once;

/// Old folder names, newest first. The first one that exists wins.
const OLD: [&str; 2] = ["lineboard", "linear-board"];
const NEW: &str = "kanbanana";

static MOVE: Once = Once::new();

/// Runs once per process. Errors only go to stderr: a failed move leaves the old
/// folder in place and the app starts with an empty new one.
pub fn move_old_folders() {
    MOVE.call_once(|| {
        for base in [dirs::data_dir(), dirs::config_dir()].into_iter().flatten() {
            move_folder(&base);
        }
    });
}

fn move_folder(base: &Path) {
    let new = base.join(NEW);
    // On macOS the data and config folders are the same, so the second call finds nothing.
    if new.exists() {
        return;
    }
    let Some(old) = OLD.iter().map(|o| base.join(o)).find(|p| p.is_dir()) else {
        return;
    };
    if let Err(e) = std::fs::rename(&old, &new) {
        eprintln!("kanbanana: cannot move {} to {}: {e}", old.display(), new.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("kb-legacy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        base
    }

    #[test]
    fn moves_the_oldest_name_when_it_is_the_only_one() {
        let base = temp("one");
        std::fs::create_dir_all(base.join("linear-board")).unwrap();
        std::fs::write(base.join("linear-board").join("credentials.json"), "{}").unwrap();
        move_folder(&base);
        assert!(!base.join("linear-board").exists());
        assert!(base.join(NEW).join("credentials.json").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn prefers_the_newest_old_name_and_never_overwrites() {
        let base = temp("two");
        std::fs::create_dir_all(base.join("lineboard")).unwrap();
        std::fs::create_dir_all(base.join("linear-board")).unwrap();
        move_folder(&base);
        assert!(!base.join("lineboard").exists());
        assert!(base.join("linear-board").exists());
        // With the new folder in place, nothing else moves.
        move_folder(&base);
        assert!(base.join("linear-board").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
