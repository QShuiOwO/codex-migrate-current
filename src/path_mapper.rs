use crate::model::PlatformKind;
use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMapping {
    pub source: String,
    pub target: String,
}

/// Check a directory destination without creating anything during preview.
pub fn validate_directory_destination(path: &Path) -> Result<()> {
    if !path.is_absolute() || (!cfg!(windows) && is_windows_path(&path.to_string_lossy())) {
        anyhow::bail!(
            "project directory must be a local absolute path: {}",
            path.display()
        );
    }
    for ancestor in path.ancestors() {
        match std::fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => return Ok(()),
            Ok(_) => anyhow::bail!(
                "project directory path is occupied by a file: {}",
                ancestor.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect project directory {}", ancestor.display()))
            }
        }
    }
    anyhow::bail!(
        "project directory has no accessible filesystem root: {}",
        path.display()
    )
}

pub fn parse_mapping(value: &str) -> Result<PathMapping> {
    let (source, target) = value
        .split_once('=')
        .ok_or_else(|| anyhow!("path mapping must use OLD=NEW: {value}"))?;
    if source.trim().is_empty() || target.trim().is_empty() {
        return Err(anyhow!(
            "path mapping cannot contain an empty side: {value}"
        ));
    }
    Ok(PathMapping {
        source: normalize(source),
        target: normalize(target),
    })
}

pub fn map_path(
    original: &str,
    mappings: &[PathMapping],
    platform: &PlatformKind,
    history_root: &Path,
    thread_id: &str,
) -> PathBuf {
    let normalized = normalize(original);
    let mut candidates = mappings
        .iter()
        .map(|mapping| (normalize(&mapping.source), &mapping.target))
        .filter(|(source, _)| source_prefix_matches(&normalized, source))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(source, _)| std::cmp::Reverse(source.len()));

    if let Some((source, target)) = candidates.first() {
        let suffix = &normalized[source.len()..];
        let mapped = mapped_suffix(target, suffix);
        return platform_path(&mapped, platform);
    }

    if path_is_usable(&normalized, platform) {
        return platform_path(&normalized, platform);
    }
    history_root.join(thread_id)
}

pub fn normalize(value: &str) -> String {
    let mut result = value.trim().replace('\\', "/");
    // Codex 0.165 can store canonical Windows paths in existing thread metadata.
    // Normalize drive and UNC spellings, but retain other device namespaces.
    if let Some(rest) = result.strip_prefix("//?/") {
        if is_windows_drive(rest) {
            result = rest.to_owned();
        } else if rest
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC/"))
        {
            let unc = &rest[4..];
            let mut components = unc.split('/');
            if components.next().is_some_and(|part| !part.is_empty())
                && components.next().is_some_and(|part| !part.is_empty())
            {
                result = format!("//{unc}");
            }
        }
    }
    while result.contains("//") && !result.starts_with("//") {
        result = result.replace("//", "/");
    }
    if result.len() > 1 && !(is_windows_drive(&result) && result.len() == 3) {
        result = result.trim_end_matches('/').to_owned();
    }
    result
}

/// Compare actual filesystem locations when available. Lexical fallback is
/// platform-specific so Windows spellings do not weaken Unix path checks.
pub fn paths_equivalent(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => return left == right,
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => return false,
        (Err(_), Err(_)) => {}
    }
    if cfg!(windows) {
        normalize(&left.to_string_lossy())
            .eq_ignore_ascii_case(&normalize(&right.to_string_lossy()))
    } else {
        left == right
    }
}

fn mapped_suffix(target: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        target.to_owned()
    } else {
        format!(
            "{}/{}",
            target.trim_end_matches(['/', '\\']),
            suffix.trim_start_matches('/')
        )
    }
}

pub fn history_only_path(root: &Path, original: &str) -> PathBuf {
    let normalized = normalize(original);
    let digest = hex::encode(Sha256::digest(normalized.as_bytes()));
    let name = normalized
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("project");
    let safe_name = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    root.join(format!("{}-{}", &digest[..12], safe_name))
}

pub fn map_explicit(
    original: &str,
    mappings: &BTreeMap<String, String>,
    platform: &PlatformKind,
) -> Option<PathBuf> {
    let normalized = normalize(original);
    mappings
        .iter()
        .map(|(source, target)| (normalize(source), target))
        .filter(|(source, _)| source_prefix_matches(&normalized, source))
        .max_by_key(|(source, _)| source.len())
        .map(|(source, target)| {
            let suffix = &normalized[source.len()..];
            platform_path(&mapped_suffix(target, suffix), platform)
        })
}

pub fn prefix_matches(path: &str, prefix: &str, insensitive: bool) -> bool {
    let (path, prefix) = if insensitive {
        (path.to_ascii_lowercase(), prefix.to_ascii_lowercase())
    } else {
        (path.to_owned(), prefix.to_owned())
    };
    path == prefix
        || path
            .strip_prefix(&prefix)
            .is_some_and(|rest| prefix.ends_with('/') || rest.starts_with('/'))
}

/// Source path semantics must not depend on the destination OS.
pub fn source_prefix_matches(path: &str, prefix: &str) -> bool {
    let path = normalize(path);
    let prefix = normalize(prefix);
    prefix_matches(&path, &prefix, is_windows_path(&prefix))
}

pub fn is_windows_path(path: &str) -> bool {
    let path = normalize(path);
    is_windows_drive(&path) || path.starts_with("//")
}

pub fn is_absolute_source_path(path: &str) -> bool {
    Path::new(path).is_absolute() || is_windows_path(path)
}

/// Resolve existing symlink ancestors even when a mapped directory is new.
/// The CLI filters sessions by its canonical working directory.
pub fn local_directory(path: &Path) -> Result<PathBuf> {
    validate_directory_destination(path)?;
    for ancestor in path.ancestors() {
        if ancestor.is_dir() {
            let resolved = ancestor.canonicalize()?.join(path.strip_prefix(ancestor)?);
            return Ok(PathBuf::from(normalize(&resolved.to_string_lossy())));
        }
    }
    unreachable!("validated destination has an existing ancestor")
}

fn path_is_usable(path: &str, platform: &PlatformKind) -> bool {
    match platform {
        PlatformKind::Windows => {
            (path.len() >= 3 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'/')
                || path.starts_with("//")
        }
        PlatformKind::Wsl => path.starts_with('/') || is_windows_drive(path),
        _ => path.starts_with('/'),
    }
}

fn platform_path(path: &str, platform: &PlatformKind) -> PathBuf {
    if matches!(platform, PlatformKind::Wsl) && is_windows_drive(path) {
        let drive = path[0..1].to_ascii_lowercase();
        let suffix = path[2..].trim_start_matches('/');
        return PathBuf::from(format!("/mnt/{drive}/{suffix}"));
    }
    PathBuf::from(path)
}

fn is_windows_drive(path: &str) -> bool {
    path.len() >= 3
        && path.as_bytes()[0].is_ascii_alphabetic()
        && path.as_bytes()[1] == b':'
        && path.as_bytes()[2] == b'/'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_windows_extended_paths_without_losing_drive_roots() {
        assert_eq!(normalize(r"\\?\D:\Projects\demo"), "D:/Projects/demo");
        assert_eq!(normalize("//?/D:/Projects/demo/"), "D:/Projects/demo");
        assert_eq!(
            normalize(r"\\?\UNC\server\share\demo"),
            "//server/share/demo"
        );
        assert_eq!(normalize("//?/unc/server/share/"), "//server/share");
        assert_eq!(normalize("D:/"), "D:/");
        assert_eq!(normalize("//?/D:/"), "D:/");
        assert_eq!(
            normalize("//?/Volume{abc}/folder"),
            "//?/Volume{abc}/folder"
        );
        assert_eq!(normalize("//./pipe/demo"), "//./pipe/demo");
    }

    #[test]
    fn maps_extended_paths_and_drive_root_without_changing_boundaries() {
        let mappings = BTreeMap::from([
            (r"\\?\C:\Work".to_owned(), "D:/Projects".to_owned()),
            ("C:/".to_owned(), "E:/Root".to_owned()),
        ]);
        for (source, target) in [
            (r"\\?\C:\Work\demo", "D:/Projects/demo"),
            ("c:/work/demo", "D:/Projects/demo"),
            ("C:/Work-other", "E:/Root/Work-other"),
            ("C:/", "E:/Root"),
        ] {
            assert_eq!(
                map_explicit(source, &mappings, &PlatformKind::Windows).unwrap(),
                PathBuf::from(target)
            );
        }
        let unc = BTreeMap::from([("//server/share".into(), "D:/Shared".into())]);
        assert_eq!(
            map_explicit(r"\\?\UNC\server\share\folder", &unc, &PlatformKind::Windows).unwrap(),
            PathBuf::from("D:/Shared/folder")
        );
        assert_eq!(
            map_path(
                "//?/D:/demo",
                &[],
                &PlatformKind::Wsl,
                Path::new("/history"),
                "id"
            ),
            PathBuf::from("/mnt/d/demo")
        );
        assert_eq!(
            map_path(
                "C:/folder",
                &[parse_mapping("C:/=D:/").unwrap()],
                &PlatformKind::Windows,
                Path::new("history"),
                "id"
            ),
            PathBuf::from("D:/folder")
        );
        assert_eq!(
            history_only_path(Path::new("history"), r"\\?\C:\Work"),
            history_only_path(Path::new("history"), "C:/Work")
        );
    }

    #[test]
    fn path_equivalence_keeps_distinct_existing_directories_distinct() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        assert!(paths_equivalent(&first.canonicalize().unwrap(), &first));
        assert!(paths_equivalent(&first.join("."), &first));
        assert!(!paths_equivalent(&first, &second));
        assert!(!paths_equivalent(&first, &root.path().join("missing")));
        #[cfg(not(windows))]
        assert!(!paths_equivalent(
            Path::new("/missing/Case"),
            Path::new("/missing/case")
        ));
    }

    #[test]
    fn longest_prefix_wins() {
        let mappings = vec![
            parse_mapping("/Users/a=D:/Users").unwrap(),
            parse_mapping("/Users/a/Projects=D:/Projects").unwrap(),
        ];
        let mapped = map_path(
            "/Users/a/Projects/demo",
            &mappings,
            &PlatformKind::Windows,
            Path::new("history"),
            "id",
        );
        assert_eq!(normalize(&mapped.to_string_lossy()), "D:/Projects/demo");
    }

    #[test]
    fn converts_windows_drive_to_wsl() {
        let mappings = vec![parse_mapping("/Users/a=D:/Work").unwrap()];
        let mapped = map_path(
            "/Users/a/demo",
            &mappings,
            &PlatformKind::Wsl,
            Path::new("/tmp/history"),
            "id",
        );
        assert_eq!(mapped, PathBuf::from("/mnt/d/Work/demo"));
    }

    #[test]
    fn unresolved_path_uses_history_root() {
        let mapped = map_path(
            "C:/work/demo",
            &[],
            &PlatformKind::Linux,
            Path::new("/tmp/history"),
            "abc",
        );
        assert_eq!(mapped, PathBuf::from("/tmp/history/abc"));
    }

    #[test]
    fn windows_mapping_is_case_insensitive() {
        let mappings = vec![parse_mapping("C:/Users/A/Work=D:/Projects").unwrap()];
        let mapped = map_path(
            "c:/users/a/work/demo",
            &mappings,
            &PlatformKind::Windows,
            Path::new("history"),
            "id",
        );
        assert_eq!(normalize(&mapped.to_string_lossy()), "D:/Projects/demo");
    }

    #[test]
    fn preserves_unc_paths_on_windows() {
        let mapped = map_path(
            r"\\server\share\demo",
            &[],
            &PlatformKind::Windows,
            Path::new("history"),
            "id",
        );
        assert_eq!(normalize(&mapped.to_string_lossy()), "//server/share/demo");
    }

    #[test]
    fn history_path_is_stable_per_project() {
        let first = history_only_path(Path::new("/history"), "/Users/a/Project A");
        let second = history_only_path(Path::new("/history"), "/Users/a/Project A");
        assert_eq!(first, second);
        assert!(first.to_string_lossy().contains("Project_A"));
    }
}
