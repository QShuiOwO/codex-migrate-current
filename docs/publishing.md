# Publishing a Windows prerelease

Keep the upstream attribution and MIT license intact. Do not upload local `.codex` data, audit output, test-result directories, toolchains, or development binaries.

1. Update Cargo versions, the changelog, README download names, and validation notes together. Verify the exact desktop/runtime versions instead of claiming compatibility with every future version.
2. Open a PR against this fork's `main`. Windows CI runs formatting, Clippy, Rust tests, a release build, notice generation, and packaging. Review its artifact before release. Real-runtime integration is a separate local check using synthetic data and the matching installed runtime; hosted CI does not run it.
3. Merge after checks pass. Tag the reviewed commit with `v` plus the Cargo version, such as `v1.1.0-current.1`, and push that tag. The release workflow validates the version and creates a **draft prerelease** with the Windows ZIP and checksum.
4. Review and publish from GitHub Releases. Alternatively, run **Release Windows prerelease** using that tag as the workflow ref, set `tag` to the same tag, and set `publish` to true. This explicit input checks/builds the tagged source again, updates draft assets, and publishes the prerelease. It never marks the release stable/latest.

The manual **Build Windows EXE** workflow creates an artifact without a Release. All Windows workflows use `scripts/package-windows.ps1` for consistent names, documentation, checksums, and notices.

```powershell
cargo build --locked --release --features gui --bins --target x86_64-pc-windows-msvc
python .\scripts\generate-notices.py --target x86_64-pc-windows-msvc --output .\dist\notices
.\scripts\package-windows.ps1
```

Test-result directories remain ignored. The checked-in validation summary omits the original machine's output path.
