//! Read-only game-library discovery primitives.
//!
//! The scanner does not identify systems, mutate files, or launch emulators.

mod model;
mod scanner;

pub use model::GameFile;
pub use scanner::{ScanError, ScanIssue, ScanOptions, ScanReport, scan_directory};
