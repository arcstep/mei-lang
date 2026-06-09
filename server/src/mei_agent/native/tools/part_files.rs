use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use rusqlite::params;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent_runtime::events::HostOpencodeEvent;

use super::super::super::{
    agent_scope_profile, llm, permission_policy, resource_tools::AgentResourceScope,
};

use super::super::{now_ms, NativeAgent};

impl NativeAgent {
    fn read_file_requires_user_confirmation(rel: &str) -> bool {
        let n = rel.replace('\\', "/");
        n.contains(".mei/local/agent/agent.sqlite") || n.contains(".mei/agent.sqlite")
    }

    pub(crate) fn run_read_file_tool(
        &self,
        session_id: &str,
        _call_id: &str,
        name: &str,
        args_json: &str,
        app_id: Option<&str>,
        resource_scope: &AgentResourceScope,
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
        if !agent_scope_profile::read_file_allowed_for_agent(&rel, app_id, resource_scope) {
            if let Err(e) = self.insert_blocked_notice(session_id, raw, "scope_denied") {
                tracing::warn!(%e, "failed to record blocked permission notice");
            }
            return format!(
                "error: read_file denied by resource visibility `{}` for current request scope; path is outside the resolved direct-ref / scene-reachable set. Try widening visibility in the author panel and retry.",
                resource_scope.resource_visibility.as_slug()
            );
        }
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
}
