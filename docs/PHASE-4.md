# Phase 4 — Metadata, artwork and firmware

Phase 4 adds local enrichment with explicit provenance. No network provider is
enabled: every value says where it came from.

## Metadata

The `metadata` table stores title, description, year, region, provider and
fetch time, keyed by game path. The local `filename` provider parses region and
year tags such as `(USA) (1995)` from ROM filenames. Records are created only
when missing, so a future provider can replace them without losing provenance.

## Artwork cache

Sidecar images next to a ROM (`game.png`, `.jpg`, `.jpeg`, `.webp`) are copied
into `$XDG_CACHE_HOME/b-emu/artwork/` under a content hash. The `artwork` table
records the source (`sidecar`) and cached path. Missing artwork is reported as
absent, never generated. The Library detail panel displays cached artwork when
available.

## BIOS / firmware

Declared `BiosRequirement` entries from all adapters are checked against the
configured BIOS directory (default: `$XDG_DATA_HOME/b-emu/bios`). Each status
records presence and SHA-256 checksum. Missing files are displayed explicitly
with a red marker in the Emulators detail panel. B-Emu never downloads or
modifies BIOS files.

## Configuration additions

```json
{
  "game_dirs": ["/home/user/ROMs"],
  "bios_dir": "/home/user/Bios",
  "emulator_executables": { "retroarch": "/opt/bin/retroarch" },
  "emulator_settings": { "retroarch": { "fullscreen": true } }
}
```

`bios_dir` and `emulator_executables` are optional. The latter overrides PATH
discovery for custom builds.

## Validation

Unit tests cover filename tag parsing, sidecar copying into the cache, missing
artwork, present/missing BIOS detection, and opt-in capability settings. The
headless smoke test verifies that metadata and artwork rows are created for a
configured ROM directory.
