use std::env;
use std::path::PathBuf;

pub const APP_ID: &str = "b-emu";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathsError {
    MissingHome,
    RelativeXdgPath(&'static str),
}

impl std::fmt::Display for PathsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingHome => {
                write!(f, "HOME is not set; cannot determine user data directories")
            }
            Self::RelativeXdgPath(name) => write!(
                f,
                "{name} must be an absolute path when it is used by B-Emu"
            ),
        }
    }
}

impl std::error::Error for PathsError {}

impl AppPaths {
    pub fn from_env() -> Result<Self, PathsError> {
        let home = absolute_env("HOME", None, "HOME")?;

        Ok(Self {
            config_dir: absolute_env(
                "XDG_CONFIG_HOME",
                Some(home.join(".config")),
                "XDG_CONFIG_HOME",
            )?
            .join(APP_ID),
            data_dir: absolute_env(
                "XDG_DATA_HOME",
                Some(home.join(".local/share")),
                "XDG_DATA_HOME",
            )?
            .join(APP_ID),
            cache_dir: absolute_env(
                "XDG_CACHE_HOME",
                Some(home.join(".cache")),
                "XDG_CACHE_HOME",
            )?
            .join(APP_ID),
            runtime_dir: optional_absolute_env("XDG_RUNTIME_DIR", "XDG_RUNTIME_DIR")?
                .map(|dir| dir.join(APP_ID)),
        })
    }

    pub fn writable_dirs(&self) -> [PathBuf; 3] {
        [
            self.config_dir.clone(),
            self.data_dir.clone(),
            self.cache_dir.clone(),
        ]
    }
}

fn absolute_env(
    name: &'static str,
    fallback: Option<PathBuf>,
    error_name: &'static str,
) -> Result<PathBuf, PathsError> {
    match env::var_os(name) {
        Some(value) => absolute_path(name, PathBuf::from(value)),
        None => fallback.ok_or(PathsError::MissingHome),
    }
    .map_err(|error| match error {
        PathsError::RelativeXdgPath(_) => PathsError::RelativeXdgPath(error_name),
        other => other,
    })
}

fn optional_absolute_env(
    name: &'static str,
    error_name: &'static str,
) -> Result<Option<PathBuf>, PathsError> {
    env::var_os(name)
        .map(|value| absolute_path(name, PathBuf::from(value)))
        .transpose()
        .map_err(|error| match error {
            PathsError::RelativeXdgPath(_) => PathsError::RelativeXdgPath(error_name),
            other => other,
        })
}

fn absolute_path(name: &'static str, path: PathBuf) -> Result<PathBuf, PathsError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(PathsError::RelativeXdgPath(name))
    }
}

#[cfg(test)]
mod tests {
    use super::{APP_ID, AppPaths};

    #[test]
    fn app_id_is_stable() {
        assert_eq!(APP_ID, "b-emu");
    }

    #[test]
    fn writable_dirs_are_user_scoped_children() {
        let paths = AppPaths {
            config_dir: PathBuf::from("/tmp/user/.config/b-emu"),
            data_dir: PathBuf::from("/tmp/user/.local/share/b-emu"),
            cache_dir: PathBuf::from("/tmp/user/.cache/b-emu"),
            runtime_dir: None,
        };

        assert!(
            paths
                .writable_dirs()
                .iter()
                .all(|path| path.ends_with(APP_ID))
        );
    }

}
