use crate::model::{ExportManifest, ScannedThread, SourceCatalog};
use crate::{discovery, merge, operations, scanner, session_index};
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST_NAME: &str = "codex-migrate-export.json";

#[derive(Debug, Serialize)]
pub struct ExportSelection {
    pub thread_ids: Vec<String>,
    pub dependency_thread_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SelectedExportSummary {
    pub output: String,
    pub selected_count: usize,
    pub dependency_count: usize,
    pub thread_count: usize,
}

fn scan_threads(source: &Path) -> Result<(PathBuf, Vec<ScannedThread>)> {
    let source = operations::resolve_codex_root(source)?;
    // Only use the configured external SQLite home for the local data root.
    // A backup selected elsewhere must use its own metadata.
    let local = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|path| path.join(".codex")));
    let is_local = local
        .and_then(|path| path.canonicalize().ok())
        .is_some_and(|path| source.canonicalize().ok().as_ref() == Some(&path));
    let sqlite_root = if is_local {
        std::env::var_os("CODEX_SQLITE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| source.clone())
    } else {
        source.clone()
    };
    let database = discovery::find_state_db(&sqlite_root)?;
    let threads = scanner::scan_codex_home(&source, database.as_deref())?;
    Ok((source, threads))
}

pub fn scan(source: &Path) -> Result<SourceCatalog> {
    let (root, threads) = scan_threads(source)?;
    Ok(operations::build_catalog(&root, &threads))
}

pub fn plan(catalog: &SourceCatalog, selected: &BTreeSet<String>) -> Result<ExportSelection> {
    let records = catalog
        .projects
        .iter()
        .flat_map(|project| &project.sessions)
        .map(|session| (session.thread.id.clone(), &session.thread))
        .collect::<HashMap<_, _>>();
    if records.len() != catalog.thread_count {
        bail!(
            "source contains duplicate thread IDs; resolve duplicate rollout files before export"
        );
    }
    let thread_ids = merge::history_dependency_order(&records, selected)?;
    let dependency_thread_ids = thread_ids
        .iter()
        .filter(|id| !selected.contains(*id))
        .cloned()
        .collect();
    Ok(ExportSelection {
        thread_ids,
        dependency_thread_ids,
    })
}

pub fn export(
    source: &Path,
    output_parent: &Path,
    selected: &BTreeSet<String>,
    mut progress: impl FnMut(String),
) -> Result<SelectedExportSummary> {
    if selected.is_empty() {
        bail!("select at least one conversation to export");
    }
    let source_root = operations::resolve_codex_root(source)?;
    discovery::ensure_codex_stopped(&source_root)?;
    let (_, threads) = scan_threads(&source_root)?;
    let catalog = operations::build_catalog(&source_root, &threads);
    let selection = plan(&catalog, selected)?;
    let source_threads = threads
        .iter()
        .map(|thread| (thread.record.id.as_str(), thread))
        .collect::<HashMap<_, _>>();
    let source_index = session_index::load(&source_root)?;

    fs::create_dir_all(output_parent)
        .with_context(|| format!("create export parent {}", output_parent.display()))?;
    if output_parent
        .canonicalize()?
        .starts_with(source_root.canonicalize()?)
    {
        bail!(
            "backup destination cannot be inside source {}",
            source_root.display()
        );
    }
    let output = output_parent.join("Codex_backup");
    if output.exists() {
        bail!(
            "{} already exists; choose another export folder",
            output.display()
        );
    }
    let staging = tempfile::Builder::new()
        .prefix(".Codex_backup.exporting-")
        .tempdir_in(output_parent)?;
    let mut manifest = ExportManifest {
        format_version: 1,
        producer_version: env!("CARGO_PKG_VERSION").to_owned(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        selected_thread_ids: selected.clone(),
        dependency_thread_ids: selection.dependency_thread_ids.clone(),
        threads: Vec::new(),
    };
    let mut index = String::new();
    for id in &selection.thread_ids {
        let thread = source_threads[id.as_str()];
        let relative = Path::new(&thread.record.archive_path);
        if !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
            || !(relative.starts_with("sessions") || relative.starts_with("archived_sessions"))
        {
            return Err(anyhow!("invalid rollout archive path for {id}"));
        }
        let destination = staging.path().join(relative);
        fs::create_dir_all(destination.parent().unwrap())?;
        // Write the scanned bytes so the manifest hash and ancestor offsets describe
        // exactly the exported snapshot, even if the source file subsequently changes.
        fs::write(&destination, &thread.content)
            .with_context(|| format!("export conversation {id}"))?;
        let thread_name = source_index
            .get(id)
            .and_then(|entry| entry.get("thread_name"))
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&thread.record.title);
        let updated_at = chrono::DateTime::from_timestamp(thread.record.updated_at, 0)
            .unwrap_or_default()
            .to_rfc3339();
        index.push_str(&serde_json::to_string(&serde_json::json!({
            "id": id, "thread_name": thread_name, "updated_at": updated_at
        }))?);
        index.push('\n');
        manifest.threads.push(thread.record.clone());
        progress(format!("Exported {} ({id})", thread.record.title));
    }
    fs::write(staging.path().join("session_index.jsonl"), index)?;
    fs::write(
        staging.path().join(MANIFEST_NAME),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    // Read the finished bundle through the same scanner used for import.
    let verified = scanner::scan_codex_home(staging.path(), None)?;
    if verified.len() != selection.thread_ids.len() {
        bail!("export verification returned an unexpected conversation count");
    }
    fs::rename(staging.path(), &output)
        .with_context(|| format!("finalize selected export at {}", output.display()))?;
    progress(format!("Created {}", output.display()));
    Ok(SelectedExportSummary {
        output: output.to_string_lossy().into_owned(),
        selected_count: selected.len(),
        dependency_count: selection.dependency_thread_ids.len(),
        thread_count: selection.thread_ids.len(),
    })
}
