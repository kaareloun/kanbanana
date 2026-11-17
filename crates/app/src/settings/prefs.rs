//! `<config_dir>/kanbanana/settings.json`: settings that are not part of the cache.

use std::path::PathBuf;

use anyhow::Context as _;
use serde_json::{Map, Value};

use crate::theme::Theme;

fn path() -> Option<PathBuf> {
    store::move_old_folders();
    Some(dirs::config_dir()?.join("kanbanana").join("settings.json"))
}

/// The saved theme. Main when the file is missing or bad.
pub fn load_theme() -> Theme {
    theme_from(&read())
}

fn theme_from(json: &str) -> Theme {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| v.get("theme")?.as_str().and_then(Theme::by_name))
        .unwrap_or(Theme::MAIN)
}

/// Writes the theme name and keeps the other keys in the file.
pub fn save_theme(name: &str) -> anyhow::Result<()> {
    save_key("theme", Value::String(name.into()))
}

/// True when the user finished or skipped the onboarding flow.
pub fn load_onboarded() -> bool {
    onboarded_from(&read())
}

/// Sets or clears the onboarding flag and keeps the other keys in the file.
pub fn save_onboarded(done: bool) -> anyhow::Result<()> {
    save_key("onboarded", Value::Bool(done))
}

fn read() -> String {
    path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default()
}

fn onboarded_from(json: &str) -> bool {
    serde_json::from_str::<Value>(json).ok().and_then(|v| v.get("onboarded")?.as_bool()).unwrap_or(false)
}

/// The file text with `key` set to `value`. A bad or missing file starts empty.
fn with_key(old: &str, key: &str, value: Value) -> anyhow::Result<String> {
    let mut map = match serde_json::from_str::<Value>(old) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    };
    map.insert(key.into(), value);
    Ok(serde_json::to_string_pretty(&Value::Object(map))? + "\n")
}

fn save_key(key: &str, value: Value) -> anyhow::Result<()> {
    let path = path().context("no config folder")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let text = with_key(&old, key, value)?;
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_falls_back_to_main() {
        assert_eq!(theme_from(r#"{"theme":"dawn"}"#), Theme::DAWN);
        assert_eq!(theme_from(r#"{"theme":"moon","x":1}"#), Theme::MOON);
        assert_eq!(theme_from(r#"{"theme":"neon"}"#), Theme::MAIN);
        assert_eq!(theme_from("not json"), Theme::MAIN);
        assert_eq!(theme_from(""), Theme::MAIN);
    }

    #[test]
    fn onboarded_flag_round_trip_keeps_other_keys() {
        assert!(!onboarded_from(""));
        assert!(!onboarded_from(r#"{"theme":"dawn"}"#));
        let text = with_key(r#"{"theme":"dawn"}"#, "onboarded", Value::Bool(true)).unwrap();
        assert!(onboarded_from(&text));
        assert_eq!(theme_from(&text), Theme::DAWN);
        let text = with_key(&text, "onboarded", Value::Bool(false)).unwrap();
        assert!(!onboarded_from(&text));
        assert_eq!(theme_from(&text), Theme::DAWN);
        assert!(onboarded_from(&with_key("not json", "onboarded", Value::Bool(true)).unwrap()));
    }
}
