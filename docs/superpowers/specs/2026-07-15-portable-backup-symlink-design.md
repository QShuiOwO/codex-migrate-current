# Portable Backup Symlink Fix Design

## Goal

Make full Codex directory backups complete successfully on Windows when the
destination is NTFS, exFAT, or FAT32 and the application is not elevated.
Release the fix as version 1.0.8 in both `codex-migrate_V1` and the public
`codex-migrate` source tree.

## Root Cause

The exporter walks the complete Codex home without following symbolic links.
When it encounters a link, it currently recreates that link at the destination.
Windows treats link creation as privileged on a normal NTFS setup, while common
removable-drive file systems such as exFAT and FAT32 do not support soft links.
This aborts the entire export after all preceding regular files have already
been copied.

## Behavior

- Continue copying every regular file and directory except the existing
  top-level login credential exclusions.
- Treat symbolic links as non-portable backup entries and skip them on every
  platform instead of recreating or dereferencing them.
- Never copy a link target. This prevents links pointing outside the Codex home
  from importing unrelated, large, or sensitive data into the backup.
- Log each skipped link with its source-relative path.
- Include the skipped-link count in the export result and completion message.
- A skipped link is a successful export with a warning, not an export failure.
- Add source and destination path context to genuine directory creation, file
  copy, traversal, and final rename errors.
- Preserve the existing staging-directory cleanup and final rename workflow.

## Compatibility

The backup remains a normal `Codex_backup` directory and does not introduce a
package format or require import changes. Sessions, SQLite databases, settings,
and other regular files are copied as before. User-managed content reachable
only through a symbolic link is intentionally not included and is reported in
the export logs.

## Tests

- A source tree containing a symbolic link exports successfully without
  creating the link or copying its target.
- The export result reports one skipped link and progress logs identify it.
- Regular files and session files are still copied and counted.
- A destination containing an existing `Codex_backup` still fails safely.
- CLI and GUI compile with the expanded export summary.
- Formatting, Clippy, all tests, and release builds pass in both source trees.

## Versioning and Synchronization

Update the crate version and user-visible version to 1.0.8. Apply equivalent
source, tests, lockfile, and documentation changes to `codex-migrate_V1` and
`codex-migrate`, then verify that their relevant source files remain identical.
