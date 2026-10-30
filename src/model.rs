use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlatformKind {
    Macos,
    Windows,
    Linux,
    Wsl,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRecord {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub cwd: String,
    pub source: String,
    pub thread_source: Option<String>,
    pub model_provider: String,
    pub cli_version: String,
    pub archived: bool,
    pub archive_path: String,
    pub sha256: String,
    pub byte_len: u64,
    pub first_user_message: String,
    pub sandbox_policy: Option<String>,
    pub approval_mode: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// Version-dependent metadata, including paginated lineage and source project.
    #[serde(default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl ThreadRecord {
    pub fn paginated(&self) -> bool {
        self.extra
            .get("history_mode")
            .and_then(serde_json::Value::as_str)
            == Some("paginated")
    }

    pub fn history_parent(&self) -> Option<&str> {
        self.extra.get("history_base")?.get("thread_id")?.as_str()
    }
}

#[derive(Debug, Clone)]
pub struct ScannedThread {
    pub record: ThreadRecord,
    pub source_path: PathBuf,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCatalog {
    pub source_codex_home: String,
    pub source_platform: PlatformKind,
    pub source_codex_version: Option<String>,
    pub projects: Vec<SourceProject>,
    pub thread_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceProject {
    pub original_cwd: String,
    pub suggested_target: Option<String>,
    pub sessions: Vec<SourceSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSession {
    pub thread: ThreadRecord,
    pub source_path: String,
}

/// Metadata for a selected-session backup, without copying unrelated databases.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportManifest {
    pub format_version: u32,
    pub producer_version: String,
    pub exported_at: String,
    pub selected_thread_ids: BTreeSet<String>,
    pub dependency_thread_ids: Vec<String>,
    pub threads: Vec<ThreadRecord>,
}

/// Desktop retains archive/project state; CLI makes selected sessions resumable.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ImportTarget {
    #[default]
    Desktop,
    Cli,
}

impl ImportTarget {
    pub fn for_platform() -> Self {
        if cfg!(target_os = "linux") {
            Self::Cli
        } else {
            Self::Desktop
        }
    }
}

impl std::fmt::Display for ImportTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Desktop => "desktop",
            Self::Cli => "cli",
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportOptions {
    #[serde(default)]
    pub target: ImportTarget,
    pub selected_thread_ids: BTreeSet<String>,
    pub mappings: BTreeMap<String, String>,
    pub history_only_projects: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub id: String,
    pub created_at: String,
    pub codex_home: String,
    pub sqlite_home: String,
    pub source_codex_home: String,
    pub backups: Vec<BackupEntry>,
    pub created_files: Vec<String>,
    #[serde(default)]
    pub created_directories: Vec<String>,
    pub replaced_files: Vec<BackupEntry>,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub original: String,
    pub backup: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MergeAction {
    Import,
    SkipIdentical,
    ReplaceWithLonger,
    KeepTargetLonger,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedThread {
    #[serde(default)]
    pub resume_in_cli: bool,
    pub thread: ThreadRecord,
    pub source_path: String,
    pub mapped_cwd: String,
    pub history_only: bool,
    pub target_path: String,
    pub action: MergeAction,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPlan {
    #[serde(default)]
    pub target: ImportTarget,
    pub source_codex_home: String,
    pub codex_home: String,
    pub threads: Vec<PlannedThread>,
    pub conflicts: usize,
    #[serde(default)]
    pub dependency_thread_ids: Vec<String>,
    #[serde(default)]
    pub mappings: BTreeMap<String, String>,
    #[serde(default)]
    pub workspace_directories: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub codex_home: String,
    pub sqlite_home: String,
    pub codex_executable: Option<String>,
    pub codex_version: Option<String>,
    pub platform: PlatformKind,
    pub wsl: bool,
    pub state_db: Option<String>,
    pub schema_version: Option<i64>,
    pub active_rollouts: usize,
    pub archived_rollouts: usize,
    pub database_threads: usize,
    pub integrity: Option<String>,
    pub issues: Vec<String>,
}
