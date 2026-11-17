//! Typed user configuration stored under the XDG config directory.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Game directories to scan. Absolute paths only; never written to by the scanner.
    #[serde(default)]
    pub game_dirs: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Json(serde_json::Error),
    RelativePath(PathBuf),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "configuration I/O error: {error}"),
            Self::Json(error) => write!(f, "invalid configuration file: {error}"),
            Self::RelativePath(path) => {
                write!(f, "game directory must be absolute: {}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl Config {
    /// Loads the config file. A missing file is a valid empty configuration.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(serde_json::from_str(&text)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Writes the config file atomically enough for user settings: create the
    /// parent directory, then replace the file contents.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text)?;
        Ok(())
    }

    /// Adds an absolute directory. Returns false when already present.
    pub fn add_dir(&mut self, dir: PathBuf) -> Result<bool, ConfigError> {
        if !dir.is_absolute() {
            return Err(ConfigError::RelativePath(dir));
        }
        if self.game_dirs.contains(&dir) {
            return Ok(false);
        }
        self.game_dirs.push(dir);
        Ok(true)
    }

    pub fn remove_dir(&mut self, dir: &Path) -> bool {
        let before = self.game_dirs.len();
        self.game_dirs.retain(|candidate| candidate != dir);
        self.game_dirs.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError};
    use std::path::PathBuf;

    fn temp_config_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("b-emu-config-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn missing_file_is_an_empty_config() {
        let path = temp_config_path("missing");
        let _ = std::fs::remove_file(&path);
        assert_eq!(Config::load(&path).unwrap(), Config::default());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.json");
        let mut config = Config::default();
        assert!(config.add_dir(PathBuf::from("/home/user/ROMs")).unwrap());

        config.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), config);
    }

    #[test]
    fn rejects_relative_and_duplicate_directories() {
        let mut config = Config::default();
        assert!(matches!(
            config.add_dir(PathBuf::from("roms")),
            Err(ConfigError::RelativePath(_))
        ));
        assert!(config.add_dir(PathBuf::from("/roms")).unwrap());
        assert!(!config.add_dir(PathBuf::from("/roms")).unwrap());
    }

    #[test]
    fn removes_a_directory() {
        let mut config = Config::default();
        config.add_dir(PathBuf::from("/roms")).unwrap();
        assert!(config.remove_dir(&PathBuf::from("/roms")));
        assert!(config.game_dirs.is_empty());
    }
}
