//! SQLite index for scanned games. Only B-Emu's own data directory is written.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, params};

use super::scanner::{KnownFile, ScannedGame};

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
            CREATE INDEX IF NOT EXISTS games_system ON games(system);
            CREATE INDEX IF NOT EXISTS games_checksum ON games(checksum);",
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

    pub fn upsert(&self, game: &ScannedGame) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO games (path, title, system, size, modified_ns, checksum, source_dir)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                system = excluded.system,
                size = excluded.size,
                modified_ns = excluded.modified_ns,
                checksum = excluded.checksum,
                source_dir = excluded.source_dir",
            params![
                game.path.to_string_lossy(),
                game.title,
                game.system,
                game.size,
                game.modified_ns,
                game.checksum,
                game.source_dir.to_string_lossy(),
            ],
        )?;
        Ok(())
    }

    /// Removes rows that were not seen during a completed scan. This covers
    /// deleted files and directories removed from the configuration.
    pub fn remove_missing(&self, seen: &HashSet<PathBuf>) -> Result<usize, rusqlite::Error> {
        let mut removed = 0;
        for path in self.all_paths()? {
            if !seen.contains(&path) {
                removed += self.conn.execute(
                    "DELETE FROM games WHERE path = ?1",
                    params![path.to_string_lossy()],
                )?;
            }
        }
        Ok(removed)
    }

    fn all_paths(&self) -> Result<Vec<PathBuf>, rusqlite::Error> {
        let mut statement = self.conn.prepare("SELECT path FROM games")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| row.map(PathBuf::from)).collect()
    }

    pub fn games(&self) -> Result<Vec<GameRecord>, rusqlite::Error> {
        self.query_games(
            "SELECT path, title, system, size, checksum, source_dir FROM games ORDER BY title",
            [],
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
}

#[cfg(test)]
mod tests {
    use super::LibraryDb;
    use crate::library::scanner::ScannedGame;

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
    fn system_summaries_are_grouped() {
        let db = LibraryDb::open_in_memory().unwrap();
        db.upsert(&game("a.nes", "NES", "a")).unwrap();
        db.upsert(&game("b.nes", "NES", "b")).unwrap();
        let systems = db.systems().unwrap();
        assert_eq!(systems[0].name, "NES");
        assert_eq!(systems[0].game_count, 2);
    }
}
