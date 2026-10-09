use crate::model::{BackupEntry, TransactionRecord};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::backup::Backup;
use rusqlite::Connection;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct ImportTransaction {
    pub root: PathBuf,
    pub record: TransactionRecord,
    database_roots: Vec<PathBuf>,
    original_databases: BTreeSet<PathBuf>,
    original_rollouts: BTreeSet<PathBuf>,
}

impl ImportTransaction {
    pub fn begin(codex_home: &Path, sqlite_home: &Path, source_codex_home: &Path) -> Result<Self> {
        let id = format!(
            "{}-{}",
            Utc::now().format("%Y%m%dT%H%M%SZ"),
            Uuid::new_v4().simple()
        );
        let root = codex_home.join("migration_transactions").join(&id);
        fs::create_dir_all(root.join("backups"))?;
        let record = TransactionRecord {
            id,
            created_at: Utc::now().to_rfc3339(),
            codex_home: codex_home.to_string_lossy().into_owned(),
            sqlite_home: sqlite_home.to_string_lossy().into_owned(),
            source_codex_home: source_codex_home.to_string_lossy().into_owned(),
            backups: Vec::new(),
            created_files: Vec::new(),
            replaced_files: Vec::new(),
            completed: false,
        };
        let original_rollouts = rollout_files(codex_home)?;
        let mut transaction = Self {
            root,
            record,
            database_roots: Vec::new(),
            original_databases: BTreeSet::new(),
            original_rollouts,
        };
        transaction.persist()?;
        Ok(transaction)
    }

    pub fn snapshot_databases(&mut self, roots: &[PathBuf]) -> Result<()> {
        self.database_roots = roots
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.original_databases = self.database_files()?;
        for path in self.original_databases.clone() {
            self.backup_sqlite_family(&path)?;
        }
        // Persist known runtime families before starting the server (crash-safe).
        for root in self.database_roots.clone() {
            for name in [
                "state_5.sqlite",
                "thread_history_1.sqlite",
                "logs_1.sqlite",
                "goals_1.sqlite",
                "memories_1.sqlite",
                "queue_1.sqlite",
                "codex-dev.db",
            ] {
                let path = root.join(name);
                if !path.exists() {
                    self.note_created(&path)?;
                    for suffix in ["-wal", "-shm"] {
                        self.note_created(&PathBuf::from(format!("{}{suffix}", path.display())))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn track_new_databases(&mut self) -> Result<()> {
        for path in self
            .database_files()?
            .difference(&self.original_databases.clone())
        {
            self.note_created(path)?;
            for suffix in ["-wal", "-shm"] {
                self.note_created(&PathBuf::from(format!("{}{suffix}", path.display())))?;
            }
        }
        // Native unarchive/archive can move a rollout to a runtime-chosen path.
        for path in rollout_files(Path::new(&self.record.codex_home))?
            .difference(&self.original_rollouts.clone())
        {
            self.note_created(path)?;
        }
        Ok(())
    }

    fn database_files(&self) -> Result<BTreeSet<PathBuf>> {
        let mut paths = BTreeSet::new();
        for root in &self.database_roots {
            if !root.is_dir() {
                continue;
            }
            for entry in fs::read_dir(root)? {
                let path = entry?.path();
                if path.is_file()
                    && matches!(
                        path.extension().and_then(|s| s.to_str()),
                        Some("sqlite" | "db")
                    )
                {
                    paths.insert(path);
                }
            }
        }
        Ok(paths)
    }

    pub fn backup_sqlite_family(&mut self, state_db: &Path) -> Result<()> {
        let name = format!(
            "{}-{}",
            self.record.backups.len(),
            state_db
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("state.sqlite")
        );
        let backup_path = self.root.join("backups").join(name);
        let source = Connection::open(state_db)?;
        let mut destination = Connection::open(&backup_path)?;
        let backup = Backup::new(&source, &mut destination)?;
        backup.run_to_completion(32, std::time::Duration::from_millis(20), None)?;
        drop(backup);
        drop(destination);
        self.record.backups.push(BackupEntry {
            original: state_db.to_string_lossy().into_owned(),
            backup: backup_path.to_string_lossy().into_owned(),
        });
        let text = state_db.to_string_lossy();
        for sidecar in [
            PathBuf::from(format!("{text}-wal")),
            PathBuf::from(format!("{text}-shm")),
        ] {
            self.record
                .created_files
                .push(sidecar.to_string_lossy().into_owned());
        }
        self.persist()
    }

    pub fn note_created(&mut self, path: &Path) -> Result<()> {
        let text = path.to_string_lossy().into_owned();
        if !self.record.created_files.contains(&text) {
            self.record.created_files.push(text);
        }
        self.persist()
    }

    pub fn backup_replaced(&mut self, path: &Path) -> Result<()> {
        self.backup_file(path, true)
    }

    pub fn complete(&mut self) -> Result<()> {
        self.record.completed = true;
        self.persist()
    }

    pub fn rollback(&self) -> Result<()> {
        rollback_record(&self.record)
    }

    fn backup_file(&mut self, path: &Path, replaced: bool) -> Result<()> {
        let text = path.to_string_lossy();
        if self.record.created_files.iter().any(|p| p == text.as_ref())
            || self
                .record
                .replaced_files
                .iter()
                .chain(&self.record.backups)
                .any(|e| e.original == text)
        {
            return Ok(());
        }
        let name = format!(
            "{}-{}",
            self.record.backups.len() + self.record.replaced_files.len(),
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("backup")
        );
        let backup = self.root.join("backups").join(name);
        fs::copy(path, &backup)
            .with_context(|| format!("backup {} to {}", path.display(), backup.display()))?;
        let entry = BackupEntry {
            original: path.to_string_lossy().into_owned(),
            backup: backup.to_string_lossy().into_owned(),
        };
        if replaced {
            self.record.replaced_files.push(entry);
        } else {
            self.record.backups.push(entry);
        }
        self.persist()
    }

    fn persist(&mut self) -> Result<()> {
        let final_path = self.root.join("transaction.json");
        let temporary = self.root.join("transaction.json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&self.record)?)?;
        fs::rename(temporary, final_path)?;
        Ok(())
    }
}

fn rollout_files(home: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    for folder in ["sessions", "archived_sessions"] {
        let root = home.join(folder);
        if !root.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = entry?;
            if entry.file_type().is_file()
                && entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl")
            {
                files.insert(entry.into_path());
            }
        }
    }
    Ok(files)
}

pub fn rollback_by_id(codex_home: &Path, id: &str) -> Result<()> {
    let path = transaction_directory(codex_home, id)?.join("transaction.json");
    let bytes =
        fs::read(&path).with_context(|| format!("read transaction record {}", path.display()))?;
    let record: TransactionRecord = serde_json::from_slice(&bytes)?;
    rollback_record(&record)
}

pub fn delete_by_ids(codex_home: &Path, ids: &[String]) -> Result<usize> {
    let mut deleted = 0;
    for id in ids {
        let directory = transaction_directory(codex_home, id)?;
        if directory.is_dir() {
            fs::remove_dir_all(&directory)
                .with_context(|| format!("delete rollback data {}", directory.display()))?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

fn transaction_directory(codex_home: &Path, id: &str) -> Result<PathBuf> {
    if id.is_empty()
        || id == "."
        || id == ".."
        || id.contains('/')
        || id.contains('\\')
        || Path::new(id).components().count() != 1
    {
        return Err(anyhow!("invalid transaction id"));
    }
    Ok(codex_home.join("migration_transactions").join(id))
}

fn rollback_record(record: &TransactionRecord) -> Result<()> {
    for path in record.created_files.iter().rev() {
        let path = Path::new(path);
        if path.is_file() {
            fs::remove_file(path)
                .with_context(|| format!("remove rollback-created file {}", path.display()))?;
        }
    }
    for entry in record
        .replaced_files
        .iter()
        .rev()
        .chain(record.backups.iter().rev())
    {
        let original = Path::new(&entry.original);
        let backup = Path::new(&entry.backup);
        if !backup.exists() {
            return Err(anyhow!("missing rollback backup {}", backup.display()));
        }
        if let Some(parent) = original.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(backup, original)
            .with_context(|| format!("restore rollback file {}", original.display()))?;
    }
    for entry in &record.backups {
        let original = Path::new(&entry.original);
        if matches!(
            original.extension().and_then(|value| value.to_str()),
            Some("sqlite" | "db")
        ) {
            let connection = Connection::open(original)?;
            connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn deletes_only_selected_transaction_directories() {
        let home = TempDir::new().unwrap();
        let root = home.path().join("migration_transactions");
        fs::create_dir_all(root.join("first").join("backups")).unwrap();
        fs::create_dir_all(root.join("second").join("backups")).unwrap();

        let deleted = delete_by_ids(home.path(), &["first".to_owned()]).unwrap();

        assert_eq!(deleted, 1);
        assert!(!root.join("first").exists());
        assert!(root.join("second").exists());
        assert!(delete_by_ids(home.path(), &["../second".to_owned()]).is_err());
        assert!(root.join("second").exists());
    }
}
