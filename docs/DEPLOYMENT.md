# B-Emu — Deployment and portability contract

**Status:** initial deployment decision  
**Target:** Linux / native Wayland  
**Installation model:** per-user, no root  
**Distribution target:** ULPM `.lpk`

## 1. Core deployment rules

B-Emu is a standalone user application. Its normal build, installation, update, execution and removal must not require root privileges.

The application must not:
- write to `/usr`, `/etc`, `/opt`, `/var` or other system-owned locations;
- require a system service, daemon, privileged helper or setuid binary;
- change system-wide environment variables, device permissions or compositor configuration;
- assume that BubbleOS or BubbleWM is installed;
- require a writable application installation directory at runtime.

The application must remain usable from a read-only application bundle.

## 2. User data locations

Use the XDG Base Directory specification, with safe defaults when variables are absent:

| Purpose | Location |
|---|---|
| Configuration | `$XDG_CONFIG_HOME/b-emu` or `~/.config/b-emu` |
| Persistent data / database | `$XDG_DATA_HOME/b-emu` or `~/.local/share/b-emu` |
| Cache / artwork | `$XDG_CACHE_HOME/b-emu` or `~/.cache/b-emu` |
| Runtime-only files | `$XDG_RUNTIME_DIR/b-emu` when available and safe |

Never store mutable data beside the executable or inside the mounted/read-only `.lpk` payload.

All directory creation must be limited to the current user's writable locations. Fail with an actionable diagnostic if a required location cannot be created or accessed.

## 3. Wayland and compositor portability

B-Emu is a normal Wayland client. It must use standard Wayland protocols and must not depend on:
- BubbleWM internals or its IPC protocol;
- BubbleOS-specific services, paths, themes or environment variables;
- compositor-specific private protocols for ordinary window display or input.

BubbleWM is a first-class test environment, not a runtime dependency. The same build must be tested on at least one independent Wayland compositor before release.

No X11 or XWayland backend or fallback is permitted. Missing, invalid or inaccessible Wayland display connections must produce a clear error and a non-zero exit status.

## 4. Runtime dependencies and host integration

The application bundle may contain application-owned user-space libraries, but it must not bundle or replace host kernel drivers, DRM devices, GPU firmware, compositor components or the host Wayland server.

The graphics stack must interoperate with the host session. GPU access, audio, controller devices, removable game directories and emulator executables must be exposed only through explicit, documented runtime permissions.

Do not assume that a sandbox profile grants access merely because the host application can access a device. Validate each required permission in the actual ULPM runtime.

## 5. ULPM package target

The final deliverable is a signed `.lpk` package installable and runnable by ULPM as an unprivileged user.

The package must:
- use a stable application ID and version;
- declare its architecture explicitly;
- contain the B-Emu executable, UI resources and required application-owned runtime files;
- declare Wayland, graphics, audio and game-controller access narrowly and explicitly;
- keep configuration, database, cache, saves and user-selected game directories outside the immutable package payload;
- install, update, roll back and uninstall without root;
- never silently weaken ULPM sandboxing when a permission or dependency is unavailable.

Build separate packages for supported CPU architectures (initially x86_64; add aarch64 after CI and runtime validation). Do not describe one architecture-specific binary as universal.

The exact manifest schema, permission names, runtime flags and packaging command must be taken from the current ULPM implementation and tested with the target ULPM release. Do not invent manifest keys or rely on undocumented behavior.

## 6. Emulator integration boundary

B-Emu is a frontend, not an emulator distribution by default.

Each emulator integration must state whether it uses:
1. a bundled, versioned emulator runtime; or
2. a compatible emulator executable explicitly made available by the host/user.

The launch adapter must use argument arrays, never shell-concatenated commands. It must validate executable availability and report missing runtime permissions or files clearly.

Game files, BIOS/firmware, saves and user-selected directories remain user-owned data. B-Emu must not silently copy, modify or delete them.

## 7. Release acceptance criteria

A release candidate is not portable until all of the following are verified:

1. Installs and uninstalls as a normal user through ULPM.
2. Runs from the installed package without writing into the package payload.
3. Runs in BubbleWM and at least one independent Wayland compositor.
4. Refuses to start without a usable Wayland connection; never falls back to X11/XWayland.
5. Uses GPU acceleration where supported and reports any renderer fallback accurately.
6. Controller, audio and game-directory access work only with the declared permissions.
7. Configuration, database and cache survive package upgrades and are not deleted by ordinary uninstall unless the user explicitly requests data removal.
8. Package signature and publisher identity are verified by ULPM.
9. Each published architecture has its own successful build and runtime validation.
10. No installation, execution or removal step requires root.

## 8. Integration policy

BubbleOS may provide B-Emu as a curated, preconfigured ULPM package after the standalone Linux release has passed its acceptance tests. BubbleOS integration must not introduce BubbleOS-only code paths into the core application.
