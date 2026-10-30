# Codex Migrate Current

[简体中文](README.zh-CN.md) · [Downloads](https://github.com/QShuiOwO/codex-migrate-current/releases) · [Ubuntu guide](docs/ubuntu-cli.md) · [Changelog](CHANGELOG.md)

A community fork of [ChenglongLi777/codex-migrate](https://github.com/ChenglongLi777/codex-migrate) for moving **local Codex code conversations between Windows ChatGPT Desktop and Ubuntu Codex CLI**. Both platforms share the source code, backup format and version; same-platform migration is supported too. Cloud ChatGPT / Chat / Work account history is outside its scope.

Current version: **1.1.0-current.7**. The upstream baseline is v1.0.8, commit `37f5300c15447921baf3b98d8ef09d07a3a60008`. This fork retains the upstream history, attribution and MIT license. It is not affiliated with or endorsed by OpenAI.

## Download and launch

Choose matching platform packages from [Releases](https://github.com/QShuiOwO/codex-migrate-current/releases). Before a version is published, use its PR Actions artifacts or build from source.

| Platform | current.7 release file | Launch the extracted GUI |
| --- | --- | --- |
| Windows x64 | `Codex-Migrate-Current-1.1.0-current.7-Windows-x64.zip` | `codex-migrate-gui.exe` |
| Ubuntu 24.04 x64 | `Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64.tar.gz` | `./codex-migrate-gui` |

Each package includes the GUI, CLI, documentation, licenses and binary checksums, with a separate archive `.sha256` file. The Windows MSVC ZIP requires the [Microsoft Visual C++ v14 x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist). The Ubuntu GUI uses system X11/Wayland and a file-picker portal; the CLI works without a desktop environment.

```powershell
# Windows: compare the archive hash with its .sha256 file
(Get-FileHash .\Codex-Migrate-Current-1.1.0-current.7-Windows-x64.zip -Algorithm SHA256).Hash.ToLower()
```

```bash
# Ubuntu: verify, extract and launch
sha256sum -c Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64.tar.gz.sha256
tar -xzf Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64.tar.gz
cd Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64
./codex-migrate-gui
```

The optional Windows single-file build remains available through `scripts/build-standalone.ps1`. Download it only if that version's Release actually includes it; the historical current.6 standalone EXE does not contain current.7's changes.

## Windows to Ubuntu

1. Use **Export sessions** on Windows to select conversations and create `Codex_backup`. Required fork ancestors are included automatically; nothing is selected by default.
2. Copy the backup and any needed project files separately to Ubuntu. Close Desktop / CLI sessions using the destination history before importing.
3. In the Ubuntu GUI, choose **Codex CLI** (the Linux default), load the backup, and normally keep `~/.codex` as the history store.
4. Map each selected project to a local work folder, including additional workspace/permission directories. Preview and import. The work folder is distinct from the `.codex` history store.
5. Open a terminal in the mapped work folder and run `codex resume`. Choose the imported conversation and continue using the destination machine's login and model configuration.

The tool accepts `backup`, `Codex_backup`, `.codex` and older compact `Codex` folders. CLI mode activates explicitly selected archived/non-interactive sessions while retaining implicit ancestors' archive state. It adapts the destination provider and preserves original metadata for repeat and round-trip comparisons.

```bash
./codex-migrate scan /media/usb/backup
./codex-migrate import /media/usb/backup --target cli \
  --map 'C:\Users\Alice\Project=/home/alice/Project' --dry-run
# Review, repeat without --dry-run, then:
cd /home/alice/Project
codex resume
```

## Ubuntu to Windows

Select conversations or projects in the Ubuntu export GUI, or repeat `--thread` in the CLI:

```bash
./codex-migrate export "$HOME/.codex" --output-parent /media/usb/export \
  --thread 'THREAD_UUID'
```

Copy `Codex_backup` to Windows and select **Desktop** in the import GUI, or run:

```powershell
.\codex-migrate.exe import 'E:\export\Codex_backup' --target desktop `
  --map '/home/alice/Project=D:\Projects\Project' --dry-run
# Review, repeat without --dry-run, then reopen Desktop.
```

Use current.7 on both machines, particularly when merging the same UUID back and forth. **Operating system and import target are independent**: Windows users can also choose `--target cli`. Desktop mode retains archive state and registers projects and multiple workspace roots. Linux defaults to `cli`; Windows/macOS default to `desktop`. macOS has not received equivalent integration validation.

## Behavior and data scope

- One migration engine handles legacy/paginated history, fork ancestors, names, projects, multiple workspaces and HTML exports.
- Windows source paths support case differences, UNC and extended-length paths; Unix source paths remain case-sensitive.
- Preview creates no workspace folders. Import creates missing folders and automatically restores database, rollout and index snapshots on failure.
- Selected backups include only selected conversations, required ancestors, indexes and migration metadata. Omitting `export --thread` makes a full directory backup that can contain sensitive settings.
- Project source, attachment entities, `thread_attachments` and generated files need separate transfer. Old paths in conversation text are preserved.

`--codex-home` selects the history store; `CODEX_SQLITE_HOME` independently selects the database directory. Migration and Codex must use the same settings for custom locations. Windows discovers the Desktop runtime; Ubuntu supports PATH and npm/nvm installations. Set `CODEX_MIGRATE_CODEX_BIN` for other locations. Paginated history requires a compatible native runtime.

Imports report a transaction ID for `codex-migrate rollback ID`. Validate before continuing the chat: rollback restores pre-import snapshots and can overwrite later changes. Diagnostic logs default to `%LOCALAPPDATA%\codex-migrate\logs` on Windows and `~/.local/share/codex-migrate/logs` on Ubuntu; `CODEX_MIGRATE_LOG_DIR` overrides the location.

## Build, test and maintain

`Cargo.toml` and `Cargo.lock` manage shared Rust dependencies. Common code lives in `src/`; platform packaging lives in `scripts/`. Feature branches merge into `main` after Windows and Ubuntu CI checks; platforms do not have separate long-lived branches.

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --features gui -- -D warnings
cargo test --locked --all-targets --features gui
cargo build --locked --release --features gui --bins
```

CI uses Rust 1.99.0; local Ubuntu validation used Rust 1.99.1 and Codex CLI 0.166.0. Build dependencies, synthetic native tests and terminal validation are documented in the [Ubuntu guide](docs/ubuntu-cli.md) and [validation record](docs/ubuntu-validation.md). Earlier Windows Desktop 26.1023.6621.0 / Codex 0.165.0-alpha.1.2 checks remain in [historical validation](docs/current-validation.md). Current.7 Windows builds/tests are recorded in its CI runs and do not replace Desktop acceptance testing.

The unified release workflow waits for both platform builds, verifies their versions and SHA-256 files, and creates one draft prerelease. See [publishing](docs/publishing.md) and [adaptation history](CURRENT-VERSION.md).

## License and feedback

[MIT](LICENSE), retaining `Copyright (c) 2026 codex-migrate contributors`. See [UPSTREAM.json](UPSTREAM.json) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for provenance and third-party licenses.

Report problems in [Issues](https://github.com/QShuiOwO/codex-migrate-current/issues). Remove private conversations, credentials and local paths from logs; never upload a complete `.codex`. See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md) and [TRADEMARKS.md](TRADEMARKS.md).
