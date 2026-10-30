# Publishing a Windows + Ubuntu prerelease

Maintain one source tree, `main` branch and Cargo version for both platforms. Keep upstream attribution and the MIT license intact. Do not upload local `.codex` data, test-result directories, toolchains or development binaries.

1. Update `Cargo.toml` / `Cargo.lock`, changelog, README package names and release notes together. Use a new version such as `1.1.0-current.7`; never overwrite an existing released version.
2. Open a feature PR against this fork's `main`. CI runs formatting, Clippy, Rust tests, release builds, license collection and packaging on Windows and Ubuntu 24.04. Ubuntu also runs synthetic native Desktop/CLI migration tests, a GUI engine test and an actual terminal resume test with Codex 0.166.0. No online model requests or personal history are used.
3. Check both platform jobs and their artifacts, then merge the PR. Record Windows Desktop manual acceptance separately; a Windows compile/test job is not a Desktop runtime acceptance test.
4. When ready to release, tag the reviewed merge commit with `v` plus the Cargo version and push the tag. **Release Windows and Ubuntu prerelease** validates the tag, builds both platforms and verifies that the two versioned archives and their SHA-256 files are present. Only then does it create a **draft prerelease** containing all four assets.
5. Review the draft packages and publish from GitHub Releases. Alternatively, dispatch that release workflow with the tag as its ref, the same `tag` input, and `publish: true`. This explicit input rebuilds the tagged source, updates draft assets and publishes the prerelease. It does not mark the release stable/latest or overwrite an already published release.

The shared Release contains:

- `Codex-Migrate-Current-VERSION-Windows-x64.zip` and `.zip.sha256`.
- `Codex-Migrate-Current-VERSION-Ubuntu-24.04-x64.tar.gz` and `.tar.gz.sha256`.

Each archive contains the GUI and CLI, common documentation, upstream/project licenses, third-party notices and binary checksums. Windows uses MSVC and requires the VC++ runtime; Ubuntu uses the native Linux build. These are separate executables from the same source version.

## Build artifacts without publishing

The manual **Build Windows EXE** and **Build Ubuntu 24.04** workflows upload Actions artifacts without creating a Release. CI uses the same reusable workflows. Ubuntu's synthetic validation JSON is a separate artifact and is never included in the public release download set.

```powershell
cargo build --locked --release --features gui --bins --target x86_64-pc-windows-msvc
python .\scripts\generate-notices.py --target x86_64-pc-windows-msvc --output .\dist\notices
.\scripts\package-windows.ps1
```

```bash
cargo build --locked --release --features gui --bins
python3 scripts/generate-notices.py --target x86_64-unknown-linux-gnu --output dist/notices-linux
python3 scripts/package-linux.py
```

Collect only the two archives and their checksums into one directory, then run `python3 scripts/check-release-assets.py DIRECTORY` before uploading. It rejects missing platform packages, wrong versioned filenames, checksum mismatches and unexpected files.

## Optional Windows standalone EXE

The single-file edition still uses `scripts/build-standalone.ps1` and the portable GNU / LLVM-MinGW MSVCRT toolchain. Build clean, tagged source; verify startup, embedded notices and hashes. Upload the version-matched standalone EXE, `.sha256`, `BUILD-INFO.json` and `VALIDATION.json` to the same draft after the automated ZIP/tar.gz release workflow completes. Record the exact source commit. Do not substitute an older standalone EXE for a newer release or publish a dirty-checkout EXE.

The standalone EXE needs no companion CLI or additional VC++ runtime. It remains optional and does not replace the shared source tree or the automated Windows MSVC ZIP.
