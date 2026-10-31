use codex_migrate::{operations, selective_export};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn fixture(root: &Path, id: &str, archived: bool, parent: Option<(&str, u64)>) -> PathBuf {
    let mut meta = json!({"id":id,"timestamp":"2026-10-01T00:00:00Z","cwd":"/old/project",
        "source":"vscode","model_provider":"openai","cli_version":"0.164.0-alpha.11.1",
        "history_mode":"paginated","runtime_workspace_roots":["/old/project"]});
    if let Some((parent, length)) = parent {
        meta["history_base"] =
            json!({"thread_id":parent,"end_ordinal_exclusive":3,"end_byte_offset":length});
    }
    let rows = [
        json!({"type":"session_meta","payload":meta}),
        json!({"type":"event_msg","payload":{"type":"user_message","message":format!("Private message for {id}")}}),
        json!({"type":"event_msg","payload":{"type":"thread_name_updated","thread_name":format!("Title {id}")}}),
    ];
    let folder = root.join(if archived {
        "archived_sessions"
    } else {
        "sessions/2026/10/01"
    });
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("rollout-{id}.jsonl"));
    fs::write(
        &path,
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>(),
    )
    .unwrap();
    path
}

fn ids(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn selected_backup_excludes_unselected_records_and_unrelated_files() {
    let source = TempDir::new().unwrap();
    let a = fixture(source.path(), "chosen", false, None);
    let b = fixture(source.path(), "unselected-secret", false, None);
    let c = fixture(source.path(), "archived", true, None);
    for name in ["auth.json", "config.toml", "logs_2.sqlite", "history.jsonl"] {
        fs::write(source.path().join(name), "unselected-secret credential").unwrap();
    }
    fs::write(source.path().join("session_index.jsonl"),
        "{\"id\":\"chosen\",\"thread_name\":\"Named in sidebar\"}\n{\"id\":\"unselected-secret\",\"thread_name\":\"secret\"}\n").unwrap();
    let originals = [&a, &b, &c].map(|path| fs::read(path).unwrap());
    let destination = TempDir::new().unwrap();
    assert_cmd::Command::cargo_bin("codex-migrate")
        .unwrap()
        .args([
            "export",
            source.path().to_str().unwrap(),
            "--output-parent",
            destination.path().to_str().unwrap(),
            "--thread",
            "chosen",
            "--thread",
            "archived",
        ])
        .assert()
        .success();
    let backup = destination.path().join("Codex_backup");
    let catalog = operations::scan_source(&backup).unwrap();
    assert_eq!(catalog.thread_count, 2);
    let sessions = &catalog.projects[0].sessions;
    assert!(sessions
        .iter()
        .any(|session| session.thread.id == "archived" && session.thread.archived));
    let files = walkdir::WalkDir::new(&backup)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .collect::<Vec<_>>();
    assert_eq!(
        files.len(),
        4,
        "only two rollouts, the index and export metadata belong in the backup"
    );
    for file in files {
        assert!(
            !String::from_utf8_lossy(&fs::read(file.path()).unwrap()).contains("unselected-secret")
        );
    }
    assert!(fs::read_to_string(backup.join("session_index.jsonl"))
        .unwrap()
        .contains("Named in sidebar"));
    for (path, before) in [&a, &b, &c].into_iter().zip(originals) {
        assert_eq!(fs::read(path).unwrap(), before);
    }
}

#[test]
fn fork_export_includes_ancestors_and_can_be_exported_again() {
    let source = TempDir::new().unwrap();
    let parent = fixture(source.path(), "parent", true, None);
    let child = fixture(
        source.path(),
        "child",
        false,
        Some(("parent", parent.metadata().unwrap().len())),
    );
    fixture(source.path(), "unrelated", false, None);
    let catalog = selective_export::scan(source.path()).unwrap();
    let plan = selective_export::plan(&catalog, &ids(&["child"])).unwrap();
    assert_eq!(plan.thread_ids, ["parent", "child"]);
    assert_eq!(plan.dependency_thread_ids, ["parent"]);
    let destination = TempDir::new().unwrap();
    let summary =
        selective_export::export(source.path(), destination.path(), &ids(&["child"]), |_| {})
            .unwrap();
    assert_eq!(
        (
            summary.selected_count,
            summary.dependency_count,
            summary.thread_count
        ),
        (1, 1, 2)
    );
    let backup = PathBuf::from(summary.output);
    assert_eq!(
        fs::read(backup.join("archived_sessions/rollout-parent.jsonl")).unwrap(),
        fs::read(parent).unwrap()
    );
    assert_eq!(
        fs::read(backup.join("sessions/2026/10/01/rollout-child.jsonl")).unwrap(),
        fs::read(child).unwrap()
    );
    let second = TempDir::new().unwrap();
    let repeated =
        selective_export::export(&backup, second.path(), &ids(&["child"]), |_| {}).unwrap();
    assert_eq!(
        operations::scan_source(Path::new(&repeated.output))
            .unwrap()
            .thread_count,
        2
    );
}

#[test]
fn invalid_selection_or_lineage_never_produces_a_backup() {
    let source = TempDir::new().unwrap();
    let path = fixture(source.path(), "child", false, Some(("missing", 10)));
    let destination = TempDir::new().unwrap();
    for selected in [BTreeSet::new(), ids(&["unknown"]), ids(&["child"])] {
        assert!(
            selective_export::export(source.path(), destination.path(), &selected, |_| {}).is_err()
        );
        assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 0);
    }
    let mut rows = fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    rows[0]["payload"]["history_base"]["thread_id"] = json!("child");
    fs::write(
        path,
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>(),
    )
    .unwrap();
    assert!(
        selective_export::export(source.path(), destination.path(), &ids(&["child"]), |_| {})
            .unwrap_err()
            .to_string()
            .contains("cyclic")
    );
}

#[test]
fn selected_backup_detects_changed_records_and_does_not_overwrite_existing_backup() {
    let source = TempDir::new().unwrap();
    fixture(source.path(), "chosen", false, None);
    let destination = TempDir::new().unwrap();
    let summary =
        selective_export::export(source.path(), destination.path(), &ids(&["chosen"]), |_| {})
            .unwrap();
    let backup = PathBuf::from(summary.output);
    let record = backup.join("sessions/2026/10/01/rollout-chosen.jsonl");
    let before = fs::read(&record).unwrap();
    assert!(
        selective_export::export(source.path(), destination.path(), &ids(&["chosen"]), |_| {})
            .unwrap_err()
            .to_string()
            .contains("already exists")
    );
    assert_eq!(fs::read(&record).unwrap(), before);
    fs::write(
        &record,
        String::from_utf8(before)
            .unwrap()
            .replace("Private message", "Changed message"),
    )
    .unwrap();
    assert!(operations::scan_source(&backup)
        .unwrap_err()
        .to_string()
        .contains("checksum mismatch"));
}

#[test]
fn inline_forks_keep_distinct_ids_and_export_without_external_parents() {
    let source = TempDir::new().unwrap();
    let parent = fixture(source.path(), "parent", false, None);
    let inherited = fs::read_to_string(&parent).unwrap();
    let mut originals = Vec::new();
    for id in ["fork-a", "fork-b"] {
        let child = fixture(source.path(), id, false, None);
        let content = fs::read_to_string(&child).unwrap();
        let (header, rest) = content.split_once('\n').unwrap();
        let mut header: Value = serde_json::from_str(header).unwrap();
        header["payload"]["forked_from_id"] = json!("parent");
        let inline = format!("{header}\n{inherited}{rest}");
        fs::write(&child, &inline).unwrap();
        originals.push((child, inline));
    }
    let catalog = selective_export::scan(source.path()).unwrap();
    assert_eq!(catalog.thread_count, 3);
    let selection = selective_export::plan(&catalog, &ids(&["fork-a", "fork-b"])).unwrap();
    assert_eq!(selection.thread_ids, ["fork-a", "fork-b"]);
    assert!(selection.dependency_thread_ids.is_empty());
    let destination = TempDir::new().unwrap();
    let summary = selective_export::export(
        source.path(),
        destination.path(),
        &ids(&["fork-a", "fork-b"]),
        |_| {},
    )
    .unwrap();
    assert_eq!(summary.thread_count, 2);
    let exported = operations::scan_source(Path::new(&summary.output)).unwrap();
    assert_eq!(exported.thread_count, 2);
    for (path, content) in &originals {
        assert_eq!(fs::read_to_string(path).unwrap(), *content);
        let exported_path =
            Path::new(&summary.output).join(path.strip_prefix(source.path()).unwrap());
        assert_eq!(fs::read_to_string(exported_path).unwrap(), *content);
    }
    let second = TempDir::new().unwrap();
    assert_eq!(
        selective_export::export(
            Path::new(&summary.output),
            second.path(),
            &ids(&["fork-a"]),
            |_| {}
        )
        .unwrap()
        .thread_count,
        1
    );
}

#[test]
fn genuinely_duplicate_rollout_ids_still_block_export() {
    let source = TempDir::new().unwrap();
    let original = fixture(source.path(), "same-id", false, None);
    let archived = source.path().join("archived_sessions");
    fs::create_dir(&archived).unwrap();
    fs::copy(original, archived.join("duplicate.jsonl")).unwrap();
    let destination = TempDir::new().unwrap();
    let error = selective_export::export(
        source.path(),
        destination.path(),
        &ids(&["same-id"]),
        |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("duplicate thread IDs"));
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 0);
}
