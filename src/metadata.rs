//! Local metadata and artwork cache with explicit provenance.
//!
//! No network provider is enabled by default. Metadata is derived from the
//! ROM filename and artwork from optional sidecar images next to the game
//! file. Every record records the provider that produced it.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::library::{GameRecord, hash_file};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub path: String,
    pub title: String,
    pub description: String,
    pub year: Option<i32>,
    pub region: String,
    pub provider: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artwork {
    pub path: String,
    pub cache_path: PathBuf,
    pub source: String,
}

fn parse_tags(stem: &str) -> (String, Option<i32>, String) {
    let mut title = String::new();
    let mut year = None;
    let mut region = String::new();

    for token in stem.split([' ', '_']) {
        let inner = token.trim_matches(|c: char| c == '(' || c == ')' || c == '[' || c == ']');
        if inner.len() == 4 && inner.chars().all(|c| c.is_ascii_digit()) {
            year = inner.parse().ok();
            continue;
        }
        if matches!(
            inner.to_ascii_lowercase().as_str(),
            "usa" | "u" | "world" | "europe" | "e" | "japan" | "j" | "france" | "fr"
        ) {
            region = inner.to_owned();
            continue;
        }
        if token.starts_with('(') || token.starts_with('[') {
            continue;
        }
        if !title.is_empty() {
            title.push(' ');
        }
        title.push_str(token);
    }

    (title.trim().to_owned(), year, region)
}

/// Local filename provider. Provenance is always "filename".
pub fn from_filename(game: &GameRecord) -> Metadata {
    let stem = game
        .path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let (title, year, region) = parse_tags(stem);

    Metadata {
        path: game.path.to_string_lossy().into_owned(),
        title: if title.is_empty() {
            stem.to_owned()
        } else {
            title
        },
        description: format!("Métadonnées déduites du nom de fichier · {}", game.system),
        year,
        region,
        provider: "filename".to_owned(),
    }
}

/// Looks for a sibling image and copies it into the artwork cache.
/// Returns `None` when no local artwork is available; nothing is guessed.
pub fn sidecar_artwork(game: &GameRecord, cache_dir: &Path) -> Option<Artwork> {
    for extension in ["png", "jpg", "jpeg", "webp"] {
        let sibling = game.path.with_extension(extension);
        if !sibling.is_file() {
            continue;
        }
        let digest = hash_file(&sibling).ok()?;
        let name = format!("{}.{}", &digest[..16.min(digest.len())], extension);
        let cache_path = cache_dir.join(name);
        if !cache_path.exists() {
            fs::create_dir_all(cache_dir).ok()?;
            fs::copy(&sibling, &cache_path).ok()?;
        }
        return Some(Artwork {
            path: game.path.to_string_lossy().into_owned(),
            cache_path,
            source: "sidecar".to_owned(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{from_filename, sidecar_artwork};
    use crate::library::GameRecord;
    use std::path::PathBuf;

    fn game(path: PathBuf) -> GameRecord {
        GameRecord {
            path,
            title: "jeu".into(),
            system: "Super Nintendo".into(),
            size: 1,
            checksum: "hash".into(),
            source_dir: "/roms".into(),
        }
    }

    #[test]
    fn filename_provider_extracts_title_year_and_region() {
        let record = game(PathBuf::from("/roms/Chrono Trigger (USA) (1995).sfc"));
        let metadata = from_filename(&record);
        assert_eq!(metadata.title, "Chrono Trigger");
        assert_eq!(metadata.year, Some(1995));
        assert_eq!(metadata.region, "USA");
        assert_eq!(metadata.provider, "filename");
    }

    #[test]
    fn sidecar_artwork_is_copied_into_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("game.sfc");
        let image = dir.path().join("game.png");
        std::fs::write(&rom, b"rom").unwrap();
        std::fs::write(&image, b"png-data").unwrap();
        let cache = dir.path().join("cache/artwork");

        let artwork = sidecar_artwork(&game(rom), &cache).unwrap();
        assert_eq!(artwork.source, "sidecar");
        assert!(artwork.cache_path.is_file());
    }

    #[test]
    fn missing_sidecar_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("game.sfc");
        std::fs::write(&rom, b"rom").unwrap();
        assert!(sidecar_artwork(&game(rom), &dir.path().join("cache")).is_none());
    }
}
