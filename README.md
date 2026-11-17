# B-Emu

**B-Emu** is a native Linux gaming and emulation frontend, built around Rust and Slint.

It aims to provide a modern, controller-first library for discovering, organizing, configuring, and launching emulators and games — from classic consoles to modern systems.

## Project principles

- **Linux only** for the initial target.
- **Wayland native only**: X11 and XWayland are not supported and must not be compiled as fallback backends.
- **Rust core** with a **Slint** user interface.
- **Automatic graphics renderer selection**, independently from the display protocol.
- **Controller and mouse navigation** throughout the application.
- Emulator integrations must use real capabilities and settings exposed by each emulator; no fabricated universal settings.
- B-Emu remains an independent project. It borrows the visual palette and visual inspiration of the BubbleOS SVG artwork only; it does not modify, depend on, or share runtime components with BubbleOS, BubbleTerm, or BubbleWM.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the architecture and [docs/PHASE-0.md](docs/PHASE-0.md) for the Wayland prototype, renderer policy, and acceptance tests.

## Status

Phase 0 validated the Wayland-only build and startup policy. Phase 1 delivered
five data-driven screens (Home, Systems, Library, Emulators, Settings) with a
unified keyboard, mouse and gamepad focus model. Phase 2 adds configurable game
directories, a read-only SHA-256 scanner and a local SQLite index that backs the
Home, Systems and Library screens. Phase 3 adds emulator discovery on `PATH`,
capability-driven settings, and supervised launching built from `OsString`
argument arrays (never shell command strings). The registry covers over a
hundred emulators and accepts per-emulator executable overrides. Phase 4 adds local
metadata with provenance, a sidecar artwork cache and BIOS/firmware validation.
Phase 5 persists preferences, applies scans transactionally, validates launch
paths before starting an emulator, and produces a signed rootless ULPM `.lpk`.

To try the shell with a non-empty library, run `bash scripts/prepare-demo.sh`:
it creates isolated XDG directories with a few dummy games and prints the
environment variables and command to launch B-Emu.

Release artifacts are built with `bash scripts/release-lpk.sh` (signed `.lpk`
plus a minimal signed repository); `bash scripts/smoke-wayland.sh` runs the
headless startup test for the binary or the package, with a parametrizable
compositor command.

CI validates formatting, the Wayland-only dependency graph, the Linux build,
unit tests, and a headless Weston startup smoke test. See
[docs/PHASE-5.md](docs/PHASE-5.md) for hardening,
[docs/PHASE-4.md](docs/PHASE-4.md) for metadata and firmware,
[docs/PHASE-3.md](docs/PHASE-3.md) for the emulator contract,
[docs/PHASE-2.md](docs/PHASE-2.md) for the library design,
[docs/GAMEPAD.md](docs/GAMEPAD.md) for the input model, and
[docs/PHASE-0.md](docs/PHASE-0.md) for the renderer and Wayland policy.

Deployment requirements and the planned rootless ULPM `.lpk` distribution model are documented in [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).
