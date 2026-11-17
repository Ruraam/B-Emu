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
    adapter!(
        "abuse",
        "Abuse",
        &["abuse"],
        &["Abuse"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "amiberry",
        "Amiberry",
        &["amiberry"],
        &["Amiga1200", "Amiga500", "Amigacd32", "Amigacdtv"],
        &[
            "adf", "adz", "bin", "chd", "cue", "dms", "dmz", "exe", "hdf", "ipf", "iso", "lha"
        ],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "azahar",
        "Azahar",
        &["azahar"],
        &["Nintendo 3DS"],
        &[
            "3ds", "app", "axf", "cci", "cia", "cxi", "elf", "squashfs", "zcci", "zcia", "zcxi"
        ],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "bigpemu",
        "Bigpemu",
        &["bigpemu"],
        &["Jaguar", "Jaguarcd"],
        &[
            "7z", "abs", "bigpimg", "cdi", "cof", "cue", "j64", "jag", "rom", "zip"
        ],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "bstone",
        "Bstone",
        &["bstone"],
        &["Bstone"],
        &["bstone"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "cannonball",
        "Cannonball",
        &["cannonball"],
        &["Cannonball"],
        &["cannonball"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "catacombgl",
        "Catacombgl",
        &["CatacombGL"],
        &["Catacomb"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "cdogs",
        "Cdogs",
        &["cdogs"],
        &["Cdogs"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "cemu",
        "Cemu",
        &["cemu"],
        &["Wii U"],
        &["rpx", "squashfs", "wua", "wud", "wuhb", "wup", "wux"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "cgenius",
        "Cgenius",
        &["CGeniusExe"],
        &["Cgenius"],
        &["cgenius"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "clk",
        "Clk",
        &["clk"],
        &[
            "Amstradcpc",
            "Archimedes",
            "Atarist",
            "Colecovision",
            "Electron",
            "Enterprise",
            "Macintosh",
            "Master System",
            "MSX",
            "Oricatmos",
            "ZX Spectrum"
        ],
        &[
            "128", "1dd", "2mg", "360", "7z", "adf", "adl", "adm", "ads", "apd", "bas", "bbc"
        ],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "corsixth",
        "Corsixth",
        &["corsix-th"],
        &["Corsixth"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "demul",
        "Demul",
        &["demul"],
        &["Cave3Rd", "Gaelco", "Hikaru"],
        &["7z", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "devilutionx",
        "Devilutionx",
        &["devilutionx"],
        &["Devilutionx"],
        &["mpq"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "dhewm3",
        "Dhewm3",
        &["dhewm3"],
        &["Doom3"],
        &["d3"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "dosbox_staging",
        "Dosbox Staging",
        &["dosbox-staging"],
        &["Dos"],
        &["cue", "dos", "dosz", "iso", "m3u", "pc", "squashfs", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "dosboxx",
        "Dosboxx",
        &["dosbox-x"],
        &["Dos"],
        &["cue", "dos", "dosz", "iso", "m3u", "pc", "squashfs", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "dxx-rebirth",
        "Dxx Rebirth",
        &["d1x-rebirth", "d2x-rebirth"],
        &["Dxx Rebirth"],
        &["d1x", "d2x"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "easyrpg",
        "Easyrpg",
        &["easyrpg-player"],
        &["Easyrpg"],
        &["easyrpg", "squashfs", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "ecwolf",
        "Ecwolf",
        &["ecwolf"],
        &["Ecwolf"],
        &["ecwolf", "pk3", "squashfs"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "eduke32",
        "Eduke32",
        &["eduke32"],
        &["Eduke32", "Fury"],
        &["eduke32", "grp"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "etlegacy",
        "Etlegacy",
        &["etlegacy"],
        &["Etlegacy"],
        &["etl"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "fallout1-ce",
        "Fallout1 Ce",
        &["fallout1ce"],
        &["Fallout1 Ce"],
        &["f1ce"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "fallout2-ce",
        "Fallout2 Ce",
        &["fallout2ce"],
        &["Fallout2 Ce"],
        &["f2ce"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "fsuae",
        "Fsuae",
        &["fs-uae"],
        &["Amiga1200", "Amiga500", "Amigacd32", "Amigacdtv"],
        &[
            "adf", "adz", "bin", "chd", "cue", "dms", "dmz", "exe", "hdf", "ipf", "iso", "lha"
        ],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "gsplus",
        "Gsplus",
        &["GSplus"],
        &["Apple2", "Apple2Gs"],
        &[
            "2mg", "7z", "chd", "dfi", "do", "dsk", "edd", "hdv", "mfi", "nib", "po", "rti"
        ],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "gzdoom",
        "Gzdoom",
        &["gzdoom"],
        &["Gzdoom"],
        &["gzdoom", "iwad", "pwad", "wad"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "hcl",
        "Hcl",
        &["hcl"],
        &["Hcl"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "hurrican",
        "Hurrican",
        &["hurrican"],
        &["Hurrican"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "hypseus-singe",
        "Hypseus Singe",
        &["hypseussinge"],
        &["Daphne", "Singe"],
        &["daphne", "squashfs"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "ikemen",
        "Ikemen",
        &["ikemen"],
        &["Ikemen"],
        &["ikemen", "pc"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "iortcw",
        "Iortcw",
        &["iortcw"],
        &["Rtcw"],
        &["rtcw"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "jazz2-native",
        "Jazz2 Native",
        &["jazz2"],
        &["Jazz2"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "lightspark",
        "Lightspark",
        &["lightspark"],
        &["Flash"],
        &["swf"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "lindbergh-loader",
        "Lindbergh Loader",
        &["lindbergh"],
        &["Lindbergh"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "model2emu",
        "Model2Emu",
        &["model2emu"],
        &["Model2"],
        &["zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "mugen",
        "Mugen",
        &["mugen"],
        &["Mugen"],
        &["pc"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "mupen64plus",
        "Mupen64Plus",
        &["mupen64plus"],
        &["Nintendo 64"],
        &["7z", "n64", "ndd", "v64", "z64", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "openbor",
        "Openbor",
        &["OpenBOR7530"],
        &["Openbor"],
        &["pak"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "openjazz",
        "Openjazz",
        &["OpenJazz"],
        &["Openjazz"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "openjk",
        "Openjk",
        &["openjk"],
        &["Jknight"],
        &["jedi"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "openjkdf2",
        "Openjkdf2",
        &["openjkdf2"],
        &["Jkdf2"],
        &["jedi"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "openmohaa",
        "Openmohaa",
        &["openmohaa"],
        &["Mohaa"],
        &["mohaa"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "pygame",
        "Pygame",
        &["pygame"],
        &["Pygame"],
        &["pygame"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "pyxel",
        "Pyxel",
        &["pyxel"],
        &["Pyxel"],
        &["py", "pyxapp"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "raze",
        "Raze",
        &["raze"],
        &["Raze"],
        &["raze"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "ruffle",
        "Ruffle",
        &["ruffle"],
        &["Flash"],
        &["swf"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "samcoupe",
        "Samcoupe",
        &["simcoupe"],
        &["Samcoupe"],
        &["cpm", "dsk", "mgt", "sad", "sbt", "sdf", "td0", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "shadps4",
        "Shadps4",
        &["shadps4"],
        &["Ps4"],
        &["ps4"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "solarus",
        "Solarus",
        &["solarus-run"],
        &["Solarus"],
        &["solarus", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "sonic-mania",
        "Sonic Mania",
        &["sonicmania"],
        &["Sonic Mania"],
        &["sman"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "sonic2013",
        "Sonic2013",
        &["sonic2013"],
        &["Sonicretro"],
        &["scd", "son"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "sonic3-air",
        "Sonic3 Air",
        &["sonic3air"],
        &["Sonic3 Air"],
        &["s3air"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "soniccd",
        "Soniccd",
        &["soniccd"],
        &["Sonicretro"],
        &["scd", "son"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "supermodel",
        "Supermodel",
        &["supermodel"],
        &["Model3"],
        &["zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "taradino",
        "Taradino",
        &["taradino"],
        &["Rott"],
        &["rott"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "theforceengine",
        "Theforceengine",
        &["theforceengine"],
        &["Theforceengine"],
        &["tfe"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "thextech",
        "Thextech",
        &["thextech"],
        &["Thextech"],
        &["smbx", "squashfs"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "tr1x",
        "Tr1X",
        &["tr1x"],
        &["Traider1"],
        &["croft"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "tr2x",
        "Tr2X",
        &["tr2x"],
        &["Traider2"],
        &["croft"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "tsugaru",
        "Tsugaru",
        &["Tsugaru_CUI"],
        &["Fmtowns"],
        &[
            "1dd", "7z", "bin", "cdr", "chd", "cqi", "cqm", "cue", "d77", "d88", "dfi", "dsk"
        ],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "tyrian",
        "Tyrian",
        &["opentyrian"],
        &["Tyrian"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "vita3k",
        "Vita3K",
        &["vita3k"],
        &["Psvita"],
        &["psvita", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "vkquake",
        "Vkquake",
        &["vkquake"],
        &["Quake"],
        &["quake"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "vkquake2",
        "Vkquake2",
        &["vkquake2"],
        &["Quake2"],
        &["7zip", "quake2", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "vkquake3",
        "Vkquake3",
        &["vkquake3"],
        &["Quake3"],
        &["quake3"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "vpinball",
        "Vpinball",
        &["vpinball"],
        &["Vpinball"],
        &["vpx"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "x16emu",
        "X16Emu",
        &["x16emu"],
        &["Commanderx16"],
        &["bas", "img", "prg"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "xash3d_fwgs",
        "Xash3D Fwgs",
        &["xash3dfwgs"],
        &["Halflife"],
        &["game"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "xemu",
        "Xemu",
        &["xemu"],
        &["Chihiro", "Xbox"],
        &["iso", "squashfs"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "xenia",
        "Xenia",
        &["xenia"],
        &["Xbox360"],
        &["iso", "xbox360", "xex", "zar"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "xenia-canary",
        "Xenia Canary",
        &["xeniacanary"],
        &["Xbox360"],
        &["iso", "xbox360", "xex", "zar"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "xroar",
        "Xroar",
        &["xroar"],
        &["Coco", "Dragon64", "Mc10"],
        &["7z", "bin", "cas", "ccc", "dmk", "dsk", "rom", "wav", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "ymir",
        "Ymir",
        &["ymir"],
        &["Saturn"],
        &["ccd", "chd", "cue", "iso", "m3u", "mds", "zip"],
        V,
        NO_CAPS,
        NO_BIOS,
        rom_only
    ),
    adapter!(
        "yquake2",
        "Yquake2",
        &["yquake2"],
        &["Quake2"],
        &["7zip", "quake2", "zip"],
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
        assert!(ADAPTERS.len() >= 100);
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
