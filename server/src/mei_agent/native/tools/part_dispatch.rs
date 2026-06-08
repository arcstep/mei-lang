use std::thread;

use serde_json::{json, Value};

use super::super::super::resource_tools::AgentResourceScope;

use super::super::NativeAgent;

impl NativeAgent {
    fn run_propose_session_patch_tool(
        &self,
        session_id: &str,
        call_id: &str,
        args: &str,
    ) -> String {
        let args_val: Value = serde_json::from_str(args).unwrap_or_else(|_| json!({}));
        let ops = match args_val.get("ops").and_then(Value::as_array) {
            Some(items) if !items.is_empty() => items,
            _ => {
                return "error: propose_session_patch requires a non-empty `ops` array".to_string()
            }
        };
        if ops.len() > 8 {
            return "error: propose_session_patch supports at most 8 ops per offer".to_string();
        }
        let mut normalized_ops = Vec::new();
        for raw in ops {
            let Some(obj) = raw.as_object() else {
                return "error: each session patch op must be an object".to_string();
            };
            let op_type = obj
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("");
            if op_type.is_empty() {
                return "error: session patch op `type` is required".to_string();
            }
            let panel_id = obj
                .get("panel_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty());
            let query_state_id = obj
                .get("query_state_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty());
            let note = obj
                .get("note")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string);
            match op_type {
                "hide_panel" | "highlight_panel" | "move_panel_front" => {
                    if panel_id.is_none() {
                        return format!(
                            "error: session patch op `{op_type}` requires non-empty panel_id"
                        );
                    }
                }
                "focus_query_state" => {
                    if query_state_id.is_none() {
                        return "error: session patch op `focus_query_state` requires non-empty query_state_id".to_string();
                    }
                }
                other => {
                    return format!(
                        "error: unsupported session patch op `{other}` (allowed: hide_panel, highlight_panel, move_panel_front, focus_query_state)"
                    );
                }
            }
            let mut next = serde_json::Map::new();
            next.insert("type".to_string(), Value::String(op_type.to_string()));
            if let Some(panel_id) = panel_id {
                next.insert("panel_id".to_string(), Value::String(panel_id.to_string()));
            }
            if let Some(query_state_id) = query_state_id {
                next.insert(
                    "query_state_id".to_string(),
                    Value::String(query_state_id.to_string()),
                );
            }
            if let Some(note) = note {
                next.insert("note".to_string(), Value::String(note));
            }
            normalized_ops.push(Value::Object(next));
        }

        let title = args_val
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("session patch offer");
        let summary = format!(
            "{title} (ops={}, non_persistent=true, scope=session)",
            normalized_ops.len()
        );
        let offer = json!({
            "schema": "mei_session_patch_offer_v1",
            "status": "proposed",
            "summary": summary,
            "session_id": session_id,
            "call_id": call_id,
            "patch": {
                "schema": "mei_session_patch_v1",
                "patch_id": format!("sespatch-{call_id}"),
                "expires_with": "session",
                "non_persistent_by_default": true,
                "ops": normalized_ops,
            }
        });
        serde_json::to_string(&offer)
            .unwrap_or_else(|e| format!("error: failed to serialize session patch offer: {e}"))
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
    pub(crate) fn dispatch_tool_calls_batch_parallel(
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

    pub(crate) fn execute_tool_body(
        &self,
        session_id: &str,
        call_id: &str,
        name: &str,
        args: &str,
        _agent_mode: &str,
        app_id: Option<&str>,
        resource_scope: &AgentResourceScope,
    ) -> String {
        match name {
            "read_file" => {
                self.run_read_file_tool(session_id, call_id, name, args, app_id, resource_scope)
            }
            "dataset_query"
            | "dataset_metric"
            | "resource_list"
            | "resource_get"
            | "resource_runtime_peek" => self.inner.resource_tools.run_resource_tool(
                &self.inner.source_root,
                app_id,
                resource_scope,
                name,
                args,
            ),
            "propose_session_patch" => self.run_propose_session_patch_tool(session_id, call_id, args),
            "skill_list" | "skill_read" | "rewrite_current_mei" => "error: authoring-only tools are no longer available in the built-in Mei access runtime; use external dev tools plus mei CLI/LSP for editing".to_string(),
            other => format!("error: tool `{other}` is not allowed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_agent() -> NativeAgent {
        let root =
            std::env::temp_dir().join(format!("mei-native-session-patch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create test root");
        NativeAgent::open(root).expect("open native agent")
    }

    #[test]
    fn propose_session_patch_tool_returns_offer_payload() {
        let agent = build_test_agent();
        let out = agent.execute_tool_body(
            "ses_1",
            "call_1",
            "propose_session_patch",
            r#"{"title":"临时观察","ops":[{"type":"highlight_panel","panel_id":"sales_overview"}]}"#,
            "ask",
            Some("examples/ds/01-dataset-baseline"),
            &AgentResourceScope::default(),
        );
        let payload: serde_json::Value = serde_json::from_str(&out).expect("offer json");
        assert_eq!(payload["schema"], "mei_session_patch_offer_v1");
        assert_eq!(payload["patch"]["schema"], "mei_session_patch_v1");
        assert_eq!(payload["patch"]["ops"][0]["type"], "highlight_panel");
    }

    #[test]
    fn propose_session_patch_tool_validates_required_target() {
        let agent = build_test_agent();
        let out = agent.execute_tool_body(
            "ses_1",
            "call_2",
            "propose_session_patch",
            r#"{"ops":[{"type":"hide_panel"}]}"#,
            "ask",
            Some("examples/ds/01-dataset-baseline"),
            &AgentResourceScope::default(),
        );
        assert!(
            out.contains("requires non-empty panel_id"),
            "unexpected output: {out}"
        );
    }
}
