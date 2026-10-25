//! User settings (PRD 7.9): `settings.json` in the data dir, written atomically.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The user moved past the Full Disk Access onboarding, granted or not (US-01).
    pub onboarding_done: bool,
}

fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

/// Defaults when the file is missing or unreadable; a corrupt file never panics.
pub fn load(data_dir: &Path) -> Settings {
    let text = match fs::read_to_string(settings_path(data_dir)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
        Err(e) => {
            tracing::warn!("ignoring unreadable settings: {e}");
            return Settings::default();
        }
    };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        tracing::warn!("ignoring corrupt settings: {e}");
        Settings::default()
    })
}

pub fn save(data_dir: &Path, settings: &Settings) -> Result<()> {
    fs::create_dir_all(data_dir)?;
    let json = serde_json::to_vec_pretty(settings).map_err(|e| Error::Settings(e.to_string()))?;
    let path = settings_path(data_dir);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?; // atomic on the same volume
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_settings_load_back() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            onboarding_done: true,
        };
        save(dir.path(), &settings).unwrap();
        assert_eq!(load(dir.path()), settings);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), Settings::default());
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(settings_path(dir.path()), "{not json").unwrap();
        assert_eq!(load(dir.path()), Settings::default());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            settings_path(dir.path()),
            r#"{"onboarding_done": true, "later": 1}"#,
        )
        .unwrap();
        assert!(load(dir.path()).onboarding_done);
    }
}
