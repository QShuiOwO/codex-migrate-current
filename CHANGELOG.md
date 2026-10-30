# Changelog

All notable changes to this project will be documented here.

The project uses semantic versioning where practical.

## [1.1.0-current.7] - 2026-10-30

- Maintain Windows x64 and Ubuntu 24.04 x64 in one source tree and version. Run both platform builds in CI and collect both packages and checksums into the same draft prerelease.
- Add Desktop/CLI import targets to the GUI and CLI. Linux defaults to CLI; Windows/macOS default to Desktop. CLI imports activate selected archived/non-interactive sessions for `codex resume` in the mapped work directory.
- Match the destination model provider and validate interactive directory/provider visibility through the native runtime. Preserve original metadata for repeat and round-trip imports, including provider changes and fork ancestors.
- Match source Windows drive/UNC/extended paths independently of the target OS, keep Unix paths case-sensitive, map Windows permission globs, and resolve local workspace symlinks.
- Discover Ubuntu npm/nvm installations from desktop launchers and use the native Linux Codex executable.
- Add synthetic cross-platform, provider, round-trip and actual CLI picker tests. Preserve the existing Desktop migration, archive, project, selective export and rollback behavior.
- Add Ubuntu build/usage/validation documentation. No cloud chat, project-source, or attachment migration is introduced.

## [1.1.0-current.6] - 2026-10-29

- Accept equivalent Windows extended-length paths (`\\?\D:\...` and extended UNC paths) in directory mappings, database validation and rollout validation. Preserve drive roots and mapping boundaries, and reject genuinely different directories.
- Fix repeat imports with Codex `0.165.0-alpha.1.2` for ordinary, archived and history-only conversations, including fork ancestors. Keep Unix path comparisons case-sensitive.
- Validate 53 regular Rust tests, one native GUI flow, and 23 native scenarios on Desktop `26.1023.6621.0` / state schema 62. Retry the previously failing current.5 synthetic target successfully without duplicating conversations or projects.

## [1.1.0-current.5] - 2026-10-24

- Automatically select original local absolute paths for additional GUI workspace folders; create missing directories during import, with no writes during preview.
- Prepare required runtime/project roots before native registration; do not create permission-only descendants such as `.git` or glob paths.
- Record newly created directories in the transaction. Rollback removes only empty folders created by that import, retaining existing folders and user files. Report directory creation failures with the path and system error.

- Validate 49 regular Rust tests, one native GUI export/import flow, 20 native scenarios and standalone startup with `codex-cli 0.164.0-alpha.11.1`.

## [1.1.0-current.4] - Local development build (included in current.5)

- Load active and archived local conversations automatically on the GUI export page. Start with no selection and support individual, project and all-session selection.
- Export selected conversations plus explicitly listed fork ancestors. Store selected metadata and index rows without copying unrelated conversations, databases, credentials, settings or logs.
- Preserve names, archive state, project metadata and multiple workspace roots through export and import. Verify exported rollout hashes and reject missing or cyclic ancestors before writing a backup.
- Add repeatable CLI `export --thread UUID` selection; exporting without `--thread` retains the complete-folder backup command.
- Add a single-file Windows build with embedded license notices and no additional VC++ runtime requirement.
- Validate 46 regular Rust tests, the synthetic GUI export/import flow, and 17 native runtime scenarios against `codex-cli 0.164.0-alpha.11.1`.

## [1.1.0-current.3] - 2026-10-10

- Count required additional workspace/permission folders in GUI pending paths and disable preview until their local folders are mapped.
- Show only extra folders required by selected sessions; history-only mode and unselected sessions do not add pending paths.
- Add a preview entry guard and a clearer project-card instruction.
- Test the unmapped visualization-folder report, selection filtering, and the native "Use project folder" flow using synthetic data.

## [1.1.0-current.2] - 2026-10-09

- Show the complete GUI error chain, expand logs on failure, and add copy/save controls.
- Persist preview/import diagnostics outside the target history, including actual runtime, paths, mappings, transaction, RPC errors and bounded credential-filtered native stderr.
- Expose additional workspace and permission directory mappings in the GUI, including JSONL-only backups.
- Reject missing workspace/project mappings during preview before beginning writes.
- Prefer the Windows Desktop runtime over a global CLI in PATH, including when PATH is absent.
- Preserve the original import error if recording new databases also fails.
- Add GUI export/preview/import testing through the real native runtime using synthetic data.

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
