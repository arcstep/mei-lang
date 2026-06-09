use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection};
use serde_json::Value;
use uuid::Uuid;

use crate::agent_runtime::bridge::{
    BridgeAbortSummary, BridgeCreateSessionRequest, BridgePendingPermission,
    BridgePermissionResponseRequest, BridgePermissionResponseSummary, BridgeSessionMessageRaw,
    BridgeSessionSummary,
};
use crate::agent_runtime::events::HostOpencodeEvent;

use super::super::workspace_snapshot_git::{WorkspaceSnapshotGit, SESSION_BASELINE_ANCHOR};

use super::{now_ms, NativeAgent};

impl NativeAgent {
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
        match WorkspaceSnapshotGit::new(self.inner.source_root.clone()).and_then(|snap_git| snap_git.track()) {
            Ok(hash) => {
                if let Ok(db) = self.inner.db.lock() {
                    if let Err(e) =
                        Self::persist_workspace_tree_hash(&db, &id, SESSION_BASELINE_ANCHOR, &hash)
                    {
                        tracing::warn!(%e, "session baseline tree snapshot persist failed");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    %e,
                    "session baseline tree track skipped (git unavailable or worktree not ready)"
                );
            }
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

    pub(super) fn next_sort_order(conn: &Connection, session_id: &str) -> Result<i64> {
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

    pub(super) fn take_abort_flag(&self, session_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut g) = self.inner.abort_flags.lock() {
            g.insert(session_id.to_string(), flag.clone());
        }
        flag
    }

    pub(super) fn clear_abort_flag(&self, session_id: &str) {
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
