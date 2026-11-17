//! Typed seed view models for the parts of the shell not backed by services yet.
//!
//! Game and system data comes from the SQLite library index, and emulators are
//! discovered on PATH. Only sidebar pages and stable accent colors live here.

use slint::{Color, SharedString};

/// Sidebar pages. This is the single source of truth for the page count.
pub fn pages() -> Vec<SharedString> {
    [
        "Accueil",
        "Systèmes",
        "Bibliothèque",
        "Émulateurs",
        "Paramètres",
    ]
    .into_iter()
    .map(SharedString::from)
    .collect()
}

/// Stable accent color for a detected system name.
pub fn system_accent(system: &str) -> Color {
    match system {
        "NES" | "Nintendo 64" | "Game Boy" => Color::from_rgb_u8(0x81, 0x8C, 0xF8),
        "Super Nintendo" | "Game Boy Advance" => Color::from_rgb_u8(0x67, 0xE8, 0xF9),
        "Mega Drive" | "Arcade" => Color::from_rgb_u8(0x38, 0xBD, 0xF8),
        _ => Color::from_rgb_u8(0x60, 0xA5, 0xFA),
    }
}

#[cfg(test)]
mod tests {
    use super::pages;

    #[test]
    fn pages_are_seeded() {
        assert_eq!(pages().len(), 5);
    }
}
