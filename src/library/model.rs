use std::path::PathBuf;

/// Filesystem metadata for a discovered regular file; not proof that it is a ROM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameFile {
    pub path: PathBuf,
    pub file_name: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub modified_unix_secs: Option<u64>,
}
