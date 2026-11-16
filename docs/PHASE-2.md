# Phase 2 — Game library foundation

## Scope of this increment

This Phase 2 foundation establishes a read-only filesystem discovery layer
and a versioned SQLite persistence layer. It does not yet add a UI workflow,
system identification, metadata providers, artwork downloads, or emulator
launching.

The scanner:

- walks user-selected directories without requiring elevated privileges;
- enumerates regular files and supports an explicit extension allowlist;
- optionally recurses into subdirectories and supports a maximum depth;
- never follows symbolic links, preventing directory loops and traversal
  outside the selected root;
- reports inaccessible descendants without aborting the entire scan;
- supports cooperative cancellation for later background-worker integration;
- sorts results deterministically for stable tests and UI presentation;
- never moves, renames, deletes, or modifies discovered files.

An empty extension allowlist means all regular files are returned. This is
intentional: the scanner does not invent a universal ROM extension catalogue.
A later system registry will provide explicit, tested system-specific formats.

## Data boundary

GameFile contains filesystem facts only: path, file name, extension, size,
and optional modification time. It does not claim that a file is a valid ROM
or identify a console/system.

The scanner is independent from Slint and SQLite. The SQLite store is a
separate library-layer module: it owns versioned schema migrations, root
records, and transactional snapshots without exposing database connections
to the UI.

## Validation

Scanner tests cover recursive discovery, deterministic ordering, extension
filtering, non-recursive scans, pre-cancelled scans, invalid roots, and
symbolic-link exclusion. Store tests cover schema creation/version checks,
root uniqueness, snapshot replacement, incomplete-scan preservation, path
containment, and cascading root removal. CI compilation and tests remain
required before this increment is considered complete.

## Next increments

1. Add a cancellable background scan coordinator and progress reporting.
2. Add an explicit system registry with tested extension-to-system mappings.
3. Connect the library view to persisted scan results.
