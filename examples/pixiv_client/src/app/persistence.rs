use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::*;

const AUTH_FILE_NAME: &str = "auth_session.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedAuth {
    version: u8,
    saved_at_epoch_seconds: u64,
    session: AuthSession,
}

pub(super) fn load_auth_session() -> Result<Option<AuthSession>> {
    load_auth_session_from_path(&auth_file_path())
}

pub(super) fn save_auth_session(session: &AuthSession) -> Result<()> {
    save_auth_session_to_path(&auth_file_path(), session)
}

fn load_auth_session_from_path(path: &Path) -> Result<Option<AuthSession>> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read auth session file `{}`", path.display()));
        }
    };

    let persisted: PersistedAuth = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse auth session json `{}`", path.display()))?;

    Ok(Some(persisted.session))
}

fn save_auth_session_to_path(path: &Path, session: &AuthSession) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create auth session directory `{}`",
                parent.display()
            )
        })?;
    }

    let payload = PersistedAuth {
        version: 1,
        saved_at_epoch_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default(),
        session: session.clone(),
    };

    let serialized = serde_json::to_string_pretty(&payload)
        .context("failed to serialize auth session payload")?;
    fs::write(path, serialized)
        .with_context(|| format!("failed to write auth session file `{}`", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

fn auth_file_path() -> PathBuf {
    auth_base_dir().join(AUTH_FILE_NAME)
}

fn auth_base_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        return home
            .join("Library")
            .join("Application Support")
            .join("bevy_xilem")
            .join("pixiv_client");
    }

    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .map(|path| path.join("AppData").join("Roaming"))
            })
            .unwrap_or_else(std::env::temp_dir);
        return base.join("bevy_xilem").join("pixiv_client");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|path| path.join(".config"))
            })
            .unwrap_or_else(std::env::temp_dir);
        base.join("bevy_xilem").join("pixiv_client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_session_round_trip_works() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bevy-xilem-pixiv-auth-test-{}-{nanos}.json",
            std::process::id()
        ));

        let sample = AuthSession {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            token_type: "bearer".to_string(),
            expires_in: 3600,
            scope: "all".to_string(),
        };

        save_auth_session_to_path(&path, &sample).expect("save should succeed");
        let loaded = load_auth_session_from_path(&path)
            .expect("load should succeed")
            .expect("session should exist");

        assert_eq!(loaded.access_token, sample.access_token);
        assert_eq!(loaded.refresh_token, sample.refresh_token);

        let _ = fs::remove_file(path);
    }
}
