use codex_migrate::{model::PlatformKind, path_mapper, rollout};
#[cfg(unix)]
use codex_migrate::{
    model::{ImportOptions, ImportTarget},
    operations,
};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
#[cfg(unix)]
use std::{collections::BTreeSet, fs, path::Path};

#[test]
fn source_path_case_rules_survive_cross_platform_mapping() {
    let mappings = BTreeMap::from([
        (r"C:\Users\Alice\Work".into(), "/home/alice/work".into()),
        (r"\\server\Share".into(), "/home/alice/share".into()),
    ]);
    assert_eq!(
        path_mapper::map_explicit(
            r"\\?\c:\users\ALICE\work\Demo",
            &mappings,
            &PlatformKind::Linux
        ),
        Some(PathBuf::from("/home/alice/work/Demo"))
    );
    assert_eq!(
        path_mapper::map_explicit(
            r"\\?\UNC\SERVER\share\demo",
            &mappings,
            &PlatformKind::Linux
        ),
        Some(PathBuf::from("/home/alice/share/demo"))
    );
    assert!(
        path_mapper::map_explicit("C:/Users/Alice/Workspace", &mappings, &PlatformKind::Linux)
            .is_none()
    );
    let reverse = BTreeMap::from([("/home/alice/Work".into(), "D:/Work".into())]);
    assert!(
        path_mapper::map_explicit("/home/alice/work", &reverse, &PlatformKind::Windows).is_none()
    );
    assert_eq!(
        path_mapper::map_explicit("/home/alice/Work/demo", &reverse, &PlatformKind::Windows),
        Some(PathBuf::from("D:/Work/demo"))
    );
}

#[test]
fn cli_conversion_is_repeatable_and_preserves_conversation_and_comparison() {
    let original = format!(
        "{}\n{}\n",
        json!({"type":"session_meta","payload":{"source":"vscode","cwd":"C:/Work","thread_source":"user","id":"test"}}),
        json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Keep C:\\Work unchanged"}]}})
    );
    let converted =
        rollout::prepare_runtime_history(original.as_bytes(), Some("local-provider"), true)
            .unwrap();
    assert_eq!(
        rollout::prepare_runtime_history(&converted, Some("local-provider"), true).unwrap(),
        converted
    );
    assert_eq!(
        rollout::canonicalize_cwd(original.as_bytes()),
        rollout::canonicalize_cwd(&converted)
    );
    let lines = std::str::from_utf8(&converted)
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(lines[0]).unwrap()["payload"]["source"],
        "cli"
    );
    assert_eq!(lines[1], original.lines().nth(1).unwrap());
}

#[cfg(unix)]
#[test]
fn windows_permissions_and_globs_map_on_linux() {
    let root = tempfile::tempdir().unwrap();
    let mappings = BTreeMap::from([("C:/Work".into(), root.path().to_string_lossy().into_owned())]);
    let original = json!({"type":"turn_context","payload":{"cwd":"c:/WORK","runtime_workspace_roots":["C:/Work"],"sandbox_policy":{"file_system":{"entries":[{"path":{"type":"glob_pattern","pattern":r"\\?\C:\Work\output\**\*.png"}}]}}}}).to_string();
    assert!(rollout::workspace_paths(original.as_bytes())
        .unwrap()
        .contains("C:/Work/output"));
    let (bytes, _) =
        rollout::rewrite_workspace_bytes(original.as_bytes(), root.path(), &mappings, false)
            .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value["payload"]["sandbox_policy"]["file_system"]["entries"][0]["path"]["pattern"],
        format!("{}/output/**/*.png", root.path().display())
    );
    assert_eq!(
        rollout::canonicalize_cwd(original.as_bytes()),
        rollout::canonicalize_cwd(&bytes)
    );
    assert!(
        path_mapper::validate_directory_destination(Path::new("//server/share/project")).is_err()
    );
}

#[cfg(unix)]
#[test]
fn cli_plan_resolves_symlinks_and_activates_only_selected_sessions() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let backup = root.path().join("backup");
    fs::create_dir_all(backup.join("archived_sessions")).unwrap();
    let parent = "11111111-1111-4111-8111-111111111111";
    let child = "22222222-2222-4222-8222-222222222222";
    for (id, base) in [(parent, None), (child, Some(parent))] {
        let mut payload = json!({"id":id,"cwd":"C:/Work","source":"vscode","history_mode":"paginated","timestamp":"2026-10-01T00:00:00Z"});
        if let Some(base) = base {
            payload["history_base"] =
                json!({"thread_id":base,"end_ordinal_exclusive":1,"end_byte_offset":0});
        }
        fs::write(
            backup.join("archived_sessions").join(format!("{id}.jsonl")),
            json!({"type":"session_meta","ordinal":0,"payload":payload}).to_string(),
        )
        .unwrap();
    }
    let real = root.path().join("real");
    fs::create_dir(&real).unwrap();
    let alias = root.path().join("alias");
    symlink(&real, &alias).unwrap();
    let target = root.path().join("target");
    let options = ImportOptions {
        target: ImportTarget::Cli,
        selected_thread_ids: BTreeSet::from([child.into()]),
        mappings: BTreeMap::from([(
            "c:/WORK".into(),
            alias.join("new project").to_string_lossy().into_owned(),
        )]),
        ..Default::default()
    };
    let plan = operations::plan_directory_import(root.path(), Some(&target), &options).unwrap();
    assert_eq!(plan.dependency_thread_ids, [parent]);
    assert!(plan.threads[0].thread.archived);
    assert!(!plan.threads[0].resume_in_cli);
    assert!(!plan.threads[1].thread.archived);
    assert!(plan.threads[1].resume_in_cli);
    assert_eq!(
        plan.threads[1].mapped_cwd,
        real.join("new project").to_string_lossy()
    );
    assert!(!target.exists());
    assert!(!real.join("new project").exists());
}
