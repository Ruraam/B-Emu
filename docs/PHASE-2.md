# Phase 2 — Library

Phase 2 adds a local game index. It does not bundle ROMs, BIOS files, or
emulators, and it never modifies game files.

## Configuration

User settings live in `$XDG_CONFIG_HOME/b-emu/config.json` (falling back to
`~/.config/b-emu/config.json`). The file is typed and minimal:

```json
{
  "game_dirs": ["/home/user/ROMs"]
}
```

Directories must be absolute. They are added and removed from the Settings
screen; B-Emu validates that the path exists and is a directory before storing
it. A missing config file is treated as an empty configuration.

## Scanning

The scanner runs on the `b-emu-scan` thread and is cancellable.

- Traversal uses `walkdir` without following symlinks; hidden directories are
  skipped.
- Only supported ROM/disc extensions are indexed (`.nes`, `.sfc`, `.md`, `.iso`,
  `.cue`, `.zip`, …). Unknown files such as `notes.txt` are ignored.
- New or changed files are hashed with SHA-256 while reading them. Unchanged
  files reuse their stored checksum, so rescans are incremental.
- Per-file errors are counted and skipped rather than aborting the scan.
  Non-UTF-8 roots and game paths are rejected explicitly because the current
  SQLite schema stores paths as UTF-8 text; no lossy path conversion is used.
- Stale rows are pruned only for roots that were accessible and scanned without
  errors. Missing roots, partial scans, descendant errors, and cancellation
  preserve their existing rows, favorites, and play history. A successful root
  can still be refreshed when a different configured root fails.

Game directories are opened read-only. All writes go to B-Emu's own XDG data
directory.

## SQLite index

The index is `$XDG_DATA_HOME/b-emu/library.sqlite3` (falling back to
`~/.local/share/b-emu/library.sqlite3`). Schema migrations are idempotent
(`CREATE TABLE IF NOT EXISTS`). The `games` table stores path, title, detected
system, size, modification time, checksum and source directory.

The service exposes games, full-text-ish search (case-insensitive `LIKE` on
title and system), recent games, counts, and per-system summaries. Files that
share a checksum are flagged as duplicates in the UI without being deleted.

Two extra tables track user state: `favorites` and `play_history`. Favorites can
be toggled per game; play history records when a game was marked as played
(the launch coordinator arrives in Phase 3, so no game process is started yet).
Rows for games removed from the index are cleaned up automatically.

## Interface

- **Home / Systems / Library** are now backed by the index. With no game
  directories configured, they show explicit empty states.
- **Settings** manages game directories, shows scan progress and the last scan
  report, and can start a new scan. With auto-scan enabled, configured
  directories are indexed at startup.
- **Library** has "Tous / Favoris" filters, a favorites star per game, and
  detail actions to toggle a favorite or record a play. The same actions are
  available from the controller (West/North) and the keyboard (`F`/`P`).
- **Home** shows recently played games and a favorites counter.

## Validation

Unit tests cover config round-trips and validation, extension detection,
read-only scanning, incremental rescans, cancellation, hidden directory
skipping, missing and partially inaccessible roots, non-UTF-8 path rejection,
SQLite upserts/removals, duplicate detection, and system grouping.
The CI Weston smoke test starts the application with a configured ROM
directory and verifies that the SQLite index is created.
