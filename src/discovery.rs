use crate::model::PlatformKind;
use crate::sqlite_adapter;
use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Environment {
    pub codex_home: PathBuf,
    pub sqlite_home: PathBuf,
    pub codex_executable: Option<PathBuf>,
    pub codex_version: Option<String>,
    pub platform: PlatformKind,
    pub wsl: bool,
    pub state_db: Option<PathBuf>,
    pub schema_version: Option<i64>,
}

pub fn discover(explicit_home: Option<&Path>) -> Result<Environment> {
    let mut codex_home = if let Some(path) = explicit_home {
        path.to_path_buf()
    } else if let Some(path) = env::var_os("CODEX_HOME") {
        PathBuf::from(path)
    } else {
        dirs::home_dir()
            .ok_or_else(|| anyhow!("cannot determine home directory"))?
            .join(".codex")
    };
    if !codex_home.is_absolute() {
        codex_home = env::current_dir()?.join(codex_home);
    }

    let mut sqlite_home = env::var_os("CODEX_SQLITE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| codex_home.clone());
    if !sqlite_home.is_absolute() {
        sqlite_home = env::current_dir()?.join(sqlite_home);
    }
    let state_db = find_state_db(&sqlite_home)?;
    let schema_version = state_db
        .as_deref()
        .and_then(|path| read_schema_version(path).ok().flatten());
    let codex_executable = if let Some(path) = env::var_os("CODEX_MIGRATE_CODEX_BIN") {
        if path.is_empty() {
            None
        } else {
            let path = PathBuf::from(path);
            if !path.is_file() {
                anyhow::bail!("CODEX_MIGRATE_CODEX_BIN is not a file: {}", path.display());
            }
            Some(path.canonicalize()?)
        }
    } else {
        find_executable("codex")
    };
    let codex_version = codex_executable
        .as_deref()
        .and_then(|path| command_version(path).ok());
    let wsl = is_wsl();
    let platform = current_platform();

    Ok(Environment {
        codex_home,
        sqlite_home,
        codex_executable,
        codex_version,
        platform,
        wsl,
        state_db,
        schema_version,
    })
}

pub fn current_platform() -> PlatformKind {
    if is_wsl() {
        PlatformKind::Wsl
    } else if cfg!(target_os = "macos") {
        PlatformKind::Macos
    } else if cfg!(target_os = "windows") {
        PlatformKind::Windows
    } else if cfg!(target_os = "linux") {
        PlatformKind::Linux
    } else {
        PlatformKind::Unknown
    }
}

pub fn ensure_codex_stopped(codex_home: &Path) -> Result<()> {
    let default_home = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")));
    if default_home.as_deref() != Some(codex_home) {
        return Ok(());
    }
    let running = if cfg!(target_os = "windows") {
        Command::new("tasklist")
            .args(["/NH"])
            .output()
            .ok()
            .is_some_and(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .to_ascii_lowercase()
                    .lines()
                    .any(|line| line.starts_with("codex.exe") || line.starts_with("chatgpt.exe"))
            })
    } else {
        let desktop_running = Command::new("pgrep")
            .args(["-f", "(Codex|ChatGPT).app/Contents/MacOS/(Codex|ChatGPT)"])
            .status()
            .is_ok_and(|status| status.success());
        let cli_running = Command::new("pgrep")
            .args(["-x", "codex"])
            .status()
            .is_ok_and(|status| status.success());
        desktop_running || cli_running
    };
    if running {
        anyhow::bail!(
            "ChatGPT/Codex is still running. Close the desktop app and all Codex CLI sessions, then retry"
        );
    }
    Ok(())
}

pub fn find_state_db(home: &Path) -> Result<Option<PathBuf>> {
    if !home.exists() {
        return Ok(None);
    }
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(home).with_context(|| format!("read {}", home.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("state_") && name.ends_with(".sqlite") {
            candidates.push(entry.path());
        }
    }
    candidates.sort_by_key(|path| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .and_then(|value| value.strip_prefix("state_"))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0)
    });
    Ok(candidates.pop())
}

fn read_schema_version(path: &Path) -> Result<Option<i64>> {
    let connection = sqlite_adapter::open_readable(path)?;
    let exists: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
        [],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Ok(None);
    }
    let version = connection.query_row(
        "SELECT max(version) FROM _sqlx_migrations WHERE success = 1",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    Ok(version)
}

fn find_executable(name: &str) -> Option<PathBuf> {
    find_executable_in(
        name,
        env::var_os("PATH").as_deref(),
        env::var_os("LOCALAPPDATA").as_deref(),
    )
}

fn find_executable_in(
    name: &str,
    path: Option<&std::ffi::OsStr>,
    _local: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    // Desktop's schema and paginated APIs must use its runtime ahead of a global CLI.
    #[cfg(windows)]
    if name == "codex" {
        if let Some(local) = _local {
            let root = PathBuf::from(local).join("OpenAI/Codex/bin");
            let mut candidates = std::fs::read_dir(root)
                .into_iter()
                .flatten()
                .filter_map(|entry| {
                    let path = entry.ok()?.path().join("codex.exe");
                    let metadata = path.metadata().ok()?;
                    if !metadata.is_file() {
                        return None;
                    }
                    Some((metadata.modified().ok()?, path))
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            if let Some((_, path)) = candidates.into_iter().next() {
                return Some(path);
            }
        }
    }
    for directory in path.into_iter().flat_map(env::split_paths) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = directory.join(format!("{name}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let bundled = PathBuf::from("/Applications/Codex.app/Contents/Resources/codex");
        if bundled.is_file() {
            return Some(bundled);
        }
    }
    None
}

fn command_version(executable: &Path) -> Result<String> {
    let mut command = Command::new(executable);
    command.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .with_context(|| format!("run {}", executable.display()))?;
    if !output.status.success() {
        return Err(anyhow!("codex --version failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn desktop_runtime_precedes_path_and_works_without_path() {
        let root = tempfile::tempdir().unwrap();
        let desktop = root.path().join("OpenAI/Codex/bin/current/codex.exe");
        let global = root.path().join("cli/codex.exe");
        for path in [&desktop, &global] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "synthetic executable").unwrap();
        }
        let path = global.parent().unwrap().as_os_str();
        assert_eq!(
            find_executable_in("codex", Some(path), Some(root.path().as_os_str())),
            Some(desktop.clone())
        );
        assert_eq!(
            find_executable_in("codex", None, Some(root.path().as_os_str())),
            Some(desktop)
        );
        assert_eq!(find_executable_in("codex", Some(path), None), Some(global));
    }
}

fn is_wsl() -> bool {
    if env::var_os("WSL_DISTRO_NAME").is_some() || env::var_os("WSL_INTEROP").is_some() {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(value) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
            return value.to_ascii_lowercase().contains("microsoft");
        }
    }
    false
}
