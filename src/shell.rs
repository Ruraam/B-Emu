//! Typed seed view models for the parts of the shell not backed by services yet.
//!
//! Game and system data now comes from the SQLite library index in Phase 2.
//! Emulator detection remains a Phase 3 concern, so its list is still seeded.

use slint::{Color, SharedString};

use crate::EmulatorRow;

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

/// Emulator availability preview. Real detection belongs to Phase 3.
pub fn emulators() -> Vec<EmulatorRow> {
    let entries = [
        ("RetroArch", "1.16.0", "Prêt", "Multi-systèmes", true),
        ("Dolphin", "2407", "Prêt", "GameCube / Wii", true),
        ("PCSX2", "1.7.0", "Détecté", "PlayStation 2", true),
        ("PPSSPP", "1.16.6", "Prêt", "PSP", true),
        ("MAME", "0.267", "Non installé", "Arcade", false),
    ];

    entries
        .into_iter()
        .map(|(name, version, status, supported, ready)| EmulatorRow {
            name: SharedString::from(name),
            version: SharedString::from(version),
            status: SharedString::from(status),
            systems: SharedString::from(supported),
            ready,
        })
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
    use super::{emulators, pages};

    #[test]
    fn pages_and_emulators_are_seeded() {
        assert_eq!(pages().len(), 5);
        assert_eq!(emulators().len(), 5);
    }
}
