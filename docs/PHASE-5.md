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

## Remaining for release

- ULPM `.lpk` packaging and permission declarations, once the target ULPM
  implementation is specified.
- Validation on BubbleWM and at least one independent Wayland compositor.
