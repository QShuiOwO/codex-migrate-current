# Codex Migrate Current

[简体中文](README.zh-CN.md) · [Downloads](https://github.com/QShuiOwO/codex-migrate-current/releases) · [Validation](docs/current-validation.md)

A community fork of [ChenglongLi777/codex-migrate](https://github.com/ChenglongLi777/codex-migrate), updated to migrate **local Codex code chats in ChatGPT Desktop**. The upstream baseline is v1.0.8, commit `37f5300c15447921baf3b98d8ef09d07a3a60008`. This fork retains the upstream history, attribution, and MIT license. It is not affiliated with or endorsed by OpenAI.

Version **1.1.0-current.2** is a **Windows x64 prerelease**. Compatibility was checked on 2026-10-08 against ChatGPT Desktop MSIX `OpenAI.Codex 26.1002.7124.0` and its native runtime `codex-cli 0.162.0-alpha.2`. It does not migrate cloud Chat/Work account history.

## What changed

- Recognizes paginated history and completed message events.
- Includes fork ancestors automatically and recalculates rewritten parent byte offsets.
- Imports native projects, multiple workspace roots, thread names, and archived chats through the matching App Server.
- Refreshes affected history projections and verifies every page of completed messages.
- Snapshots SQLite databases, rollouts, and indexes; native registration failures trigger rollback.
- Preserves legacy history and includes fork ancestors in HTML exports.

The CLI and bilingual GUI share the updated migration engine. See [CURRENT-VERSION.md](CURRENT-VERSION.md) for detailed adaptation notes in Chinese.

## Download

Download `Codex-Migrate-Current-1.1.0-current.2-Windows-x64.zip` and its `.sha256` file from [this fork's Releases](https://github.com/QShuiOwO/codex-migrate-current/releases). Extract the archive:

- `codex-migrate-gui.exe`: graphical application.
- `codex-migrate.exe`: command-line application.
- `SHA256SUMS.txt`: executable checksums.
- `LICENSE`, `THIRD_PARTY_NOTICES.txt`, and documentation: attribution and usage information.

Release binaries are built by GitHub Actions for Windows MSVC and are unsigned. They require the [Microsoft Visual C++ v14 x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist). If Windows reports a missing `VCRUNTIME140.dll`, install Microsoft's x64 Redistributable. Compare the ZIP checksum before running them:

```powershell
(Get-FileHash .\Codex-Migrate-Current-1.1.0-current.2-Windows-x64.zip -Algorithm SHA256).Hash.ToLower()
```

macOS/Linux source support is inherited from upstream, but this adaptation has no equivalent integration validation or binary releases for those platforms.

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

The adaptation passed **39 Rust tests and 16 synthetic integration scenarios** against the runtime listed above; formatting and Clippy also passed. An opt-in native test also passed the synthetic GUI export/options/preview/import flow. The MSVC release package is downloaded and checked before publication; final binary validation is reported in the release notes. A full GUI import into a real account was not performed. Results are in [docs/current-validation.md](docs/current-validation.md).

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

Map the additional workspace/permission folders listed in the GUI alongside the main project. Missing mappings are rejected during preview before target writes. Windows discovery now prefers the Desktop runtime over a global CLI in PATH; an explicit `CODEX_MIGRATE_CODEX_BIN` override still takes precedence.
