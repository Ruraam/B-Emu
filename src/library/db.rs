//! SQLite index for scanned games. Only B-Emu's own data directory is written.

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, params};

use super::scanner::{KnownFile, ScanOutcome, ScannedGame};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    pub path: PathBuf,
    pub title: String,
    pub system: String,
    pub size: i64,
    pub checksum: String,
    pub source_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub games: i32,
    pub systems: i32,
    pub duplicates: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemSummary {
    pub name: String,
    pub game_count: i32,
}

pub struct LibraryDb {
    conn: Connection,
}

fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as i64)
        .unwrap_or(0)
}

const UPSERT_GAME_SQL: &str =
    "INSERT INTO games (path, title, system, size, modified_ns, checksum, source_dir)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
     ON CONFLICT(path) DO UPDATE SET
        title = excluded.title,
        system = excluded.system,
        size = excluded.size,
        modified_ns = excluded.modified_ns,
        checksum = excluded.checksum,
        source_dir = excluded.source_dir";

fn path_to_text(path: &Path) -> Result<&str, rusqlite::Error> {
    path.to_str().ok_or_else(|| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(io::Error::new(
            io::ErrorKind::InvalidData,
            "filesystem path is not valid UTF-8",
        )))
    })
}

fn upsert_on(conn: &Connection, game: &ScannedGame) -> Result<(), rusqlite::Error> {
    let path = path_to_text(&game.path)?;
    let source_dir = path_to_text(&game.source_dir)?;
    conn.execute(
        UPSERT_GAME_SQL,
        params![
            path,
            game.title,
            game.system,
            game.size,
            game.modified_ns,
            game.checksum,
            source_dir,
        ],
    )?;
    Ok(())
}

fn all_paths_on(conn: &Connection) -> Result<Vec<PathBuf>, rusqlite::Error> {
    let mut statement = conn.prepare("SELECT path FROM games")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|row| row.map(PathBuf::from)).collect()
}

fn remove_paths_on(
    conn: &Connection,
    paths: impl IntoIterator<Item = PathBuf>,
    seen: &HashSet<PathBuf>,
) -> Result<usize, rusqlite::Error> {
    let mut removed = 0;
    for path in paths {
        if !seen.contains(&path) {
            let key = path_to_text(&path)?;
            removed += conn.execute("DELETE FROM games WHERE path = ?1", params![key])?;
            conn.execute("DELETE FROM favorites WHERE path = ?1", params![key])?;
            conn.execute("DELETE FROM play_history WHERE path = ?1", params![key])?;
        }
    }
    Ok(removed)
}

fn remove_missing_on(conn: &Connection, seen: &HashSet<PathBuf>) -> Result<usize, rusqlite::Error> {
    remove_paths_on(conn, all_paths_on(conn)?, seen)
}

/// Prune only entries owned by roots whose scan completed without errors.
/// Matching the stored source directory exactly prevents a successful parent
/// root from pruning entries belonging to a separately configured child root.
fn remove_missing_from_roots_on(
    conn: &Connection,
    seen: &HashSet<PathBuf>,
    completed_roots: &[PathBuf],
) -> Result<usize, rusqlite::Error> {
    let mut statement = conn.prepare("SELECT path, source_dir FROM games")?;
    let rows = statement.query_map([], |row| {
        Ok((
            PathBuf::from(row.get::<_, String>(0)?),
            PathBuf::from(row.get::<_, String>(1)?),
        ))
    })?;
    let mut candidates = Vec::new();
    for row in rows {
        let (path, source_dir) = row?;
        if completed_roots.iter().any(|root| root == &source_dir) {
            candidates.push(path);
        }
    }
    remove_paths_on(conn, candidates, seen)
}

impl LibraryDb {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS games (
                path TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                system TEXT NOT NULL,
                size INTEGER NOT NULL,
                modified_ns INTEGER NOT NULL,
                checksum TEXT NOT NULL,
                source_dir TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS favorites (
                path TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS play_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL,
                played_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS metadata (
                path TEXT PRIMARY KEY,
                title TEXT,
                description TEXT,
                year INTEGER,
                region TEXT,
                provider TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS artwork (
                path TEXT PRIMARY KEY,
                cache_path TEXT NOT NULL,
                source TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS games_system ON games(system);
            CREATE INDEX IF NOT EXISTS games_checksum ON games(checksum);
            CREATE INDEX IF NOT EXISTS play_history_played_at ON play_history(played_at DESC);",
        )
    }

    pub fn known_files(&self) -> Result<HashMap<PathBuf, KnownFile>, rusqlite::Error> {
        let mut statement = self
            .conn
            .prepare("SELECT path, size, modified_ns, checksum FROM games")?;
        let rows = statement.query_map([], |row| {
            Ok((
                PathBuf::from(row.get::<_, String>(0)?),
                KnownFile {
                    size: row.get(1)?,
                    modified_ns: row.get(2)?,
                    checksum: row.get(3)?,
                },
            ))
        })?;
        rows.collect()
    }

    // Convenience APIs kept for callers that index a single game outside a
    // full scan (and used by unit tests); scans use the transactional path.
    #[allow(dead_code)]
    pub fn upsert(&self, game: &ScannedGame) -> Result<(), rusqlite::Error> {
        upsert_on(&self.conn, game)
    }

    /// Removes rows that were not seen during a completed scan. This covers
    /// deleted files and directories removed from the configuration.
    #[allow(dead_code)]
    pub fn remove_missing(&self, seen: &HashSet<PathBuf>) -> Result<usize, rusqlite::Error> {
        remove_missing_on(&self.conn, seen)
    }

    /// Applies a whole scan in one transaction: either every discovered game
    /// is indexed (and optionally pruned) or nothing is committed.
    pub fn apply_scan(
        &mut self,
        games: &[(ScannedGame, ScanOutcome)],
        seen: &HashSet<PathBuf>,
        completed_roots: &[PathBuf],
    ) -> Result<usize, rusqlite::Error> {
        let transaction = self.conn.transaction()?;
        for (game, _) in games {
            upsert_on(&transaction, game)?;
        }
        let removed = remove_missing_from_roots_on(&transaction, seen, completed_roots)?;
        transaction.commit()?;
        Ok(removed)
    }

    pub fn games(&self) -> Result<Vec<GameRecord>, rusqlite::Error> {
        self.query_games(
            "SELECT path, title, system, size, checksum, source_dir FROM games ORDER BY title",
            [],
        )
    }

    pub fn recent(&self, limit: i32) -> Result<Vec<GameRecord>, rusqlite::Error> {
        self.query_games(
            "SELECT path, title, system, size, checksum, source_dir FROM games
             ORDER BY modified_ns DESC, title LIMIT ?1",
            [limit],
        )
    }

    pub fn favorites(&self) -> Result<Vec<GameRecord>, rusqlite::Error> {
        self.query_games(
            "SELECT g.path, g.title, g.system, g.size, g.checksum, g.source_dir
             FROM favorites f JOIN games g ON g.path = f.path
             ORDER BY f.created_at DESC",
            [],
        )
    }

    pub fn favorite_paths(&self) -> Result<HashSet<String>, rusqlite::Error> {
        let mut statement = self.conn.prepare("SELECT path FROM favorites")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    /// Returns true when the game is now a favorite.
    pub fn toggle_favorite(&self, path: &str) -> Result<bool, rusqlite::Error> {
        let existing: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM favorites WHERE path = ?1)",
            params![path],
            |row| row.get(0),
        )?;

        if existing {
            self.conn
                .execute("DELETE FROM favorites WHERE path = ?1", params![path])?;
            Ok(false)
        } else {
            self.conn.execute(
                "INSERT INTO favorites (path, created_at) VALUES (?1, ?2)",
                params![path, now_ns()],
            )?;
            Ok(true)
        }
    }

    pub fn mark_played(&self, path: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO play_history (path, played_at) VALUES (?1, ?2)",
            params![path, now_ns()],
        )?;
        Ok(())
    }

    pub fn recently_played(&self, limit: i32) -> Result<Vec<GameRecord>, rusqlite::Error> {
        self.query_games(
            "SELECT g.path, g.title, g.system, g.size, g.checksum, g.source_dir
             FROM play_history h JOIN games g ON g.path = h.path
             GROUP BY g.path
             ORDER BY MAX(h.played_at) DESC LIMIT ?1",
            [limit],
        )
    }

    pub fn search(&self, query: &str) -> Result<Vec<GameRecord>, rusqlite::Error> {
        let pattern = format!("%{}%", query.trim().to_lowercase());
        if pattern.trim_matches('%').is_empty() {
            return self.games();
        }
        self.query_games(
            "SELECT path, title, system, size, checksum, source_dir FROM games
             WHERE lower(title) LIKE ?1 OR lower(system) LIKE ?1 ORDER BY title",
            [pattern],
        )
    }

    fn query_games(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<GameRecord>, rusqlite::Error> {
        let mut statement = self.conn.prepare(sql)?;
        let rows = statement.query_map(params, |row| {
            Ok(GameRecord {
                path: PathBuf::from(row.get::<_, String>(0)?),
                title: row.get(1)?,
                system: row.get(2)?,
                size: row.get(3)?,
                checksum: row.get(4)?,
                source_dir: PathBuf::from(row.get::<_, String>(5)?),
            })
        })?;
        rows.collect()
    }

    pub fn counts(&self) -> Result<Counts, rusqlite::Error> {
        let games: i32 = self
            .conn
            .query_row("SELECT COUNT(*) FROM games", [], |row| row.get(0))?;
        let systems: i32 =
            self.conn
                .query_row("SELECT COUNT(DISTINCT system) FROM games", [], |row| {
                    row.get(0)
                })?;
        let duplicates: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM (SELECT checksum FROM games GROUP BY checksum HAVING COUNT(*) > 1)",
            [],
            |row| row.get(0),
        )?;
        Ok(Counts {
            games,
            systems,
            duplicates,
        })
    }

    pub fn systems(&self) -> Result<Vec<SystemSummary>, rusqlite::Error> {
        let mut statement = self
            .conn
            .prepare("SELECT system, COUNT(*) FROM games GROUP BY system ORDER BY COUNT(*) DESC")?;
        let rows = statement.query_map([], |row| {
            Ok(SystemSummary {
                name: row.get(0)?,
                game_count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_metadata(
        &self,
        metadata: &crate::metadata::Metadata,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO metadata (path, title, description, year, region, provider, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title, description = excluded.description,
                year = excluded.year, region = excluded.region,
                provider = excluded.provider, fetched_at = excluded.fetched_at",
            params![
                metadata.path,
                metadata.title,
                metadata.description,
                metadata.year,
                metadata.region,
                metadata.provider,
                now_ns(),
            ],
        )?;
        Ok(())
    }

    pub fn metadata_map(
        &self,
    ) -> Result<HashMap<String, crate::metadata::Metadata>, rusqlite::Error> {
        let mut statement = self
            .conn
            .prepare("SELECT path, title, description, year, region, provider FROM metadata")?;
        let rows = statement.query_map([], |row| {
            let path: String = row.get(0)?;
            Ok((
                path.clone(),
                crate::metadata::Metadata {
                    path,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    year: row.get(3)?,
                    region: row.get(4)?,
                    provider: row.get(5)?,
                },
            ))
        })?;
        rows.collect()
    }

    pub fn upsert_artwork(
        &self,
        artwork: &crate::metadata::Artwork,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO artwork (path, cache_path, source, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
                cache_path = excluded.cache_path, source = excluded.source",
            params![
                artwork.path,
                path_to_text(&artwork.cache_path)?,
                artwork.source,
                now_ns(),
            ],
        )?;
        Ok(())
    }

    pub fn artwork_map(
        &self,
    ) -> Result<HashMap<String, crate::metadata::Artwork>, rusqlite::Error> {
        let mut statement = self
            .conn
            .prepare("SELECT path, cache_path, source FROM artwork")?;
        let rows = statement.query_map([], |row| {
            let path: String = row.get(0)?;
            Ok((
                path.clone(),
                crate::metadata::Artwork {
                    path,
                    cache_path: PathBuf::from(row.get::<_, String>(1)?),
                    source: row.get(2)?,
                },
            ))
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::LibraryDb;
    use crate::library::scanner::{ScanOutcome, ScannedGame};
    use std::{collections::HashSet, path::PathBuf};

    fn game(name: &str, system: &str, checksum: &str) -> ScannedGame {
        ScannedGame {
            path: format!("/roms/{name}").into(),
            title: name.to_owned(),
            system: system.to_owned(),
            size: 42,
            modified_ns: 1,
            checksum: checksum.to_owned(),
            source_dir: "/roms".into(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn database_rejects_non_utf8_paths_instead_of_coercing_them() {
        use std::os::unix::ffi::OsStringExt;

        let db = LibraryDb::open_in_memory().unwrap();
        let invalid = ScannedGame {
            path: PathBuf::from(std::ffi::OsString::from_vec(b"/roms/rom-\xff.nes".to_vec())),
            title: "rom".to_owned(),
            system: "Nintendo".to_owned(),
            size: 1,
            modified_ns: 1,
            checksum: "hash".to_owned(),
            source_dir: PathBuf::from("/roms"),
        };

        assert!(db.upsert(&invalid).is_err());
        assert!(db.games().unwrap().is_empty());
    }

    #[test]
    fn upsert_and_query() {
        let db = LibraryDb::open_in_memory().unwrap();
        db.upsert(&game("a.nes", "NES", "hash-a")).unwrap();
        db.upsert(&game("b.sfc", "Super Nintendo", "hash-b"))
            .unwrap();

        assert_eq!(db.games().unwrap().len(), 2);
        assert_eq!(db.search("SUPER").unwrap().len(), 1);
        let counts = db.counts().unwrap();
        assert_eq!(counts.games, 2);
        assert_eq!(counts.systems, 2);
        assert_eq!(counts.duplicates, 0);
    }

    #[test]
    fn updates_do_not_duplicate_paths() {
        let db = LibraryDb::open_in_memory().unwrap();
        let mut entry = game("a.nes", "NES", "hash-1");
        db.upsert(&entry).unwrap();
        entry.title = "Titre corrigé".into();
        db.upsert(&entry).unwrap();

        assert_eq!(db.games().unwrap().len(), 1);
        assert_eq!(db.games().unwrap()[0].title, "Titre corrigé");
    }

    #[test]
    fn detects_duplicate_checksums() {
        let db = LibraryDb::open_in_memory().unwrap();
        db.upsert(&game("a.nes", "NES", "same")).unwrap();
        db.upsert(&game("b.nes", "NES", "same")).unwrap();
        assert_eq!(db.counts().unwrap().duplicates, 1);
    }

    #[test]
    fn removes_files_not_seen_in_a_completed_scan() {
        let db = LibraryDb::open_in_memory().unwrap();
        db.upsert(&game("a.nes", "NES", "a")).unwrap();
        db.upsert(&game("b.nes", "NES", "b")).unwrap();

        let seen = [game("a.nes", "NES", "a").path].into_iter().collect();
        assert_eq!(db.remove_missing(&seen).unwrap(), 1);
        assert_eq!(db.games().unwrap().len(), 1);
    }

    #[test]
    fn apply_scan_commits_and_prunes_only_when_asked() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let entry = game("a.nes", "NES", "a");
        let seen = HashSet::from([entry.path.clone()]);

        db.apply_scan(&[(entry, ScanOutcome::Added)], &seen, &[])
            .unwrap();
        assert_eq!(db.games().unwrap().len(), 1);

        db.apply_scan(&[], &HashSet::new(), &[PathBuf::from("/roms")])
            .unwrap();
        assert!(db.games().unwrap().is_empty());
    }

    #[test]
    fn scoped_pruning_preserves_entries_from_unavailable_roots() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let available = game("available.nes", "NES", "a");
        let unavailable = ScannedGame {
            path: "/external/missing.sfc".into(),
            title: "missing".into(),
            system: "Super Nintendo".into(),
            size: 42,
            modified_ns: 1,
            checksum: "b".into(),
            source_dir: "/external".into(),
        };
        db.upsert(&available).unwrap();
        db.upsert(&unavailable).unwrap();
        db.toggle_favorite("/external/missing.sfc").unwrap();
        db.mark_played("/external/missing.sfc").unwrap();

        let seen = HashSet::from([available.path.clone()]);
        let removed = db
            .apply_scan(
                &[(available, ScanOutcome::Unchanged)],
                &seen,
                &[PathBuf::from("/roms")],
            )
            .unwrap();

        assert_eq!(removed, 0);
        assert_eq!(db.games().unwrap().len(), 2);
        assert!(
            db.favorite_paths()
                .unwrap()
                .contains("/external/missing.sfc")
        );
        assert_eq!(db.recently_played(10).unwrap().len(), 1);
    }

    #[test]
    fn favorites_can_be_toggled_and_listed() {
        let db = LibraryDb::open_in_memory().unwrap();
        let path = game("a.nes", "NES", "a");
        db.upsert(&path).unwrap();

        let key = path.path.to_string_lossy().to_string();
        assert!(db.toggle_favorite(&key).unwrap());
        assert!(db.favorite_paths().unwrap().contains(&key));
        assert_eq!(db.favorites().unwrap().len(), 1);

        assert!(!db.toggle_favorite(&key).unwrap());
        assert!(db.favorites().unwrap().is_empty());
    }

    #[test]
    fn play_history_tracks_recent_games() {
        let db = LibraryDb::open_in_memory().unwrap();
        db.upsert(&game("a.nes", "NES", "a")).unwrap();
        db.upsert(&game("b.nes", "NES", "b")).unwrap();

        db.mark_played("/roms/a.nes").unwrap();
        db.mark_played("/roms/b.nes").unwrap();
        db.mark_played("/roms/a.nes").unwrap();

        let recent = db.recently_played(2).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].title, "a.nes");
    }

    #[test]
    fn removing_a_game_cleans_up_favorites_and_history() {
        let db = LibraryDb::open_in_memory().unwrap();
        db.upsert(&game("a.nes", "NES", "a")).unwrap();
        db.toggle_favorite("/roms/a.nes").unwrap();
        db.mark_played("/roms/a.nes").unwrap();
        db.remove_missing(&HashSet::new()).unwrap();

        assert!(db.favorite_paths().unwrap().is_empty());
        assert!(db.recently_played(10).unwrap().is_empty());
    }

    #[test]
    fn system_summaries_are_grouped() {
        let db = LibraryDb::open_in_memory().unwrap();
        db.upsert(&game("a.nes", "NES", "a")).unwrap();
        db.upsert(&game("b.nes", "NES", "b")).unwrap();
        let systems = db.systems().unwrap();
        assert_eq!(systems[0].name, "NES");
        assert_eq!(systems[0].game_count, 2);
    }
}
