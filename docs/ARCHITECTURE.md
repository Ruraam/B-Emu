# B-Emu — Architecture

**Status:** initial architecture decision  
**Target:** Linux / Wayland  
**Core:** Rust  
**UI:** Slint  
**Rendering:** automatic capability-based selection

## 1. Scope and non-goals

B-Emu is a standalone Linux emulation frontend. Its job is to manage a game library and coordinate installed emulator applications. It is not itself an emulator and does not bundle copyrighted game ROMs, firmware, or BIOS files.

The initial release targets Linux desktops running Wayland.

Explicit non-goals:

- No X11 backend.
- No XWayland fallback.
- No Windows or macOS target in the initial architecture.
- No Kodi integration.
- No modifications to BubbleOS, BubbleTerm, or BubbleWM.
- No dependency on BubbleWM IPC, BubbleOS services, or compositor-private protocols for normal application operation.
- No privileged installation, system daemon, setuid helper, or root requirement.
- No shared theme/runtime repository requirement.
- No invented emulator settings: each integration exposes only settings supported by the detected emulator and version.

B-Emu may use BubbleOS artwork as a visual reference for its own palette. This is a design choice, not a technical dependency.

B-Emu is a standard Wayland client. BubbleWM is a first-class development and validation environment, but any compatible Wayland compositor must be able to host the same standalone application build. The rootless installation, XDG data locations, and ULPM `.lpk` release contract are defined in [DEPLOYMENT.md](DEPLOYMENT.md).

## 2. High-level components

```text
B-Emu
├── Application shell (Slint)
│   ├── Home / recently played
│   ├── Systems and collections
│   ├── Game library and details
│   ├── Emulator settings
│   ├── BIOS and firmware status
│   └── Preferences
│
├── Application core (Rust)
│   ├── Library service
│   ├── System registry
│   ├── Emulator registry
│   ├── Launch coordinator
│   ├── Configuration service
│   ├── Metadata service
│   ├── Input / controller service
│   └── Diagnostics and logging
│
├── Persistence
│   ├── SQLite database
│   └── User configuration and cache
│
└── Emulator adapters
    ├── RetroArch
    ├── Dolphin
    ├── PCSX2
    ├── PPSSPP
    ├── MAME
    └── Additional adapters added and tested individually
```

The UI must not launch emulator processes or manipulate database files directly. It calls typed Rust application services. Emulator-specific behavior stays inside adapters.

## 3. Display protocol: Wayland only

Use Slint's Winit backend with the Wayland-specific Cargo feature, not the generic Winit backend.

Implementation constraints:

- Enable the Wayland-specific backend feature only.
- Do not enable the X11 backend feature or the generic feature that bundles both X11 and Wayland.
- Do not enable Slint's Qt backend.
- Do not silently switch to X11 or XWayland.
- At startup, verify that a Wayland display connection can be established. If it cannot, show a clear diagnostic and exit.
- Keep renderer selection independent: changing the graphics renderer must never change the window-system backend.

The build configuration and dependency tree will be checked in CI to ensure X11 support has not accidentally entered the application through a default feature.

## 4. Automatic renderer strategy

Renderer selection is a graphics capability decision, not a display-protocol fallback.

Desired preference order:

1. Hardware-accelerated Vulkan through WGPU when supported and stable on the detected driver.
2. A compatible accelerated alternative where explicitly implemented and tested.
3. Software rendering as a compatibility fallback.

The exact Slint feature combination and runtime selection mechanism must be proven with a small Wayland-only prototype before being frozen in Cargo configuration. Do not assume that enabling multiple renderer features automatically provides a complete fallback chain.

At startup, record the selected renderer, adapter/device information when available, and any fallback reason. Provide a diagnostic mode to print this information.

Vello is not part of the initial default path because Slint documents it as experimental and not automatically selected.

## 5. Rust module boundaries

Proposed workspace layout:

```text
.
├── Cargo.toml
├── crates/
│   ├── b-emu-app/          # application entry point and lifecycle
│   ├── b-emu-core/         # domain types and orchestration
│   ├── b-emu-library/      # scanning, indexing, collections
│   ├── b-emu-emulators/    # emulator registry and adapters
│   ├── b-emu-config/       # typed settings and persistence
│   ├── b-emu-input/        # gamepad discovery and mapping
│   └── b-emu-platform/     # Wayland/runtime and process integration
├── ui/
│   ├── app.slint
│   ├── components/
│   └── screens/
├── assets/
│   ├── icons/
│   └── artwork/
├── docs/
└── tests/
```

This is a proposed boundary map, not a commitment to create every crate immediately. Start with the smallest compilable workspace and split crates when their APIs are stable.

## 6. Library and persistence

SQLite is the initial local index. It stores game records, detected systems, emulator associations, play history, and metadata references. The Phase 2 implementation and its safety rules are documented in [PHASE-2.md](PHASE-2.md).

ROM and disc image files remain in the user's chosen directories. B-Emu indexes them; it does not silently move or rewrite them.

Scanning must be:

- Read-only by default.
- Incremental and cancellable.
- Safe around removable drives and inaccessible paths.
- Able to identify duplicate files without deleting them.
- Explicit before any destructive operation.

Artwork and metadata are cached separately from original game files.

## 7. Emulator adapter contract

Every emulator adapter should report:

- Emulator identity and detected version.
- Installation path and availability.
- Supported systems and file extensions.
- Launch command construction using argument arrays (never shell-concatenated commands).
- Supported configuration capabilities.
- BIOS/firmware requirements when known.
- Exit status and diagnostic output.

Settings are capability-driven. An adapter must not expose a setting unless it can verify that the installed emulator version supports it. Unknown settings remain hidden rather than guessed.

The first adapters should be selected based on availability and testability on Debian-based Linux, then expanded.

## 8. Controller and mouse input

The interface is controller-first but remains fully usable with a mouse and keyboard.

The input service owns controller discovery and hotplug through GilRs, button/axis normalization, and controller disconnect handling. Polling runs on a dedicated thread and reaches Slint only through `Weak::upgrade_in_event_loop`; the UI thread is never blocked by evdev polling. The worker is stopped and joined when the main window exits.

The current shell maps vertical navigation, confirm, and back-to-home. Horizontal actions are normalized for later screens but do not change the vertical sidebar. Analog navigation uses activation and return thresholds to avoid jitter.

Access remains rootless and follows the active Linux seat/session ACL policy. B-Emu never changes `/dev/input` permissions, adds the user to the `input` group, or requests root. If access is unavailable, the gamepad worker reports it and keyboard/mouse input remains available. See [GAMEPAD.md](GAMEPAD.md) for the mapping and runtime details.

## 9. Visual direction

B-Emu has its own visual implementation. It may reuse the colors sampled from BubbleOS's SVG artwork:

- Deep backgrounds: `#020617`, `#0A1530`, `#14123A`
- Cyan: `#22D3EE`, `#67E8F9`
- Blue: `#38BDF8`, `#60A5FA`
- Indigo: `#818CF8`
- Text: `#E0F2FE`

Use restrained glass surfaces, readable typography, clear focus states, and motion that can be reduced or disabled. Game artwork must remain visually dominant; neon effects must not compromise readability.

## 10. Delivery phases

### Phase 0 — Architecture proof

- Create a minimal Slint + Rust application.
- Compile with Wayland-only Winit support.
- Prove that it starts in a Wayland session.
- Prove that it refuses to start without Wayland.
- Inspect the dependency/build feature graph for X11 support.
- Test renderer detection and fallback on real hardware and software rendering.

### Phase 1 — Application shell

- Main window and navigation.
- Home, systems, library, details, and settings screens.
- Keyboard, mouse, and controller focus model.
- Theme tokens and accessibility/reduced-motion behavior.

### Phase 2 — Library

- Configurable game directories.
- Safe scanner and SQLite index.
- File type/system detection.
- Collections, favorites, recent games, and search.

### Phase 3 — Emulator management

- Emulator discovery.
- Adapter contract and first integrations.
- Launch coordinator and process supervision.
- Per-emulator, per-version settings.

### Phase 4 — Metadata and firmware

- Optional metadata providers.
- Artwork cache.
- BIOS/firmware inventory and validation.
- Clear provenance and missing-file diagnostics.

### Phase 5 — Hardening

- Automated tests and CI.
- Wayland-only build verification.
- Crash-safe database updates.
- Security review of process launch and file handling.
- Build, sign, install, update, roll back, and uninstall architecture-specific ULPM `.lpk` packages without root.
- Validate the same package on BubbleWM and at least one independent Wayland compositor.

## 11. Architecture acceptance criteria

The architecture is not considered validated until:

1. The application builds on Linux with only the Wayland Winit backend enabled.
2. The resulting dependency/build configuration contains no X11 backend.
3. The application runs inside a Wayland session.
4. The application refuses to launch when no Wayland display is available.
5. Renderer selection and fallback are observable in logs.
6. The UI can be navigated without a mouse.
7. A fake/test emulator adapter can be launched safely without shell command concatenation.

## 12. BubbleWM and deployment independence

- B-Emu communicates with the compositor through standard Wayland client protocols provided by its Wayland backend.
- Do not add a dependency on the BubbleWM workspace IPC crate for window creation, input, focus, or rendering.
- BubbleWM may be used for nested development and integration tests, but it is not required at build time or runtime.
- All mutable application state belongs in user-owned XDG directories; the executable and packaged payload must be treated as read-only.
- ULPM packaging is a release target, not a dependency of the development binary. The exact package manifest and permission declarations must be validated against the target ULPM implementation before release.
