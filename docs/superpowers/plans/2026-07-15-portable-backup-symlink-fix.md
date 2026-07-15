# Portable Backup Symlink Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export complete Codex backups on Windows and removable drives without requiring symbolic-link privileges or filesystem support.

**Architecture:** Keep the existing staging-directory export pipeline and change only entry handling: regular files and directories remain copied, while symbolic links become explicitly reported skips. Extend `ExportSummary` so CLI and GUI completion messages disclose the skipped-link count, and add path context to genuine I/O failures.

**Tech Stack:** Rust 2021, `walkdir`, `anyhow`, `serde`, `eframe/egui`, Cargo integration tests.

## Global Constraints

- Release version is exactly `1.0.8`.
- Apply equivalent source, test, lockfile, and user-visible changes to `codex-migrate` and `codex-migrate_V1`.
- Keep backups as ordinary `Codex_backup` directories with no package or manifest format.
- Continue excluding only the existing top-level login credential filenames.
- Never recreate or dereference symbolic links; skip them on every platform.
- Log every skipped symbolic link and report the total at completion.
- Preserve staging cleanup and final same-parent rename behavior.

---

### Task 1: Regression Test and Portable Export Core

**Files:**
- Modify: `tests/cli_smoke.rs`
- Modify: `src/operations.rs`

**Interfaces:**
- Consumes: `operations::export_directory(source, output_parent, progress)`.
- Produces: `ExportSummary { output, thread_count, skipped_symlink_count }` and progress messages beginning with `Skipped symbolic link:`.

- [ ] **Step 1: Write the failing Unix regression test**

Add a `#[cfg(unix)]` test that creates a regular file and a symbolic link whose target is outside the Codex home, exports the directory, and asserts that the regular file exists, the link and target contents are absent, the summary count is one, and the progress log names the skipped link:

```rust
#[cfg(unix)]
#[test]
fn export_skips_symbolic_links_without_copying_targets() {
    use std::os::unix::fs::symlink;

    let source = TempDir::new().unwrap();
    create_codex_home(source.path(), "11111111-2222-3333-4444-555555555555");
    fs::write(source.path().join("regular.txt"), "regular").unwrap();
    let external = TempDir::new().unwrap();
    fs::write(external.path().join("outside.txt"), "outside").unwrap();
    symlink(
        external.path().join("outside.txt"),
        source.path().join("linked-outside.txt"),
    )
    .unwrap();

    let destination = TempDir::new().unwrap();
    let mut progress = Vec::new();
    let summary = codex_migrate::operations::export_directory(
        source.path(),
        destination.path(),
        |message| progress.push(message),
    )
    .unwrap();

    let backup = destination.path().join("Codex_backup");
    assert_eq!(summary.skipped_symlink_count, 1);
    assert_eq!(fs::read_to_string(backup.join("regular.txt")).unwrap(), "regular");
    assert!(!backup.join("linked-outside.txt").exists());
    assert!(progress
        .iter()
        .any(|line| line.contains("Skipped symbolic link: linked-outside.txt")));
}
```

- [ ] **Step 2: Run the regression test and verify RED**

Run:

```bash
cargo test --test cli_smoke export_skips_symbolic_links_without_copying_targets --features gui
```

Expected: compilation fails because `ExportSummary` has no `skipped_symlink_count`, proving the new behavior is not implemented.

- [ ] **Step 3: Implement skipped-link handling and contextual errors**

In `src/operations.rs`, import `anyhow::Context`, add the summary field, change the copy result to include a third count, and replace link recreation with a skip:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSummary {
    pub output: String,
    pub thread_count: usize,
    pub skipped_symlink_count: usize,
}

if entry.file_type().is_symlink() {
    skipped_symlink_count += 1;
    progress(format!("Skipped symbolic link: {}", relative.display()));
    continue;
}
```

Return `(file_count, thread_count, skipped_symlink_count)`, populate the public summary, log the final skipped count, remove both platform-specific `copy_symlink` functions, and wrap traversal, directory creation, file copy, and final rename errors with source/destination paths using `with_context`.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
cargo test --test cli_smoke export_skips_symbolic_links_without_copying_targets --features gui
```

Expected: one test passes and the external target is not copied.

- [ ] **Step 5: Run existing export smoke coverage**

Run:

```bash
cargo test --test cli_smoke exports_scans_and_dry_runs_directory_backup --features gui
```

Expected: one test passes; credentials remain excluded and regular Skills, logs, sessions, and SQLite files remain copied.

- [ ] **Step 6: Commit the core fix**

```bash
git add src/operations.rs tests/cli_smoke.rs
git commit -m "Fix portable backups with symbolic links"
```

### Task 2: CLI, GUI, Version, and Source-Tree Synchronization

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/bin/codex-migrate-gui.rs`
- Modify: `Cargo.toml`
- Modify mechanically: `Cargo.lock`
- Apply the same changes to the matching files under `../codex-migrate_V1/`.

**Interfaces:**
- Consumes: `ExportSummary.skipped_symlink_count` from Task 1.
- Produces: bilingual GUI completion detail and CLI output that disclose skipped links.

- [ ] **Step 1: Add a failing CLI output assertion**

Extend the Unix regression test to execute the CLI against a fresh source containing one link and assert:

```rust
.stdout(predicates::str::contains("skipped 1 symbolic link(s)"));
```

- [ ] **Step 2: Run the CLI assertion and verify RED**

Run the focused test from Task 1. Expected: failure because current CLI output does not report skipped links.

- [ ] **Step 3: Update CLI and GUI completion messages**

Append this clause to CLI export output:

```rust
"; skipped {} symbolic link(s)"
```

For GUI completion, keep the session count as the primary message and append a bilingual warning to `detail` when the count is nonzero:

```rust
let detail = if summary.skipped_symlink_count == 0 {
    summary.output
} else {
    format!(
        "{}\n{} {} {}",
        summary.output,
        tr(zh, "已跳过", "Skipped"),
        summary.skipped_symlink_count,
        tr(zh, "个符号链接，详情见日志", "symbolic link(s); see logs for details")
    )
};
```

- [ ] **Step 4: Bump and synchronize version 1.0.8**

Set `version = "1.0.8"` in both `Cargo.toml` files. Apply identical changed Rust and test files to both trees, then run Cargo metadata/build commands so both lockfiles record `codex-migrate` version `1.0.8`.

- [ ] **Step 5: Verify focused tests in both trees**

Run the focused regression test in `codex-migrate` and `codex-migrate_V1`. Expected: both pass and report one skipped symbolic link.

- [ ] **Step 6: Commit public source integration**

```bash
git add Cargo.toml Cargo.lock src/cli.rs src/bin/codex-migrate-gui.rs tests/cli_smoke.rs
git commit -m "Release portable backup fix as 1.0.8"
```

### Task 3: Full Verification

**Files:**
- Verify all modified files in both source trees.

**Interfaces:**
- Consumes: completed Tasks 1 and 2.
- Produces: release-ready evidence for tests, lint, builds, and source synchronization.

- [ ] **Step 1: Format both trees**

Run `cargo fmt --all -- --check` in each tree. Expected: exit code 0.

- [ ] **Step 2: Run Clippy in both trees**

Run `cargo clippy --all-targets --features gui -- -D warnings` in each tree. Expected: exit code 0 with no warnings.

- [ ] **Step 3: Run all tests in both trees**

Run `cargo test --all-targets --features gui` in each tree. Expected: all tests pass with zero failures.

- [ ] **Step 4: Build release binaries in both trees**

Run `cargo build --release --features gui --bins` in each tree. Expected: exit code 0.

- [ ] **Step 5: Verify synchronization and version**

Compare `src/operations.rs`, `src/cli.rs`, `src/bin/codex-migrate-gui.rs`, `tests/cli_smoke.rs`, and package versions between both trees. Expected: relevant contents and version `1.0.8` match.

- [ ] **Step 6: Inspect repository state**

Run `git status --short` and `git log -3 --oneline` in `codex-migrate`. Expected: implementation commits are present and no unintended files are modified.
