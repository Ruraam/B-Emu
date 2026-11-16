use std::{
    collections::HashSet,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};

use super::GameFile;

/// Controls a read-only scan. An empty extension set accepts every regular file.
#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub recursive: bool,
    pub max_depth: Option<usize>,
    pub extensions: HashSet<String>,
}

impl ScanOptions {
    pub fn recursive() -> Self {
        Self {
            recursive: true,
            max_depth: None,
            extensions: HashSet::new(),
        }
    }

    pub fn with_extensions<I, S>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.extensions = extensions
            .into_iter()
            .map(|extension| {
                extension
                    .as_ref()
                    .trim()
                    .trim_start_matches('.')
                    .to_ascii_lowercase()
            })
            .filter(|extension| !extension.is_empty())
            .collect();
        self
    }

    fn accepts_extension(&self, extension: Option<&str>) -> bool {
        self.extensions.is_empty() || extension.is_some_and(|value| self.extensions.contains(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct ScanReport {
    pub files: Vec<GameFile>,
    pub issues: Vec<ScanIssue>,
    pub cancelled: bool,
}

#[derive(Debug)]
pub enum ScanError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    RootIsSymlink(PathBuf),
    RootNotDirectory(PathBuf),
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "cannot inspect {}: {source}", path.display())
            }
            Self::RootIsSymlink(path) => write!(
                formatter,
                "library root must not be a symbolic link: {}",
                path.display()
            ),
            Self::RootNotDirectory(path) => {
                write!(formatter, "library root is not a directory: {}", path.display())
            }
        }
    }
}

impl Error for ScanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::RootIsSymlink(_) | Self::RootNotDirectory(_) => None,
        }
    }
}

/// Enumerate regular files without following symlinks or modifying the filesystem.
///
/// Inaccessible descendants are reported and do not abort the scan. Root
/// inspection errors are fatal. Cancellation is cooperative via the atomic flag.
pub fn scan_directory(
    root: &Path,
    options: &ScanOptions,
    cancel: &AtomicBool,
) -> Result<ScanReport, ScanError> {
    let root_metadata = fs::symlink_metadata(root).map_err(|source| ScanError::Io {
        path: root.to_path_buf(),
        source,
    })?;

    if root_metadata.file_type().is_symlink() {
        return Err(ScanError::RootIsSymlink(root.to_path_buf()));
    }
    if !root_metadata.is_dir() {
        return Err(ScanError::RootNotDirectory(root.to_path_buf()));
    }

    let mut report = ScanReport::default();
    visit_directory(root, 0, options, cancel, &mut report);
    report
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    report.cancelled = cancel.load(Ordering::Acquire);
    Ok(report)
}

fn visit_directory(
    directory: &Path,
    depth: usize,
    options: &ScanOptions,
    cancel: &AtomicBool,
    report: &mut ScanReport,
) {
    if cancel.load(Ordering::Acquire) {
        return;
    }

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            report.issues.push(ScanIssue {
                path: directory.to_path_buf(),
                message: error.to_string(),
            });
            return;
        }
    };

    for entry in entries {
        if cancel.load(Ordering::Acquire) {
            return;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report.issues.push(ScanIssue {
                    path: directory.to_path_buf(),
                    message: error.to_string(),
                });
                continue;
            }
        };
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                report.issues.push(ScanIssue {
                    path,
                    message: error.to_string(),
                });
                continue;
            }
        };

        // Never follow symlinks: avoid loops and traversal outside the selected root.
        if metadata.file_type().is_symlink() {
            continue;
        }

        if metadata.is_dir() {
            let within_depth = options.max_depth.is_none_or(|limit| depth < limit);
            if options.recursive && within_depth {
                visit_directory(&path, depth + 1, options, cancel, report);
            }
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        if !options.accepts_extension(extension.as_deref()) {
            continue;
        }

        let file_name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        let modified_unix_secs = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs());

        report.files.push(GameFile {
            path,
            file_name,
            extension,
            size_bytes: metadata.len(),
            modified_unix_secs,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("b-emu-library-{}-{nonce}-{id}", std::process::id()));
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

    fn write_file(path: &Path, contents: &[u8]) {
        let mut file = File::create(path).unwrap();
        file.write_all(contents).unwrap();
    }

    #[test]
    fn scans_regular_files_recursively_and_sorts_results() {
        let temp = TempDir::new();
        fs::create_dir(temp.path().join("nested")).unwrap();
        write_file(&temp.path().join("z.rom"), b"123");
        write_file(&temp.path().join("nested").join("a.iso"), b"12345");

        let report = scan_directory(
            temp.path(),
            &ScanOptions::recursive(),
            &AtomicBool::new(false),
        )
        .unwrap();

        assert_eq!(report.files.len(), 2);
        assert!(report.files[0].path.ends_with("a.iso"));
        assert!(report.files[1].path.ends_with("z.rom"));
        assert_eq!(report.files[1].size_bytes, 3);
        assert!(report.issues.is_empty());
        assert!(!report.cancelled);
    }

    #[test]
    fn filters_extensions_case_insensitively() {
        let temp = TempDir::new();
        write_file(&temp.path().join("game.ZIP"), b"zip");
        write_file(&temp.path().join("disc.iso"), b"iso");
        let options = ScanOptions::recursive().with_extensions([".zip"]);
        let report = scan_directory(temp.path(), &options, &AtomicBool::new(false)).unwrap();
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].extension.as_deref(), Some("zip"));
    }

    #[test]
    fn non_recursive_scan_does_not_enter_child_directories() {
        let temp = TempDir::new();
        fs::create_dir(temp.path().join("nested")).unwrap();
        write_file(&temp.path().join("root.rom"), b"root");
        write_file(&temp.path().join("nested").join("child.rom"), b"child");
        let options = ScanOptions {
            recursive: false,
            ..ScanOptions::default()
        };
        let report = scan_directory(temp.path(), &options, &AtomicBool::new(false)).unwrap();
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].file_name, "root.rom");
    }

    #[test]
    fn cancellation_returns_a_marked_partial_report() {
        let temp = TempDir::new();
        write_file(&temp.path().join("game.rom"), b"rom");
        let cancel = AtomicBool::new(true);
        let report = scan_directory(temp.path(), &ScanOptions::recursive(), &cancel).unwrap();
        assert!(report.cancelled);
        assert!(report.files.is_empty());
    }

    #[test]
    fn rejects_a_regular_file_as_the_library_root() {
        let temp = TempDir::new();
        let file = temp.path().join("not-a-directory");
        write_file(&file, b"x");
        let error =
            scan_directory(&file, &ScanOptions::recursive(), &AtomicBool::new(false)).unwrap_err();
        assert!(matches!(error, ScanError::RootNotDirectory(_)));
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_symbolic_links() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let outside = TempDir::new();
        write_file(&outside.path().join("outside.rom"), b"outside");
        symlink(outside.path(), temp.path().join("linked-directory")).unwrap();
        let report = scan_directory(
            temp.path(),
            &ScanOptions::recursive(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(report.files.is_empty());
    }
}
