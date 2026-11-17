//! Typed view models for the Phase 1 application shell.
//!
//! Phase 1 has no library scanner or emulator registry yet. These seeds keep
//! every screen data-driven so Phase 2/3 services can replace them without
//! touching the Slint interface. Nothing here touches the filesystem or spawns
//! processes.

use slint::{Color, SharedString};

use crate::{EmulatorRow, GameCard, SystemCard};

fn cyan() -> Color {
    Color::from_rgb_u8(0x67, 0xE8, 0xF9)
}

fn blue() -> Color {
    Color::from_rgb_u8(0x38, 0xBD, 0xF8)
}

fn indigo() -> Color {
    Color::from_rgb_u8(0x81, 0x8C, 0xF8)
}

fn game(title: &str, system: &str, detail: &str, accent: Color) -> GameCard {
    GameCard {
        title: SharedString::from(title),
        system: SharedString::from(system),
        detail: SharedString::from(detail),
        accent,
    }
}

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

/// The single game shown in the "Reprendre" card.
pub fn last_played() -> Vec<GameCard> {
    vec![game(
        "Super Mario World",
        "Super Nintendo",
        "Dernière session il y a 2 jours · Monde 7",
        cyan(),
    )]
}

/// Games shown on the Home screen.
pub fn recent_games() -> Vec<GameCard> {
    vec![
        game(
            "The Legend of Zelda",
            "Nintendo 64",
            "12 h de jeu",
            indigo(),
        ),
        game("Sonic the Hedgehog", "Mega Drive", "À rejouer", blue()),
        game(
            "Final Fantasy VII",
            "PlayStation",
            "Disque 1 · 34 h",
            cyan(),
        ),
        game("Street Fighter II", "Arcade", "Favori", indigo()),
    ]
}

/// Full library index used by the Library screen. This is what a Phase 2
/// scanner will eventually populate.
pub fn all_games() -> Vec<GameCard> {
    let mut games = recent_games();
    games.extend([
        game("Metroid Prime", "GameCube", "22 h de jeu", blue()),
        game("Chrono Trigger", "Super Nintendo", "Terminé", cyan()),
        game("Pac-Man", "Arcade", "Classique", indigo()),
        game("Gran Turismo 2", "PlayStation", "Disque 1", blue()),
    ]);
    games
}

/// Case-insensitive filter on title and system.
pub fn filter_games(games: &[GameCard], query: &str) -> Vec<GameCard> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return games.to_vec();
    }

    games
        .iter()
        .filter(|game| {
            game.title.to_lowercase().contains(&needle)
                || game.system.to_lowercase().contains(&needle)
        })
        .cloned()
        .collect()
}

/// Systems detected for the shell preview.
pub fn systems() -> Vec<SystemCard> {
    let entries = [
        ("Super Nintendo", "Nintendo", 48, indigo()),
        ("Nintendo 64", "Nintendo", 21, cyan()),
        ("PlayStation", "Sony", 63, blue()),
        ("Mega Drive", "Sega", 34, indigo()),
        ("GameCube", "Nintendo", 17, cyan()),
        ("Arcade", "Multiple", 92, blue()),
    ];

    entries
        .into_iter()
        .map(|(name, manufacturer, game_count, accent)| SystemCard {
            name: SharedString::from(name),
            manufacturer: SharedString::from(manufacturer),
            game_count,
            accent,
        })
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

#[cfg(test)]
mod tests {
    use super::{all_games, emulators, filter_games, systems};

    #[test]
    fn filters_by_title_and_system() {
        let games = all_games();
        assert_eq!(filter_games(&games, "zelda").len(), 1);
        assert_eq!(filter_games(&games, "PLAYSTATION").len(), 2);
        assert!(filter_games(&games, "mario").is_empty());
        assert_eq!(filter_games(&games, "").len(), games.len());
        assert!(filter_games(&games, "inexistant").is_empty());
    }

    #[test]
    fn seeds_are_non_empty_for_preview_screens() {
        assert!(!all_games().is_empty());
        assert_eq!(systems().len(), 6);
        assert_eq!(emulators().len(), 5);
    }
}
