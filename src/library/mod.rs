//! Library index service: scanning, indexing and querying local game files.

mod db;
mod scanner;
mod systems;

pub use db::{Counts, GameRecord, SystemSummary};
pub use scanner::{ScanOutcome, ScanProgress, hash_file};

use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use db::LibraryDb;

#[derive(Debug)]
pub enum LibraryError {
    Db(rusqlite::Error),
    Io(io::Error),
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Db(error) => write!(f, "library database error: {error}"),
            Self::Io(error) => write!(f, "library I/O error: {error}"),
        }
    }
}

impl std::error::Error for LibraryError {}

impl From<rusqlite::Error> for LibraryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

impl From<io::Error> for LibraryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub errors: usize,
    pub cancelled: bool,
}

pub struct Library {
    db: LibraryDb,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self, LibraryError> {
        Ok(Self {
            db: LibraryDb::open(path)?,
        })
    }

    pub fn open_in_memory() -> Result<Self, LibraryError> {
        Ok(Self {
            db: LibraryDb::open_in_memory()?,
        })
    }

    pub fn games(&self) -> Result<Vec<GameRecord>, LibraryError> {
        Ok(self.db.games()?)
    }

    pub fn search(&self, query: &str) -> Result<Vec<GameRecord>, LibraryError> {
        Ok(self.db.search(query)?)
    }

    pub fn recent(&self, limit: i32) -> Result<Vec<GameRecord>, LibraryError> {
        Ok(self.db.recent(limit)?)
    }

    pub fn favorites(&self) -> Result<Vec<GameRecord>, LibraryError> {
        Ok(self.db.favorites()?)
    }

    pub fn favorite_paths(&self) -> Result<HashSet<String>, LibraryError> {
        Ok(self.db.favorite_paths()?)
    }

    /// Returns true when the game is now a favorite.
    pub fn toggle_favorite(&self, path: &str) -> Result<bool, LibraryError> {
        Ok(self.db.toggle_favorite(path)?)
    }

    pub fn mark_played(&self, path: &str) -> Result<(), LibraryError> {
        Ok(self.db.mark_played(path)?)
    }

    pub fn recently_played(&self, limit: i32) -> Result<Vec<GameRecord>, LibraryError> {
        Ok(self.db.recently_played(limit)?)
    }

    pub fn metadata_map(
        &self,
    ) -> Result<std::collections::HashMap<String, crate::metadata::Metadata>, LibraryError> {
        Ok(self.db.metadata_map()?)
    }

    pub fn artwork_map(
        &self,
    ) -> Result<std::collections::HashMap<String, crate::metadata::Artwork>, LibraryError> {
        Ok(self.db.artwork_map()?)
    }

    /// Fills missing metadata from local, provenance-tracked providers.
    pub fn enrich_all(&self, cache_dir: &Path) -> Result<usize, LibraryError> {
        let mut enriched = 0;
        let metadata = self.db.metadata_map()?;
        let artwork = self.db.artwork_map()?;

        for game in self.games()? {
            if !metadata.contains_key(&path_string(&game.path)) {
                self.db
                    .upsert_metadata(&crate::metadata::from_filename(&game))?;
                enriched += 1;
            }
            if !artwork.contains_key(&path_string(&game.path)) {
                if let Some(found) = crate::metadata::sidecar_artwork(&game, cache_dir) {
                    self.db.upsert_artwork(&found)?;
                }
            }
        }
        Ok(enriched)
    }

    pub fn counts(&self) -> Result<Counts, LibraryError> {
        Ok(self.db.counts()?)
    }

    pub fn systems(&self) -> Result<Vec<SystemSummary>, LibraryError> {
        Ok(self.db.systems()?)
    }

    /// Scans configured roots. Never writes to game directories; only B-Emu's
    /// own SQLite index is modified.
    pub fn scan(
        &mut self,
        roots: &[PathBuf],
        cancel: &AtomicBool,
        on_progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<ScanReport, LibraryError> {
        let known = self.db.known_files()?;
        let mut report = ScanReport::default();
        let mut seen = HashSet::new();
        let mut discovered = Vec::new();
        let mut completed_roots = Vec::new();

        for root in roots {
            // A missing or unavailable root is not evidence that its games
            // were deleted. Keep its existing database rows untouched.
            if !root.is_dir() {
                report.errors += 1;
                continue;
            }

            let result = scanner::scan_root(root, &known, cancel, on_progress);
            report.errors += result.errors;
            report.cancelled |= result.cancelled;
            if result.errors == 0 && !result.cancelled {
                completed_roots.push(root.clone());
            }

            for (game, outcome) in result.games {
                seen.insert(game.path.clone());
                match outcome {
                    ScanOutcome::Added => report.added += 1,
                    ScanOutcome::Updated => report.updated += 1,
                    ScanOutcome::Unchanged => report.unchanged += 1,
                }
                discovered.push((game, outcome));
            }

            if report.cancelled {
                break;
            }
        }

        // One transaction: a crash cannot leave a half-written index. Prune
        // only roots that were accessible and scanned without any errors.
        // Successful roots can still be updated when another root fails.
        report.removed = self
            .db
            .apply_scan(&discovered, &seen, &completed_roots)?;
        Ok(report)
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::Library;
    use std::{fs, sync::atomic::AtomicBool};

    #[test]
    fn indexes_a_real_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("SNES")).unwrap();
        fs::write(dir.path().join("SNES/mario.sfc"), b"rom").unwrap();
        fs::write(dir.path().join("notes.txt"), b"ignore").unwrap();

        let mut library = Library::open_in_memory().unwrap();
        let report = library
            .scan(
                &[dir.path().to_path_buf()],
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .unwrap();

        assert_eq!(report.added, 1);
        assert_eq!(library.games().unwrap().len(), 1);
        assert_eq!(library.counts().unwrap().systems, 1);
    }

    #[test]
    fn rescanning_is_incremental() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("game.nes"), b"rom").unwrap();
        let mut library = Library::open_in_memory().unwrap();
        let roots = [dir.path().to_path_buf()];

        library
            .scan(&roots, &AtomicBool::new(false), &mut |_| {})
            .unwrap();
        let second = library
            .scan(&roots, &AtomicBool::new(false), &mut |_| {})
            .unwrap();

        assert_eq!(second.added, 0);
        assert_eq!(second.unchanged, 1);
    }

    #[test]
    fn missing_root_does_not_delete_indexed_games_or_user_data() {
        let dir = tempfile::tempdir().unwrap();
        let roms = dir.path().join("roms");
        fs::create_dir(&roms).unwrap();
        let path = roms.join("game.nes");
        fs::write(&path, b"rom").unwrap();

        let mut library = Library::open_in_memory().unwrap();
        library.scan(&[roms.clone()], &AtomicBool::new(false), &mut |_| {}).unwrap();
        let key = path.to_string_lossy().to_string();
        library.toggle_favorite(&key).unwrap();
        library.mark_played(&key).unwrap();

        fs::remove_dir_all(&roms).unwrap();
        let report = library.scan(&[roms], &AtomicBool::new(false), &mut |_| {}).unwrap();

        assert_eq!(report.removed, 0);
        assert_eq!(report.errors, 1);
        assert_eq!(library.games().unwrap().len(), 1);
        assert!(library.favorite_paths().unwrap().contains(&key));
        assert_eq!(library.recently_played(10).unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn descendant_scan_error_preserves_existing_entries_for_that_root() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let roms = dir.path().join("roms");
        let blocked = roms.join("blocked");
        fs::create_dir_all(&blocked).unwrap();
        let path = blocked.join("game.nes");
        fs::write(&path, b"rom").unwrap();

        let mut library = Library::open_in_memory().unwrap();
        library.scan(&[roms.clone()], &AtomicBool::new(false), &mut |_| {}).unwrap();
        let key = path.to_string_lossy().to_string();
        library.toggle_favorite(&key).unwrap();
        library.mark_played(&key).unwrap();

        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o000)).unwrap();
        let report = library.scan(&[roms], &AtomicBool::new(false), &mut |_| {}).unwrap();
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(report.errors > 0);
        assert_eq!(report.removed, 0);
        assert_eq!(library.games().unwrap().len(), 1);
        assert!(library.favorite_paths().unwrap().contains(&key));
        assert_eq!(library.recently_played(10).unwrap().len(), 1);
    }

    #[test]
    fn pruning_only_happens_after_a_completed_scan() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.nes");
        fs::write(&path, b"rom").unwrap();
        let mut library = Library::open_in_memory().unwrap();
        let roots = [dir.path().to_path_buf()];
        library
            .scan(&roots, &AtomicBool::new(false), &mut |_| {})
            .unwrap();

        fs::remove_file(&path).unwrap();
        let cancelled = library
            .scan(&roots, &AtomicBool::new(true), &mut |_| {})
            .unwrap();
        assert!(cancelled.cancelled);
        assert_eq!(library.games().unwrap().len(), 1);

        let completed = library
            .scan(&roots, &AtomicBool::new(false), &mut |_| {})
            .unwrap();
        assert_eq!(completed.removed, 1);
        assert!(library.games().unwrap().is_empty());
    }
}
