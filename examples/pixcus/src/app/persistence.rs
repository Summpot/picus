use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::*;

const AUTH_FILE_NAME: &str = "auth_session.json";
const AUTH_NAMESPACE: &str = "picus_core";
const LEGACY_AUTH_NAMESPACE: &str = concat!("pi", "cus");
const AUTH_APP_DIR: &str = "pixcus";
const LEGACY_AUTH_APP_DIR: &str = "pixiv_client";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StoredAuthState {
    pub session: AuthSession,
    pub user_summary: Option<AuthUserSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedAuth {
    version: u8,
    saved_at_epoch_seconds: u64,
    session: AuthSession,
    #[serde(default)]
    user_summary: Option<AuthUserSummary>,
}

pub(super) fn load_auth_state() -> Result<Option<StoredAuthState>> {
    let primary = auth_file_path();
    if let Some(auth) = load_auth_state_from_path(&primary)? {
        return Ok(Some(auth));
    }

    for legacy in legacy_auth_file_paths() {
        if let Some(auth) = load_auth_state_from_path(&legacy)? {
            return Ok(Some(auth));
        }
    }

    Ok(None)
}

pub(super) fn save_auth_state(auth: &StoredAuthState) -> Result<()> {
    save_auth_state_to_path(&auth_file_path(), auth)
}

pub(super) fn clear_auth_state() -> Result<()> {
    clear_auth_state_at_path(&auth_file_path())?;
    for legacy in legacy_auth_file_paths() {
        clear_auth_state_at_path(&legacy)?;
    }
    Ok(())
}

fn load_auth_state_from_path(path: &Path) -> Result<Option<StoredAuthState>> {
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

    Ok(Some(StoredAuthState {
        session: persisted.session,
        user_summary: persisted.user_summary,
    }))
}

fn save_auth_state_to_path(path: &Path, auth: &StoredAuthState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create auth session directory `{}`",
                parent.display()
            )
        })?;
    }

    let payload = PersistedAuth {
        version: 2,
        saved_at_epoch_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default(),
        session: auth.session.clone(),
        user_summary: auth.user_summary.clone(),
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

fn clear_auth_state_at_path(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to clear auth session file `{}`", path.display())),
    }
}

fn auth_file_path() -> PathBuf {
    auth_base_dir(AUTH_NAMESPACE, AUTH_APP_DIR).join(AUTH_FILE_NAME)
}

fn legacy_auth_file_paths() -> [PathBuf; 2] {
    [
        auth_base_dir(AUTH_NAMESPACE, LEGACY_AUTH_APP_DIR).join(AUTH_FILE_NAME),
        auth_base_dir(LEGACY_AUTH_NAMESPACE, LEGACY_AUTH_APP_DIR).join(AUTH_FILE_NAME),
    ]
}

fn auth_base_dir(namespace: &str, app_dir: &str) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        home.join("Library")
            .join("Application Support")
            .join(namespace)
            .join(app_dir)
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
        return base.join(namespace).join(app_dir);
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
        base.join(namespace).join(app_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_state_round_trip_preserves_user_summary() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "{AUTH_NAMESPACE}-pixiv-auth-test-{}-{nanos}.json",
            std::process::id()
        ));

        let sample = StoredAuthState {
            session: AuthSession {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                token_type: "bearer".to_string(),
                expires_in: 3600,
                scope: "all".to_string(),
            },
            user_summary: Some(AuthUserSummary {
                id: 99,
                name: "summpot".to_string(),
                account: Some("user_knrk3528".to_string()),
                avatar_url: Some("https://example.com/avatar.png".to_string()),
            }),
        };

        save_auth_state_to_path(&path, &sample).expect("save should succeed");
        let loaded = load_auth_state_from_path(&path)
            .expect("load should succeed")
            .expect("auth state should exist");

        assert_eq!(loaded, sample);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_auth_state_accepts_legacy_payload_without_user_summary() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let legacy_path = std::env::temp_dir().join(format!(
            "{LEGACY_AUTH_NAMESPACE}-pixiv-auth-legacy-test-{}-{nanos}.json",
            std::process::id()
        ));

        let legacy_payload = r#"{
            "version": 1,
            "saved_at_epoch_seconds": 123,
            "session": {
                "access_token": "legacy-access",
                "refresh_token": "legacy-refresh",
                "token_type": "bearer",
                "expires_in": 3600,
                "scope": "all"
            }
        }"#;

        fs::write(&legacy_path, legacy_payload).expect("legacy payload should write");
        let loaded = load_auth_state_from_path(&legacy_path)
            .expect("legacy load should succeed")
            .expect("legacy auth state should exist");

        assert_eq!(loaded.session.access_token, "legacy-access");
        assert_eq!(loaded.session.refresh_token, "legacy-refresh");
        assert!(loaded.user_summary.is_none());

        let _ = fs::remove_file(legacy_path);
    }

    #[test]
    fn clear_auth_state_removes_persisted_file() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "{AUTH_NAMESPACE}-pixiv-auth-clear-test-{}-{nanos}.json",
            std::process::id()
        ));

        let sample = StoredAuthState {
            session: AuthSession {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                token_type: "bearer".to_string(),
                expires_in: 3600,
                scope: "all".to_string(),
            },
            user_summary: None,
        };

        save_auth_state_to_path(&path, &sample).expect("save should succeed");
        clear_auth_state_at_path(&path).expect("clear should succeed");
        assert!(
            load_auth_state_from_path(&path)
                .expect("load after clear should succeed")
                .is_none()
        );
    }
}
