//! Declarative emulator adapter contract.
//!
//! An adapter only declares data and an argument builder. Arguments are always
//! returned as an `OsString` array: no command string is ever concatenated or
//! passed through a shell.

use std::{collections::BTreeSet, ffi::OsString, path::Path};

/// A setting that is only exposed when the adapter declares support for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    pub key: &'static str,
    pub label: &'static str,
    pub flag: &'static str,
}

/// Known firmware requirement. Detection of presence belongs to Phase 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiosRequirement {
    pub system: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Adapter {
    pub id: &'static str,
    pub name: &'static str,
    pub executable: &'static str,
    pub systems: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub version_args: &'static [&'static str],
    pub capabilities: &'static [Capability],
    pub bios: &'static [BiosRequirement],
    pub build_args: fn(&Path, &BTreeSet<String>) -> Vec<OsString>,
}

fn simple_args(
    rom: &Path,
    enabled: &BTreeSet<String>,
    capability_flags: &[Capability],
) -> Vec<OsString> {
    let mut args = Vec::new();
    for capability in capability_flags {
        if enabled.contains(capability.key) {
            args.push(OsString::from(capability.flag));
        }
    }
    args.push(rom.as_os_str().to_owned());
    args
}

const RETROARCH_CAPS: &[Capability] = &[
    Capability {
        key: "fullscreen",
        label: "Plein écran",
        flag: "--fullscreen",
    },
    Capability {
        key: "mute_audio",
        label: "Couper le son",
        flag: "--mute-audio",
    },
];

const DOLPHIN_CAPS: &[Capability] = &[Capability {
    key: "fullscreen",
    label: "Plein écran",
    flag: "--fullscreen",
}];

const PCSX2_CAPS: &[Capability] = &[Capability {
    key: "fullscreen",
    label: "Plein écran",
    flag: "--fullscreen",
}];

const PPSSPP_CAPS: &[Capability] = &[Capability {
    key: "fullscreen",
    label: "Plein écran",
    flag: "--fullscreen",
}];

const MAME_CAPS: &[Capability] = &[Capability {
    key: "windowed",
    label: "Fenêtré",
    flag: "-window",
}];

fn retroarch_args(rom: &Path, enabled: &BTreeSet<String>) -> Vec<OsString> {
    simple_args(rom, enabled, RETROARCH_CAPS)
}

fn dolphin_args(rom: &Path, enabled: &BTreeSet<String>) -> Vec<OsString> {
    let mut args = Vec::new();
    for capability in DOLPHIN_CAPS {
        if enabled.contains(capability.key) {
            args.push(OsString::from(capability.flag));
        }
    }
    args.push(OsString::from("-e"));
    args.push(rom.as_os_str().to_owned());
    args
}

fn pcsx2_args(rom: &Path, enabled: &BTreeSet<String>) -> Vec<OsString> {
    simple_args(rom, enabled, PCSX2_CAPS)
}

fn ppsspp_args(rom: &Path, enabled: &BTreeSet<String>) -> Vec<OsString> {
    simple_args(rom, enabled, PPSSPP_CAPS)
}

fn mame_args(rom: &Path, enabled: &BTreeSet<String>) -> Vec<OsString> {
    simple_args(rom, enabled, MAME_CAPS)
}

/// First adapters selected for availability and testability on Debian.
pub const ADAPTERS: &[Adapter] = &[
    Adapter {
        id: "retroarch",
        name: "RetroArch",
        executable: "retroarch",
        systems: &[
            "NES",
            "Super Nintendo",
            "Mega Drive",
            "Nintendo 64",
            "Game Boy",
            "Game Boy Advance",
            "PlayStation",
            "Arcade",
        ],
        extensions: &[
            "nes", "sfc", "smc", "md", "gen", "z64", "v64", "n64", "gb", "gbc", "gba", "cue",
            "chd", "zip",
        ],
        version_args: &["--version"],
        capabilities: RETROARCH_CAPS,
        bios: &[],
        build_args: retroarch_args,
    },
    Adapter {
        id: "dolphin",
        name: "Dolphin",
        executable: "dolphin-emu",
        systems: &["GameCube"],
        extensions: &["iso", "gcm", "gcn"],
        version_args: &["--version"],
        capabilities: DOLPHIN_CAPS,
        bios: &[],
        build_args: dolphin_args,
    },
    Adapter {
        id: "pcsx2",
        name: "PCSX2",
        executable: "PCSX2",
        systems: &["PlayStation 2"],
        extensions: &["iso", "cue", "chd"],
        version_args: &["--version"],
        capabilities: PCSX2_CAPS,
        bios: &[BiosRequirement {
            system: "PlayStation 2",
            name: "SCPH-10000.bin",
        }],
        build_args: pcsx2_args,
    },
    Adapter {
        id: "ppsspp",
        name: "PPSSPP",
        executable: "ppsspp",
        systems: &["PSP"],
        extensions: &["iso", "cso", "pbp"],
        version_args: &["--version"],
        capabilities: PPSSPP_CAPS,
        bios: &[],
        build_args: ppsspp_args,
    },
    Adapter {
        id: "mame",
        name: "MAME",
        executable: "mame",
        systems: &["Arcade"],
        extensions: &["zip"],
        version_args: &["-version"],
        capabilities: MAME_CAPS,
        bios: &[],
        build_args: mame_args,
    },
];

pub fn by_id(id: &str) -> Option<&'static Adapter> {
    ADAPTERS.iter().find(|adapter| adapter.id == id)
}

pub fn supporting_system<'a>(
    system: &str,
    available: &'a [super::EmulatorInstallation],
) -> Option<&'a super::EmulatorInstallation> {
    available
        .iter()
        .find(|install| install.available && install.adapter.systems.contains(&system))
}

#[cfg(test)]
mod tests {
    use super::{ADAPTERS, by_id};
    use std::collections::BTreeSet;

    #[test]
    fn adapters_have_unique_ids() {
        let mut ids: Vec<_> = ADAPTERS.iter().map(|adapter| adapter.id).collect();
        ids.sort_unstable();
        let unique: BTreeSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn lookup_by_id() {
        assert_eq!(by_id("retroarch").unwrap().name, "RetroArch");
        assert!(by_id("unknown").is_none());
    }

    #[test]
    fn arguments_are_a_plain_array_without_shell_metacharacters() {
        let adapter = by_id("retroarch").unwrap();
        let rom = std::path::Path::new("/roms/mario & luigi.sfc");
        let enabled = BTreeSet::from(["fullscreen".to_string()]);
        let args = (adapter.build_args)(rom, &enabled);
        let values: Vec<_> = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert_eq!(values[0], "--fullscreen");
        assert_eq!(values[1], "/roms/mario & luigi.sfc");
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn dolphin_uses_an_argument_array() {
        let adapter = by_id("dolphin").unwrap();
        let args = (adapter.build_args)(std::path::Path::new("/roms/game.iso"), &BTreeSet::new());
        let values: Vec<_> = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        assert_eq!(values, vec!["-e", "/roms/game.iso"]);
    }
}
