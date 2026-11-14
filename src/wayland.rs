use std::env;
use std::ffi::OsString;
use std::os::unix::fs::FileTypeExt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaylandError {
    MissingDisplay,
    InvalidDisplayName,
    MissingRuntimeDir,
    InvalidRuntimeDir,
    SocketNotFound(PathBuf),
    SocketNotSocket(PathBuf),
}

impl std::fmt::Display for WaylandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDisplay => write!(
                f,
                "WAYLAND_DISPLAY is not set; B-Emu requires a native Wayland session"
            ),
            Self::InvalidDisplayName => write!(
                f,
                "WAYLAND_DISPLAY must be an absolute socket path or a relative socket name"
            ),
            Self::MissingRuntimeDir => write!(
                f,
                "XDG_RUNTIME_DIR is not set; cannot locate the Wayland socket"
            ),
            Self::InvalidRuntimeDir => {
                write!(f, "XDG_RUNTIME_DIR must be an absolute directory")
            }
            Self::SocketNotFound(path) => write!(
                f,
                "Wayland socket does not exist or is inaccessible: {}",
                path.display()
            ),
            Self::SocketNotSocket(path) => write!(
                f,
                "Wayland display path is not a Unix socket: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for WaylandError {}

pub fn wayland_socket_from_env() -> Result<PathBuf, WaylandError> {
    socket_path_from_values(
        env::var_os("WAYLAND_DISPLAY"),
        env::var_os("XDG_RUNTIME_DIR"),
    )
}

fn socket_path_from_values(
    display: Option<OsString>,
    runtime: Option<OsString>,
) -> Result<PathBuf, WaylandError> {
    let display = display.ok_or(WaylandError::MissingDisplay)?;
    let display = PathBuf::from(display);

    let socket = if display.is_absolute() {
        display
    } else {
        if display.as_os_str().is_empty()
            || display.components().any(|component| {
                matches!(component, Component::ParentDir | Component::RootDir)
            })
        {
            return Err(WaylandError::InvalidDisplayName);
        }

        let runtime = runtime.ok_or(WaylandError::MissingRuntimeDir)?;
        let runtime = PathBuf::from(runtime);
        if !runtime.is_absolute() {
            return Err(WaylandError::InvalidRuntimeDir);
        }

        runtime.join(display)
    };

    validate_socket_path(&socket)?;
    Ok(socket)
}

fn validate_socket_path(path: &Path) -> Result<(), WaylandError> {
    let metadata = path
        .metadata()
        .map_err(|_| WaylandError::SocketNotFound(path.to_path_buf()))?;

    if metadata.file_type().is_socket() {
        Ok(())
    } else {
        Err(WaylandError::SocketNotSocket(path.to_path_buf()))
    }
}

#[cfg(test)]
mod tests {
    use super::{socket_path_from_values, WaylandError};
    use std::ffi::OsString;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestSocket {
        dir: PathBuf,
        _listener: UnixListener,
    }

    impl TestSocket {
        fn new() -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "b-emu-wayland-test-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let listener = UnixListener::bind(dir.join("wayland-test")).unwrap();

            Self {
                dir,
                _listener: listener,
            }
        }

        fn socket(&self) -> PathBuf {
            self.dir.join("wayland-test")
        }
    }

    impl Drop for TestSocket {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn value(value: &str) -> Option<OsString> {
        Some(OsString::from(value))
    }

    #[test]
    fn missing_display_is_rejected() {
        assert_eq!(
            socket_path_from_values(None, None),
            Err(WaylandError::MissingDisplay)
        );
    }

    #[test]
    fn relative_display_requires_runtime_directory() {
        assert_eq!(
            socket_path_from_values(value("wayland-0"), None),
            Err(WaylandError::MissingRuntimeDir)
        );
    }

    #[test]
    fn relative_runtime_directory_is_rejected() {
        assert_eq!(
            socket_path_from_values(value("wayland-0"), value("relative")),
            Err(WaylandError::InvalidRuntimeDir)
        );
    }

    #[test]
    fn parent_relative_display_is_rejected() {
        assert_eq!(
            socket_path_from_values(value("../wayland-0"), value("/run/user/1000")),
            Err(WaylandError::InvalidDisplayName)
        );
    }

    #[test]
    fn absolute_display_path_does_not_require_runtime_directory() {
        let socket = TestSocket::new();
        assert_eq!(
            socket_path_from_values(
                Some(socket.socket().as_os_str().to_os_string()),
                None
            ),
            Ok(socket.socket())
        );
    }

    #[test]
    fn relative_display_resolves_under_runtime_directory() {
        let socket = TestSocket::new();
        assert_eq!(
            socket_path_from_values(
                value("wayland-test"),
                Some(socket.dir.as_os_str().to_os_string())
            ),
            Ok(socket.socket())
        );
    }

    #[test]
    fn missing_socket_is_reported() {
        let path = PathBuf::from("/tmp/b-emu-socket-that-does-not-exist");
        assert_eq!(
            socket_path_from_values(
                Some(path.as_os_str().to_os_string()),
                None
            ),
            Err(WaylandError::SocketNotFound(path))
        );
    }

    #[test]
    fn regular_file_is_not_accepted_as_socket() {
        let socket = TestSocket::new();
        let file = socket.dir.join("regular-file");
        std::fs::write(&file, b"not a socket").unwrap();

        assert_eq!(
            socket_path_from_values(
                Some(file.as_os_str().to_os_string()),
                None
            ),
            Err(WaylandError::SocketNotSocket(file))
        );
    }
}
