# Changelog

All notable changes to this project will be documented here.

The project uses semantic versioning where practical.

## [1.1.0-current.1] - 2026-10-08 (local compatibility fork)

- Target the unified ChatGPT Desktop native runtime 0.162.0-alpha.2.
- Preserve paginated metadata, import fork ancestors in dependency order, and rebase byte hints after path mapping.
- Require native registration and verify all paginated history pages and completed item IDs.
- Rebuild native projects and thread assignments, including multiple workspace roots.
- Map structured workspace/permission roots, rebuild affected history projections, and preserve archived state through native APIs.
- Snapshot all existing SQLite families and track newly created DBs and moved rollouts for rollback.
- Support completed-message events and inherited fork history in HTML exports.
- Build Windows CLI/GUI and verify synthetic migrations without touching real user history.

## [Unreleased]

- Export backups as complete `.codex` directory copies instead of minimal
  session-only folders, preserving databases, settings, Skills, logs, caches,
  and other local contents while excluding root-level login credential files.
- Name exported folders `Codex_backup` so they remain visible on macOS and Linux.

## [1.0.7] - 2026-06-21

- Use a zero-pixel optical text offset on Windows while retaining the existing macOS/Linux alignment.
- Embed the project icon and version metadata into Windows executables.
- Hide the console window when launching the Windows release GUI.

## [1.0.6] - 2026-06-21

- Added multi-select deletion for rollback snapshots.
- Added a default-selected tri-state “Select all” control for rollback records.
- Embedded user images and tool screenshots directly in exported single-file HTML.
- Added completion dialogs for import, backup export, HTML export, and path repair.
- Added Chinese and English interfaces with system-language detection.
- Added path repair, parent-directory mapping, and HTML conversation export.
- Added selective project/session import, conflict preview, transactional rollback, and Codex Desktop project registration.
