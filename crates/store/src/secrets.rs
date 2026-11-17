//! API keys in a file that only the user can read:
//! `<config_dir>/kanbanana/credentials.json`, mode 0600, one key per workspace slug.
//! This is how the gh CLI and Claude Code keep tokens on Linux. It needs no keyring
//! daemon and never shows a password prompt.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// `$KANBANANA_CREDENTIALS`, else `<config_dir>/kanbanana/credentials.json`.
pub fn credentials_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("KANBANANA_CREDENTIALS") {
        return Ok(PathBuf::from(p));
    }
    crate::move_old_folders();
    let dir = dirs::config_dir().ok_or(Error::NoDataDir)?;
    Ok(dir.join("kanbanana").join("credentials.json"))
}

pub fn set_api_key(slug: &str, key: &str) -> Result<()> {
    let path = credentials_path()?;
    let mut keys = read_keys(&path)?;
    keys.insert(slug.to_string(), key.trim().to_string());
    write_keys(&path, &keys)
}

/// None when there is no key for this workspace.
pub fn get_api_key(slug: &str) -> Result<Option<String>> {
    Ok(read_keys(&credentials_path()?)?.remove(slug))
}

/// Does nothing when the key is already gone.
pub fn delete_api_key(slug: &str) -> Result<()> {
    let path = credentials_path()?;
    let mut keys = read_keys(&path)?;
    if keys.remove(slug).is_some() {
        write_keys(&path, &keys)?;
    }
    Ok(())
}

fn read_keys(path: &Path) -> Result<BTreeMap<String, String>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| Error::Corrupt(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.into()),
    }
}

/// Writes a temp file with mode 0600 and renames it over the old one, so a crash
/// never leaves a half-written file and other users can never read the keys.
fn write_keys(path: &Path, keys: &BTreeMap<String, String>) -> Result<()> {
    let dir = path.parent().ok_or(Error::NoDataDir)?;
    fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(keys)
        .map_err(|e| Error::Corrupt(format!("cannot encode credentials: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(&tmp)?;
    file.write_all(&json)?;
    file.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_with_private_mode() {
        let path = std::env::temp_dir()
            .join(format!("lb-cred-test-{}", std::process::id()))
            .join("credentials.json");
        let mut keys = read_keys(&path).unwrap();
        assert!(keys.is_empty());
        keys.insert("acme".into(), "lin_api_x".into());
        write_keys(&path, &keys).unwrap();
        assert_eq!(read_keys(&path).unwrap().get("acme").map(String::as_str), Some("lin_api_x"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        keys.remove("acme");
        write_keys(&path, &keys).unwrap();
        assert!(read_keys(&path).unwrap().is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
