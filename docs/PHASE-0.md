# Phase 0 — Wayland prototype

This phase validates the platform boundary before the full frontend is built.

## Current implementation

- Rust application using Slint 1.18.
- Slint Winit backend compiled with `backend-winit-wayland` only.
- Slint default features are disabled. The required `compat-1-18` feature is enabled explicitly.
- No X11 backend feature, Qt backend, XWayland fallback, or LinuxKMS backend is enabled.
- Startup refuses to continue when `WAYLAND_DISPLAY` is absent. If the Wayland socket cannot be reached, backend initialization must fail and the application exits with an error.
- The initial renderer set is FemtoVG plus the software renderer.

## Renderer behavior

The initial automatic selection is intentionally conservative:

1. FemtoVG is the preferred renderer and uses OpenGL.
2. Slint can fall back to its software renderer if the GPU renderer cannot initialize.

WGPU/Vulkan and Skia are not enabled in this first prototype. They add separate dependency and driver considerations and will only be introduced after a tested comparison. The display protocol is never part of renderer fallback: the application remains Wayland-only.

Slint documents the Winit backend's renderer selection and its fallback behavior in the [Winit backend guide](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/).

## CI checks

The `.github/workflows/phase-0.yml` workflow:

- checks Rust formatting;
- inspects Cargo's resolved feature graph and rejects known X11 backend features;
- compiles all Rust targets on Ubuntu with Wayland/OpenGL development libraries;
- runs all unit tests with `cargo test --all-targets`, including deterministic XDG path tests and Unix Wayland socket path tests.

The feature audit is a guard, not a substitute for a runtime test on a real Wayland compositor.

## Manual acceptance tests

Run these on a Linux machine:

1. In a Wayland session, launch `cargo run`: the prototype window should appear.
2. In an X11-only session, unset `WAYLAND_DISPLAY` and launch the binary: it must exit with the explicit Wayland requirement message.
3. In an X11 session with `WAYLAND_DISPLAY` unset, setting `DISPLAY` must not make the application start.
4. In a Wayland session with GPU rendering unavailable, verify whether Slint successfully selects the software renderer. Record the actual result and logs.
5. Inspect `cargo tree -e features` and confirm no X11 backend feature is present.

A green compile does not prove runtime behavior. The runtime acceptance tests remain open until executed on real sessions.
