//! Read-only game directory scanner.
//!
//! It walks configured roots, recognizes supported ROM/disc extensions,
//! computes SHA-256 checksums for new or changed files, and reports progress.
//! Source files are only ever opened for reading.

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use super::systems::system_for_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFile {
    pub size: i64,
    pub modified_ns: i64,
    pub checksum: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanOutcome {
    Added,
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedGame {
    pub path: PathBuf,
    pub title: String,
    pub system: String,
    pub size: i64,
    pub modified_ns: i64,
    pub checksum: String,
    pub source_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub scanned: usize,
    pub total: usize,
    pub current: PathBuf,
}

#[derive(Debug, Default)]
pub struct ScanResult {
    pub games: Vec<(ScannedGame, ScanOutcome)>,
    pub cancelled: bool,
    pub errors: usize,
}

fn modified_ns(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as i64)
        .unwrap_or(0)
}

/// SHA-256 of a file streamed in chunks. Used by the scanner and the BIOS
/// inventory; files are only ever opened for reading.
pub fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.depth() > 0
        && entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with('.'))
}

/// Scans one root. `known` contains files already in the index, keyed by path.
pub fn scan_root(
    root: &Path,
    known: &HashMap<PathBuf, KnownFile>,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(ScanProgress),
) -> ScanResult {
    let mut result = ScanResult::default();
    let mut files = Vec::new();

    // SQLite stores paths as TEXT. Reject non-UTF-8 roots explicitly rather
    // than silently indexing a lossy path that may collide with another file.
    if root.to_str().is_none() {
        result.errors += 1;
        return result;
    }

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_hidden(entry))
    {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            return result;
        }

        match entry {
            Ok(entry) if entry.file_type().is_file() && system_for_path(entry.path()).is_some() => {
                if entry.path().to_str().is_none() {
                    result.errors += 1;
                } else {
                    files.push(entry.into_path());
                }
            }
            Ok(_) => {}
            Err(_) => result.errors += 1,
        }
    }

    let total = files.len();
    for (index, path) in files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            return result;
        }

        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => {
                result.errors += 1;
                continue;
            }
        };

        let size = metadata.len() as i64;
        let modified = modified_ns(&metadata);

        let (checksum, outcome) = match known.get(path) {
            Some(known)
                if known.size == size
                    && known.modified_ns == modified
                    && !known.checksum.is_empty() =>
            {
                (known.checksum.clone(), ScanOutcome::Unchanged)
            }
            Some(_) => match hash_file(path) {
                Ok(value) => (value, ScanOutcome::Updated),
                Err(_) => {
                    result.errors += 1;
                    continue;
                }
            },
            None => match hash_file(path) {
                Ok(value) => (value, ScanOutcome::Added),
                Err(_) => {
                    result.errors += 1;
                    continue;
                }
            },
        };

        let title = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Sans titre")
            .to_owned();
        let system = system_for_path(path).unwrap_or("Autre").to_owned();

        result.games.push((
            ScannedGame {
                path: path.clone(),
                title,
                system,
                size,
                modified_ns: modified,
                checksum,
                source_dir: root.to_path_buf(),
            },
            outcome,
        ));

        on_progress(ScanProgress {
            scanned: index + 1,
            total,
            current: path.clone(),
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::{ScanOutcome, scan_root};
    use std::{collections::HashMap, fs, sync::atomic::AtomicBool};

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_game_paths_without_lossy_indexing() {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir().unwrap();
        let name = std::ffi::OsString::from_vec(b"rom-\\xff.nes".to_vec());
        fs::write(dir.path().join(name), b"rom").unwrap();

        let result = scan_root(
            dir.path(),
            &HashMap::new(),
            &AtomicBool::new(false),
            &mut |_| {},
        );

        assert_eq!(result.errors, 1);
        assert!(result.games.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_scan_roots() {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join(std::ffi::OsString::from_vec(b"roms-\\xff".to_vec()));
        fs::create_dir(&root).unwrap();

        let result = scan_root(
            &root,
            &HashMap::new(),
            &AtomicBool::new(false),
            &mut |_| {},
        );

        assert_eq!(result.errors, 1);
        assert!(result.games.is_empty());
    }

    #[test]
    fn discovers_supported_files_only() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("mario.sfc"), b"rom").unwrap();
        fs::write(dir.path().join("notes.txt"), b"ignore").unwrap();

        let result = scan_root(
            dir.path(),
            &HashMap::new(),
            &AtomicBool::new(false),
            &mut |_| {},
        );

        assert_eq!(result.games.len(), 1);
        assert_eq!(result.games[0].0.title, "mario");
        assert_eq!(result.games[0].0.system, "Super Nintendo");
        assert_eq!(result.games[0].1, ScanOutcome::Added);
        assert!(!result.games[0].0.checksum.is_empty());
    }

    #[test]
    fn unchanged_files_keep_their_checksum_without_rehashing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.nes");
        fs::write(&path, b"rom").unwrap();

        let first = scan_root(
            dir.path(),
            &HashMap::new(),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        let game = &first.games[0].0;
        let known = HashMap::from([(
            game.path.clone(),
            super::KnownFile {
                size: game.size,
                modified_ns: game.modified_ns,
                checksum: "cached".into(),
            },
        )]);

        let second = scan_root(dir.path(), &known, &AtomicBool::new(false), &mut |_| {});
        assert_eq!(second.games[0].1, ScanOutcome::Unchanged);
        assert_eq!(second.games[0].0.checksum, "cached");
    }

    #[test]
    fn hidden_directories_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        fs::write(dir.path().join(".hidden/secret.nes"), b"rom").unwrap();
        fs::write(dir.path().join("visible.nes"), b"rom").unwrap();

        let result = scan_root(
            dir.path(),
            &HashMap::new(),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        assert_eq!(result.games.len(), 1);
        assert_eq!(result.games[0].0.title, "visible");
    }

    #[test]
    fn cancellation_stops_the_scan() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.nes"), b"a").unwrap();
        fs::write(dir.path().join("b.nes"), b"b").unwrap();
        let cancel = AtomicBool::new(true);

        let result = scan_root(dir.path(), &HashMap::new(), &cancel, &mut |_| {});
        assert!(result.cancelled);
        assert!(result.games.is_empty());
    }
}
