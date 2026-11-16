# Library persistence design

## Goals

The first persistence layer stores only facts needed to reproduce a filesystem
scan. It must remain independent from Slint, emulator launchers, metadata
providers, and artwork services.

The database is user-owned application data at
`$XDG_DATA_HOME/b-emu/library.sqlite3` (with the existing XDG fallback).
No root access or system-wide database is required.

## Stable identity

- A library root is identified by its normalized absolute path.
- A discovered file is identified by its normalized absolute path within a
  root. The same file may appear under more than one configured root.
- Do not use inode numbers as persistent identity: they are filesystem-local
  and can change when files are copied or restored.
- Do not resolve or follow symbolic links. Keep the scanner's no-symlink
  traversal guarantee.
- Paths are stored as UTF-8 only when representable. The implementation must
  report a clear error for non-UTF-8 paths rather than silently replacing
  bytes with lossy text.

## Schema version 1

Use SQLite `PRAGMA user_version` as the authoritative schema version.
Migrations run in a transaction and must reject a database with a newer
unsupported version instead of downgrading it.

### `library_roots`

| Column | Type | Rules |
| --- | --- | --- |
| `id` | INTEGER | Primary key |
| `path` | TEXT | Not null, unique, absolute |
| `recursive` | INTEGER | Not null, boolean (0 or 1) |
| `enabled` | INTEGER | Not null, boolean (0 or 1), default 1 |
| `created_at` | INTEGER | Not null, Unix seconds |
| `updated_at` | INTEGER | Not null, Unix seconds |

### `games`

| Column | Type | Rules |
| --- | --- | --- |
| `id` | INTEGER | Primary key |
| `root_id` | INTEGER | Not null, foreign key to `library_roots(id)`, cascade delete |
| `path` | TEXT | Not null, absolute |
| `file_name` | TEXT | Not null |
| `extension` | TEXT | Nullable, lowercase without leading dot |
| `size_bytes` | INTEGER | Not null, non-negative |
| `modified_unix_secs` | INTEGER | Nullable, non-negative |
| `discovered_at` | INTEGER | Not null, Unix seconds |
| | | Unique constraint on (`root_id`, `path`) |

Add indexes for `games(root_id)`, `games(root_id, file_name)`, and
`games(root_id, extension)`. Enable foreign-key enforcement on every
connection with `PRAGMA foreign_keys = ON`.

The schema intentionally has no console/system, ROM validity, checksum,
scraper metadata, artwork, emulator, or launch configuration columns. Those
belong to later domain-specific migrations.

## Scan persistence contract

1. Run filesystem enumeration outside the database transaction.
2. Never hold a SQLite write transaction while walking directories.
3. Persist a completed, non-cancelled scan in one transaction.
4. Upsert all discovered files for the selected root.
5. Delete stale rows for that root only after a complete scan with no
   cancellation and no scan issues. A partial or inaccessible scan must never
   erase previously known games.
6. A cancelled scan is not committed as a replacement snapshot.
7. Use prepared statements and parameter binding for every path/value.
8. Use a bounded busy timeout; surface lock errors to the caller rather than
   blocking the UI thread.
9. Keep all database operations off the Slint event-loop thread.

## Migration and recovery rules

- A new database is initialized transactionally at schema version 1.
- An existing version 1 database is opened without destructive recreation.
- A database with a higher `user_version` is rejected with an actionable
  error.
- A failed migration rolls back completely.
- Never silently delete, recreate, or overwrite a corrupt database. Return a
  typed error and preserve the original file for recovery.
- WAL mode may be enabled after verifying the target is a regular user-owned
  database file; checkpointing and close behavior must be covered by tests.

## Implementation boundary

The future Rust store module should expose typed operations for opening and
migrating the database, adding/updating/removing roots, loading a root's
snapshot, and committing a completed scan. It should not expose raw
`rusqlite::Connection` values outside the library module.

The store must be independently testable with temporary database files.
Tests should cover fresh creation, idempotent reopen, schema-version
rejection, migration rollback, foreign-key behavior, path uniqueness,
snapshot replacement, preservation after partial/cancelled scans, and
concurrent-reader/busy handling.

## Dependency choice

Use `rusqlite` with its `bundled` feature for a predictable, self-contained
SQLite build across supported Linux distributions. Keep the dependency
inside the library layer. Generate and commit `Cargo.lock` with Cargo; never
hand-edit its dependency graph.
