# Phase 3 — Emulator management

Phase 3 discovers installed emulators, builds launch commands safely, and
supervises the resulting processes. B-Emu remains a frontend: no emulator or
BIOS file is bundled.

## Adapter contract

Each adapter in `src/emulators/adapters.rs` declares:

- stable id and display name;
- executable name and version arguments;
- supported systems and file extensions;
- capabilities (settings that are actually supported by the emulator);
- known BIOS/firmware requirements;
- a function that builds the launch argument array.

Capabilities are opt-in: a setting is only written to the config and passed to
the emulator when the adapter declares it. Unknown settings are never invented
or shown. Capability values are stored per emulator id under
`emulator_settings` in `config.json`.

## Discovery

`discovery.rs` searches `PATH` for declared executable aliases and probes their
version with a bounded two-second timeout. The registry covers over a hundred
emulators, from the curated core (RetroArch, Dolphin, PCSX2, PPSSPP, MAME,
DuckStation, Mednafen, …) to a broad set of standalone and port systems
(Azahar, BigPEmu, Cemu, CENgine, Citron, Demul, Flycast, FSUAE, GSplus,
Mupen64Plus, OpenBOR, Redream, Ryujinx, ShadPS4, Supermodel, Tsugaru, Vita3K,
Xenia, Ymir, …).

The registry is self-contained: B-Emu does not depend on, bundle or execute any
external distribution's components. No capabilities are fabricated for the
broad set of adapters: they expose no settings until real per-version
capabilities are verified.

Adding another emulator is one `adapter!` entry in `adapters.rs`: id, display
name, executable aliases, systems, extensions, version arguments, declared
capabilities, BIOS requirements and an argument builder. Users can override an
executable per emulator id through `emulator_executables` in `config.json` for
custom builds. A missing or non-executable binary is simply not reported, and
discovery uses an injectable probe in tests so no real emulator is started.

## Safe launching

`launch.rs` builds a `Command` from an `OsString` array. There is no shell, no
command string concatenation, and no argument interpolation. Paths containing
spaces or shell metacharacters remain a single argument. A fake adapter is
tested end-to-end (spawn, capture output, non-zero exit status, missing
executable).

`launch_worker.rs` spawns the process off the UI thread, records the game as
played, then waits for the child and logs its exit status and diagnostics.
Play history therefore becomes automatic when a game is launched.

## Interface

- The **Emulators** screen is backed by discovery. Its detail panel lists only
  declared capabilities and allows toggling them with a click or with the
  controller (navigate to the setting and press A).
- The **Library** detail panel can launch a game with the first ready adapter
  that declares support for its system. If none is available, the status line
  says so instead of attempting a fallback command.
- BIOS requirements are displayed when declared; presence validation belongs
  to Phase 4.

## Validation

Unit tests cover unique adapter ids, argument-array construction with
metacharacters in paths, fake-adapter spawning and exit status, version
parsing, PATH discovery with an injected probe, and opt-in capability settings.
The headless Weston smoke test still verifies startup with a configured library.
