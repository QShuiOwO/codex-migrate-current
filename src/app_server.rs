use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// One native runtime per transaction. Drop releases DB handles before rollback.
pub struct AppServer {
    child: Child,
    stdin: ChildStdin,
    receiver: mpsc::Receiver<Value>,
    next_id: i64,
    stderr: Arc<Mutex<std::collections::VecDeque<String>>>,
    stderr_reader: Option<std::thread::JoinHandle<()>>,
}

impl AppServer {
    pub fn start(codex: &Path, home: &Path, sqlite_home: &Path) -> Result<Self> {
        let mut command = Command::new(codex);
        command
            .args(["app-server", "--listen", "stdio://"])
            .env("CODEX_HOME", home)
            .env("CODEX_SQLITE_HOME", sqlite_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("start App Server {}", codex.display()))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("missing stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing stderr"))?;
        let stderr_tail = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let tail = stderr_tail.clone();
        let stderr_reader = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Ok(mut lines) = tail.lock() {
                    // Only retain bounded diagnostics; successful RPC bodies are never logged.
                    lines.push_back(crate::diagnostics::redact(
                        &line.chars().take(2048).collect::<String>(),
                    ));
                    if lines.len() > 32 {
                        lines.pop_front();
                    }
                }
            }
        });
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    if sender.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        let mut server = Self {
            child,
            stdin,
            receiver,
            next_id: 1,
            stderr: stderr_tail,
            stderr_reader: Some(stderr_reader),
        };
        server.request("initialize", json!({
            "clientInfo": {"name": "codex_migrate", "title": "Codex Migrate", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi": true}
        }))?;
        server.send(json!({"method": "initialized", "params": {}}))?;
        Ok(server)
    }

    pub fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let result = self.request_inner(method, params);
        if result.is_err() {
            // Stop the child and drain stderr before returning so early crashes retain their reason.
            let status = self.child.try_wait().ok().flatten();
            self.stop();
            let stderr = self
                .stderr
                .lock()
                .map(|lines| lines.iter().cloned().collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            return result.with_context(|| {
                format!("Native RPC {method}; exit={status:?}; stderr tail:\n{stderr}")
            });
        }
        result
    }

    fn request_inner(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"id": id, "method": method, "params": params}))?;
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let value = self
                .receiver
                .recv_timeout(remaining)
                .with_context(|| format!("App Server {method} timed out or exited"))?;
            if value.get("id").and_then(Value::as_i64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                bail!("App Server {method} failed: {error}");
            }
            return value
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow!("{method} returned no result"));
        }
    }

    pub fn register(
        &mut self,
        path: &Path,
        id: &str,
        cwd: &Path,
        roots: &[String],
        paginated: bool,
    ) -> Result<()> {
        let mut params = json!({"threadId": id, "path": path.to_string_lossy(),
            "cwd": cwd.to_string_lossy(), "excludeTurns": true});
        if paginated {
            params["runtimeWorkspaceRoots"] = json!(roots);
        }
        let result = self
            .request("thread/resume", params)
            .with_context(|| format!("register thread {id}"))?;
        if result["thread"]["id"].as_str() != Some(id) {
            bail!("runtime registered a different thread for {id}");
        }
        if paginated && result["thread"]["historyMode"].as_str() != Some("paginated") {
            bail!("runtime did not preserve paginated history for {id}");
        }
        self.request(
            "thread/read",
            json!({"threadId": id, "includeTurns": false}),
        )?;
        Ok(())
    }

    /// Validate every page; a metadata row alone does not prove readable history.
    pub fn verify_history(&mut self, id: &str, rollout: &Path) -> Result<(usize, usize)> {
        let (turns, _) = self.page_count(
            "thread/turns/list",
            json!({"threadId": id, "itemsView": "notLoaded", "limit": 100}),
        )?;
        let (items, projected_ids) =
            self.page_count("thread/items/list", json!({"threadId": id, "limit": 100}))?;
        let bytes = std::fs::read(rollout)?;
        let mut expected = HashSet::new();
        for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            let value: Value = serde_json::from_slice(line)?;
            if value["type"] == "event_msg" && value["payload"]["type"] == "item_completed" {
                if let Some(item) = value["payload"]["item"]["id"].as_str() {
                    expected.insert(item.to_owned());
                }
            }
        }
        let missing = expected.difference(&projected_ids).collect::<Vec<_>>();
        if !missing.is_empty() {
            bail!(
                "paginated history {id} lost {} completed item(s) during projection",
                missing.len()
            );
        }
        Ok((turns, items))
    }

    fn page_count(&mut self, method: &str, mut params: Value) -> Result<(usize, HashSet<String>)> {
        let mut cursors = HashSet::new();
        let mut ids = HashSet::new();
        let mut count = 0;
        loop {
            let page = self.request(method, params.clone())?;
            let data = page["data"]
                .as_array()
                .ok_or_else(|| anyhow!("{method} returned no data array"))?;
            count += data.len();
            for entry in data {
                if let Some(id) = entry["id"]
                    .as_str()
                    .or_else(|| entry["item"]["id"].as_str())
                {
                    ids.insert(id.to_owned());
                }
            }
            match page.get("nextCursor") {
                Some(Value::String(cursor)) if !cursor.is_empty() => {
                    if !cursors.insert(cursor.clone()) {
                        bail!("{method} repeated a pagination cursor");
                    }
                    params["cursor"] = json!(cursor);
                }
                Some(Value::Null) | None => break,
                _ => bail!("{method} returned an invalid pagination cursor"),
            }
        }
        Ok((count, ids))
    }

    fn send(&mut self, value: Value) -> Result<()> {
        serde_json::to_writer(&mut self.stdin, &value)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_runtime_exit_keeps_stderr_and_rpc_method() {
        // The Rust test runner rejects --listen, providing a deterministic stderr failure.
        let home = tempfile::tempdir().unwrap();
        let error =
            match AppServer::start(&std::env::current_exe().unwrap(), home.path(), home.path()) {
                Ok(_) => panic!("test runner unexpectedly implemented App Server"),
                Err(error) => error,
            };
        let message = format!("{error:#}");
        assert!(message.contains("Native RPC initialize"));
        assert!(message.contains("stderr tail:"));
        assert!(message.contains("listen"));
    }
}
