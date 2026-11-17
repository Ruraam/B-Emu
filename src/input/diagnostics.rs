//! Actionable diagnostics when the gamepad subsystem cannot see controllers.

use std::{fs, path::Path};

/// Formats a diagnostic from facts we can observe without privileges.
pub fn access_message(readable_event_nodes: usize, has_runtime_dir: bool) -> &'static str {
    if readable_event_nodes > 0 {
        "gamepad devices are visible, but the input subsystem could not be initialized"
    } else if has_runtime_dir {
        "no input device is readable by this user; check the active seat/session ACLs (uaccess). \
         B-Emu never requires root or changes /dev/input permissions"
    } else {
        "no input device is readable and XDG_RUNTIME_DIR is unset; start B-Emu from a logged-in \
         Wayland session so seat ACLs are applied"
    }
}

/// Counts `/dev/input/event*` nodes the current process can read.
pub fn readable_event_nodes() -> usize {
    let Ok(entries) = fs::read_dir(Path::new("/dev/input")) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("event"))
        .filter(|entry| fs::File::open(entry.path()).is_ok())
        .count()
}

pub fn log_input_diagnostic() {
    let nodes = readable_event_nodes();
    let has_runtime = std::env::var_os("XDG_RUNTIME_DIR").is_some();
    eprintln!(
        "[b-emu] input diagnostic: {}",
        access_message(nodes, has_runtime)
    );
}

#[cfg(test)]
mod tests {
    use super::access_message;

    #[test]
    fn visible_devices_get_a_short_message() {
        assert!(access_message(2, true).contains("could not be initialized"));
    }

    #[test]
    fn missing_access_points_to_seat_acls_without_suggesting_root() {
        let message = access_message(0, true);
        assert!(message.contains("seat/session ACLs"));
        assert!(!message.to_lowercase().contains("run as root"));
    }

    #[test]
    fn missing_runtime_dir_is_reported() {
        assert!(access_message(0, false).contains("XDG_RUNTIME_DIR"));
    }
}
