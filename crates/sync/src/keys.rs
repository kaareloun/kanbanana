//! Checks a new API key before the user adds the workspace.

use linear_api::Client;

use crate::KeyInfo;

pub(crate) fn check_key(key: &str) -> Result<KeyInfo, String> {
    let client = Client::new(key.trim());
    let viewer = client.viewer().map_err(|e| e.to_string())?;
    // The slug only names the team rows here. They are not stored.
    let catalog = client.catalog("check").map_err(|e| e.to_string())?;
    Ok(KeyInfo {
        user_name: viewer.name,
        suggested_slug: suggest_slug(&viewer.organization_name),
        organization_name: viewer.organization_name,
        team_names: catalog.teams.into_iter().map(|t| t.name).collect(),
    })
}

/// Lowercase organization name. Each run of other characters than ASCII letters
/// and digits becomes one '-', with none at the ends.
pub fn suggest_slug(organization_name: &str) -> String {
    let mut slug = String::new();
    for c in organization_name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(suggest_slug("Acme"), "acme");
        assert_eq!(suggest_slug("Iron Baltic OÜ"), "iron-baltic-o");
        assert_eq!(suggest_slug("  Bluebird & Co. "), "bluebird-co");
        assert_eq!(suggest_slug("a__b--c"), "a-b-c");
        assert_eq!(suggest_slug("Õun"), "un");
        assert_eq!(suggest_slug("***"), "");
    }
}
