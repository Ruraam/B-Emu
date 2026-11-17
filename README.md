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

Phase 0 validated the Wayland-only build and startup policy. Phase 1 is in
progress on the `phase-1-*` branches: the shell now has five data-driven screens
(Home, Systems, Library, Emulators, Settings) with a unified keyboard, mouse and
gamepad focus model. The screens are seeded from typed Rust view models in
`src/shell.rs`; real library scanning and emulator detection arrive in Phases 2
and 3.

CI validates formatting, the Wayland-only dependency graph, the Linux build,
unit tests, and a headless Weston startup smoke test. See [docs/GAMEPAD.md](docs/GAMEPAD.md)
for the input model and [docs/PHASE-0.md](docs/PHASE-0.md) for the renderer and
Wayland policy.

Deployment requirements and the planned rootless ULPM `.lpk` distribution model are documented in [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).
