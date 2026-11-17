//! File extension to system detection.
//!
//! Detection is deliberately conservative: unknown extensions are skipped
//! rather than guessed. The scanner never opens files for writing.

use std::path::Path;

/// Returns the system name for a supported ROM/disc extension.
pub fn system_for_extension(extension: &str) -> Option<&'static str> {
    match extension.to_ascii_lowercase().as_str() {
        "nes" => Some("NES"),
        "sfc" | "smc" => Some("Super Nintendo"),
        "md" | "gen" => Some("Mega Drive"),
        "z64" | "v64" | "n64" => Some("Nintendo 64"),
        "gba" => Some("Game Boy Advance"),
        "gb" | "gbc" => Some("Game Boy"),
        "cue" | "chd" | "pbp" | "iso" => Some("PlayStation"),
        "zip" => Some("Arcade"),
        "rom" => Some("Autre"),
        _ => None,
    }
}

pub fn system_for_path(path: &Path) -> Option<&'static str> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(system_for_extension)
}

#[cfg(test)]
mod tests {
    use super::{system_for_extension, system_for_path};
    use std::path::Path;

    #[test]
    fn extension_mapping_is_case_insensitive() {
        assert_eq!(system_for_extension("SFC"), Some("Super Nintendo"));
        assert_eq!(system_for_extension("iso"), Some("PlayStation"));
    }

    #[test]
    fn unsupported_extensions_are_skipped() {
        assert_eq!(system_for_extension("txt"), None);
        assert_eq!(system_for_extension(""), None);
    }

    #[test]
    fn detects_from_a_path() {
        assert_eq!(
            system_for_path(Path::new("/roms/mario.SFC")),
            Some("Super Nintendo")
        );
        assert_eq!(system_for_path(Path::new("/roms/notes.txt")), None);
    }
}
