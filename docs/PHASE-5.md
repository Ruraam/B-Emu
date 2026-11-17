# Phase 5 — Hardening

## Preferences

`auto_scan`, `vibration` and `reduced_motion` are persisted in `config.json`.
They are loaded into the Slint properties at startup and written back on every
toggle. The vibration preference also drives the gamepad worker's force
feedback flag, and the reduced-motion preference drives the shared Slint
animation duration.

## Crash-safe index updates

A scan applies all discovered games and the optional prune in a **single
SQLite transaction** (`LibraryDb::apply_scan`). A crash or an error therefore
leaves either the previous index or the complete new one, never a partial
state. Pruning is skipped when a scan is cancelled.

## Launch safety

Before a game is launched, B-Emu verifies that:

- the path resolves to an existing regular file;
- its canonical path (symlinks resolved) is inside one of the configured game
  directories.

The launch itself uses an `OsString` argument array and never a shell command.
A CI audit (`scripts/check-launch-safety.sh`) rejects shell invocation patterns
and formatted command strings in the source.

## Validation

Unit tests cover preference defaults and round-trips, transactional scan
application and pruning, argument-array construction, and input diagnostics.
CI checks formatting, Wayland-only features, launch safety, compilation and all
tests, then runs a headless Weston smoke test.

## ULPM packaging

`packaging/manifest.json` is a Schema v5 manifest for a signed `.lpk`. The
package is built with `scripts/package-lpk.sh`, which compiles the release
binary, stages a pre-structured payload (`meta/manifest.json` + `bin/b-emu`),
and calls the ULPM CLI. ULPM is a **build-time tool only**: the development
binary has no runtime dependency on it or on any compositor-specific service.

Permissions are deliberately minimal:

| Permission | Value | Reason |
| :--- | :--- | :--- |
| `network` | false | B-Emu has no network provider |
| `gui`, `gpu`, `dri` | true | Native Wayland client with FemtoVG/software rendering |
| `input` | true | Rootless gamepad access via the active seat |
| `audio` | true | Needed by emulators launched from the frontend |
| `hidraw`, `usb`, `kvm` | false | Not used |
| `filesystem.home` | isolated | Config/data/cache persist under `~/.var/app/io.lpk.b-emu` |
| `filesystem.downloads` | none | Not used |
| `filesystem.custom` | [] | Users add their own read-only ROM binds via ULPM overrides |

Because game directories live outside the sandbox by default, users expose
them with a local override (an ULPM override never modifies the package):

```bash
ulpm override io.lpk.b-emu 'permissions.filesystem.custom=[{"path":"/home/user/ROMs","mode":"ro"}]'
ulpm run io.lpk.b-emu
```

Build and validate:

```bash
bash scripts/package-lpk.sh
ulpm inspect target/b-emu_amd64.lpk
ulpm run target/b-emu_amd64.lpk
```

The smoke test starts the packaged application under a headless Wayland
compositor inside the Bubblewrap sandbox and verifies that it stays running.

## Remaining for release

- Validation on BubbleWM and at least one independent Wayland compositor.
- Distribution of the release public key through the repository index.
