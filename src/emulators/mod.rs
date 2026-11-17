//! Emulator adapters, discovery and supervised process launching.

pub mod adapters;
pub mod discovery;
pub mod launch;

pub use discovery::EmulatorInstallation;
pub use launch::{ExitResult, LaunchError, SpawnedEmulator};

use adapters::supporting_system;

/// First ready adapter that declares support for a system.
pub fn for_system<'a>(
    system: &str,
    installations: &'a [EmulatorInstallation],
) -> Option<&'a EmulatorInstallation> {
    supporting_system(system, installations)
}
