//! Turns the names Claude uses (state names, "me", label names) into Linear ids.

use anyhow::{Context, anyhow, bail};
use model::{BoardData, Issue, Priority};
use store::Db;

/// Finds a cached issue. Without a workspace it searches all workspaces.
pub fn issue(db: &Db, ws: Option<&str>, identifier: &str) -> anyhow::Result<Issue> {
    store::find_issue(db, ws, identifier)?.ok_or_else(|| {
        anyhow!(
            "No issue {identifier} in the local cache. Check the id, or open Kanbanana once \
             so it syncs."
        )
    })
}

pub fn board(db: &Db, ws: &str) -> anyhow::Result<BoardData> {
    Ok(store::load_board(db, ws)?)
}

pub fn client(ws: &str) -> anyhow::Result<linear_api::Client> {
    let key = store::secrets::get_api_key(ws)?
        .with_context(|| format!("No API key for workspace {ws}. Add it in Kanbanana."))?;
    Ok(linear_api::Client::new(&key))
}

pub fn state_id(board: &BoardData, team_id: &str, name: &str) -> anyhow::Result<String> {
    let states: Vec<_> = board.states.iter().filter(|s| s.team_id == team_id).collect();
    states
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .map(|s| s.id.clone())
        .ok_or_else(|| {
            let names: Vec<_> = states.iter().map(|s| s.name.as_str()).collect();
            anyhow!("Unknown state \"{name}\". This team has: {}.", names.join(", "))
        })
}

/// "me", "none", a name, a display name or an email-like prefix.
pub fn user_id(board: &BoardData, who: &str) -> anyhow::Result<Option<String>> {
    match who.to_ascii_lowercase().as_str() {
        "none" | "" => return Ok(None),
        "me" => {
            return board
                .viewer_id
                .clone()
                .map(Some)
                .ok_or_else(|| anyhow!("The app has not synced the key owner yet."));
        }
        _ => {}
    }
    let found: Vec<_> = board
        .users
        .iter()
        .filter(|u| {
            u.name.eq_ignore_ascii_case(who)
                || u.display_name.eq_ignore_ascii_case(who)
                || u.name.to_lowercase().starts_with(&who.to_lowercase())
        })
        .collect();
    match found.as_slice() {
        [u] => Ok(Some(u.id.clone())),
        [] => bail!("No user matches \"{who}\"."),
        many => {
            let names: Vec<_> = many.iter().map(|u| u.name.as_str()).collect();
            bail!("\"{who}\" matches more than one user: {}.", names.join(", "))
        }
    }
}

/// Labels that apply to the team: its own and the workspace-wide ones.
pub fn label_ids(board: &BoardData, team_id: &str, names: &[String]) -> anyhow::Result<Vec<String>> {
    names
        .iter()
        .map(|name| {
            board
                .labels
                .iter()
                .find(|l| {
                    l.name.eq_ignore_ascii_case(name)
                        && l.team_id.as_deref().is_none_or(|t| t == team_id)
                })
                .map(|l| l.id.clone())
                .ok_or_else(|| anyhow!("Unknown label \"{name}\"."))
        })
        .collect()
}

pub fn priority(name: &str) -> anyhow::Result<Priority> {
    Priority::ALL
        .into_iter()
        .find(|p| p.label().eq_ignore_ascii_case(name) || (name.eq_ignore_ascii_case("none") && *p == Priority::None))
        .ok_or_else(|| anyhow!("Unknown priority \"{name}\". Use urgent, high, medium, low or none."))
}

/// Issue ids in free text, for example the branch "web-214-refund" gives "WEB-214".
pub fn identifiers_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_alphabetic() && (i == 0 || !chars[i - 1].is_ascii_alphanumeric()) {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphanumeric() && !chars[i].is_ascii_digit() {
                i += 1;
            }
            let key_len = i - start;
            if key_len >= 2 && key_len <= 7 && i < chars.len() && chars[i] == '-' {
                let digits_start = i + 1;
                let mut j = digits_start;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                let ends_clean = j == chars.len() || !chars[j].is_ascii_alphanumeric();
                if j > digits_start && ends_clean {
                    let key: String = chars[start..i].iter().collect();
                    let num: String = chars[digits_start..j].iter().collect();
                    let id = format!("{}-{}", key.to_ascii_uppercase(), num);
                    if !out.contains(&id) {
                        out.push(id);
                    }
                    i = j;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_ids_in_branch_names() {
        assert_eq!(identifiers_in("web-214-refund-state"), vec!["WEB-214"]);
        assert_eq!(identifiers_in("feature/NW-12_and_BB-7"), vec!["NW-12", "BB-7"]);
        assert_eq!(identifiers_in("fix: x (BB-1234)"), vec!["BB-1234"]);
        assert!(identifiers_in("refund-webhook").is_empty());
    }

    #[test]
    fn parses_priorities() {
        assert_eq!(priority("urgent").unwrap(), Priority::Urgent);
        assert_eq!(priority("none").unwrap(), Priority::None);
        assert!(priority("whatever").is_err());
    }
}
