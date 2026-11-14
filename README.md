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

Phase 0 prototype is in progress on the `phase-0-wayland-prototype` branch. CI is being used to validate the Wayland-only dependency graph and Linux build before the full interface is developed.
