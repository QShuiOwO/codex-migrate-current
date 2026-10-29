#![cfg(windows)]

use codex_migrate::{path_mapper, rollout, sqlite_adapter};
use rusqlite::{params, Connection};
use serde_json::json;
use std::{fs, path::PathBuf};

#[test]
fn database_and_rollout_accept_native_windows_paths_but_reject_other_directories() {
    let root = tempfile::tempdir().unwrap();
    let work = root.path().join("project");
    let other = root.path().join("other");
    fs::create_dir(&work).unwrap();
    fs::create_dir(&other).unwrap();
    let native = work.canonicalize().unwrap();
    assert!(native.to_string_lossy().starts_with(r"\\?\"));
    let expected = PathBuf::from(path_mapper::normalize(&work.to_string_lossy()));
    let record = root.path().join("rollout.jsonl");
    let bytes = ["session_meta", "turn_context"]
        .map(|kind| json!({"type":kind,"payload":{"cwd":native}}).to_string())
        .join("\n");
    fs::write(&record, bytes).unwrap();
    let database = root.path().join("state_5.sqlite");
    let db = Connection::open(&database).unwrap();
    db.execute_batch(
        "CREATE TABLE threads (id TEXT PRIMARY KEY, cwd TEXT, rollout_path TEXT, preview TEXT);",
    )
    .unwrap();
    db.execute(
        "INSERT INTO threads VALUES (?1, ?2, ?3, ?4)",
        params![
            "test",
            native.to_string_lossy(),
            record.to_string_lossy(),
            "synthetic"
        ],
    )
    .unwrap();
    drop(db);
    sqlite_adapter::validate_visible_thread(&database, "test", &expected).unwrap();
    rollout::validate_cwd_file(&record, &expected).unwrap();
    assert!(sqlite_adapter::validate_visible_thread(&database, "test", &other).is_err());
    assert!(rollout::validate_cwd_file(&record, &other).is_err());
}
