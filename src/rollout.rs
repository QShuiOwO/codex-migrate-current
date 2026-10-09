use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn rewrite_cwd_bytes(content: &[u8], cwd: &Path) -> Result<(Vec<u8>, usize)> {
    rewrite_workspace_bytes(content, cwd, &BTreeMap::new(), false)
}

pub fn rewrite_workspace_bytes(
    content: &[u8],
    cwd: &Path,
    mappings: &BTreeMap<String, String>,
    history_only: bool,
) -> Result<(Vec<u8>, usize)> {
    let cwd = cwd.to_string_lossy();
    let mut output = Vec::with_capacity(content.len());
    let mut changed = 0;
    for (line_number, line) in content.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let mut value: Value = serde_json::from_slice(line)
            .with_context(|| format!("parse rollout line {}", line_number + 1))?;
        let original = value.clone();
        if matches!(
            value.get("type").and_then(Value::as_str),
            Some("session_meta" | "turn_context")
        ) {
            if let Some(payload) = value.get_mut("payload").and_then(Value::as_object_mut) {
                let old_cwd = payload
                    .get("cwd")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let mut effective = mappings.clone();
                if !old_cwd.is_empty() {
                    effective
                        .entry(crate::path_mapper::normalize(&old_cwd))
                        .or_insert(cwd.to_string());
                }
                for key in ["runtime_workspace_roots", "workspace_roots"] {
                    if let Some(roots) = payload.get_mut(key).and_then(Value::as_array_mut) {
                        for root in roots {
                            if let Some(path) = root.as_str() {
                                *root = Value::String(map_root(
                                    path,
                                    &effective,
                                    cwd.as_ref(),
                                    history_only,
                                )?);
                            }
                        }
                    }
                }
                if let Some(policy) = payload.get_mut("sandbox_policy") {
                    rewrite_policy_roots(policy, &effective, cwd.as_ref(), history_only)?;
                }
                if payload.get("cwd").and_then(Value::as_str) != Some(cwd.as_ref()) {
                    payload.insert("cwd".to_owned(), Value::String(cwd.to_string()));
                }
            }
        }
        if value != original {
            changed += 1;
            output.extend_from_slice(&serde_json::to_vec(&value)?);
        } else {
            output.extend_from_slice(line);
        }
        output.push(b'\n');
    }
    Ok((output, changed))
}

fn map_root(
    path: &str,
    mappings: &BTreeMap<String, String>,
    cwd: &str,
    history_only: bool,
) -> Result<String> {
    if history_only {
        return Ok(cwd.to_owned());
    }
    let mapped =
        crate::path_mapper::map_explicit(path, mappings, &crate::discovery::current_platform())
            .or_else(|| Path::new(path).is_dir().then(|| path.into()))
            .ok_or_else(|| {
                anyhow::anyhow!("workspace root requires an explicit mapping: {path}")
            })?;
    if !mapped.is_absolute() {
        anyhow::bail!("mapped root must be absolute: {}", mapped.display());
    }
    Ok(mapped.to_string_lossy().into_owned())
}

pub fn rewrite_policy_roots(
    policy: &mut Value,
    mappings: &BTreeMap<String, String>,
    cwd: &str,
    history_only: bool,
) -> Result<()> {
    if let Some(roots) = policy
        .get_mut("writable_roots")
        .and_then(Value::as_array_mut)
    {
        for root in roots {
            if let Some(path) = root.as_str() {
                *root = Value::String(map_root(path, mappings, cwd, history_only)?);
            }
        }
    }
    // Current managed permission profiles encode literal paths separately from
    // special roots such as `root`, `tmpdir`, and `workspace_roots`.
    if let Some(entries) = policy
        .get_mut("file_system")
        .and_then(|v| v.get_mut("entries"))
        .and_then(Value::as_array_mut)
    {
        for entry in entries {
            if let Some(path) = entry.get_mut("path").filter(|p| p["type"] == "path") {
                for key in ["path", "value"] {
                    if let Some(original) = path[key].as_str() {
                        path[key] = Value::String(map_root(original, mappings, cwd, history_only)?);
                    }
                }
            } else if let Some(path) = entry
                .get_mut("path")
                .filter(|p| p["type"] == "glob_pattern")
            {
                if let Some(pattern) = path["pattern"]
                    .as_str()
                    .filter(|p| Path::new(p).is_absolute())
                {
                    path["pattern"] =
                        Value::String(map_root(pattern, mappings, cwd, history_only)?);
                }
            }
        }
    }
    Ok(())
}

/// Recompute the byte hint after mapped metadata changes the parent's length.
/// Ordinals and the exclusive fork boundary are preserved.
pub fn rebase_history_offsets(
    content: &[u8],
    parents: &BTreeMap<String, std::path::PathBuf>,
) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(content.len());
    for line in content
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
    {
        let mut value: Value = serde_json::from_slice(line)?;
        let mut changed = false;
        if value["type"] == "session_meta" {
            if let Some(base) = value["payload"]
                .get_mut("history_base")
                .filter(|base| base.is_object())
            {
                let id = base["thread_id"].as_str().unwrap_or_default();
                let end = base["end_ordinal_exclusive"]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("invalid history_base cutoff for {id}"))?;
                let path = parents.get(id).ok_or_else(|| {
                    anyhow::anyhow!("history_base parent was not installed: {id}")
                })?;
                let parent_bytes = fs::read(path)?;
                let mut offset = 0u64;
                for record in parent_bytes.split_inclusive(|b| *b == b'\n') {
                    if record.iter().all(u8::is_ascii_whitespace) {
                        offset += record.len() as u64;
                        continue;
                    }
                    let parent: Value = serde_json::from_slice(record)?;
                    if parent["ordinal"].as_u64().is_some_and(|n| n >= end) {
                        break;
                    }
                    offset += record.len() as u64;
                }
                if base["end_byte_offset"].as_u64() != Some(offset) {
                    base["end_byte_offset"] = Value::from(offset);
                    changed = true;
                }
            }
        }
        if changed {
            output.extend(serde_json::to_vec(&value)?);
        } else {
            output.extend_from_slice(line);
        }
        output.push(b'\n');
    }
    Ok(output)
}

pub fn rewrite_cwd_file(path: &Path, cwd: &Path) -> Result<usize> {
    let original = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let (rewritten, changed) = rewrite_cwd_bytes(&original, cwd)?;
    if changed > 0 {
        let temporary = path.with_extension("jsonl.tmp");
        fs::write(&temporary, rewritten)?;
        fs::rename(&temporary, path)?;
    }
    Ok(changed)
}

pub fn validate_cwd_file(path: &Path, cwd: &Path) -> Result<()> {
    let content = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let expected = cwd.to_string_lossy();
    let mut session_meta = 0;
    for (line_number, line) in content.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_slice(line)
            .with_context(|| format!("parse rollout line {}", line_number + 1))?;
        if !matches!(
            value.get("type").and_then(Value::as_str),
            Some("session_meta" | "turn_context")
        ) {
            continue;
        }
        let Some(actual) = value
            .get("payload")
            .and_then(|payload| payload.get("cwd"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if actual != expected {
            anyhow::bail!(
                "{} contains cwd {actual}, expected {}",
                path.display(),
                cwd.display()
            );
        }
        if value.get("type").and_then(Value::as_str) == Some("session_meta") {
            session_meta += 1;
        }
    }
    if session_meta == 0 {
        anyhow::bail!("{} has no mapped session_meta cwd", path.display());
    }
    Ok(())
}

pub fn canonicalize_cwd(content: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(content.len());
    for line in content.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let mut value: Value = serde_json::from_slice(line).ok()?;
        if matches!(
            value.get("type").and_then(Value::as_str),
            Some("session_meta" | "turn_context")
        ) {
            if let Some(payload) = value.get_mut("payload").and_then(Value::as_object_mut) {
                if payload.contains_key("cwd") {
                    payload.insert("cwd".to_owned(), Value::String("<mapped-cwd>".to_owned()));
                }
                for key in ["runtime_workspace_roots", "workspace_roots"] {
                    if let Some(roots) = payload.get_mut(key).and_then(Value::as_array_mut) {
                        for root in roots {
                            if root.is_string() {
                                *root = Value::String("<mapped-root>".to_owned());
                            }
                        }
                    }
                }
                if let Some(roots) = payload
                    .get_mut("sandbox_policy")
                    .and_then(|p| p.get_mut("writable_roots"))
                    .and_then(Value::as_array_mut)
                {
                    for root in roots {
                        if root.is_string() {
                            *root = Value::String("<mapped-root>".to_owned());
                        }
                    }
                }
                if let Some(entries) = payload
                    .get_mut("sandbox_policy")
                    .and_then(|p| p.get_mut("file_system"))
                    .and_then(|p| p.get_mut("entries"))
                    .and_then(Value::as_array_mut)
                {
                    for entry in entries {
                        if let Some(path) = entry.get_mut("path").filter(|p| p["type"] == "path") {
                            for key in ["path", "value"] {
                                if path.get(key).is_some_and(Value::is_string) {
                                    path[key] = Value::String("<mapped-root>".to_owned());
                                }
                            }
                        }
                    }
                }
                if let Some(base) = payload
                    .get_mut("history_base")
                    .filter(|base| base.is_object())
                {
                    base["end_byte_offset"] = Value::from(0);
                }
            }
        }
        output.extend_from_slice(&serde_json::to_vec(&value).ok()?);
        output.push(b'\n');
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_only_structured_cwd_fields() {
        let input = br#"{"type":"session_meta","payload":{"cwd":"/old","note":"/old"}}
{"type":"event_msg","payload":{"message":"/old"}}
"#;
        let (output, changed) = rewrite_cwd_bytes(input, Path::new("/new")).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(changed, 1);
        assert!(text.contains("\"cwd\":\"/new\""));
        assert!(text.contains("\"note\":\"/old\""));
        assert!(text.contains("\"message\":\"/old\""));
    }

    #[test]
    fn canonical_form_ignores_mapped_cwd() {
        let old = br#"{"type":"session_meta","payload":{"id":"1","cwd":"/old"}}"#;
        let new = br#"{"type":"session_meta","payload":{"id":"1","cwd":"C:\\new"}}"#;
        assert_eq!(canonicalize_cwd(old), canonicalize_cwd(new));
    }

    #[test]
    fn maps_runtime_roots_and_managed_permissions_without_changing_messages() {
        let home = tempfile::TempDir::new().unwrap();
        let new = home.path().join("new");
        let other = home.path().join("other");
        let mappings = BTreeMap::from([(
            "/old/second".to_owned(),
            other.to_string_lossy().into_owned(),
        )]);
        let input = serde_json::json!({"type":"session_meta","payload":{"cwd":"/old/work",
            "runtime_workspace_roots":["/old/work","/old/second"],"note":"/old/work",
            "sandbox_policy":{"type":"managed","file_system":{"entries":[
                {"path":{"type":"path","path":"/old/work"},"access":"write"},
                {"path":{"type":"special","value":{"kind":"root"}},"access":"read"}]}}}});
        let (bytes, changes) =
            rewrite_workspace_bytes(input.to_string().as_bytes(), &new, &mappings, false).unwrap();
        let output: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(changes, 1);
        assert_eq!(
            output["payload"]["runtime_workspace_roots"][1],
            other.to_string_lossy().as_ref()
        );
        assert_eq!(
            output["payload"]["sandbox_policy"]["file_system"]["entries"][0]["path"]["path"],
            new.to_string_lossy().as_ref()
        );
        assert_eq!(output["payload"]["note"], "/old/work");
        assert_eq!(
            output["payload"]["sandbox_policy"]["file_system"]["entries"][1]["path"]["type"],
            "special"
        );
    }

    #[test]
    fn unmapped_extra_root_is_rejected_and_history_only_uses_stub() {
        let home = tempfile::TempDir::new().unwrap();
        let input = br#"{"type":"session_meta","payload":{"cwd":"/old","runtime_workspace_roots":["/old","/nonexistent-remote-root-20261008"]}}"#;
        assert!(rewrite_workspace_bytes(input, home.path(), &BTreeMap::new(), false).is_err());
        let (bytes, _) =
            rewrite_workspace_bytes(input, home.path(), &BTreeMap::new(), true).unwrap();
        let output: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            output["payload"]["runtime_workspace_roots"][1],
            home.path().to_string_lossy().as_ref()
        );
    }

    #[test]
    fn recomputes_fork_byte_hint_at_exclusive_ordinal() {
        let home = tempfile::TempDir::new().unwrap();
        let parent = home.path().join("parent.jsonl");
        let first = b"{\"ordinal\":0,\"type\":\"session_meta\",\"payload\":{\"cwd\":\"a-long-mapped-root\"}}\n";
        let mut bytes = first.to_vec();
        bytes.extend_from_slice(b"{\"ordinal\":1,\"type\":\"event_msg\",\"payload\":{}}\n");
        fs::write(&parent, bytes).unwrap();
        let child = br#"{"type":"session_meta","payload":{"history_base":{"thread_id":"parent","end_ordinal_exclusive":1,"end_byte_offset":1}}}"#;
        let rebased =
            rebase_history_offsets(child, &BTreeMap::from([("parent".to_owned(), parent)]))
                .unwrap();
        let value: Value = serde_json::from_slice(&rebased).unwrap();
        assert_eq!(
            value["payload"]["history_base"]["end_byte_offset"],
            first.len()
        );
        assert_eq!(value["payload"]["history_base"]["end_ordinal_exclusive"], 1);
    }
}
