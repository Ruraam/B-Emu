//! Declarative emulator adapter registry.
//!
//! Adding an emulator is one `adapter!` entry: names, executable aliases,
//! systems, extensions, and an argument builder. Arguments are always returned
//! as an `OsString` array; no command string is ever concatenated or passed to
//! a shell.

use std::{collections::BTreeSet, ffi::OsString, path::Path};

/// A setting that is only exposed when the adapter declares support for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    pub key: &'static str,
    pub label: &'static str,
    pub flag: &'static str,
}

/// Known firmware requirement. Presence validation lives in `src/bios.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiosRequirement {
    pub system: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Adapter {
    pub id: &'static str,
    pub name: &'static str,
    pub executables: &'static [&'static str],
    pub systems: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub version_args: &'static [&'static str],
    pub capabilities: &'static [Capability],
    pub bios: &'static [BiosRequirement],
    pub build_args: fn(&Path, &BTreeSet<String>) -> Vec<OsString>,
}

const CAP_FULLSCREEN: &[Capability] = &[Capability {
    key: "fullscreen",
    label: "Plein écran",
    flag: "--fullscreen",
}];
const CAP_WINDOWED: &[Capability] = &[Capability {
    key: "windowed",
    label: "Fenêtré",
    flag: "-window",
}];
const CAP_RETROARCH: &[Capability] = &[
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

fn rom_only(rom: &Path, _enabled: &BTreeSet<String>) -> Vec<OsString> {
    vec![rom.as_os_str().to_owned()]
}

macro_rules! caps_builder {
    ($name:ident, $caps:expr) => {
        fn $name(rom: &Path, enabled: &BTreeSet<String>) -> Vec<OsString> {
            let mut args: Vec<OsString> = Vec::new();
            for capability in $caps {
                if enabled.contains(capability.key) {
                    args.push(OsString::from(capability.flag));
                }
            }
            args.push(rom.as_os_str().to_owned());
            args
        }
    };
}

caps_builder!(fullscreen_args, CAP_FULLSCREEN);
caps_builder!(windowed_args, CAP_WINDOWED);
caps_builder!(retroarch_args_fn, CAP_RETROARCH);

fn dolphin_args(rom: &Path, enabled: &BTreeSet<String>) -> Vec<OsString> {
    let mut args = fullscreen_args(rom, enabled);
    args.insert(args.len() - 1, OsString::from("-e"));
    args
}

macro_rules! adapter {
    ($id:expr, $name:expr, $exes:expr, $systems:expr, $exts:expr,
     $version:expr, $caps:expr, $bios:expr, $build:expr) => {
        Adapter {
            id: $id,
            name: $name,
            executables: $exes,
            systems: $systems,
            extensions: $exts,
            version_args: $version,
            capabilities: $caps,
            bios: $bios,
            build_args: $build,
        }
    };
}

const V: &[&str] = &["--version"];
const V_SHORT: &[&str] = &["-version"];
const NO_CAPS: &[Capability] = &[];
const NO_BIOS: &[BiosRequirement] = &[];

/// Broad registry. Discovery simply checks PATH for these executable aliases,
/// so adding an emulator here is enough for it to appear in the interface.
pub const ADAPTERS: &[Adapter] = &[
    adapter!(
        "retroarch",
        "RetroArch",
        &["retroarch"],
        &[
            "NES",
            "Super Nintendo",
            "Mega Drive",
            "Nintendo 64",
            "Game Boy",
            "Game Boy Advance",
            "PlayStation",
            "Arcade"
        ],
        &[
            "nes", "sfc", "smc", "md", "gen", "z64", "v64", "n64", "gb", "gbc", "gba", "cue",
            "chd", "zip"
        ],
        V,
        CAP_RETROARCH,
        NO_BIOS,
        retroarch_args_fn
    ),
    adapter!(
        "dolphin",
        "Dolphin",
        &["dolphin-emu", "dolphin"],
        &["GameCube", "Wii"],
        &["iso", "gcm", "gcn", "wbfs"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        dolphin_args
    ),
    adapter!(
        "pcsx2",
        "PCSX2",
        &["PCSX2", "pcsx2"],
        &["PlayStation 2"],
        &["iso", "cue", "chd", "bin"],
        V,
        CAP_FULLSCREEN,
        &[BiosRequirement {
            system: "PlayStation 2",
            name: "SCPH-10000.bin"
        }],
        fullscreen_args
    ),
    adapter!(
        "ppsspp",
        "PPSSPP",
        &["ppsspp", "PPSSPP"],
        &["PSP"],
        &["iso", "cso", "pbp"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "mame",
        "MAME",
        &["mame", "mame64"],
        &["Arcade"],
        &["zip"],
        V_SHORT,
        CAP_WINDOWED,
        NO_BIOS,
        windowed_args
    ),
    adapter!(
        "duckstation",
        "DuckStation",
        &["duckstation"],
        &["PlayStation"],
        &["cue", "chd", "bin", "pbp"],
        V,
        CAP_FULLSCREEN,
        &[BiosRequirement {
            system: "PlayStation",
            name: "scph5501.bin"
        }],
        fullscreen_args
    ),
    adapter!(
        "mednafen",
        "Mednafen",
        &["mednafen"],
        &[
            "NES",
            "Super Nintendo",
            "PlayStation",
            "Neo Geo Pocket",
            "WonderSwan"
        ],
        &["nes", "sfc", "cue", "ngp", "ws"],
        V_SHORT,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "bsnes",
        "bsnes",
        &["bsnes"],
        &["Super Nintendo"],
        &["sfc", "smc"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "snes9x",
        "Snes9x",
        &["snes9x", "snes9x-gtk"],
        &["Super Nintendo"],
        &["sfc", "smc"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "mesen",
        "Mesen",
        &["mesen", "Mesen"],
        &["NES", "Super Nintendo", "Game Boy"],
        &["nes", "sfc", "gb", "gbc"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "fceux",
        "FCEUX",
        &["fceux"],
        &["NES"],
        &["nes"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "nestopia",
        "Nestopia",
        &["nestopia"],
        &["NES"],
        &["nes"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "vbam",
        "VisualBoyAdvance",
        &["vbam", "visualboyadvance"],
        &["Game Boy", "Game Boy Advance"],
        &["gb", "gbc", "gba"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "mgba",
        "mGBA",
        &["mgba-qt", "mgba"],
        &["Game Boy", "Game Boy Advance"],
        &["gb", "gbc", "gba"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "sameboy",
        "SameBoy",
        &["sameboy"],
        &["Game Boy"],
        &["gb", "gbc"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "gambatte",
        "Gambatte",
        &["gambatte"],
        &["Game Boy"],
        &["gb", "gbc"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "yabause",
        "Yabause",
        &["yabause"],
        &["Saturn"],
        &["cue", "bin", "iso"],
        V,
        CAP_FULLSCREEN,
        &[BiosRequirement {
            system: "Saturn",
            name: "saturn_bios.bin"
        }],
        fullscreen_args
    ),
    adapter!(
        "redream",
        "Redream",
        &["redream"],
        &["Dreamcast"],
        &["cue", "bin", "chd", "gdi"],
        V,
        CAP_FULLSCREEN,
        &[BiosRequirement {
            system: "Dreamcast",
            name: "dc_boot.bin"
        }],
        fullscreen_args
    ),
    adapter!(
        "flycast",
        "Flycast",
        &["flycast"],
        &["Dreamcast", "Naomi"],
        &["cue", "bin", "chd", "gdi"],
        V,
        CAP_FULLSCREEN,
        &[BiosRequirement {
            system: "Dreamcast",
            name: "dc_boot.bin"
        }],
        fullscreen_args
    ),
    adapter!(
        "citra",
        "Citra",
        &["citra"],
        &["Nintendo 3DS"],
        &["3ds", "cxi", "cia"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "yuzu",
        "Yuzu",
        &["yuzu"],
        &["Nintendo Switch"],
        &["nsp", "xci"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "rpcs3",
        "RPCS3",
        &["rpcs3"],
        &["PlayStation 3"],
        &["ps3", "pkg"],
        V,
        CAP_FULLSCREEN,
        &[BiosRequirement {
            system: "PlayStation 3",
            name: "PS3UPDAT.PUP"
        }],
        fullscreen_args
    ),
    adapter!(
        "melonds",
        "melonDS",
        &["melonds", "melonDS"],
        &["Nintendo DS"],
        &["nds", "dsi"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "desmume",
        "DeSmuME",
        &["desmume"],
        &["Nintendo DS"],
        &["nds"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "dosbox",
        "DOSBox",
        &["dosbox"],
        &["DOS"],
        &["zip", "exe"],
        V,
        CAP_WINDOWED,
        NO_BIOS,
        windowed_args
    ),
    adapter!(
        "scummvm",
        "ScummVM",
        &["scummvm"],
        &["PC"],
        &["zip"],
        V,
        CAP_FULLSCREEN,
        NO_BIOS,
        fullscreen_args
    ),
    adapter!(
        "vice",
        "VICE",
        &["x64sc", "vice"],
        &["Commodore 64"],
        &["d64", "prg", "zip"],
        V_SHORT,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "hatari",
        "Hatari",
        &["hatari"],
        &["Atari ST"],
        &["st", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "fuse",
        "Fuse",
        &["fuse"],
        &["ZX Spectrum"],
        &["tap", "z80", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "openmsx",
        "openMSX",
        &["openmsx"],
        &["MSX"],
        &["rom", "dsk", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "ares",
        "ares",
        &["ares"],
        &[
            "NES",
            "Super Nintendo",
            "Nintendo 64",
            "PlayStation",
            "Arcade"
        ],
        &["nes", "sfc", "z64", "cue", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
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
    fn registry_is_large_and_has_unique_ids() {
        assert!(ADAPTERS.len() >= 30);
        let mut ids: Vec<_> = ADAPTERS.iter().map(|adapter| adapter.id).collect();
        ids.sort_unstable();
        let unique: BTreeSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn every_adapter_declares_an_executable() {
        assert!(
            ADAPTERS
                .iter()
                .all(|adapter| !adapter.executables.is_empty())
        );
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
        assert_eq!(values, vec!["--fullscreen", "/roms/mario & luigi.sfc"]);
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
