use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::opencode::{
    bridge::{
        BridgeAbortSummary, BridgeCreateSessionRequest, BridgeDiffSummary, BridgeFileDiffSummary,
        BridgeHealthResponse, BridgePendingPermission, BridgePermissionResponseRequest,
        BridgePermissionResponseSummary, BridgePromptRequest, BridgePromptSummary,
        BridgeRevertRequest, BridgeRevertSummary, BridgeSessionMessageRaw, BridgeSessionSummary,
        BridgeUnrevertSummary,
    },
    events::{host_part_summary_from_stored, HostOpencodeEvent},
};

use super::{
    llm, llm_config, permission_policy,
    resource_tools::{self, AgentResourceScope, NoopResourceToolExecutor, ResourceToolExecutor},
    skill_tools,
    workspace_snapshot_git::{WorkspaceSnapshotGit, SESSION_BASELINE_ANCHOR},
};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  directory TEXT NOT NULL,
  parent_id TEXT,
  created_ms INTEGER NOT NULL,
  updated_ms INTEGER NOT NULL,
  additions INTEGER NOT NULL DEFAULT 0,
  deletions INTEGER NOT NULL DEFAULT 0,
  files INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  role TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  info_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS parts (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_parts_msg ON parts(message_id, sort_order);
CREATE TABLE IF NOT EXISTS revert_journal (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  snapshot_json TEXT NOT NULL,
  created_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_revert_journal_session ON revert_journal(session_id, id);
CREATE TABLE IF NOT EXISTS pending_permissions (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  permission TEXT NOT NULL,
  patterns_json TEXT NOT NULL,
  metadata_json TEXT NOT NULL,
  queue_kind TEXT NOT NULL,
  resolution TEXT NOT NULL DEFAULT 'pending',
  created_ms INTEGER NOT NULL,
  updated_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_pending_permissions_list ON pending_permissions(queue_kind, resolution, session_id);
CREATE TABLE IF NOT EXISTS session_diff_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  anchor_message_id TEXT NOT NULL,
  diff_text TEXT NOT NULL,
  additions INTEGER NOT NULL DEFAULT 0,
  deletions INTEGER NOT NULL DEFAULT 0,
  captured_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_session_diff_anchor ON session_diff_snapshots(session_id, anchor_message_id, id);
CREATE TABLE IF NOT EXISTS workspace_tree_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  anchor_message_id TEXT NOT NULL,
  tree_hash TEXT NOT NULL,
  captured_ms INTEGER NOT NULL,
  UNIQUE(session_id, anchor_message_id)
);
CREATE INDEX IF NOT EXISTS idx_workspace_tree_session ON workspace_tree_snapshots(session_id, captured_ms);
"#;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// LLM 调用所需环境是否齐全（与 `send_prompt_blocking` 一致）。
pub(crate) fn native_llm_env_ready() -> bool {
    llm_config::llm_env_ready(None)
}

struct Inner {
    db: Mutex<Connection>,
    source_root: PathBuf,
    /// 用于解析 meilang-author skill 安装目录（通常与 `AppState.package_root` 一致）。
    skill_package_root: PathBuf,
    event_tx: broadcast::Sender<HostOpencodeEvent>,
    /// 会话进行中时存在；`abort_session` 置位以中断流式读取。
    abort_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    resource_tools: Arc<dyn ResourceToolExecutor>,
}

#[derive(Clone)]
pub struct NativeAgent {
    inner: Arc<Inner>,
}

impl NativeAgent {
    /// 无场景 resource 工具（noop）；供测试与仅嵌入 `http/pages` 的构建使用，`mei serve` 主路径用 [`Self::open_with_resource_tools`]。
    #[allow(dead_code)]
    pub fn open(source_root: PathBuf) -> Result<Self> {
        let skill_root = source_root.clone();
        Self::open_with_resource_tools(
            source_root,
            skill_root,
            Arc::new(NoopResourceToolExecutor::default()),
        )
    }

    pub fn open_with_resource_tools(
        source_root: PathBuf,
        skill_package_root: PathBuf,
        resource_tools: Arc<dyn ResourceToolExecutor>,
    ) -> Result<Self> {
        let mei = source_root.join(".mei");
        std::fs::create_dir_all(&mei).with_context(|| format!("create {}", mei.display()))?;
        let db_path = mei.join("agent.sqlite");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("open sqlite {}", db_path.display()))?;
        conn.execute_batch(SCHEMA).context("agent sqlite schema")?;
        let (event_tx, _rx) = broadcast::channel::<HostOpencodeEvent>(512);
        Ok(Self {
            inner: Arc::new(Inner {
                db: Mutex::new(conn),
                source_root,
                skill_package_root,
                event_tx,
                abort_flags: Mutex::new(HashMap::new()),
                resource_tools,
            }),
        })
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<HostOpencodeEvent> {
        self.inner.event_tx.subscribe()
    }

    fn emit(&self, ev: HostOpencodeEvent) {
        let _ = self.inner.event_tx.send(ev);
    }

    pub fn worktree_string(&self) -> String {
        self.inner.source_root.display().to_string()
    }

    pub fn vcs_summary_blocking(&self) -> (bool, Option<String>) {
        let root = &self.inner.source_root;
        let inside = Command::new("git")
            .args([
                "-C",
                root.as_os_str().to_str().unwrap_or(""),
                "rev-parse",
                "--is-inside-work-tree",
            ])
            .output();
        let detected = match inside {
            Ok(o) => o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true",
            Err(_) => false,
        };
        if !detected {
            return (false, None);
        }
        let branch = Command::new("git")
            .args([
                "-C",
                root.as_os_str().to_str().unwrap_or(""),
                "branch",
                "--show-current",
            ])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        (true, branch)
    }

    pub fn health_response(&self) -> BridgeHealthResponse {
        let llm_ok = native_llm_env_ready();
        let (vcs_detected, vcs_branch) = self.vcs_summary_blocking();
        let wt = self.worktree_string();
        let mut history_reason = if vcs_detected {
            None
        } else {
            Some("native: message-level soft revert; git diff requires a worktree".to_string())
        };
        if !llm_ok {
            history_reason = Some(
                "内置助手缺少 LLM 环境变量：若设置了 OPENAI_IMITATORS，则每个前缀需 {PREFIX}_BASE_URL、{PREFIX}_API_KEY、{PREFIX}_COMPLETION_MODEL（逗号分隔多模型时首项为默认）；未设置时沿用 QWEN_* 或 MEI_LLM_OPENAI_*。可用 MEI_LLM_DEFAULT_PROVIDER 指定默认前缀。"
                    .to_string(),
            );
        }
        let history_available = llm_ok && vcs_detected;
        BridgeHealthResponse {
            server_url: "mei://native-agent".to_string(),
            healthy: llm_ok,
            version: concat!("mei-agent-", env!("CARGO_PKG_VERSION")).to_string(),
            expected_worktree: Some(wt.clone()),
            project_worktree: Some(wt),
            vcs_detected,
            vcs_branch,
            history_available,
            history_reason,
        }
    }

    pub fn list_sessions_blocking(&self) -> Result<Vec<BridgeSessionSummary>> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let mut stmt = db
            .prepare(
                "SELECT id, title, directory, created_ms, updated_ms, additions, deletions, files \
             FROM sessions ORDER BY updated_ms DESC",
            )
            .context("prepare list sessions")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(BridgeSessionSummary {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    directory: r.get(2)?,
                    created_at_ms: r
                        .get::<_, Option<i64>>(3)?
                        .map(|x| u64::try_from(x).unwrap_or(0)),
                    updated_at_ms: r
                        .get::<_, Option<i64>>(4)?
                        .map(|x| u64::try_from(x).unwrap_or(0)),
                    additions: r.get::<_, i64>(5)? as u64,
                    deletions: r.get::<_, i64>(6)? as u64,
                    files: r.get::<_, i64>(7)? as u64,
                })
            })
            .context("query sessions")?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn create_session_blocking(
        &self,
        req: BridgeCreateSessionRequest,
    ) -> Result<BridgeSessionSummary> {
        let id = format!("ses_{}", Uuid::new_v4());
        let title = req
            .title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "新会话".to_string());
        let directory = self.worktree_string();
        let t = now_ms();
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        db.execute(
            "INSERT INTO sessions (id, title, directory, parent_id, created_ms, updated_ms, additions, deletions, files) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 0)",
            params![
                id,
                title,
                directory,
                req.parent_id,
                t,
                t,
            ],
        )
        .context("insert session")?;
        drop(db);
        let snap_git = WorkspaceSnapshotGit::new(self.inner.source_root.clone());
        if let Ok(hash) = snap_git.track() {
            if let Ok(db) = self.inner.db.lock() {
                if let Err(e) = Self::persist_workspace_tree_hash(
                    &db,
                    &id,
                    SESSION_BASELINE_ANCHOR,
                    &hash,
                ) {
                    tracing::warn!(%e, "session baseline tree snapshot persist failed");
                }
            }
        } else {
            tracing::warn!("session baseline tree track skipped (git unavailable or worktree not ready)");
        }
        Ok(BridgeSessionSummary {
            id,
            title,
            directory,
            created_at_ms: Some(t as u64),
            updated_at_ms: Some(t as u64),
            additions: 0,
            deletions: 0,
            files: 0,
        })
    }

    pub fn session_messages_blocking(
        &self,
        session_id: &str,
    ) -> Result<Vec<BridgeSessionMessageRaw>> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let mut stmt = db
            .prepare(
                "SELECT id, info_json FROM messages WHERE session_id = ?1 ORDER BY sort_order ASC",
            )
            .context("prepare messages")?;
        let ids: Vec<(String, String)> = stmt
            .query_map(params![session_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .context("query messages")?
            .collect::<Result<_, _>>()?;
        let mut out = Vec::new();
        for (mid, info_json) in ids {
            let info: Value = serde_json::from_str(&info_json).unwrap_or(Value::Null);
            let mut pstm =
                db.prepare("SELECT json FROM parts WHERE message_id = ?1 ORDER BY sort_order ASC")?;
            let parts: Vec<Value> = pstm
                .query_map(params![mid], |r| {
                    let s: String = r.get(0)?;
                    Ok(serde_json::from_str::<Value>(&s).unwrap_or(Value::Null))
                })?
                .collect::<Result<_, _>>()?;
            out.push(BridgeSessionMessageRaw { info, parts });
        }
        Ok(out)
    }

    fn next_sort_order(conn: &Connection, session_id: &str) -> Result<i64> {
        let v: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM messages WHERE session_id = ?1",
                params![session_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(v)
    }

    pub fn abort_session_blocking(&self, session_id: &str) -> Result<BridgeAbortSummary> {
        if let Ok(g) = self.inner.abort_flags.lock() {
            if let Some(f) = g.get(session_id) {
                f.store(true, Ordering::SeqCst);
            }
        }
        Ok(BridgeAbortSummary {
            session_id: session_id.to_string(),
            aborted: true,
        })
    }

    fn take_abort_flag(&self, session_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut g) = self.inner.abort_flags.lock() {
            g.insert(session_id.to_string(), flag.clone());
        }
        flag
    }

    fn clear_abort_flag(&self, session_id: &str) {
        if let Ok(mut g) = self.inner.abort_flags.lock() {
            g.remove(session_id);
        }
    }

    const MAX_TOOL_PART_OUTPUT: usize = 80_000;

    fn truncate_tool_output_for_store(s: &str, max: usize) -> String {
        if s.len() <= max {
            return s.to_string();
        }
        format!(
            "{}\n… (truncated for storage/UI, {} bytes total)",
            &s[..max],
            s.len()
        )
    }

    fn tool_display_fields(name: &str, args: &Value) -> (Option<String>, Option<String>) {
        match name {
            "read_file" => {
                let p = args.get("path").and_then(Value::as_str).unwrap_or("");
                (Some("read_file".into()), Some(p.to_string()))
            }
            "dataset_query" => {
                let id = args.get("id").and_then(Value::as_str).unwrap_or("");
                (Some(format!("dataset_query `{id}`")), Some(id.to_string()))
            }
            "skill_list" => (Some("skill_list".into()), None),
            "skill_read" => {
                let p = args.get("path").and_then(Value::as_str).unwrap_or("");
                (Some("skill_read".into()), Some(p.to_string()))
            }
            _ => (Some(name.to_string()), None),
        }
    }

    fn emit_part_upsert_from_value(&self, part: &Value) -> Result<()> {
        let Some(summary) = host_part_summary_from_stored(part) else {
            anyhow::bail!("part normalize failed");
        };
        let sid = part
            .get("sessionID")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        self.emit(HostOpencodeEvent::MessagePartUpsert {
            session_id: sid,
            message_id: summary.message_id.clone(),
            part: summary,
        });
        Ok(())
    }

    /// 在一批 `tool` parts 之后追加空 `text` part，并把后续 `append_part_text` 的目标切换到该 part，
    /// 使 `ORDER BY sort_order` 与对话时间线一致（避免「整段正文永远在工具块之前」）。
    fn ensure_assistant_text_continuation_part(
        &self,
        session_id: &str,
        message_id: &str,
        active_text_part_id: &Arc<Mutex<String>>,
    ) -> Result<()> {
        let last_json: Option<String> = {
            let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
            match db.query_row(
                "SELECT json FROM parts WHERE message_id = ?1 ORDER BY sort_order DESC LIMIT 1",
                params![message_id],
                |r| r.get(0),
            ) {
                Ok(s) => Some(s),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(e.into()),
            }
        };
        let Some(last_json) = last_json else {
            return Ok(());
        };
        let v: Value = serde_json::from_str(&last_json)?;
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
        if ty != "tool" {
            return Ok(());
        }

        let new_part_id = format!("prt_{}", Uuid::new_v4());
        let part = json!({
            "id": new_part_id,
            "messageID": message_id,
            "sessionID": session_id,
            "type": "text",
            "text": "",
        });
        {
            let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
            let sort_order: i64 = db.query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM parts WHERE message_id = ?1",
                params![message_id],
                |r| r.get(0),
            )?;
            db.execute(
                "INSERT INTO parts (id, message_id, session_id, sort_order, json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    new_part_id,
                    message_id,
                    session_id,
                    sort_order,
                    part.to_string(),
                ],
            )?;
        }
        self.emit_part_upsert_from_value(&part)?;

        let mut g = active_text_part_id
            .lock()
            .map_err(|_| anyhow!("assistant text part lock poisoned"))?;
        *g = new_part_id;
        Ok(())
    }

    fn insert_tool_part_running(
        &self,
        session_id: &str,
        assistant_message_id: &str,
        call_id: &str,
        tool_name: &str,
        title: Option<&str>,
        input_path: Option<&str>,
    ) -> Result<String> {
        let part_id = format!("prt_{}", Uuid::new_v4());
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let sort_order: i64 = db.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM parts WHERE message_id = ?1",
            params![assistant_message_id],
            |r| r.get(0),
        )?;
        let state = json!({
            "status": "running",
            "title": title.unwrap_or(tool_name),
            "input": { "filePath": input_path.unwrap_or("") },
        });
        let part = json!({
            "id": part_id,
            "messageID": assistant_message_id,
            "sessionID": session_id,
            "type": "tool",
            "callID": call_id,
            "tool": tool_name,
            "state": state,
        });
        let ps = part.to_string();
        db.execute(
            "INSERT INTO parts (id, message_id, session_id, sort_order, json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&part_id, assistant_message_id, session_id, sort_order, ps],
        )?;
        drop(db);
        self.emit_part_upsert_from_value(&part)?;
        Ok(part_id)
    }

    fn update_tool_part_finished(
        &self,
        session_id: &str,
        assistant_message_id: &str,
        part_id: &str,
        status: &str,
        output: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let s: String = db.query_row(
            "SELECT json FROM parts WHERE id = ?1 AND message_id = ?2",
            params![part_id, assistant_message_id],
            |r| r.get(0),
        )?;
        let mut v: Value = serde_json::from_str(&s)?;
        let state = v
            .get_mut("state")
            .and_then(|x| x.as_object_mut())
            .ok_or_else(|| anyhow!("tool part missing state"))?;
        state.insert("status".into(), json!(status));
        match output {
            Some(o) => {
                state.insert("output".into(), json!(o));
            }
            None => {
                state.remove("output");
            }
        }
        match error {
            Some(e) => {
                state.insert("error".into(), json!(e));
            }
            None => {
                state.remove("error");
            }
        }
        let ns = v.to_string();
        db.execute(
            "UPDATE parts SET json = ?1 WHERE id = ?2 AND session_id = ?3",
            params![ns, part_id, session_id],
        )?;
        drop(db);
        self.emit_part_upsert_from_value(&v)?;
        Ok(())
    }

    fn dispatch_tool_with_part_tracking(
        &self,
        session_id: &str,
        assistant_message_id: &str,
        call_id: &str,
        name: &str,
        args: &str,
        app_id: Option<&str>,
        resource_scope: &AgentResourceScope,
    ) -> String {
        let args_val: Value = serde_json::from_str(args).unwrap_or_else(|_| json!({}));
        let (title, input_path) = Self::tool_display_fields(name, &args_val);
        let part_id = match self.insert_tool_part_running(
            session_id,
            assistant_message_id,
            call_id,
            name,
            title.as_deref(),
            input_path.as_deref(),
        ) {
            Ok(pid) => pid,
            Err(e) => return format!("error: failed to record tool part: {e}"),
        };

        let output_raw =
            self.execute_tool_body(session_id, call_id, name, args, app_id, resource_scope);

        let err_mode = output_raw.starts_with("error:");
        let stored = Self::truncate_tool_output_for_store(&output_raw, Self::MAX_TOOL_PART_OUTPUT);
        let (status, out_opt, err_opt): (&str, Option<&str>, Option<&str>) = if err_mode {
            ("error", None, Some(stored.as_str()))
        } else {
            ("completed", Some(stored.as_str()), None)
        };
        if let Err(e) = self.update_tool_part_finished(
            session_id,
            assistant_message_id,
            &part_id,
            status,
            out_opt,
            err_opt,
        ) {
            tracing::warn!(%e, "tool part finalize failed");
        }

        output_raw
    }

    /// 同一轮 `tool_calls` 内：先顺序落库（保证 `sort_order`），再 **并行** 执行工具体，最后顺序写回状态。
    /// 与「每执行一个工具就再问一次 LLM」不同：仍是一次 `chat/completions` 返回多个 tool_call 后统一执行。
    fn dispatch_tool_calls_batch_parallel(
        &self,
        session_id: &str,
        assistant_message_id: &str,
        batch: &[(String, String, String)],
        app_id: Option<&str>,
        resource_scope: &AgentResourceScope,
    ) -> Vec<String> {
        if batch.is_empty() {
            return Vec::new();
        }
        if batch.len() == 1 {
            let (id, n, a) = &batch[0];
            return vec![self.dispatch_tool_with_part_tracking(
                session_id,
                assistant_message_id,
                id,
                n,
                a,
                app_id,
                resource_scope,
            )];
        }

        let mut rows: Vec<(String, String, String, String)> = Vec::with_capacity(batch.len());
        for (call_id, name, args) in batch {
            let args_val: Value = serde_json::from_str(args).unwrap_or_else(|_| json!({}));
            let (title, input_path) = Self::tool_display_fields(name, &args_val);
            let part_id = match self.insert_tool_part_running(
                session_id,
                assistant_message_id,
                call_id,
                name,
                title.as_deref(),
                input_path.as_deref(),
            ) {
                Ok(pid) => pid,
                Err(e) => {
                    return vec![format!("error: failed to record tool part: {e}"); batch.len()];
                }
            };
            rows.push((part_id, call_id.clone(), name.clone(), args.clone()));
        }

        let sid = session_id.to_string();
        let app_owned = app_id.map(|s| s.to_string());
        let scope_owned = resource_scope.clone();

        let outputs: Vec<String> = thread::scope(|s| {
            let mut handles = Vec::new();
            for (_part_id, call_id, name, args) in &rows {
                let agent = self.clone();
                let sid_i = sid.clone();
                let cid = call_id.clone();
                let n = name.clone();
                let a = args.clone();
                let app_i = app_owned.clone();
                let sc = scope_owned.clone();
                handles.push(s.spawn(move || {
                    agent.execute_tool_body(
                        &sid_i,
                        &cid,
                        n.as_str(),
                        a.as_str(),
                        app_i.as_deref(),
                        &sc,
                    )
                }));
            }
            let mut out = Vec::with_capacity(handles.len());
            for h in handles {
                let piece = match h.join() {
                    Ok(v) => v,
                    Err(_) => "error: tool execution thread panicked".to_string(),
                };
                out.push(piece);
            }
            out
        });

        let mut result_strings = Vec::with_capacity(outputs.len());
        for ((part_id, _, _, _), output_raw) in rows.iter().zip(outputs) {
            let err_mode = output_raw.starts_with("error:");
            let stored =
                Self::truncate_tool_output_for_store(&output_raw, Self::MAX_TOOL_PART_OUTPUT);
            let (status, out_opt, err_opt): (&str, Option<&str>, Option<&str>) = if err_mode {
                ("error", None, Some(stored.as_str()))
            } else {
                ("completed", Some(stored.as_str()), None)
            };
            if let Err(e) = self.update_tool_part_finished(
                session_id,
                assistant_message_id,
                part_id,
                status,
                out_opt,
                err_opt,
            ) {
                tracing::warn!(%e, "tool part finalize failed");
            }
            result_strings.push(output_raw);
        }
        result_strings
    }

    fn execute_tool_body(
        &self,
        session_id: &str,
        call_id: &str,
        name: &str,
        args: &str,
        app_id: Option<&str>,
        resource_scope: &AgentResourceScope,
    ) -> String {
        match name {
            "read_file" => self.run_read_file_tool(session_id, call_id, name, args, app_id),
            "dataset_query" => self.inner.resource_tools.run_resource_tool(
                &self.inner.source_root,
                app_id,
                resource_scope,
                name,
                args,
            ),
            "skill_list" => skill_tools::execute_skill_list(&self.inner.skill_package_root),
            "skill_read" => skill_tools::execute_skill_read(&self.inner.skill_package_root, args),
            other => format!("error: tool `{other}` is not allowed"),
        }
    }

    fn capture_session_diff_snapshot(
        &self,
        session_id: &str,
        anchor_message_id: &str,
        diff_rel: Option<&str>,
    ) -> Result<()> {
        let (vcs, _) = self.vcs_summary_blocking();
        let (diff_text, additions, deletions) = if vcs {
            git_worktree_diff(&self.inner.source_root, diff_rel)
        } else {
            (
                "(native diff snapshot: no git worktree at capture time)\n".to_string(),
                0,
                0,
            )
        };
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let t = now_ms() as i64;
        db.execute(
            "INSERT INTO session_diff_snapshots (session_id, anchor_message_id, diff_text, additions, deletions, captured_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                anchor_message_id,
                diff_text,
                additions as i64,
                deletions as i64,
                t,
            ],
        )?;
        Ok(())
    }

    pub async fn send_prompt(
        &self,
        session_id: &str,
        request: BridgePromptRequest,
    ) -> Result<BridgePromptSummary> {
        let this = self.clone();
        let sid = session_id.to_string();
        tokio::task::spawn_blocking(move || this.send_prompt_blocking(&sid, request))
            .await
            .map_err(|e| anyhow::anyhow!("spawn prompt: {e}"))?
    }

    fn send_prompt_blocking(
        &self,
        session_id: &str,
        request: BridgePromptRequest,
    ) -> Result<BridgePromptSummary> {
        let (_user_msg_id, assistant_msg_id, part_id) =
            self.insert_user_and_assistant_placeholder(session_id, &request)?;

        let conn = llm_config::resolve_llm(request.model.as_ref())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let llm_config::LlmConnection {
            base_url,
            api_key,
            model,
        } = conn;

        let mut messages = self.build_llm_messages(session_id, request.system.as_deref())?;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .context("blocking http client")?;

        tracing::info!(
            target: "mei_agent",
            session_id = %session_id,
            model = %model,
            "native prompt stream start"
        );

        let sid = session_id.to_string();
        let sid_tool = sid.clone();
        let abort_flag = self.take_abort_flag(session_id);
        let tools = resource_tools::all_tool_definitions();
        let resource_scope = AgentResourceScope {
            scene_id: request.scene_id.clone(),
            entry_id: request.entry_id.clone(),
            target_file: request.target_file.clone(),
        };
        let app_id = request.app_id.clone();
        let assistant_for_tools = assistant_msg_id.clone();
        let active_text_part_id = Arc::new(Mutex::new(part_id.clone()));
        let active_for_delta = active_text_part_id.clone();
        let active_for_post_tools = active_text_part_id.clone();
        let agent_tool = self.clone();
        let agent_delta = self.clone();
        let agent_post = self.clone();
        let sid_delta = sid.clone();
        let sid_post = sid.clone();
        let mid_delta = assistant_msg_id.clone();
        let mid_post = assistant_msg_id.clone();
        let stream_res = llm::stream_chat_with_tools_blocking(
            &client,
            base_url.trim_end_matches('/'),
            &api_key,
            &model,
            &mut messages,
            &tools,
            Some(abort_flag.as_ref()),
            move |batch: &[(String, String, String)]| {
                agent_tool.dispatch_tool_calls_batch_parallel(
                    &sid_tool,
                    &assistant_for_tools,
                    batch,
                    app_id.as_deref(),
                    &resource_scope,
                )
            },
            move |d| {
                let pid = active_for_delta
                    .lock()
                    .map_err(|_| anyhow!("assistant text part lock poisoned"))?
                    .clone();
                let server_ts_ms = now_ms();
                agent_delta.append_part_text(&sid_delta, &mid_delta, &pid, d)?;
                agent_delta.emit(HostOpencodeEvent::MessagePartDelta {
                    session_id: sid_delta.clone(),
                    message_id: mid_delta.clone(),
                    part_id: pid.clone(),
                    field: "text".to_string(),
                    delta: d.to_string(),
                    server_ts_ms: Some(server_ts_ms),
                });
                tracing::info!(
                    target: "mei_agent.delta",
                    session_id = %sid_delta,
                    message_id = %mid_delta,
                    part_id = %pid,
                    server_ts_ms,
                    delta_preview = %d.chars().take(24).collect::<String>(),
                    "assistant delta emitted"
                );
                Ok(())
            },
            move || {
                agent_post.ensure_assistant_text_continuation_part(
                    &sid_post,
                    &mid_post,
                    &active_for_post_tools,
                )
            },
        );

        match &stream_res {
            Ok(()) => {
                self.finalize_assistant(&sid, &assistant_msg_id, "stop")?;
                self.emit(HostOpencodeEvent::MessageInfo {
                    session_id: sid.clone(),
                    message_id: assistant_msg_id.clone(),
                    role: "assistant".to_string(),
                    finish: Some("stop".to_string()),
                });
                let diff_rel = request
                    .target_file
                    .as_deref()
                    .and_then(llm::sanitize_relative_path);
                if let Err(e) = self.capture_session_diff_snapshot(
                    &sid,
                    &assistant_msg_id,
                    diff_rel.as_deref(),
                ) {
                    tracing::warn!(%e, "session diff snapshot failed");
                }
                let snap_git = WorkspaceSnapshotGit::new(self.inner.source_root.clone());
                if let Ok(hash) = snap_git.track() {
                    if let Ok(db) = self.inner.db.lock() {
                        if let Err(e) = Self::persist_workspace_tree_hash(
                            &db,
                            &sid,
                            &assistant_msg_id,
                            &hash,
                        ) {
                            tracing::warn!(%e, "workspace tree snapshot persist failed");
                        }
                    }
                } else {
                    tracing::warn!("workspace tree track skipped after assistant (git unavailable)");
                }
            }
            Err(e) => {
                let err = e.to_string();
                self.set_message_error(&sid, &assistant_msg_id, &err)?;
                self.emit(HostOpencodeEvent::MessageInfo {
                    session_id: sid.clone(),
                    message_id: assistant_msg_id.clone(),
                    role: "assistant".to_string(),
                    finish: None,
                });
            }
        }

        self.touch_session_updated(&sid)?;
        self.clear_abort_flag(&sid);
        self.read_prompt_summary(&sid, &assistant_msg_id)
    }

    fn insert_user_and_assistant_placeholder(
        &self,
        session_id: &str,
        request: &BridgePromptRequest,
    ) -> Result<(String, String, String)> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = ?1)",
            params![session_id],
            |r| r.get(0),
        )?;
        if !exists {
            anyhow::bail!("session not found");
        }
        let user_msg_id = format!("msg_{}", Uuid::new_v4());
        let assistant_msg_id = format!("msg_{}", Uuid::new_v4());
        let part_id = format!("prt_{}", Uuid::new_v4());
        let t = now_ms();
        let u_order = Self::next_sort_order(&db, session_id)?;
        let user_text = request.text.trim().to_string();
        let user_info = json!({
            "id": user_msg_id,
            "sessionID": session_id,
            "role": "user",
            "time": { "created": t, "updated": t },
            "agent": request.agent.clone().unwrap_or_else(|| "build".to_string()),
        });
        db.execute(
            "INSERT INTO messages (id, session_id, role, sort_order, info_json) VALUES (?1, ?2, 'user', ?3, ?4)",
            params![
                user_msg_id,
                session_id,
                u_order,
                user_info.to_string(),
            ],
        )?;
        let user_part_id = format!("prt_{}", Uuid::new_v4());
        let user_part = json!({
            "id": user_part_id,
            "messageID": user_msg_id,
            "sessionID": session_id,
            "type": "text",
            "text": user_text,
        });
        db.execute(
            "INSERT INTO parts (id, message_id, session_id, sort_order, json) VALUES (?1, ?2, ?3, 0, ?4)",
            params![user_part_id, user_msg_id, session_id, user_part.to_string()],
        )?;

        let a_order = u_order + 1;
        let default_model = model_from_env();
        let completion_model = request
            .model
            .as_ref()
            .map(|m| m.model_id.as_str())
            .unwrap_or(default_model.as_str());
        let provider_id = request
            .model
            .as_ref()
            .map(|m| m.provider_id.clone())
            .unwrap_or_else(llm_config::default_provider_id_for_ui);
        let assistant_info = json!({
            "id": assistant_msg_id,
            "sessionID": session_id,
            "role": "assistant",
            "time": { "created": t, "updated": t },
            "providerID": provider_id,
            "modelID": completion_model,
        });
        db.execute(
            "INSERT INTO messages (id, session_id, role, sort_order, info_json) VALUES (?1, ?2, 'assistant', ?3, ?4)",
            params![
                assistant_msg_id,
                session_id,
                a_order,
                assistant_info.to_string(),
            ],
        )?;
        let part_json = json!({
            "id": part_id,
            "messageID": assistant_msg_id,
            "sessionID": session_id,
            "type": "text",
            "text": "",
        });
        db.execute(
            "INSERT INTO parts (id, message_id, session_id, sort_order, json) VALUES (?1, ?2, ?3, 0, ?4)",
            params![part_id, assistant_msg_id, session_id, part_json.to_string()],
        )?;
        drop(db);
        self.emit(HostOpencodeEvent::MessageInfo {
            session_id: session_id.to_string(),
            message_id: user_msg_id.clone(),
            role: "user".to_string(),
            finish: Some("stop".to_string()),
        });
        self.emit(HostOpencodeEvent::MessagePartUpsert {
            session_id: session_id.to_string(),
            message_id: user_msg_id.clone(),
            part: crate::opencode::events::HostOpencodePartSummary {
                part_id: user_part_id.clone(),
                message_id: user_msg_id.clone(),
                part_type: "text".to_string(),
                text: Some(request.text.clone()),
                tool: None,
                raw: None,
            },
        });
        self.emit(HostOpencodeEvent::MessageInfo {
            session_id: session_id.to_string(),
            message_id: assistant_msg_id.clone(),
            role: "assistant".to_string(),
            finish: None,
        });
        self.emit(HostOpencodeEvent::MessagePartUpsert {
            session_id: session_id.to_string(),
            message_id: assistant_msg_id.clone(),
            part: crate::opencode::events::HostOpencodePartSummary {
                part_id: part_id.clone(),
                message_id: assistant_msg_id.clone(),
                part_type: "text".to_string(),
                text: Some(String::new()),
                tool: None,
                raw: None,
            },
        });
        Ok((user_msg_id, assistant_msg_id, part_id))
    }

    /// 构造发给 OpenAI 兼容接口的 `messages`：可选 `system`（来自 enrich，不落库）；跳过末尾空 assistant。
    fn build_llm_messages(&self, session_id: &str, system: Option<&str>) -> Result<Vec<Value>> {
        let rows = self.session_messages_blocking(session_id)?;
        let mut messages = Vec::new();
        if let Some(s) = system {
            if !s.trim().is_empty() {
                messages.push(json!({ "role": "system", "content": s }));
            }
        }
        for row in rows {
            let role = row
                .info
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user");
            let mut text = String::new();
            for p in &row.parts {
                if p.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(t) = p.get("text").and_then(Value::as_str) {
                        text.push_str(t);
                    }
                }
            }
            if role == "assistant" && text.is_empty() {
                continue;
            }
            messages.push(json!({ "role": role, "content": text }));
        }
        Ok(messages)
    }

    fn append_part_text(
        &self,
        session_id: &str,
        message_id: &str,
        part_id: &str,
        delta: &str,
    ) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let s: String = db.query_row(
            "SELECT json FROM parts WHERE id = ?1 AND message_id = ?2",
            params![part_id, message_id],
            |r| r.get(0),
        )?;
        let mut v: Value = serde_json::from_str(&s)?;
        let cur = v.get("text").and_then(Value::as_str).unwrap_or("");
        v["text"] = Value::String(format!("{cur}{delta}"));
        let ns = v.to_string();
        db.execute(
            "UPDATE parts SET json = ?1 WHERE id = ?2 AND session_id = ?3",
            params![ns, part_id, session_id],
        )?;
        let t = now_ms();
        let info_s: String = db.query_row(
            "SELECT info_json FROM messages WHERE id = ?1",
            params![message_id],
            |r| r.get(0),
        )?;
        let mut info: Value = serde_json::from_str(&info_s)?;
        if let Some(obj) = info.get_mut("time").and_then(Value::as_object_mut) {
            obj.insert("updated".to_string(), json!(t));
        }
        db.execute(
            "UPDATE messages SET info_json = ?1 WHERE id = ?2",
            params![info.to_string(), message_id],
        )?;
        Ok(())
    }

    fn set_message_error(&self, session_id: &str, message_id: &str, err: &str) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let s: String = db.query_row(
            "SELECT info_json FROM messages WHERE id = ?1",
            params![message_id],
            |r| r.get(0),
        )?;
        let mut info: Value = serde_json::from_str(&s)?;
        info["error"] = json!(err);
        let t = now_ms();
        if let Some(obj) = info.get_mut("time").and_then(Value::as_object_mut) {
            obj.insert("updated".to_string(), json!(t));
        }
        db.execute(
            "UPDATE messages SET info_json = ?1 WHERE id = ?2 AND session_id = ?3",
            params![info.to_string(), message_id, session_id],
        )?;
        Ok(())
    }

    fn finalize_assistant(&self, session_id: &str, message_id: &str, finish: &str) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let s: String = db.query_row(
            "SELECT info_json FROM messages WHERE id = ?1",
            params![message_id],
            |r| r.get(0),
        )?;
        let mut info: Value = serde_json::from_str(&s)?;
        let t = now_ms();
        info["finish"] = json!(finish);
        if let Some(obj) = info.get_mut("time").and_then(Value::as_object_mut) {
            obj.insert("updated".to_string(), json!(t));
        }
        db.execute(
            "UPDATE messages SET info_json = ?1 WHERE id = ?2 AND session_id = ?3",
            params![info.to_string(), message_id, session_id],
        )?;
        Ok(())
    }

    fn touch_session_updated(&self, session_id: &str) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let t = now_ms();
        db.execute(
            "UPDATE sessions SET updated_ms = ?1 WHERE id = ?2",
            params![t, session_id],
        )?;
        Ok(())
    }

    fn read_prompt_summary(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<BridgePromptSummary> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let info_s: String = db.query_row(
            "SELECT info_json FROM messages WHERE id = ?1",
            params![message_id],
            |r| r.get(0),
        )?;
        let info: Value = serde_json::from_str(&info_s)?;
        let mut stmt =
            db.prepare("SELECT json FROM parts WHERE message_id = ?1 ORDER BY sort_order")?;
        let parts: Vec<Value> = stmt
            .query_map(params![message_id], |r| {
                let s: String = r.get(0)?;
                Ok(serde_json::from_str::<Value>(&s).unwrap_or(Value::Null))
            })?
            .collect::<Result<_, _>>()?;
        let mut texts = Vec::new();
        let mut part_types = Vec::new();
        for p in &parts {
            if let Some(pt) = p.get("type").and_then(Value::as_str) {
                part_types.push(pt.to_string());
                if pt == "text" {
                    if let Some(tx) = p.get("text").and_then(Value::as_str) {
                        texts.push(tx.to_string());
                    }
                }
            }
        }
        Ok(BridgePromptSummary {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            provider_id: info
                .get("providerID")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            model_id: info
                .get("modelID")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            finish: info
                .get("finish")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            texts,
            part_types,
            error: info.get("error").cloned(),
        })
    }

    pub fn session_diff_blocking(
        &self,
        session_id: &str,
        message_id: Option<&str>,
        diff_rel: Option<&str>,
    ) -> Result<BridgeDiffSummary> {
        let root = &self.inner.source_root;
        if let Some(mid) = message_id {
            let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
            let row = db.query_row(
                "SELECT diff_text, additions, deletions, captured_ms FROM session_diff_snapshots \
                 WHERE session_id = ?1 AND anchor_message_id = ?2 ORDER BY id DESC LIMIT 1",
                params![session_id, mid],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                },
            );
            drop(db);
            match row {
                Ok((mut diff_text, _add_i, _del_i, cap_ms)) => {
                    if let Some(rel) = diff_rel {
                        diff_text = filter_unified_diff_for_rel_path(&diff_text, rel);
                    }
                    let (additions, deletions) = count_diff_lines(&diff_text);
                    let anchor = match diff_rel {
                        Some(rel) => format!(
                            "(diff snapshot for assistant message {mid}; captured_ms={cap_ms}; path={rel})"
                        ),
                        None => format!(
                            "(diff snapshot for assistant message {mid}; captured_ms={cap_ms})"
                        ),
                    };
                    let file_label = diff_rel
                        .map(str::to_string)
                        .unwrap_or_else(|| anchor.clone());
                    return Ok(BridgeDiffSummary {
                        session_id: session_id.to_string(),
                        message_id: message_id.map(ToString::to_string),
                        additions,
                        deletions,
                        files: vec![BridgeFileDiffSummary {
                            file: file_label,
                            additions,
                            deletions,
                            before: String::new(),
                            after: diff_text,
                        }],
                    });
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(e) => return Err(e.into()),
            }
        }

        let (after, additions, deletions) = git_worktree_diff(root, diff_rel);
        let before = String::new();
        let anchor = message_id
            .map(|m| format!("(live git diff; session anchor {session_id} @ {m}; no snapshot row)"))
            .unwrap_or_else(|| format!("(live git diff; session {session_id})"));
        let file_label = diff_rel.map(str::to_string).unwrap_or_else(|| anchor.clone());
        Ok(BridgeDiffSummary {
            session_id: session_id.to_string(),
            message_id: message_id.map(ToString::to_string),
            additions,
            deletions,
            files: vec![BridgeFileDiffSummary {
                file: file_label,
                additions,
                deletions,
                before: before.clone(),
                after: after.clone(),
            }],
        })
    }

    fn persist_workspace_tree_hash(
        conn: &Connection,
        session_id: &str,
        anchor_message_id: &str,
        tree_hash: &str,
    ) -> Result<()> {
        let t = now_ms();
        conn.execute(
            "INSERT INTO workspace_tree_snapshots (session_id, anchor_message_id, tree_hash, captured_ms) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(session_id, anchor_message_id) DO UPDATE SET \
               tree_hash = excluded.tree_hash, captured_ms = excluded.captured_ms",
            params![session_id, anchor_message_id, tree_hash, t],
        )?;
        Ok(())
    }

    fn query_last_assistant_tree_before_sort(
        conn: &Connection,
        session_id: &str,
        before_sort: i64,
    ) -> Result<Option<String>> {
        let mut stmt = conn
            .prepare(
                "SELECT w.tree_hash FROM workspace_tree_snapshots w \
                 INNER JOIN messages m ON m.id = w.anchor_message_id AND m.session_id = w.session_id \
                 WHERE w.session_id = ?1 AND m.role = 'assistant' AND m.sort_order < ?2 \
                 ORDER BY m.sort_order DESC LIMIT 1",
            )
            .context("prepare last assistant tree")?;
        let mut rows = stmt.query_map(params![session_id, before_sort], |r| {
            r.get::<_, String>(0)
        })?;
        match rows.next() {
            Some(Ok(s)) => Ok(Some(s)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    fn query_session_baseline_tree(
        conn: &Connection,
        session_id: &str,
    ) -> Result<Option<String>> {
        match conn.query_row(
            "SELECT tree_hash FROM workspace_tree_snapshots \
             WHERE session_id = ?1 AND anchor_message_id = ?2",
            params![session_id, SESSION_BASELINE_ANCHOR],
            |r| r.get::<_, String>(0),
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn revert_blocking(
        &self,
        session_id: &str,
        request: &BridgeRevertRequest,
    ) -> Result<BridgeRevertSummary> {
        let snap_git = WorkspaceSnapshotGit::new(self.inner.source_root.clone());

        let (mids, snap_val, target_opt) = {
            let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
            let sort_order: i64 = db.query_row(
                "SELECT sort_order FROM messages WHERE id = ?1 AND session_id = ?2",
                params![request.message_id, session_id],
                |r| r.get(0),
            )?;
            let mids: Vec<String> = db
                .prepare(
                    "SELECT id FROM messages WHERE session_id = ?1 AND sort_order >= ?2 ORDER BY sort_order",
                )?
                .query_map(params![session_id, sort_order], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            if mids.is_empty() {
                let t = now_ms();
                db.execute(
                    "UPDATE sessions SET updated_ms = ?1 WHERE id = ?2",
                    params![t, session_id],
                )?;
                return Ok(BridgeRevertSummary {
                    session_id: session_id.to_string(),
                    message_id: request.message_id.clone(),
                    part_id: request.part_id.clone(),
                    reverted: true,
                });
            }

            let first_so: i64 = db.query_row(
                "SELECT sort_order FROM messages WHERE id = ?1 AND session_id = ?2",
                params![&mids[0], session_id],
                |r| r.get(0),
            )?;

            let mut target_opt =
                Self::query_last_assistant_tree_before_sort(&db, session_id, first_so)?;
            if target_opt.is_none() {
                target_opt = Self::query_session_baseline_tree(&db, session_id)?;
            }

            let snap_val = Self::collect_revert_snapshot(&db, session_id, &mids)?;
            drop(db);
            (mids, snap_val, target_opt)
        };

        let mut snap_val = snap_val;
        if let Some(ref tgt) = target_opt {
            match snap_git.track() {
                Ok(baseline) => match snap_git.restore_worktree(tgt) {
                    Ok(()) => {
                        if let Value::Object(ref mut o) = snap_val {
                            o.insert(
                                "workspace_baseline_tree".to_string(),
                                json!(baseline),
                            );
                            o.insert("workspace_target_tree".to_string(), json!(tgt));
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            %e,
                            "revert: workspace restore failed; continuing message-only revert"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(%e, "revert: baseline track failed; skipping file restore");
                }
            }
        }

        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        if !mids.is_empty() {
            let snap_s = snap_val.to_string();
            db.execute(
                "INSERT INTO revert_journal (session_id, snapshot_json, created_ms) VALUES (?1, ?2, ?3)",
                params![session_id, snap_s, now_ms()],
            )
            .context("insert revert journal")?;
        }
        for mid in &mids {
            db.execute(
                "DELETE FROM workspace_tree_snapshots WHERE session_id = ?1 AND anchor_message_id = ?2",
                params![session_id, mid],
            )
            .ok();
        }
        for mid in &mids {
            db.execute("DELETE FROM parts WHERE message_id = ?1", params![mid])?;
        }
        for mid in &mids {
            db.execute(
                "DELETE FROM messages WHERE id = ?1 AND session_id = ?2",
                params![mid, session_id],
            )?;
        }
        let t = now_ms();
        db.execute(
            "UPDATE sessions SET updated_ms = ?1 WHERE id = ?2",
            params![t, session_id],
        )?;
        Ok(BridgeRevertSummary {
            session_id: session_id.to_string(),
            message_id: request.message_id.clone(),
            part_id: request.part_id.clone(),
            reverted: true,
        })
    }

    fn collect_revert_snapshot(
        conn: &Connection,
        session_id: &str,
        mids: &[String],
    ) -> Result<Value> {
        let mut messages = Vec::new();
        for mid in mids {
            let row: (String, i64, String) = conn.query_row(
                "SELECT role, sort_order, info_json FROM messages WHERE id = ?1 AND session_id = ?2",
                params![mid, session_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let mut pstm = conn.prepare(
                "SELECT id, sort_order, json FROM parts WHERE message_id = ?1 ORDER BY sort_order",
            )?;
            let parts: Vec<Value> = pstm
                .query_map(params![mid], |r| {
                    let pid: String = r.get(0)?;
                    let ps: i64 = r.get(1)?;
                    let pj: String = r.get(2)?;
                    Ok(json!({"id": pid, "sort_order": ps, "json": pj}))
                })?
                .collect::<Result<_, _>>()?;
            messages.push(json!({
                "id": mid,
                "role": row.0,
                "sort_order": row.1,
                "info_json": row.2,
                "parts": parts,
            }));
        }
        Ok(json!({ "messages": messages }))
    }

    pub fn unrevert_blocking(&self, session_id: &str) -> Result<BridgeUnrevertSummary> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let journal: Result<(i64, String), rusqlite::Error> = db.query_row(
            "SELECT id, snapshot_json FROM revert_journal WHERE session_id = ?1 ORDER BY id DESC LIMIT 1",
            params![session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );
        let (jid, snap_s) = match journal {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(BridgeUnrevertSummary {
                    session_id: session_id.to_string(),
                    restored: false,
                });
            }
            Err(e) => return Err(e.into()),
        };
        let snap: Value = serde_json::from_str(&snap_s).context("parse revert snapshot")?;
        let workspace_baseline = snap
            .get("workspace_baseline_tree")
            .and_then(Value::as_str)
            .map(str::to_string);
        let Some(arr) = snap.get("messages").and_then(Value::as_array) else {
            anyhow::bail!("invalid revert snapshot");
        };
        let next_base: i64 = db.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM messages WHERE session_id = ?1",
            params![session_id],
            |r| r.get(0),
        )?;
        let mut order = next_base;
        for m in arr {
            let id = m
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("snapshot message id"))?;
            let role = m.get("role").and_then(Value::as_str).unwrap_or("user");
            let info_json = m.get("info_json").and_then(Value::as_str).unwrap_or("{}");
            db.execute(
                "INSERT INTO messages (id, session_id, role, sort_order, info_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, session_id, role, order, info_json],
            )
            .with_context(|| format!("restore message {id}"))?;
            if let Some(parts) = m.get("parts").and_then(Value::as_array) {
                for (pi, p) in parts.iter().enumerate() {
                    let pid = p
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("part id"))?;
                    let ps: i64 = p
                        .get("sort_order")
                        .and_then(Value::as_i64)
                        .unwrap_or(pi as i64);
                    let pj = p.get("json").and_then(Value::as_str).unwrap_or("{}");
                    db.execute(
                        "INSERT INTO parts (id, message_id, session_id, sort_order, json) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![pid, id, session_id, ps, pj],
                    )
                    .with_context(|| format!("restore part {pid}"))?;
                }
            }
            order += 1;
        }
        if let Some(ref h) = workspace_baseline {
            if let Err(e) =
                WorkspaceSnapshotGit::new(self.inner.source_root.clone()).restore_worktree(h)
            {
                tracing::warn!(%e, "unrevert: workspace baseline restore failed");
            }
        }
        db.execute("DELETE FROM revert_journal WHERE id = ?1", params![jid])
            .context("delete revert journal")?;
        let t = now_ms();
        db.execute(
            "UPDATE sessions SET updated_ms = ?1 WHERE id = ?2",
            params![t, session_id],
        )?;
        Ok(BridgeUnrevertSummary {
            session_id: session_id.to_string(),
            restored: true,
        })
    }

    fn read_file_requires_user_confirmation(rel: &str) -> bool {
        let n = rel.replace('\\', "/");
        n.contains(".mei/agent.sqlite")
    }

    fn run_read_file_tool(
        &self,
        session_id: &str,
        _call_id: &str,
        name: &str,
        args_json: &str,
        app_id: Option<&str>,
    ) -> String {
        if name != "read_file" {
            return format!("error: tool `{name}` is not allowed");
        }
        let args: Value = match serde_json::from_str(args_json) {
            Ok(v) => v,
            Err(e) => return format!("error: invalid tool arguments JSON: {e}"),
        };
        let raw = args.get("path").and_then(Value::as_str).unwrap_or("");
        if raw.trim().is_empty() {
            return "error: path is required".to_string();
        }
        if llm::sanitize_relative_path(raw).is_none() {
            if let Err(e) = self.insert_blocked_notice(session_id, raw, "external_directory") {
                tracing::warn!(%e, "failed to record blocked permission notice");
            }
            return "error: path must be a relative path without '..'".to_string();
        }
        let rel = llm::sanitize_relative_path(raw).unwrap_or_default();
        if Self::read_file_requires_user_confirmation(&rel) {
            return match self.request_read_confirmation_and_wait(session_id, &rel, args_json) {
                Ok(()) => self.try_read_file_with_app_prefix(args_json, app_id, &rel),
                Err(e) => format!("error: {e}"),
            };
        }
        self.try_read_file_with_app_prefix(args_json, app_id, &rel)
    }

    /// 若路径未包含 `app_id/` 且首次读失败，则自动重试 `{app_id}/{path}`（workspace 根相对）。
    fn try_read_file_with_app_prefix(
        &self,
        args_json: &str,
        app_id: Option<&str>,
        rel: &str,
    ) -> String {
        let first = llm::execute_read_file_under_root(&self.inner.source_root, args_json);
        if !first.starts_with("error: file not found:") {
            return first;
        }
        let n = rel.replace('\\', "/");
        let Some(aid) = app_id.map(str::trim).filter(|a| !a.is_empty()) else {
            return first;
        };
        let prefix = format!("{}/", aid.trim_end_matches('/'));
        if n.starts_with(&prefix) || n == aid {
            return first;
        }
        let retry_rel = format!(
            "{}/{}",
            aid.trim_end_matches('/'),
            n.trim_start_matches('/')
        );
        let Some(sanitized) = llm::sanitize_relative_path(&retry_rel) else {
            return first;
        };
        if Self::read_file_requires_user_confirmation(&sanitized) {
            return first;
        }
        let retry_args = serde_json::to_string(&json!({ "path": sanitized }))
            .unwrap_or_else(|_| args_json.to_string());
        let second = llm::execute_read_file_under_root(&self.inner.source_root, &retry_args);
        if second.starts_with("error:") {
            first
        } else {
            second
        }
    }

    fn insert_pending_row(
        &self,
        session_id: &str,
        permission: &str,
        patterns: &[String],
        metadata: Value,
        queue_kind: &str,
    ) -> Result<String> {
        let id = format!("perm_{}", Uuid::new_v4());
        let patterns_json = serde_json::to_string(patterns).context("patterns json")?;
        let metadata_json = metadata.to_string();
        let t = now_ms();
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        db.execute(
            "INSERT INTO pending_permissions (id, session_id, permission, patterns_json, metadata_json, queue_kind, resolution, created_ms, updated_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?7)",
            params![
                id,
                session_id,
                permission,
                patterns_json,
                metadata_json,
                queue_kind,
                t,
            ],
        )
        .with_context(|| format!("insert pending permission {id}"))?;
        Ok(id)
    }

    fn insert_blocked_notice(
        &self,
        session_id: &str,
        raw_path: &str,
        permission: &str,
    ) -> Result<()> {
        let patterns = vec![raw_path.to_string()];
        let id = self.insert_pending_row(
            session_id,
            permission,
            &patterns,
            json!({"source": "native_tool_guard"}),
            "blocked_notice",
        )?;
        let (path, requires_admin, message) =
            permission_policy::classify_blocked_permission(permission, &patterns);
        self.emit(HostOpencodeEvent::PermissionBlocked {
            session_id: session_id.to_string(),
            permission_id: id,
            permission: permission.to_string(),
            path,
            patterns,
            requires_admin,
            message,
        });
        Ok(())
    }

    fn peek_permission_resolution(
        &self,
        permission_id: &str,
        session_id: &str,
    ) -> Result<Option<String>> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let row: Result<String, rusqlite::Error> = db.query_row(
            "SELECT resolution FROM pending_permissions WHERE id = ?1 AND session_id = ?2",
            params![permission_id, session_id],
            |r| r.get(0),
        );
        match row {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn force_reject_permission(&self, permission_id: &str, session_id: &str) {
        let t = now_ms();
        if let Ok(db) = self.inner.db.lock() {
            let _ = db.execute(
                "UPDATE pending_permissions SET resolution = 'rejected', updated_ms = ?1 \
                 WHERE id = ?2 AND session_id = ?3 AND resolution = 'pending'",
                params![t, permission_id, session_id],
            );
        }
    }

    fn wait_pending_permission_resolution(
        &self,
        permission_id: &str,
        session_id: &str,
    ) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if Instant::now() > deadline {
                self.force_reject_permission(permission_id, session_id);
                anyhow::bail!("permission wait timed out");
            }
            match self.peek_permission_resolution(permission_id, session_id)? {
                Some(s) if s == "approved" => return Ok(()),
                Some(s) if s == "rejected" => anyhow::bail!("permission denied by user"),
                Some(s) if s == "pending" => thread::sleep(Duration::from_millis(50)),
                None => anyhow::bail!("permission request missing"),
                Some(other) => anyhow::bail!("unexpected permission state: {other}"),
            }
        }
    }

    fn request_read_confirmation_and_wait(
        &self,
        session_id: &str,
        rel: &str,
        _args_json: &str,
    ) -> Result<()> {
        let patterns = vec![rel.to_string()];
        let id = self.insert_pending_row(
            session_id,
            "external_directory",
            &patterns,
            json!({"tool": "read_file", "path": rel}),
            "awaiting_user",
        )?;
        self.emit(HostOpencodeEvent::PermissionRequested {
            session_id: session_id.to_string(),
            permission_id: id.clone(),
            permission: "external_directory".to_string(),
            patterns,
            metadata: json!({"tool": "read_file", "path": rel}),
        });
        self.wait_pending_permission_resolution(&id, session_id)
    }

    pub fn list_pending_permissions_blocking(&self) -> Vec<BridgePendingPermission> {
        let db = match self.inner.db.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let mut stmt = match db.prepare(
            "SELECT id, session_id, permission, patterns_json, metadata_json \
             FROM pending_permissions WHERE queue_kind = 'blocked_notice' AND resolution = 'pending' \
             ORDER BY created_ms ASC",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = match stmt.query_map([], |r| {
            let id: String = r.get(0)?;
            let session_id: String = r.get(1)?;
            let permission: String = r.get(2)?;
            let patterns_json: String = r.get(3)?;
            let metadata_json: String = r.get(4)?;
            let patterns: Vec<String> =
                serde_json::from_str(&patterns_json).unwrap_or_else(|_| Vec::new());
            let metadata: Value =
                serde_json::from_str(&metadata_json).unwrap_or_else(|_| Value::Null);
            Ok(BridgePendingPermission {
                id,
                session_id,
                permission,
                patterns,
                metadata,
            })
        }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn respond_permission_blocking(
        &self,
        session_id: &str,
        permission_id: &str,
        req: &BridgePermissionResponseRequest,
    ) -> Result<BridgePermissionResponseSummary> {
        let pid = permission_id.trim();
        if pid.is_empty() {
            anyhow::bail!("permission_id is required");
        }
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let current: String = db
            .query_row(
                "SELECT resolution FROM pending_permissions WHERE id = ?1 AND session_id = ?2",
                params![pid, session_id],
                |r| r.get(0),
            )
            .context("permission not found")?;
        if current != "pending" {
            anyhow::bail!("permission already resolved");
        }
        let r = req.response.trim().to_ascii_lowercase();
        let new_resolution = if r == "approve" || r == "approved" {
            "approved"
        } else if r == "reject" || r == "rejected" {
            "rejected"
        } else {
            anyhow::bail!("response must be approve or reject");
        };
        let t = now_ms();
        db.execute(
            "UPDATE pending_permissions SET resolution = ?1, updated_ms = ?2 WHERE id = ?3 AND session_id = ?4",
            params![new_resolution, t, pid, session_id],
        )
        .context("update permission")?;
        drop(db);
        self.emit(HostOpencodeEvent::PermissionResolved {
            session_id: session_id.to_string(),
            permission_id: pid.to_string(),
            response: req.response.clone(),
        });
        Ok(BridgePermissionResponseSummary {
            session_id: session_id.to_string(),
            permission_id: pid.to_string(),
            response: req.response.clone(),
            applied: true,
        })
    }
}

#[cfg(test)]
#[test]
fn blocked_notice_from_bad_path_lists_and_reject() {
    use std::fs;

    let dir = std::env::temp_dir().join(format!("mei_native_perm_{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let agent = NativeAgent::open(dir.clone()).unwrap();
    let sid = "ses_test";
    let out = agent.run_read_file_tool(sid, "", "read_file", r#"{"path":"../bad"}"#, None);
    assert!(out.contains("error"), "{out}");
    let pending = agent.list_pending_permissions_blocking();
    assert_eq!(pending.len(), 1, "{pending:?}");
    let id = pending[0].id.clone();
    agent
        .respond_permission_blocking(
            sid,
            &id,
            &BridgePermissionResponseRequest {
                response: "reject".into(),
            },
        )
        .unwrap();
    assert!(agent.list_pending_permissions_blocking().is_empty());
    let _ = fs::remove_dir_all(&dir);
}

fn model_from_env() -> String {
    llm_config::resolve_llm(None)
        .map(|c| c.model)
        .unwrap_or_default()
}

fn normalize_diff_rel_path(rel: &str) -> String {
    rel.replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn paths_match_for_diff_filter(git_path: &str, rel: &str) -> bool {
    let g = normalize_diff_rel_path(git_path);
    let r = normalize_diff_rel_path(rel);
    if g.is_empty() || r.is_empty() {
        return false;
    }
    g == r || g.ends_with(&format!("/{r}")) || r.ends_with(&format!("/{g}"))
}

fn unified_diff_git_line_matches_path(git_line: &str, rel: &str) -> bool {
    let parts: Vec<&str> = git_line.split_whitespace().collect();
    if parts.len() < 4 || parts[0] != "diff" || parts[1] != "--git" {
        return false;
    }
    for token in parts.iter().skip(2) {
        let p = token
            .strip_prefix("a/")
            .or_else(|| token.strip_prefix("b/"))
            .unwrap_or(token);
        if paths_match_for_diff_filter(p, rel) {
            return true;
        }
    }
    false
}

/// 从整工作区 unified diff 中只保留与 `rel` 对应文件的 hunk（兼容旧版「全仓快照」）。
fn filter_unified_diff_for_rel_path(diff: &str, rel: &str) -> String {
    let r = normalize_diff_rel_path(rel);
    if r.is_empty() {
        return diff.to_string();
    }
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut keep = false;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            if !current.is_empty() && keep {
                blocks.push(current.join("\n"));
            }
            current.clear();
            keep = unified_diff_git_line_matches_path(line, &r);
            current.push(line);
        } else if !current.is_empty() {
            current.push(line);
        }
    }
    if !current.is_empty() && keep {
        blocks.push(current.join("\n"));
    }
    blocks.join("\n")
}

fn git_worktree_diff(root: &std::path::Path, rel: Option<&str>) -> (String, u64, u64) {
    let root_s = root.as_os_str().to_str().unwrap_or("");
    let mut cmd = Command::new("git");
    cmd.args(["-C", root_s, "diff", "--no-color"]);
    if let Some(p) = rel {
        if !p.is_empty() {
            cmd.arg("--");
            cmd.arg(p);
        }
    }
    match cmd.output() {
        Ok(o) if o.status.success() => {
            let diff = String::from_utf8_lossy(&o.stdout).to_string();
            let (a, d) = count_diff_lines(&diff);
            (diff, a, d)
        }
        _ => (String::new(), 0, 0),
    }
}

fn count_diff_lines(diff: &str) -> (u64, u64) {
    let mut add = 0u64;
    let mut del = 0u64;
    for line in diff.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            add += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            del += 1;
        }
    }
    (add, del)
}

fn event_matches_session(ev: &HostOpencodeEvent, session_id: &str) -> bool {
    match ev {
        HostOpencodeEvent::SessionStatus { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::MessageInfo { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::MessagePartUpsert { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::MessagePartDelta { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::MessagePartRemoved { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::PermissionRequested { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::PermissionBlocked { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::PermissionResolved { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::DebugRawEvent { session_id: s, .. } => s.as_deref() == Some(session_id),
    }
}

pub fn encode_host_event_line(ev: &HostOpencodeEvent) -> Option<String> {
    serde_json::to_string(ev)
        .ok()
        .map(|s| format!("data: {s}\n\n"))
}

pub fn filter_session_event(ev: HostOpencodeEvent, session_id: &str) -> Option<HostOpencodeEvent> {
    event_matches_session(&ev, session_id).then_some(ev)
}
