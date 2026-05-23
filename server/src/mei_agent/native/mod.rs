use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection};
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::agent_runtime::{
    bridge::{
        BridgeAbortSummary, BridgeCreateSessionRequest, BridgeHealthResponse,
        BridgePendingPermission, BridgePermissionResponseRequest, BridgePermissionResponseSummary,
        BridgeSessionMessageRaw, BridgeSessionSummary,
    },
    events::HostOpencodeEvent,
};

use super::{
    llm_config,
    resource_tools::{NoopResourceToolExecutor, ResourceToolExecutor},
    workspace_snapshot_git::{WorkspaceSnapshotGit, SESSION_BASELINE_ANCHOR},
};

mod diff;
mod events;
mod prompt;
mod session_io;
mod tools;

pub use events::{encode_host_event_line, filter_session_event};

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

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(super) fn model_from_env() -> String {
    llm_config::resolve_llm(None)
        .map(|c| c.model)
        .unwrap_or_default()
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
                if let Err(e) =
                    Self::persist_workspace_tree_hash(&db, &id, SESSION_BASELINE_ANCHOR, &hash)
                {
                    tracing::warn!(%e, "session baseline tree snapshot persist failed");
                }
            }
        } else {
            tracing::warn!(
                "session baseline tree track skipped (git unavailable or worktree not ready)"
            );
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
    use crate::mei_agent::resource_tools::AgentResourceScope;
    use std::fs;

    let dir = std::env::temp_dir().join(format!("mei_native_perm_{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let agent = NativeAgent::open(dir.clone()).unwrap();
    let sid = "ses_test";
    let scope = AgentResourceScope::default();
    let out = agent.run_read_file_tool(sid, "", "read_file", r#"{"path":"../bad"}"#, None, &scope);
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
