use crate::app_server;
use crate::desktop_state;
use crate::discovery::{self, Environment};
use crate::html_export::{self, HtmlExportSummary};
use crate::merge;
use crate::model::{
    DiagnosticReport, ImportOptions, ImportPlan, MergeAction, PlatformKind, ScannedThread,
    SourceCatalog, SourceProject, SourceSession,
};
use crate::path_mapper::{map_explicit, normalize};
use crate::rollout;
use crate::scanner;
use crate::session_index;
use crate::sqlite_adapter;
use crate::transaction::{self, ImportTransaction};
use crate::validator;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSummary {
    pub output: String,
    pub thread_count: usize,
    pub skipped_symlink_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub transaction_id: String,
    pub imported: usize,
    pub refreshed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub migration_check: DiagnosticReport,
    pub codex_doctor: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub id: String,
    pub created_at: String,
    pub source_codex_home: String,
    pub completed: bool,
}

pub fn diagnose(codex_home: Option<&Path>) -> Result<DiagnosticReport> {
    let environment = discovery::discover(codex_home)?;
    validator::diagnose(&environment)
}

pub fn resolve_codex_root(selected: &Path) -> Result<PathBuf> {
    if is_codex_root(selected) {
        return Ok(selected.to_path_buf());
    }
    let candidates = ["Codex_backup", "Codex", ".codex"]
        .into_iter()
        .map(|name| selected.join(name))
        .filter(|path| is_codex_root(path))
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(anyhow!(
            "{} is not a Codex folder and contains no Codex_backup/Codex/.codex child",
            selected.display()
        )),
        _ => Err(anyhow!(
            "{} contains multiple Codex backup folders; select the intended folder directly",
            selected.display()
        )),
    }
}

pub fn scan_source(selected: &Path) -> Result<SourceCatalog> {
    let source_root = resolve_codex_root(selected)?;
    let state_db = discovery::find_state_db(&source_root)?;
    let threads = scanner::scan_codex_home(&source_root, state_db.as_deref())?;
    if threads.is_empty() {
        return Err(anyhow!(
            "no session rollout files found in {}",
            source_root.display()
        ));
    }
    Ok(build_catalog(&source_root, &threads))
}

pub fn scan_local(codex_home: Option<&Path>) -> Result<SourceCatalog> {
    let environment = discovery::discover(codex_home)?;
    scan_source(&environment.codex_home)
}

pub fn rebind_existing(
    codex_home: Option<&Path>,
    options: &ImportOptions,
    progress: impl FnMut(String),
) -> Result<ImportSummary> {
    let environment = discovery::discover(codex_home)?;
    import_directory(
        &environment.codex_home,
        Some(&environment.codex_home),
        options,
        progress,
    )
}

pub fn export_html(
    codex_home: Option<&Path>,
    selected_ids: &std::collections::BTreeSet<String>,
    output_root: Option<&Path>,
) -> Result<HtmlExportSummary> {
    let environment = discovery::discover(codex_home)?;
    let threads =
        scanner::scan_codex_home(&environment.codex_home, environment.state_db.as_deref())?;
    html_export::export_threads(&threads, selected_ids, output_root)
}

pub fn export_directory(
    source: &Path,
    output_parent: &Path,
    mut progress: impl FnMut(String),
) -> Result<ExportSummary> {
    let source_root = resolve_codex_root(source)?;
    discovery::ensure_codex_stopped(&source_root)?;
    fs::create_dir_all(output_parent).with_context(|| {
        format!(
            "failed to create backup parent directory {}",
            output_parent.display()
        )
    })?;

    let source_canonical = source_root
        .canonicalize()
        .with_context(|| format!("failed to resolve source {}", source_root.display()))?;
    let output_parent_canonical = output_parent.canonicalize().with_context(|| {
        format!(
            "failed to resolve backup parent {}",
            output_parent.display()
        )
    })?;
    if output_parent_canonical.starts_with(&source_canonical) {
        return Err(anyhow!(
            "backup destination {} cannot be inside source {}",
            output_parent.display(),
            source_root.display()
        ));
    }

    let output = output_parent.join("Codex_backup");
    if output.exists() {
        return Err(anyhow!(
            "{} already exists; choose another parent folder or remove the existing backup",
            output.display()
        ));
    }

    let staging = output_parent.join(format!(
        ".Codex_backup.exporting-{}",
        uuid::Uuid::new_v4().simple()
    ));
    progress(format!(
        "Copying all contents from {}",
        source_root.display()
    ));
    let copy_result = copy_codex_home(&source_root, &staging, &mut progress);
    let (file_count, thread_count, skipped_symlink_count) = match copy_result {
        Ok(summary) => summary,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&staging, &output) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error).with_context(|| {
            format!(
                "failed to finalize backup by renaming {} to {}",
                staging.display(),
                output.display()
            )
        });
    }
    progress(format!("Created {}", output.display()));
    progress(format!("Copied {file_count} files"));
    progress(format!("Skipped {skipped_symlink_count} symbolic links"));
    Ok(ExportSummary {
        output: output.to_string_lossy().into_owned(),
        thread_count,
        skipped_symlink_count,
    })
}

fn copy_codex_home(
    source_root: &Path,
    destination_root: &Path,
    progress: &mut impl FnMut(String),
) -> Result<(usize, usize, usize)> {
    fs::create_dir_all(destination_root).with_context(|| {
        format!(
            "failed to create backup staging directory {}",
            destination_root.display()
        )
    })?;
    let mut file_count = 0;
    let mut thread_count = 0;
    let mut skipped_symlink_count = 0;

    for entry in WalkDir::new(source_root).follow_links(false) {
        let entry = entry.with_context(|| {
            format!("failed to traverse Codex source {}", source_root.display())
        })?;
        let relative = entry.path().strip_prefix(source_root).with_context(|| {
            format!(
                "failed to derive relative path for {} from {}",
                entry.path().display(),
                source_root.display()
            )
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if is_login_credential_path(relative) {
            continue;
        }
        let destination = destination_root.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination).with_context(|| {
                format!(
                    "failed to create backup directory {} for source {}",
                    destination.display(),
                    entry.path().display()
                )
            })?;
            continue;
        }
        if entry.file_type().is_symlink() {
            skipped_symlink_count += 1;
            progress(format!("Skipped symbolic link: {}", relative.display()));
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create backup parent {} for source {}",
                    parent.display(),
                    entry.path().display()
                )
            })?;
        }
        if entry.file_type().is_file() {
            fs::copy(entry.path(), &destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    entry.path().display(),
                    destination.display()
                )
            })?;
            file_count += 1;
            if relative.extension().and_then(|value| value.to_str()) == Some("jsonl")
                && (relative.starts_with("sessions") || relative.starts_with("archived_sessions"))
            {
                thread_count += 1;
            }
            if file_count % 100 == 0 {
                progress(format!("Copied {file_count} files"));
            }
        }
    }

    Ok((file_count, thread_count, skipped_symlink_count))
}

fn is_login_credential_path(relative: &Path) -> bool {
    if relative.components().count() != 1 {
        return false;
    }
    matches!(
        relative
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("auth.json") | Some("auth.json.bak") | Some("credentials.json") | Some("tokens.json")
    )
}

pub fn plan_directory_import(
    source: &Path,
    codex_home: Option<&Path>,
    options: &ImportOptions,
) -> Result<ImportPlan> {
    crate::diagnostics::run(
        "preview",
        source,
        codex_home,
        options,
        &mut |_| {},
        |progress| plan_directory_import_inner(source, codex_home, options, progress),
    )
}

fn plan_directory_import_inner(
    source: &Path,
    codex_home: Option<&Path>,
    options: &ImportOptions,
    progress: &mut dyn FnMut(String),
) -> Result<ImportPlan> {
    let source_root = resolve_codex_root(source)?;
    let state_db = discovery::find_state_db(&source_root)?;
    let source_threads = scanner::scan_codex_home(&source_root, state_db.as_deref())?;
    let catalog = build_catalog(&source_root, &source_threads);
    let environment = discovery::discover(codex_home)?;
    report_environment(&environment, progress);
    validate_options(&catalog, options)?;
    merge::build_plan(&catalog, &source_threads, &environment, options)
}

pub fn import_directory(
    source: &Path,
    codex_home: Option<&Path>,
    options: &ImportOptions,
    mut progress: impl FnMut(String),
) -> Result<ImportSummary> {
    crate::diagnostics::run(
        "import",
        source,
        codex_home,
        options,
        &mut progress,
        |progress| import_directory_inner(source, codex_home, options, progress),
    )
}

fn import_directory_inner(
    source: &Path,
    codex_home: Option<&Path>,
    options: &ImportOptions,
    mut progress: impl FnMut(String),
) -> Result<ImportSummary> {
    let source_root = resolve_codex_root(source)?;
    let state_db = discovery::find_state_db(&source_root)?;
    progress("Scanning source Codex folder".to_owned());
    let source_threads = scanner::scan_codex_home(&source_root, state_db.as_deref())?;
    let catalog = build_catalog(&source_root, &source_threads);
    validate_options(&catalog, options)?;
    let initial_environment = discovery::discover(codex_home)?;
    report_environment(&initial_environment, &mut progress);
    discovery::ensure_codex_stopped(&initial_environment.codex_home)?;
    let plan = merge::build_plan(&catalog, &source_threads, &initial_environment, options)?;
    if plan.conflicts > 0 {
        return Err(anyhow!(
            "import contains {} selected divergent UUID conflict(s)",
            plan.conflicts
        ));
    }
    execute_import(
        &initial_environment,
        &source_root,
        &source_threads,
        &plan,
        &mut progress,
    )
}

fn report_environment(environment: &Environment, progress: &mut dyn FnMut(String)) {
    progress(format!(
        "Runtime={} version={} CODEX_HOME={} CODEX_SQLITE_HOME={} state_db={} schema={:?}",
        environment
            .codex_executable
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "not found".into()),
        environment.codex_version.as_deref().unwrap_or("unknown"),
        environment.codex_home.display(),
        environment.sqlite_home.display(),
        environment
            .state_db
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "none".into()),
        environment.schema_version,
    ));
}

pub fn verify(codex_home: Option<&Path>) -> Result<VerificationReport> {
    let environment = discovery::discover(codex_home)?;
    let report = validator::diagnose(&environment)?;
    let codex_doctor = validator::run_codex_doctor(&environment)?;
    if !report.issues.is_empty() {
        anyhow::bail!("verification found {} issue(s)", report.issues.len());
    }
    Ok(VerificationReport {
        migration_check: report,
        codex_doctor,
    })
}

pub fn list_transactions(codex_home: Option<&Path>) -> Result<Vec<TransactionSummary>> {
    let environment = discovery::discover(codex_home)?;
    let root = environment.codex_home.join("migration_transactions");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut transactions = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let record_path = entry.path().join("transaction.json");
        if !record_path.is_file() {
            continue;
        }
        let record: crate::model::TransactionRecord =
            serde_json::from_slice(&fs::read(record_path)?)?;
        transactions.push(TransactionSummary {
            id: record.id,
            created_at: record.created_at,
            source_codex_home: record.source_codex_home,
            completed: record.completed,
        });
    }
    transactions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(transactions)
}

pub fn rollback(codex_home: Option<&Path>, transaction_id: &str) -> Result<()> {
    let environment = discovery::discover(codex_home)?;
    discovery::ensure_codex_stopped(&environment.codex_home)?;
    if let Some(state_db) = environment.state_db.as_deref() {
        sqlite_adapter::check_write_lock(state_db)?;
    }
    transaction::rollback_by_id(&environment.codex_home, transaction_id)
}

pub fn delete_transactions(codex_home: Option<&Path>, transaction_ids: &[String]) -> Result<usize> {
    let environment = discovery::discover(codex_home)?;
    transaction::delete_by_ids(&environment.codex_home, transaction_ids)
}

fn execute_import(
    initial_environment: &Environment,
    source_root: &Path,
    source_threads: &[ScannedThread],
    plan: &ImportPlan,
    progress: &mut impl FnMut(String),
) -> Result<ImportSummary> {
    let mut environment = initial_environment.clone();
    let modern = plan.threads.iter().any(|t| t.thread.paginated())
        || environment
            .state_db
            .as_deref()
            .map(sqlite_adapter::has_projects)
            .transpose()?
            .unwrap_or(false);
    if environment.codex_executable.is_none() && (modern || environment.state_db.is_none()) {
        anyhow::bail!("current native Codex runtime is required; set CODEX_MIGRATE_CODEX_BIN to its codex.exe path");
    }
    fs::create_dir_all(&environment.codex_home)?;
    fs::create_dir_all(&environment.sqlite_home)?;
    if let Some(state_db) = environment.state_db.as_deref() {
        sqlite_adapter::check_write_lock(state_db)?;
    }
    progress("Creating rollback snapshot".to_owned());
    let mut transaction = ImportTransaction::begin(
        &environment.codex_home,
        &environment.sqlite_home,
        source_root,
    )?;
    progress(format!("Transaction={}", transaction.record.id));
    transaction.snapshot_databases(&[
        environment.codex_home.clone(),
        environment.sqlite_home.clone(),
        environment.codex_home.join("sqlite"),
    ])?;

    let result = (|| {
        let staging = transaction.root.join("staging");
        fs::create_dir_all(&staging)?;
        let existing_threads =
            scanner::scan_codex_home(&environment.codex_home, environment.state_db.as_deref())?;
        let existing_by_id = existing_threads
            .iter()
            .map(|thread| (thread.record.id.clone(), thread.source_path.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut invalidated = plan
            .threads
            .iter()
            .filter(|t| t.thread.paginated())
            .map(|t| t.thread.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        loop {
            let before = invalidated.len();
            for thread in &existing_threads {
                if thread
                    .record
                    .history_parent()
                    .is_some_and(|id| invalidated.contains(id))
                {
                    invalidated.insert(thread.record.id.clone());
                }
            }
            if before == invalidated.len() {
                break;
            }
        }
        sqlite_adapter::invalidate_history(
            &environment.sqlite_home.join("thread_history_1.sqlite"),
            &invalidated,
        )?;
        let mut server = match environment.codex_executable.as_deref() {
            Some(codex) => match app_server::AppServer::start(
                codex,
                &environment.codex_home,
                &environment.sqlite_home,
            ) {
                Ok(server) => Some(server),
                Err(error) if !modern && environment.state_db.is_some() => {
                    eprintln!("warning: legacy SQLite fallback: {error:#}");
                    None
                }
                Err(error) => return Err(error),
            },
            None => None,
        };
        let source_by_path = source_threads
            .iter()
            .map(|thread| {
                (
                    thread.source_path.to_string_lossy().into_owned(),
                    thread.content.as_slice(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut indexed_ids = Vec::new();
        let mut imported = 0;
        let mut refreshed_count = 0;
        let mut skipped = 0;
        let mut installed = existing_by_id.clone();
        for planned in &plan.threads {
            let target = PathBuf::from(&planned.target_path);
            let mapped_cwd = PathBuf::from(&planned.mapped_cwd);
            match planned.action {
                MergeAction::SkipIdentical | MergeAction::KeepTargetLonger => {
                    progress(format!("Refreshing Codex index for {}", planned.thread.id));
                    fs::create_dir_all(&mapped_cwd)?;
                    let existing_bytes = fs::read(&target)?;
                    let (rewritten, _) = rollout::rewrite_workspace_bytes(
                        &existing_bytes,
                        &mapped_cwd,
                        &plan.mappings,
                        planned.history_only,
                    )?;
                    let rewritten = rollout::rebase_history_offsets(&rewritten, &installed)?;
                    transaction.backup_replaced(&target)?;
                    if existing_bytes != rewritten {
                        fs::write(&target, &rewritten)?;
                    }
                    let actual = register_and_index(
                        &mut environment,
                        server.as_mut(),
                        planned,
                        &target,
                        modern,
                        &plan.mappings,
                    )?;
                    installed.insert(planned.thread.id.clone(), actual);
                    indexed_ids.push(planned.thread.id.clone());
                    refreshed_count += 1;
                    skipped += 1;
                    continue;
                }
                MergeAction::Conflict => unreachable!("conflicts are rejected before import"),
                MergeAction::Import | MergeAction::ReplaceWithLonger => {}
            }
            progress(format!("Importing {}", planned.thread.id));
            let fallback;
            let source_bytes = match source_by_path.get(&planned.source_path) {
                Some(bytes) => *bytes,
                None => {
                    fallback = fs::read(&planned.source_path)?;
                    fallback.as_slice()
                }
            };
            let (bytes, _) = rollout::rewrite_workspace_bytes(
                source_bytes,
                &mapped_cwd,
                &plan.mappings,
                planned.history_only,
            )?;
            let bytes = rollout::rebase_history_offsets(&bytes, &installed)?;
            if planned.action == MergeAction::ReplaceWithLonger {
                if let Some(previous) = existing_by_id.get(&planned.thread.id) {
                    if previous != &target && previous.is_file() {
                        transaction.backup_replaced(previous)?;
                        fs::remove_file(previous)?;
                    }
                }
            }
            if target.exists() {
                transaction.backup_replaced(&target)?;
            } else {
                transaction.note_created(&target)?;
            }
            let staged = staging.join(&planned.thread.id);
            fs::write(&staged, bytes)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if target.exists() {
                fs::remove_file(&target)?;
            }
            fs::rename(&staged, &target).or_else(|_| {
                fs::copy(&staged, &target)?;
                fs::remove_file(&staged)
            })?;
            fs::create_dir_all(&mapped_cwd)?;
            let actual = register_and_index(
                &mut environment,
                server.as_mut(),
                planned,
                &target,
                modern,
                &plan.mappings,
            )?;
            installed.insert(planned.thread.id.clone(), actual);
            indexed_ids.push(planned.thread.id.clone());
            imported += 1;
        }
        if !plan.threads.is_empty() {
            progress("Synchronizing Codex session index".to_owned());
            let index_path = session_index::path(&environment.codex_home);
            if index_path.exists() {
                transaction.backup_replaced(&index_path)?;
            } else {
                transaction.note_created(&index_path)?;
            }
            let promoted =
                session_index::sync_imported(&environment.codex_home, source_root, &plan.threads)?;
            if let Some(state_db) = environment.state_db.as_deref() {
                for (thread_id, updated_at) in promoted {
                    sqlite_adapter::promote_thread(state_db, &thread_id, updated_at)?;
                }
            }
        }
        let native_projects = environment
            .state_db
            .as_deref()
            .map(sqlite_adapter::has_projects)
            .transpose()?
            .unwrap_or(false);
        if native_projects && !plan.threads.is_empty() {
            progress("Registering native Desktop projects and thread assignments".to_owned());
            register_native_projects(
                &environment,
                server
                    .as_mut()
                    .ok_or_else(|| anyhow!("native project API requires App Server"))?,
                plan,
            )?;
        }
        let projects = plan
            .threads
            .iter()
            .filter(|thread| !thread.history_only)
            .map(|thread| thread.mapped_cwd.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if !native_projects && !projects.is_empty() {
            progress("Registering projects in Codex Desktop".to_owned());
            let state_path = desktop_state::state_path(&environment.codex_home);
            if state_path.exists() {
                transaction.backup_replaced(&state_path)?;
            } else {
                transaction.note_created(&state_path)?;
            }
            desktop_state::register_projects(&state_path, &projects)?;
        }
        progress("Validating imported threads".to_owned());
        for planned in plan.threads.iter().filter(|t| t.thread.paginated()) {
            let (turns, items) = server
                .as_mut()
                .ok_or_else(|| anyhow!("paginated history requires App Server"))?
                .verify_history(&planned.thread.id, &installed[&planned.thread.id])?;
            progress(format!(
                "Verified paginated history {}: {turns} turns, {items} items",
                planned.thread.id
            ));
        }
        if let Some(server) = server.as_mut() {
            for planned in plan.threads.iter().filter(|t| t.thread.archived) {
                server.request(
                    "thread/archive",
                    serde_json::json!({"threadId": planned.thread.id}),
                )?;
                let path = sqlite_adapter::thread_rollout_path(
                    environment
                        .state_db
                        .as_deref()
                        .ok_or_else(|| anyhow!("state DB absent"))?,
                    &planned.thread.id,
                )?
                .ok_or_else(|| anyhow!("archived rollout absent"))?;
                installed.insert(planned.thread.id.clone(), PathBuf::from(path));
            }
        }
        let refreshed = discovery::discover(Some(&environment.codex_home))?;
        validator::verify_expected_threads(&refreshed, indexed_ids)?;
        session_index::verify_contains(
            &refreshed.codex_home,
            plan.threads.iter().map(|planned| planned.thread.id.clone()),
        )?;
        if let Some(state_db) = refreshed.state_db.as_deref() {
            for planned in &plan.threads {
                sqlite_adapter::validate_visible_thread(
                    state_db,
                    &planned.thread.id,
                    Path::new(&planned.mapped_cwd),
                )?;
                rollout::validate_cwd_file(
                    &installed[&planned.thread.id],
                    Path::new(&planned.mapped_cwd),
                )?;
            }
            let integrity = sqlite_adapter::integrity_check(state_db)?;
            if integrity != "ok" {
                anyhow::bail!("SQLite integrity check returned {integrity}");
            }
        }
        // Drop releases all native DB handles before success or rollback.
        drop(server);
        Ok((imported, refreshed_count, skipped))
    })();

    // New schema families created by this runtime also belong to the rollback.
    let result = match (result, transaction.track_new_databases()) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(error.context("record new database families")),
        (Err(error), Err(tracking_error)) => Err(error.context(format!(
            "record new database families also failed: {tracking_error:#}"
        ))),
    };

    let (imported, refreshed, skipped) = match result {
        Ok(result) => result,
        Err(error) => {
            progress("Import failed; restoring rollback snapshot".to_owned());
            let rollback_error = transaction.rollback().err();
            if let Some(rollback_error) = rollback_error {
                return Err(anyhow!(
                    "import failed: {error:#}; automatic rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error.context("import failed and was rolled back"));
        }
    };
    transaction.complete()?;
    progress("Import completed".to_owned());
    Ok(ImportSummary {
        transaction_id: transaction.record.id,
        imported,
        refreshed,
        skipped,
    })
}

fn register_and_index(
    environment: &mut Environment,
    mut server: Option<&mut app_server::AppServer>,
    planned: &crate::model::PlannedThread,
    rollout_path: &Path,
    modern: bool,
    mappings: &BTreeMap<String, String>,
) -> Result<PathBuf> {
    let thread = &planned.thread;
    let cwd = Path::new(&planned.mapped_cwd);
    let history_only = planned.history_only;
    let first = fs::read_to_string(rollout_path)?
        .lines()
        .next()
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?
        .unwrap_or_default();
    let roots = first["payload"]["runtime_workspace_roots"]
        .as_array()
        .map(|roots| {
            roots
                .iter()
                .filter_map(|r| r.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| vec![cwd.to_string_lossy().into_owned()]);
    let mut actual = rollout_path.to_path_buf();
    let mut record = thread.clone();
    if thread.archived {
        if let Some(server) = server.as_mut() {
            let refreshed = discovery::discover(Some(&environment.codex_home))?;
            let database = refreshed
                .state_db
                .as_deref()
                .ok_or_else(|| anyhow!("runtime did not initialize the state DB"))?;
            sqlite_adapter::upsert_thread(database, thread, rollout_path, cwd)?;
            let result = server.request(
                "thread/unarchive",
                serde_json::json!({"threadId":thread.id}),
            )?;
            actual = PathBuf::from(
                result["thread"]["path"]
                    .as_str()
                    .ok_or_else(|| anyhow!("thread/unarchive returned no path"))?,
            );
            record.archived = false;
        }
    }
    let app_server_result = server
        .ok_or_else(|| anyhow!("Codex executable was not found"))
        .and_then(|server| server.register(&actual, &thread.id, cwd, &roots, thread.paginated()));
    let refreshed = discovery::discover(Some(&environment.codex_home))?;
    let state_db = refreshed
        .state_db
        .as_deref()
        .ok_or_else(|| anyhow!("state database was not found"))?;
    if let Err(error) = app_server_result {
        if modern || thread.paginated() || sqlite_adapter::has_projects(state_db)? {
            return Err(
                error.context("native registration is mandatory for current history/projects")
            );
        }
        eprintln!(
            "warning: App Server registration failed for {}; using SQLite fallback: {error:#}",
            thread.id
        );
    }
    if let Some(policy) = &record.sandbox_policy {
        if let Ok(mut policy) = serde_json::from_str::<serde_json::Value>(policy) {
            let mut effective = mappings.clone();
            effective
                .entry(normalize(&thread.cwd))
                .or_insert(cwd.to_string_lossy().into_owned());
            rollout::rewrite_policy_roots(
                &mut policy,
                &effective,
                &cwd.to_string_lossy(),
                history_only,
            )?;
            record.sandbox_policy = Some(policy.to_string());
        }
    }
    sqlite_adapter::upsert_thread(state_db, &record, &actual, cwd)?;
    *environment = refreshed;
    Ok(actual)
}

fn register_native_projects(
    environment: &Environment,
    server: &mut app_server::AppServer,
    plan: &ImportPlan,
) -> Result<()> {
    use serde_json::{json, Value};
    let mut groups = BTreeMap::<String, Vec<&crate::model::PlannedThread>>::new();
    for thread in plan.threads.iter().filter(|t| t.history_only) {
        server.request(
            "thread/metadata/update",
            json!({"threadId": thread.thread.id, "projectId": ""}),
        )?;
    }
    for thread in plan.threads.iter().filter(|t| {
        !t.history_only
            && t.thread
                .extra
                .get("_migrate_existing_dependency")
                .and_then(Value::as_bool)
                != Some(true)
    }) {
        let project = thread.thread.extra.get("source_project");
        let key = project
            .and_then(|p| p["id"].as_str())
            .map(|id| format!("project:{id}"))
            .unwrap_or_else(|| format!("cwd:{}", normalize(&thread.mapped_cwd)));
        groups.entry(key).or_default().push(thread);
    }
    for (key, members) in groups {
        let first = members[0];
        let source_project = first.thread.extra.get("source_project");
        let name = source_project
            .and_then(|p| p["name"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                Path::new(&first.mapped_cwd)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Migrated project".to_owned())
            });
        let mut effective = plan.mappings.clone();
        for member in &members {
            effective
                .entry(normalize(&member.thread.cwd))
                .or_insert(member.mapped_cwd.clone());
        }
        let source_roots = source_project
            .and_then(|p| p["roots"].as_array())
            .cloned()
            .unwrap_or_else(|| vec![json!(first.thread.cwd)]);
        let mut roots = Vec::new();
        for root in source_roots.iter().filter_map(Value::as_str) {
            let mapped = map_explicit(root, &effective, &environment.platform)
                .or_else(|| Path::new(root).is_dir().then(|| PathBuf::from(root)))
                .ok_or_else(|| anyhow!("source project root requires a mapping: {root}"))?;
            if !mapped.is_absolute() {
                anyhow::bail!("project root must be absolute: {}", mapped.display());
            }
            let path = mapped.to_string_lossy().into_owned();
            if !roots.contains(&path) {
                roots.push(path);
            }
        }
        if roots.is_empty() {
            roots.push(first.mapped_cwd.clone());
        }
        let ids = members
            .iter()
            .map(|t| t.thread.id.clone())
            .collect::<Vec<_>>();
        let metadata = source_project
            .and_then(|p| p.get("metadata"))
            .filter(|v| v.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        // Include mapped roots in the key: rebind creates/uses the destination
        // project instead of mutating unrelated members of the source project.
        let digest = hex::encode(Sha256::digest(
            format!("{key}|{}", roots.join("|")).as_bytes(),
        ));
        let result = server.request("project/import", json!({"idempotencyKey": format!("codex-migrate-v2-{digest}"), "name": name,
            "roots": roots.iter().map(|p| json!({"path": p})).collect::<Vec<_>>(), "metadata": metadata, "threads": ids}))?;
        let project_id = result["project"]["id"]
            .as_str()
            .ok_or_else(|| anyhow!("project/import returned no project ID"))?;
        // An idempotent import can return an existing project. Explicitly assign
        // every selected thread so repeated imports with a larger selection work.
        for id in &ids {
            server.request(
                "thread/metadata/update",
                json!({"threadId": id, "projectId": project_id}),
            )?;
        }
        sqlite_adapter::validate_project(
            environment
                .state_db
                .as_deref()
                .ok_or_else(|| anyhow!("state DB absent"))?,
            project_id,
            &ids,
            &roots,
        )?;
    }
    Ok(())
}

fn validate_options(catalog: &SourceCatalog, options: &ImportOptions) -> Result<()> {
    let known = catalog
        .projects
        .iter()
        .flat_map(|project| project.sessions.iter())
        .map(|session| session.thread.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for thread_id in &options.selected_thread_ids {
        if !known.contains(thread_id.as_str()) {
            return Err(anyhow!("unknown selected thread: {thread_id}"));
        }
    }
    for project in &catalog.projects {
        let selected = project
            .sessions
            .iter()
            .any(|session| options.selected_thread_ids.contains(&session.thread.id));
        if !selected {
            continue;
        }
        let cwd = normalize(&project.original_cwd);
        let platform = discovery::current_platform();
        if map_explicit(&cwd, &options.mappings, &platform).is_none()
            && !Path::new(&cwd).is_dir()
            && !options.history_only_projects.contains(&cwd)
        {
            return Err(anyhow!(
                "selected project requires a local folder or history-only mode: {}",
                project.original_cwd
            ));
        }
    }
    Ok(())
}

fn build_catalog(source_root: &Path, threads: &[ScannedThread]) -> SourceCatalog {
    let mut groups = BTreeMap::<String, Vec<&ScannedThread>>::new();
    for thread in threads {
        groups
            .entry(normalize(&thread.record.cwd))
            .or_default()
            .push(thread);
    }
    let projects = groups
        .into_iter()
        .map(|(cwd, mut sessions)| {
            sessions.sort_by_key(|session| std::cmp::Reverse(session.record.updated_at));
            SourceProject {
                suggested_target: Path::new(&cwd)
                    .is_dir()
                    .then(|| PathBuf::from(&cwd).to_string_lossy().into_owned()),
                original_cwd: cwd,
                sessions: sessions
                    .into_iter()
                    .map(|thread| SourceSession {
                        thread: thread.record.clone(),
                        source_path: thread.source_path.to_string_lossy().into_owned(),
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    let source_platform = infer_source_platform(threads);
    let source_codex_version = threads
        .iter()
        .map(|thread| thread.record.cli_version.clone())
        .find(|version| !version.is_empty());
    SourceCatalog {
        source_codex_home: source_root.to_string_lossy().into_owned(),
        source_platform,
        source_codex_version,
        thread_count: threads.len(),
        projects,
    }
}

fn infer_source_platform(threads: &[ScannedThread]) -> PlatformKind {
    let paths = threads
        .iter()
        .map(|thread| normalize(&thread.record.cwd))
        .collect::<Vec<_>>();
    if paths.iter().any(|path| {
        path.len() >= 3 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':'
    }) {
        PlatformKind::Windows
    } else if paths.iter().any(|path| path.starts_with("/mnt/")) {
        PlatformKind::Wsl
    } else if paths
        .iter()
        .any(|path| path.starts_with("/Users/") || path.starts_with("/Volumes/"))
    {
        PlatformKind::Macos
    } else {
        PlatformKind::Linux
    }
}

fn is_codex_root(path: &Path) -> bool {
    path.is_dir()
        && (path.join("sessions").is_dir()
            || path.join("archived_sessions").is_dir()
            || discovery::find_state_db(path).ok().flatten().is_some())
}

pub fn content_fingerprint(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
