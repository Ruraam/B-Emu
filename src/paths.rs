use std::env;
use std::ffi::OsString;
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
        Self::from_values(
            env::var_os("HOME"),
            env::var_os("XDG_CONFIG_HOME"),
            env::var_os("XDG_DATA_HOME"),
            env::var_os("XDG_CACHE_HOME"),
            env::var_os("XDG_RUNTIME_DIR"),
        )
    }

    fn from_values(
        home: Option<OsString>,
        config: Option<OsString>,
        data: Option<OsString>,
        cache: Option<OsString>,
        runtime: Option<OsString>,
    ) -> Result<Self, PathsError> {
        let home = home.map(PathBuf::from).ok_or(PathsError::MissingHome)?;
        let home = absolute_path("HOME", home)?;

        Ok(Self {
            config_dir: xdg_path(config, home.join(".config"), "XDG_CONFIG_HOME")?.join(APP_ID),
            data_dir: xdg_path(data, home.join(".local/share"), "XDG_DATA_HOME")?.join(APP_ID),
            cache_dir: xdg_path(cache, home.join(".cache"), "XDG_CACHE_HOME")?.join(APP_ID),
            runtime_dir: runtime
                .map(|value| absolute_path("XDG_RUNTIME_DIR", PathBuf::from(value)))
                .transpose()?
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

fn xdg_path(
    value: Option<OsString>,
    fallback: PathBuf,
    name: &'static str,
) -> Result<PathBuf, PathsError> {
    match value {
        Some(value) => absolute_path(name, PathBuf::from(value)),
        None => Ok(fallback),
    }
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
    use super::{APP_ID, AppPaths, PathsError};
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn values(
        home: Option<&str>,
        config: Option<&str>,
        data: Option<&str>,
        cache: Option<&str>,
        runtime: Option<&str>,
    ) -> Result<AppPaths, PathsError> {
        AppPaths::from_values(
            home.map(OsString::from),
            config.map(OsString::from),
            data.map(OsString::from),
            cache.map(OsString::from),
            runtime.map(OsString::from),
        )
    }

    #[test]
    fn app_id_is_stable() {
        assert_eq!(APP_ID, "b-emu");
    }

    #[test]
    fn missing_home_is_rejected() {
        assert_eq!(
            values(None, None, None, None, None),
            Err(PathsError::MissingHome)
        );
    }

    #[test]
    fn relative_home_is_rejected() {
        assert_eq!(
            values(Some("home/user"), None, None, None, None),
            Err(PathsError::RelativeXdgPath("HOME"))
        );
    }

    #[test]
    fn xdg_defaults_are_resolved_under_home() {
        let paths = values(Some("/home/test"), None, None, None, None).unwrap();

        assert_eq!(paths.config_dir, PathBuf::from("/home/test/.config/b-emu"));
        assert_eq!(
            paths.data_dir,
            PathBuf::from("/home/test/.local/share/b-emu")
        );
        assert_eq!(paths.cache_dir, PathBuf::from("/home/test/.cache/b-emu"));
        assert_eq!(paths.runtime_dir, None);
    }

    #[test]
    fn absolute_xdg_overrides_are_used() {
        let paths = values(
            Some("/home/test"),
            Some("/config"),
            Some("/data"),
            Some("/cache"),
            Some("/run/user/1000"),
        )
        .unwrap();

        assert_eq!(paths.config_dir, PathBuf::from("/config/b-emu"));
        assert_eq!(paths.data_dir, PathBuf::from("/data/b-emu"));
        assert_eq!(paths.cache_dir, PathBuf::from("/cache/b-emu"));
        assert_eq!(
            paths.runtime_dir,
            Some(PathBuf::from("/run/user/1000/b-emu"))
        );
    }

    #[test]
    fn relative_xdg_overrides_are_rejected_by_name() {
        assert_eq!(
            values(Some("/home/test"), Some("config"), None, None, None),
            Err(PathsError::RelativeXdgPath("XDG_CONFIG_HOME"))
        );
        assert_eq!(
            values(Some("/home/test"), None, Some("data"), None, None),
            Err(PathsError::RelativeXdgPath("XDG_DATA_HOME"))
        );
        assert_eq!(
            values(Some("/home/test"), None, None, Some("cache"), None),
            Err(PathsError::RelativeXdgPath("XDG_CACHE_HOME"))
        );
        assert_eq!(
            values(Some("/home/test"), None, None, None, Some("runtime")),
            Err(PathsError::RelativeXdgPath("XDG_RUNTIME_DIR"))
        );
    }

    #[test]
    fn writable_dirs_are_user_scoped_children() {
        let paths = values(Some("/home/test"), None, None, None, None).unwrap();

        assert!(
            paths
                .writable_dirs()
                .iter()
                .all(|path| path.ends_with(APP_ID))
        );
    }
}
