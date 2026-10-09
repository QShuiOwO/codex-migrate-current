use crate::model::ImportOptions;
use anyhow::{Context, Result};
use chrono::Utc;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A log lives outside the target history, so rollback cannot erase the failure.
pub fn run<T>(
    operation: &str,
    source: &Path,
    target: Option<&Path>,
    options: &ImportOptions,
    progress: &mut impl FnMut(String),
    action: impl FnOnce(&mut dyn FnMut(String)) -> Result<T>,
) -> Result<T> {
    let directory = std::env::var_os("CODEX_MIGRATE_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("codex-migrate/logs")
        });
    run_in(
        &directory, operation, source, target, options, progress, action,
    )
}

fn run_in<T>(
    directory: &Path,
    operation: &str,
    source: &Path,
    target: Option<&Path>,
    options: &ImportOptions,
    progress: &mut impl FnMut(String),
    action: impl FnOnce(&mut dyn FnMut(String)) -> Result<T>,
) -> Result<T> {
    let path = directory.join(format!(
        "{}-{operation}-{}.log",
        Utc::now().format("%Y%m%dT%H%M%SZ"),
        uuid::Uuid::new_v4()
    ));
    let mut log = (|| -> Result<File> {
        fs::create_dir_all(directory)?;
        Ok(OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?)
    })();
    let mut write_error = None;
    let mut write = |message: &str| {
        if let Ok(file) = &mut log {
            if let Err(error) = writeln!(file, "{} {}", Utc::now().to_rfc3339(), redact(message))
                .and_then(|_| file.flush())
            {
                write_error = Some(error.to_string());
            }
        }
    };
    write(&format!(
        "operation={operation} version={} os={} arch={} source={} target={}\noptions={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        source.display(),
        target
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "auto".into()),
        serde_json::to_string(options)?
    ));
    let result = action(&mut |message| {
        write(&message);
        progress(message);
    });
    match &result {
        Ok(_) => write("RESULT success"),
        Err(error) => write(&format!("RESULT failure\n{error:#}")),
    }
    let description = match (log, write_error) {
        (Ok(_), None) => format!("Diagnostic log / 诊断日志: {}", path.display()),
        (Err(error), _) => format!("Could not create diagnostic log / 无法创建诊断日志: {error:#}"),
        (_, Some(error)) => format!(
            "Diagnostic log incomplete / 诊断日志写入不完整: {}: {error}",
            path.display()
        ),
    };
    progress(description.clone());
    result.with_context(|| description)
}

/// Suppress whole credential-bearing lines rather than partially exposing secrets.
pub(crate) fn redact(message: &str) -> String {
    message
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if [
                "authorization",
                "api_key",
                "api-key",
                "access_token",
                "refresh_token",
                "bearer ",
                "sk-",
            ]
            .iter()
            .any(|key| lower.contains(key))
            {
                "[credential-bearing line redacted]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_keeps_root_cause_and_persistent_log() {
        let root = tempfile::tempdir().unwrap();
        let error = run_in::<()>(
            root.path(),
            "import",
            Path::new("source"),
            None,
            &ImportOptions::default(),
            &mut |_| {},
            |progress| {
                progress("Creating rollback snapshot".into());
                Err(anyhow::anyhow!("missing workspace root: C:/old/artifacts")
                    .context("import failed and was rolled back"))
            },
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("missing workspace root"));
        assert!(format!("{error:#}").contains("诊断日志"));
        let path = fs::read_dir(root.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let log = fs::read_to_string(path).unwrap();
        assert!(log.contains("Creating rollback snapshot"));
        assert!(log.contains("RESULT failure"));
        assert!(log.contains("C:/old/artifacts"));
    }

    #[test]
    fn log_creation_failure_does_not_hide_import_error() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        fs::write(&path, "occupied").unwrap();
        let error = run_in::<()>(
            &path,
            "import",
            Path::new("source"),
            None,
            &ImportOptions::default(),
            &mut |_| {},
            |_| Err(anyhow::anyhow!("native failure")),
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("无法创建诊断日志"));
        assert!(message.contains("native failure"));
        assert!(
            !redact("Authorization: Bearer secret\napi_key=secret\nordinary error")
                .contains("secret")
        );
    }
}
