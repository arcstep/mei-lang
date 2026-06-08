use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::AtomicBool,
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::Connection;
use tokio::sync::broadcast;
use crate::agent_runtime::{
    bridge::BridgeHealthResponse,
    events::HostOpencodeEvent,
};

use super::{
    llm_config,
    resource_tools::{NoopResourceToolExecutor, ResourceToolExecutor},
};

mod diff;
mod events;
mod prompt;
mod session_io;
mod session_snapshot;
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
        Self::open_with_resource_tools(
            source_root,
            Arc::new(NoopResourceToolExecutor::default()),
        )
    }

    pub fn open_with_resource_tools(
        source_root: PathBuf,
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
                event_tx,
                abort_flags: Mutex::new(HashMap::new()),
                resource_tools,
            }),
        })
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<HostOpencodeEvent> {
        self.inner.event_tx.subscribe()
    }

    pub(super) fn emit(&self, ev: HostOpencodeEvent) {
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
        use crate::http::agent_api::AUTHORING_WRITEBACK_RETIRED_HISTORY_HINT;

        let mut history_reason =
            Some(AUTHORING_WRITEBACK_RETIRED_HISTORY_HINT.to_string());
        if !llm_ok {
            history_reason = Some(
                "内置助手缺少 LLM 环境变量：若设置了 OPENAI_IMITATORS，则每个前缀需 {PREFIX}_BASE_URL、{PREFIX}_API_KEY、{PREFIX}_COMPLETION_MODEL（逗号分隔多模型时首项为默认）；未设置时沿用 QWEN_* 或 MEI_LLM_OPENAI_*。可用 MEI_LLM_DEFAULT_PROVIDER 指定默认前缀。"
                    .to_string(),
            );
        }
        let history_available = false;
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
}

#[cfg(test)]
#[test]
fn blocked_notice_from_bad_path_lists_and_reject() {
    use crate::agent_runtime::bridge::BridgePermissionResponseRequest;
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
