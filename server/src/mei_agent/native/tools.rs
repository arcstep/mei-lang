use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use rusqlite::params;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent_runtime::events::{host_part_summary_from_stored, HostOpencodeEvent};

use super::super::{
    agent_scope_profile, llm, permission_policy, resource_tools::AgentResourceScope, skill_tools,
};

use super::{NativeAgent, now_ms};
impl NativeAgent {
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
            "rewrite_current_mei" => {
                let path = args
                    .get("target_file")
                    .and_then(Value::as_str)
                    .unwrap_or("<current-target>");
                (Some("rewrite_current_mei".into()), Some(path.to_string()))
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
    pub(super) fn ensure_assistant_text_continuation_part(
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
        agent_mode: &str,
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

        let output_raw = self.execute_tool_body(
            session_id,
            call_id,
            name,
            args,
            agent_mode,
            app_id,
            resource_scope,
        );

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
    pub(super) fn dispatch_tool_calls_batch_parallel(
        &self,
        session_id: &str,
        assistant_message_id: &str,
        batch: &[(String, String, String)],
        agent_mode: &str,
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
                agent_mode,
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
        let mode_owned = agent_mode.to_string();

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
                let mode_i = mode_owned.clone();
                handles.push(s.spawn(move || {
                    agent.execute_tool_body(
                        &sid_i,
                        &cid,
                        n.as_str(),
                        a.as_str(),
                        mode_i.as_str(),
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
        agent_mode: &str,
        app_id: Option<&str>,
        resource_scope: &AgentResourceScope,
    ) -> String {
        let build_mode = agent_mode.trim().eq_ignore_ascii_case("build");
        match name {
            "read_file" => self.run_read_file_tool(session_id, call_id, name, args, app_id, resource_scope),
            "dataset_query" | "dataset_metric" => self.inner.resource_tools.run_resource_tool(
                &self.inner.source_root,
                app_id,
                resource_scope,
                name,
                args,
            ),
            "rewrite_current_mei" if build_mode => {
                self.run_rewrite_current_mei_tool(args, resource_scope)
            }
            "skill_list" if build_mode => skill_tools::execute_skill_list(
                &self.inner.skill_package_root,
                &self.inner.source_root,
            ),
            "skill_read" if build_mode => skill_tools::execute_skill_read(
                &self.inner.skill_package_root,
                &self.inner.source_root,
                args,
            ),
            "skill_list" | "skill_read" | "rewrite_current_mei" => {
                format!(
                    "error: tool `{name}` is disabled in `{}` mode",
                    agent_mode.trim()
                )
            }
            other => format!("error: tool `{other}` is not allowed"),
        }
    }
    fn read_file_requires_user_confirmation(rel: &str) -> bool {
        let n = rel.replace('\\', "/");
        n.contains(".mei/agent.sqlite")
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

    fn run_rewrite_current_mei_tool(
        &self,
        args_json: &str,
        resource_scope: &AgentResourceScope,
    ) -> String {
        let args: Value = match serde_json::from_str(args_json) {
            Ok(v) => v,
            Err(e) => return format!("error: invalid tool arguments JSON: {e}"),
        };
        let content = args.get("content").and_then(Value::as_str).unwrap_or("");
        if content.trim().is_empty() {
            return "error: content is required".to_string();
        }
        let Some(scope_target) = resource_scope
            .target_file
            .as_deref()
            .and_then(llm::sanitize_relative_path)
        else {
            return "error: current request has no valid target_file; rewrite_current_mei requires a scoped `.mei` target".to_string();
        };
        if !scope_target.to_ascii_lowercase().ends_with(".mei") {
            return format!(
                "error: target_file `{scope_target}` is not a `.mei` file; rewrite_current_mei only supports `.mei`"
            );
        }
        if let Some(raw_target) = args.get("target_file").and_then(Value::as_str) {
            let Some(arg_target) = llm::sanitize_relative_path(raw_target) else {
                return "error: target_file must be a relative path without `..`".to_string();
            };
            if arg_target != scope_target {
                return format!(
                    "error: target_file mismatch; expected `{scope_target}`, got `{arg_target}`"
                );
            }
        }
        let full = self.inner.source_root.join(&scope_target);
        let Some(parent) = full.parent() else {
            return format!("error: invalid target path `{}`", full.display());
        };
        if let Err(e) = std::fs::create_dir_all(parent) {
            return format!("error: failed to create target parent directory: {e}");
        }
        let Ok(canonical_root) = self.inner.source_root.canonicalize() else {
            return "error: cannot canonicalize workspace root".to_string();
        };
        let Ok(canonical_parent) = parent.canonicalize() else {
            return "error: cannot canonicalize target parent directory".to_string();
        };
        if !canonical_parent.starts_with(&canonical_root) {
            return "error: target path escapes workspace root".to_string();
        }
        if let Err(e) = std::fs::write(&full, content.as_bytes()) {
            return format!(
                "error: failed to write target file `{}`: {e}",
                full.display()
            );
        }
        format!(
            "ok: rewrote `{}` ({} bytes)",
            scope_target.replace('\\', "/"),
            content.len()
        )
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
