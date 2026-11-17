//! BIOS/firmware inventory and missing-file diagnostics.

use std::path::Path;

use crate::{emulators::adapters::BiosRequirement, library::hash_file};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiosStatus {
    pub emulator_id: String,
    pub name: String,
    pub system: String,
    pub present: bool,
    pub checksum: Option<String>,
}

/// Checks declared BIOS requirements against the user's BIOS directory.
/// Missing files are reported explicitly; nothing is downloaded or guessed.
pub fn inventory(dir: &Path, requirements: &[(String, BiosRequirement)]) -> Vec<BiosStatus> {
    requirements
        .iter()
        .map(|(emulator_id, requirement)| {
            let candidate = dir.join(requirement.name);
            let found = if candidate.is_file() {
                Some(candidate)
            } else {
                let lowercase = dir.join(requirement.name.to_lowercase());
                lowercase.is_file().then_some(lowercase)
            };
            let checksum = found.as_deref().and_then(|path| hash_file(path).ok());

            BiosStatus {
                emulator_id: emulator_id.clone(),
                name: requirement.name.to_owned(),
                system: requirement.system.to_owned(),
                present: found.is_some(),
                checksum,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::inventory;
    use crate::emulators::adapters::BiosRequirement;
    use std::fs;

    const REQUIREMENT: BiosRequirement = BiosRequirement {
        system: "PlayStation 2",
        name: "SCPH-10000.bin",
    };

    #[test]
    fn missing_bios_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let statuses = inventory(dir.path(), &[("pcsx2".into(), REQUIREMENT)]);
        assert_eq!(statuses.len(), 1);
        assert!(!statuses[0].present);
        assert_eq!(statuses[0].checksum, None);
    }

    #[test]
    fn present_bios_is_hashed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("SCPH-10000.bin"), b"bios").unwrap();
        let statuses = inventory(dir.path(), &[("pcsx2".into(), REQUIREMENT)]);
        assert!(statuses[0].present);
        assert!(
            statuses[0]
                .checksum
                .as_ref()
                .is_some_and(|value| value.len() == 64)
        );
    }
}
