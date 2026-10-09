use crate::discovery::Environment;
use crate::model::{
    ImportOptions, ImportPlan, MergeAction, PlannedThread, ScannedThread, SourceCatalog,
};
use crate::path_mapper::{history_only_path, map_explicit, normalize};
use crate::rollout;
use crate::scanner;
use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub fn build_plan(
    catalog: &SourceCatalog,
    source_threads: &[ScannedThread],
    environment: &Environment,
    options: &ImportOptions,
) -> Result<ImportPlan> {
    let existing =
        scanner::scan_codex_home(&environment.codex_home, environment.state_db.as_deref())?;
    let existing = existing
        .into_iter()
        .map(|thread| (thread.record.id.clone(), thread))
        .collect::<HashMap<_, _>>();
    let mut source = source_threads
        .iter()
        .map(|thread| (thread.record.id.clone(), thread))
        .collect::<HashMap<_, _>>();
    let source_ids = source.keys().cloned().collect::<HashSet<_>>();
    for (id, thread) in &existing {
        source.entry(id.clone()).or_insert(thread);
    }
    let order = dependency_order(&source, &options.selected_thread_ids)?;
    let dependency_thread_ids = order
        .iter()
        .filter(|id| !options.selected_thread_ids.contains(*id))
        .cloned()
        .collect();
    let mut threads = Vec::new();

    for thread_id in &order {
        let source_thread = source
            .get(thread_id)
            .ok_or_else(|| anyhow!("selected thread does not exist in source: {thread_id}"))?;
        let original_cwd = normalize(&source_thread.record.cwd);
        let implicit = !options.selected_thread_ids.contains(thread_id);
        let mapped =
            map_explicit(&original_cwd, &options.mappings, &environment.platform).or_else(|| {
                Path::new(&original_cwd)
                    .is_dir()
                    .then(|| PathBuf::from(&original_cwd))
            });
        let history_only =
            options.history_only_projects.contains(&original_cwd) || (implicit && mapped.is_none());
        let mapped_cwd = if history_only {
            history_only_path(
                &environment.codex_home.join("migration_history"),
                &original_cwd,
            )
        } else {
            mapped.ok_or_else(|| anyhow!("selected project is not mapped: {original_cwd}"))?
        };
        let (action, mut reason) = match existing.get(thread_id) {
            None => (MergeAction::Import, "thread UUID is not present".to_owned()),
            Some(target) if target.record.sha256 == source_thread.record.sha256 => (
                MergeAction::SkipIdentical,
                "source and target hashes match".to_owned(),
            ),
            Some(target) => compare_contents(&source_thread.content, &target.content),
        };
        if implicit {
            reason = format!("required history_base ancestor; {reason}");
        }
        let target_path = match (&action, existing.get(thread_id)) {
            (MergeAction::SkipIdentical | MergeAction::KeepTargetLonger, Some(existing_thread)) => {
                existing_thread.source_path.clone()
            }
            _ => environment
                .codex_home
                .join(&source_thread.record.archive_path),
        };
        let mut record = if action == MergeAction::KeepTargetLonger {
            existing[thread_id].record.clone()
        } else {
            source_thread.record.clone()
        };
        record.cwd = source_thread.record.cwd.clone();
        // Keep UI metadata from the longer target, but project identity belongs
        // to the source selection so repeating an import uses the same key.
        if let Some(project) = source_thread.record.extra.get("source_project") {
            record
                .extra
                .insert("source_project".to_owned(), project.clone());
        } else {
            record.extra.remove("source_project");
        }
        if implicit && !source_ids.contains(thread_id) {
            record.extra.insert(
                "_migrate_existing_dependency".to_owned(),
                serde_json::Value::Bool(true),
            );
        }
        threads.push(PlannedThread {
            thread: record,
            source_path: source_thread.source_path.to_string_lossy().into_owned(),
            mapped_cwd: mapped_cwd.to_string_lossy().into_owned(),
            history_only,
            target_path: target_path.to_string_lossy().into_owned(),
            action,
            reason,
        });
    }
    // Keep topological order: register ancestors before their forks.
    let conflicts = threads
        .iter()
        .filter(|thread| thread.action == MergeAction::Conflict)
        .count();
    Ok(ImportPlan {
        source_codex_home: catalog.source_codex_home.clone(),
        codex_home: environment.codex_home.to_string_lossy().into_owned(),
        threads,
        conflicts,
        dependency_thread_ids,
        mappings: options.mappings.clone(),
    })
}

fn dependency_order(
    source: &HashMap<String, &ScannedThread>,
    selected: &std::collections::BTreeSet<String>,
) -> Result<Vec<String>> {
    fn visit(
        id: &str,
        source: &HashMap<String, &ScannedThread>,
        visiting: &mut HashSet<String>,
        done: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<()> {
        if done.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.to_owned()) {
            anyhow::bail!("cyclic history_base lineage at {id}");
        }
        let thread = source
            .get(id)
            .ok_or_else(|| anyhow!("missing history_base ancestor or selected thread: {id}"))?;
        if let Some(parent) = thread.record.history_parent() {
            visit(parent, source, visiting, done, order)?;
        }
        visiting.remove(id);
        done.insert(id.to_owned());
        order.push(id.to_owned());
        Ok(())
    }
    let mut order = Vec::new();
    let mut visiting = HashSet::new();
    let mut done = HashSet::new();
    for id in selected {
        visit(id, source, &mut visiting, &mut done, &mut order)?;
    }
    Ok(order)
}

fn compare_contents(source: &[u8], target: &[u8]) -> (MergeAction, String) {
    if let (Some(source), Some(target)) = (
        rollout::canonicalize_cwd(source),
        rollout::canonicalize_cwd(target),
    ) {
        if source == target {
            return (
                MergeAction::SkipIdentical,
                "source and target differ only by mapped cwd metadata".to_owned(),
            );
        }
        if source.starts_with(&target) {
            return (
                MergeAction::ReplaceWithLonger,
                "target rollout is a normalized prefix of source".to_owned(),
            );
        }
        if target.starts_with(&source) {
            return (
                MergeAction::KeepTargetLonger,
                "source rollout is a normalized prefix of target".to_owned(),
            );
        }
    }
    if source.starts_with(target) {
        (
            MergeAction::ReplaceWithLonger,
            "target rollout is a byte-for-byte prefix of source".to_owned(),
        )
    } else if target.starts_with(source) {
        (
            MergeAction::KeepTargetLonger,
            "source rollout is a byte-for-byte prefix of target".to_owned(),
        )
    } else {
        (
            MergeAction::Conflict,
            "same UUID has divergent rollout content".to_owned(),
        )
    }
}

pub fn selected_source_bytes<'a>(
    source_threads: &'a [ScannedThread],
    source_path: &Path,
) -> Option<&'a [u8]> {
    source_threads
        .iter()
        .find(|thread| thread.source_path == source_path)
        .map(|thread| thread.content.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_prefix_and_conflict() {
        assert_eq!(
            compare_contents(b"abc", b"ab").0,
            MergeAction::ReplaceWithLonger
        );
        assert_eq!(
            compare_contents(b"ab", b"abc").0,
            MergeAction::KeepTargetLonger
        );
        assert_eq!(compare_contents(b"abc", b"abd").0, MergeAction::Conflict);
        assert_eq!(
            compare_contents(
                br#"{"type":"session_meta","payload":{"cwd":"/old"}}"#,
                br#"{"type":"session_meta","payload":{"cwd":"/new"}}"#
            )
            .0,
            MergeAction::SkipIdentical
        );
    }

    #[test]
    fn lineage_is_topological_and_rejects_missing_or_cyclic_ancestors() {
        let home = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(home.path().join("sessions")).unwrap();
        for (id, parent) in [("parent", None), ("child", Some("parent"))] {
            let mut meta = serde_json::json!({"id":id,"cwd":"/old","history_mode":"paginated"});
            if let Some(parent) = parent {
                meta["history_base"] = serde_json::json!({"thread_id":parent});
            }
            std::fs::write(
                home.path().join("sessions").join(format!("{id}.jsonl")),
                serde_json::json!({"type":"session_meta","payload":meta}).to_string(),
            )
            .unwrap();
        }
        let mut threads = scanner::scan_codex_home(home.path(), None).unwrap();
        let selected = std::collections::BTreeSet::from(["child".to_owned()]);
        let map = threads.iter().map(|t| (t.record.id.clone(), t)).collect();
        assert_eq!(
            dependency_order(&map, &selected).unwrap(),
            ["parent", "child"]
        );
        let missing = threads
            .iter()
            .filter(|t| t.record.id != "parent")
            .map(|t| (t.record.id.clone(), t))
            .collect();
        assert!(dependency_order(&missing, &selected)
            .unwrap_err()
            .to_string()
            .contains("missing"));
        threads
            .iter_mut()
            .find(|t| t.record.id == "parent")
            .unwrap()
            .record
            .extra
            .insert(
                "history_base".to_owned(),
                serde_json::json!({"thread_id":"child"}),
            );
        let cyclic = threads.iter().map(|t| (t.record.id.clone(), t)).collect();
        assert!(dependency_order(&cyclic, &selected)
            .unwrap_err()
            .to_string()
            .contains("cyclic"));
    }
}
