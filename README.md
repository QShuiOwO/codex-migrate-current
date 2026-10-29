# Codex Migrate Current

[简体中文](README.zh-CN.md) · [Downloads](https://github.com/QShuiOwO/codex-migrate-current/releases) · [Validation](docs/current-validation.md)

A community fork of [ChenglongLi777/codex-migrate](https://github.com/ChenglongLi777/codex-migrate), updated to migrate **local Codex code chats in ChatGPT Desktop**. The upstream baseline is v1.0.8, commit `37f5300c15447921baf3b98d8ef09d07a3a60008`. This fork retains the upstream history, attribution, and MIT license. It is not affiliated with or endorsed by OpenAI.

Version **1.1.0-current.6** is a **Windows x64 prerelease**. Compatibility was checked on 2026-10-29 against Desktop `26.1023.6621.0` and its native runtime `codex-cli 0.165.0-alpha.1.2`. It does not migrate cloud Chat/Work account history.

**Update 2026-10-29:** current.6 fixes repeat-import failures with Desktop `26.1023.6621.0` / `codex-cli 0.165.0-alpha.1.2`. It accepts equivalent Windows extended-length paths while still rejecting different directories. The standalone EXE is `Codex-Migrate-Current-1.1.0-current.6-Standalone-Windows-x64.exe`. Upgrade from current.5 when using this newer runtime; current.5 can fail when importing an existing conversation again.

## What changed

**1.1.0-current.6** retains selective export, automatic creation of missing additional folders during import, and a single-file Windows EXE. Opening **Export sessions** automatically lists active and archived local conversations with nothing selected. Choose individual conversations, a project, or all records, choose an output folder, then export. Required fork ancestors are listed before export.

Selected backups contain rollout JSONL files, selected index rows, and `codex-migrate-export.json` metadata, retaining names and project relationships without copying unrelated conversations, databases, settings or logs. Import these backups with current.4 or a later version supporting the format. Project files and attachment entities still need separate transfer.

CLI selection is repeatable: `codex-migrate export SOURCE --output-parent DEST --thread UUID1 --thread UUID2`. Omitting `--thread` retains the original complete-directory backup command.

- Recognizes paginated history and completed message events.
- Includes fork ancestors automatically and recalculates rewritten parent byte offsets.
- Imports native projects, multiple workspace roots, thread names, and archived chats through the matching App Server.
- Refreshes affected history projections and verifies every page of completed messages.
- Snapshots SQLite databases, rollouts, and indexes; native registration failures trigger rollback.
- Preserves legacy history and includes fork ancestors in HTML exports.

The CLI and bilingual GUI share the updated migration engine. See [CURRENT-VERSION.md](CURRENT-VERSION.md) for detailed adaptation notes in Chinese.

## Download

For the GUI, download [the standalone EXE](https://github.com/QShuiOwO/codex-migrate-current/releases/download/v1.1.0-current.6/Codex-Migrate-Current-1.1.0-current.6-Standalone-Windows-x64.exe) from [current.6](https://github.com/QShuiOwO/codex-migrate-current/releases/tag/v1.1.0-current.6) and run it directly. No companion CLI or additional VC++ runtime is needed. The optional ZIP below contains both GUI and CLI.

Download `Codex-Migrate-Current-1.1.0-current.6-Windows-x64.zip` and its `.sha256` file from [this fork's Releases](https://github.com/QShuiOwO/codex-migrate-current/releases). Extract the archive:

- `codex-migrate-gui.exe`: graphical application.
- `codex-migrate.exe`: command-line application.
- `SHA256SUMS.txt`: executable checksums.
- `LICENSE`, `THIRD_PARTY_NOTICES.txt`, and documentation: attribution and usage information.

The optional ZIP binaries are built by GitHub Actions for Windows MSVC and are unsigned. They require the [Microsoft Visual C++ v14 x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist). If Windows reports a missing `VCRUNTIME140.dll`, install Microsoft's x64 Redistributable. Compare the ZIP checksum before running them:

```powershell
(Get-FileHash .\Codex-Migrate-Current-1.1.0-current.6-Windows-x64.zip -Algorithm SHA256).Hash.ToLower()
```

macOS/Linux source support is inherited from upstream, but this adaptation has no equivalent integration validation or binary releases for those platforms.

### Standalone Windows executable

The current source produces `Codex-Migrate-Current-1.1.0-current.6-Standalone-Windows-x64.exe`. Copy and launch this single file; no extraction, companion CLI, or additional VC++ runtime installation is needed. Current-history migration still requires a compatible Desktop / Codex runtime on the destination computer. Transfer conversation backups and project files separately.

Settings includes the upstream MIT attribution and an option to save the complete embedded license notices. With the maintenance workspace's portable Rust GNU and LLVM-MinGW MSVCRT toolchains in `../.toolchain`, run `./scripts/build-standalone.ps1 -Python python`. The script collects notices, runs formatting, Clippy and tests, builds the binaries, and rejects non-system DLL imports. The EXE and checksum are written to `dist/standalone/`. This local standalone build is separate from the existing GitHub MSVC ZIP workflow.

## Use

1. Back up the source `.codex` directory. Keep backups private: they contain conversations and local configuration.
2. Completely exit ChatGPT Desktop and all Codex CLI sessions before import or rollback.
3. Scan the backup, select chats, map all workspace roots, and review the dry-run plan. Fork ancestors may be added to the selection.
4. Import and check the resulting chats before continuing work. Keep the reported transaction ID for rollback.

The tool accepts `.codex`, `Codex_backup`, and the upstream legacy `Codex` backup layout. It does not copy project source files. Paginated history requires the matching native `codex.exe`. Windows discovery checks `%LOCALAPPDATA%\OpenAI\Codex\bin\*\codex.exe`; set `CODEX_MIGRATE_CODEX_BIN` explicitly if necessary.

```powershell
.\codex-migrate.exe scan 'D:\Backup\Codex_backup'
.\codex-migrate.exe import 'D:\Backup\Codex_backup' `
  --thread 'THREAD_UUID' --map 'C:\OldProject=D:\NewProject' --dry-run

# Remove --dry-run after reviewing the plan.
# Add --map arguments for additional workspace roots.
.\codex-migrate.exe rollback 'TRANSACTION_ID'
```

`--history-only 'OLD_CWD'` creates a placeholder workspace without registering a real project. `--codex-home` selects the target history directory; also check `CODEX_SQLITE_HOME` when it is set, because it independently selects the database directory. To try an isolated target, set both to a separate directory. Run `codex-migrate --help` for all commands.

## Validation and limitations

The current.6 GNU standalone build passes **53 regular Rust tests, one native GUI flow, and 23 native migration scenarios**, including automatic directory creation, failure rollback and file-collision protection. Formatting and Clippy pass. Tests use synthetic conversations. `VALIDATION.json` beside the standalone EXE records the current checks; historical release validation remains in [docs/current-validation.md](docs/current-validation.md).

Cloud account history, attachment entities, `thread_attachments`, generated artifacts, other operating systems, and future runtimes are outside this validation. Rollback restores snapshots taken before migration; later chats or edits can be overwritten. Verify the import before resuming chats, and back up current data before a later rollback.

## Build

```powershell
git clone https://github.com/QShuiOwO/codex-migrate-current.git
cd codex-migrate-current
cargo test --locked --all-targets --features gui
cargo build --locked --release --features gui --bins
```

Rust 1.99.0 was used for validation. Windows builds also need a compatible native C/C++ toolchain. The optional runtime suite uses Python 3.11+ and synthetic data only:

```powershell
python .\tests\current_runtime.py --codex-bin 'C:\ActualRuntime\codex.exe' `
  --migrate-bin "$PWD\target\release\codex-migrate.exe"
```

Actions validates Windows builds, generates dependency notices, and packages the executables. A version tag creates a draft prerelease; the workflow's explicit `publish` input publishes it after review. See [docs/publishing.md](docs/publishing.md).

## License and provenance

[MIT](LICENSE), retaining `Copyright (c) 2026 codex-migrate contributors`. Changes in this fork are also MIT licensed. See [UPSTREAM.json](UPSTREAM.json) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for provenance and binary/font notices.

Report compatibility issues in [this fork](https://github.com/QShuiOwO/codex-migrate-current/issues). Remove private conversations, credentials, and paths; do not upload a complete `.codex`. See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [TRADEMARKS.md](TRADEMARKS.md).

## Import diagnostics (current.2)

The GUI displays the complete error chain, expands the log panel on failure, and supports copying or saving logs. Preview/import logs are written automatically to `%LOCALAPPDATA%\codex-migrate\logs\`; the exact filename is shown in the error. Logs survive rollback and include the selected runtime, data roots, mappings, failure stages, and native stderr diagnostics. Review local paths and thread IDs before sharing a log.

In current.5, additional GUI folders default to their original local absolute paths. Missing folders show "Create during import" and are created only when import runs; preview does not write them. You can still choose another folder or use the main project folder. Paths incompatible with this OS still need an explicit mapping. Windows discovery now prefers the Desktop runtime over a global CLI in PATH; an explicit `CODEX_MIGRATE_CODEX_BIN` override still takes precedence.

Creation failures include the path and system error in the diagnostic log. Files are never overwritten by directory creation. Failed imports and manual rollback remove only newly created, empty directories, preserving existing folders and user files. Creating an empty directory does not restore its original project files or attachments.
