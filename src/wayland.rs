use std::env;
use std::path::{Path, PathBuf};

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
                "WAYLAND_DISPLAY must name a Wayland socket, not an absolute or parent-relative path"
            ),
            Self::MissingRuntimeDir => write!(
                f,
                "XDG_RUNTIME_DIR is not set; cannot locate the Wayland socket"
            ),
            Self::InvalidRuntimeDir => write!(
                f,
                "XDG_RUNTIME_DIR must be an absolute directory"
            ),
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
    let display = env::var_os("WAYLAND_DISPLAY").ok_or(WaylandError::MissingDisplay)?;
    let display = PathBuf::from(display);

    if display.is_absolute()
        || display
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(WaylandError::InvalidDisplayName);
    }

    let runtime = env::var_os("XDG_RUNTIME_DIR").ok_or(WaylandError::MissingRuntimeDir)?;
    let runtime = PathBuf::from(runtime);

    if !runtime.is_absolute() {
        return Err(WaylandError::InvalidRuntimeDir);
    }

    let socket = runtime.join(display);
    let metadata = socket
        .symlink_metadata()
        .map_err(|_| WaylandError::SocketNotFound(socket.clone()))?;

    if !metadata.file_type().is_socket() {
        return Err(WaylandError::SocketNotSocket(socket));
    }

    Ok(socket)
}

#[cfg(test)]
pub fn validate_socket_path(path: &Path) -> Result<(), WaylandError> {
    let metadata = path
        .symlink_metadata()
        .map_err(|_| WaylandError::SocketNotFound(path.to_path_buf()))?;

    if metadata.file_type().is_socket() {
        Ok(())
    } else {
        Err(WaylandError::SocketNotSocket(path.to_path_buf()))
    }
}
