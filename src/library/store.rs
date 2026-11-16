//! SQLite persistence for the user's game-library index.
//!
//! Filesystem scanning remains separate. Only complete scans may replace a
//! root's stored snapshot; partial scans never delete previously indexed files.

use std::{
    error::Error,
    fmt,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{GameFile, ScanReport};

const SCHEMA_VERSION: i64 = 1;
const BUSY_TIMEOUT_SECS: u64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRoot {
    pub id: i64,
    pub path: PathBuf,
    pub recursive: bool,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug)]
pub enum StoreError {
    Sql(rusqlite::Error),
    InvalidPath(PathBuf),
    NonUtf8Path(PathBuf),
    PathOutsideRoot { path: PathBuf, root: PathBuf },
    InvalidFileSize(PathBuf),
    InvalidTimestamp,
    UnsupportedSchemaVersion(i64),
    IncompleteScan,
    RootNotFound(i64),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(error) => write!(f, "library database error: {error}"),
            Self::InvalidPath(path) => write!(f, "library path must be absolute and normalized: {}", path.display()),
            Self::NonUtf8Path(path) => write!(f, "library path is not valid UTF-8: {}", path.display()),
            Self::PathOutsideRoot { path, root } => write!(f, "game path {} is outside library root {}", path.display(), root.display()),
            Self::InvalidFileSize(path) => write!(f, "file size is too large for SQLite: {}", path.display()),
            Self::InvalidTimestamp => write!(f, "system time is outside the supported Unix timestamp range"),
            Self::UnsupportedSchemaVersion(version) => write!(f, "library database schema version {version} is newer than supported version {SCHEMA_VERSION}"),
            Self::IncompleteScan => write!(f, "refusing to replace library snapshot with an incomplete or cancelled scan"),
            Self::RootNotFound(id) => write!(f, "library root {id} does not exist"),
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sql(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sql(value)
    }
}

pub struct LibraryStore {
    connection: Connection,
}

impl LibraryStore {
    /// Open or create a database at a user-owned path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    /// Open an in-memory database, primarily for isolated tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, StoreError> {
        connection.busy_timeout(std::time::Duration::from_secs(BUSY_TIMEOUT_SECS))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&connection)?;
        Ok(Self { connection })
    }

    /// Add a root, or update its recursive setting if it already exists.
    pub fn add_root(&mut self, path: &Path, recursive: bool) -> Result<i64, StoreError> {
        let normalized = normalize_absolute_path(path)?;
        let path_text = path_to_text(&normalized)?;
        let now = unix_now()?;
        let transaction = self.connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        transaction.execute(
            "INSERT INTO library_roots (path, recursive, enabled, created_at, updated_at)
             VALUES (?1, ?2, 1, ?3, ?3)
             ON CONFLICT(path) DO UPDATE SET
                 recursive = excluded.recursive,
                 enabled = 1,
                 updated_at = excluded.updated_at",
            params![path_text, recursive as i64, now],
        )?;

        let id = transaction.query_row(
            "SELECT id FROM library_roots WHERE path = ?1",
            [path_text],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn remove_root(&mut self, root_id: i64) -> Result<bool, StoreError> {
        let changed = self.connection.execute(
            "DELETE FROM library_roots WHERE id = ?1",
            [root_id],
        )?;
        Ok(changed != 0)
    }

    pub fn list_roots(&self) -> Result<Vec<LibraryRoot>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, path, recursive, enabled, created_at, updated_at
             FROM library_roots ORDER BY path",
        )?;
        let rows = statement.query_map([], |row| {
            let path: String = row.get(1)?;
            Ok(LibraryRoot {
                id: row.get(0)?,
                path: PathBuf::from(path),
                recursive: row.get::<_, i64>(2)? != 0,
                enabled: row.get::<_, i64>(3)? != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::from)
    }

    pub fn games_for_root(&self, root_id: i64) -> Result<Vec<GameFile>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT path, file_name, extension, size_bytes, modified_unix_secs
             FROM games WHERE root_id = ?1 ORDER BY path",
        )?;
        let rows = statement.query_map([root_id], |row| {
            let path: String = row.get(0)?;
            let size_bytes: i64 = row.get(3)?;
            Ok(GameFile {
                path: PathBuf::from(path),
                file_name: row.get(1)?,
                extension: row.get(2)?,
                size_bytes: size_bytes as u64,
                modified_unix_secs: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::from)
    }

    /// Atomically replace one root's snapshot with a complete scan report.
    ///
    /// Any scan issue or cancellation rejects the operation and preserves the
    /// old snapshot. All paths are validated before the transaction begins.
    pub fn commit_scan(
        &mut self,
        root_id: i64,
        report: &ScanReport,
    ) -> Result<(), StoreError> {
        if report.cancelled || !report.issues.is_empty() {
            return Err(StoreError::IncompleteScan);
        }

        let root_text: String = self.connection
            .query_row(
                "SELECT path FROM library_roots WHERE id = ?1",
                [root_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::RootNotFound(root_id))?;
        let root = PathBuf::from(root_text);
        let mut validated = Vec::with_capacity(report.files.len());

        for game in &report.files {
            let path = normalize_absolute_path(&game.path)?;
            if !path.starts_with(&root) {
                return Err(StoreError::PathOutsideRoot {
                    path,
                    root: root.clone(),
                });
            }
            let path_text = path_to_text(&path)?;
            let size = i64::try_from(game.size_bytes)
                .map_err(|_| StoreError::InvalidFileSize(game.path.clone()))?;
            let modified = game.modified_unix_secs
                .map(i64::try_from)
                .transpose()
                .map_err(|_| StoreError::InvalidTimestamp)?;
            validated.push((
                path_text,
                game.file_name.clone(),
                game.extension.clone(),
                size,
                modified,
            ));
        }

        let now = unix_now()?;
        let transaction = self.connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS scan_seen_paths (
                path TEXT PRIMARY KEY
             );
             DELETE FROM scan_seen_paths;",
        )?;

        for (path, file_name, extension, size, modified) in &validated {
            transaction.execute(
                "INSERT INTO games
                    (root_id, path, file_name, extension, size_bytes, modified_unix_secs, discovered_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(root_id, path) DO UPDATE SET
                    file_name = excluded.file_name,
                    extension = excluded.extension,
                    size_bytes = excluded.size_bytes,
                    modified_unix_secs = excluded.modified_unix_secs,
                    discovered_at = excluded.discovered_at",
                params![root_id, path, file_name, extension, size, modified, now],
            )?;
            transaction.execute(
                "INSERT INTO scan_seen_paths (path) VALUES (?1)",
                [path],
            )?;
        }

        transaction.execute(
            "DELETE FROM games
             WHERE root_id = ?1
               AND path NOT IN (SELECT path FROM scan_seen_paths)",
            [root_id],
        )?;
        transaction.execute(
            "UPDATE library_roots SET updated_at = ?1 WHERE id = ?2",
            params![now, root_id],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn migrate(connection: &Connection) -> Result<(), StoreError> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchemaVersion(version));
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }

    let transaction = connection.unchecked_transaction()?;
    transaction.execute_batch(
        "CREATE TABLE library_roots (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            recursive INTEGER NOT NULL CHECK (recursive IN (0, 1)),
            enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
         );

         CREATE TABLE games (
            id INTEGER PRIMARY KEY,
            root_id INTEGER NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
            path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            extension TEXT,
            size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
            modified_unix_secs INTEGER CHECK (modified_unix_secs IS NULL OR modified_unix_secs >= 0),
            discovered_at INTEGER NOT NULL,
            UNIQUE (root_id, path)
         );

         CREATE INDEX games_root_id_idx ON games(root_id);
         CREATE INDEX games_root_file_name_idx ON games(root_id, file_name);
         CREATE INDEX games_root_extension_idx ON games(root_id, extension);

         PRAGMA user_version = 1;",
    )?;
    transaction.commit()?;
    Ok(())
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf, StoreError> {
    if !path.is_absolute() {
        return Err(StoreError::InvalidPath(path.to_path_buf()));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(StoreError::InvalidPath(path.to_path_buf()));
                }
            }
        }
    }
    Ok(normalized)
}

fn path_to_text(path: &Path) -> Result<&str, StoreError> {
    path.to_str().ok_or_else(|| StoreError::NonUtf8Path(path.to_path_buf()))
}

fn unix_now() -> Result<i64, StoreError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::InvalidTimestamp)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| StoreError::InvalidTimestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "b-emu-store-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn game(path: PathBuf, size_bytes: u64) -> GameFile {
        GameFile {
            file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
            extension: path.extension().map(|value| value.to_string_lossy().to_ascii_lowercase()),
            path,
            size_bytes,
            modified_unix_secs: Some(1_700_000_000),
        }
    }

    #[test]
    fn creates_schema_and_reopens_without_recreating_it() {
        let store = LibraryStore::open_in_memory().unwrap();
        let version: i64 = store.connection.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(store.list_roots().unwrap().len(), 0);
    }

    #[test]
    fn rejects_a_newer_schema_version() {
        let connection = Connection::open_in_memory().unwrap();
        connection.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
        let error = LibraryStore::from_connection(connection).unwrap_err();
        assert!(matches!(error, StoreError::UnsupportedSchemaVersion(2)));
    }

    #[test]
    fn roots_are_unique_and_update_the_recursive_setting() {
        let mut store = LibraryStore::open_in_memory().unwrap();
        let root = PathBuf::from("/tmp/b-emu-library");
        let first = store.add_root(&root, true).unwrap();
        let second = store.add_root(&root, false).unwrap();
        assert_eq!(first, second);
        let roots = store.list_roots().unwrap();
        assert_eq!(roots.len(), 1);
        assert!(!roots[0].recursive);
    }

    #[test]
    fn complete_scan_replaces_snapshot_atomically() {
        let mut store = LibraryStore::open_in_memory().unwrap();
        let root = PathBuf::from("/tmp/b-emu-library");
        let root_id = store.add_root(&root, true).unwrap();
        let first = game(root.join("old.rom"), 10);
        store.commit_scan(root_id, &ScanReport {
            files: vec![first],
            issues: vec![],
            cancelled: false,
        }).unwrap();

        store.commit_scan(root_id, &ScanReport {
            files: vec![game(root.join("new.iso"), 20)],
            issues: vec![],
            cancelled: false,
        }).unwrap();

        let games = store.games_for_root(root_id).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].file_name, "new.iso");
    }

    #[test]
    fn incomplete_scan_preserves_existing_snapshot() {
        let mut store = LibraryStore::open_in_memory().unwrap();
        let root = PathBuf::from("/tmp/b-emu-library");
        let root_id = store.add_root(&root, true).unwrap();
        store.commit_scan(root_id, &ScanReport {
            files: vec![game(root.join("known.rom"), 10)],
            issues: vec![],
            cancelled: false,
        }).unwrap();

        let result = store.commit_scan(root_id, &ScanReport {
            files: vec![],
            issues: vec![super::super::ScanIssue {
                path: root.clone(),
                message: "permission denied".into(),
            }],
            cancelled: false,
        });
        assert!(matches!(result, Err(StoreError::IncompleteScan)));
        assert_eq!(store.games_for_root(root_id).unwrap().len(), 1);
    }

    #[test]
    fn rejects_files_outside_the_registered_root() {
        let mut store = LibraryStore::open_in_memory().unwrap();
        let root_id = store.add_root(Path::new("/tmp/b-emu-library"), true).unwrap();
        let result = store.commit_scan(root_id, &ScanReport {
            files: vec![game(PathBuf::from("/tmp/outside.rom"), 10)],
            issues: vec![],
            cancelled: false,
        });
        assert!(matches!(result, Err(StoreError::PathOutsideRoot { .. })));
        assert!(store.games_for_root(root_id).unwrap().is_empty());
    }

    #[test]
    fn removing_a_root_cascades_to_its_games() {
        let mut store = LibraryStore::open_in_memory().unwrap();
        let root = PathBuf::from("/tmp/b-emu-library");
        let root_id = store.add_root(&root, true).unwrap();
        store.commit_scan(root_id, &ScanReport {
            files: vec![game(root.join("game.rom"), 10)],
            issues: vec![],
            cancelled: false,
        }).unwrap();
        assert!(store.remove_root(root_id).unwrap());
        assert!(store.games_for_root(root_id).unwrap().is_empty());
    }
}
