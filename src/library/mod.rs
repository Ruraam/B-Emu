//! Read-only game-library discovery primitives.
//!
//! The scanner does not identify systems, mutate files, or launch emulators.

mod model;
mod scanner;

pub use model::GameFile;
pub use scanner::{scan_directory, ScanError, ScanIssue, ScanOptions, ScanReport};
