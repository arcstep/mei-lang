use std::thread;

use serde_json::{json, Value};

use super::super::super::{resource_tools::AgentResourceScope, skill_tools};

use super::super::NativeAgent;

impl NativeAgent {
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
        agent_mode: &str,
        app_id: Option<&str>,
        resource_scope: &AgentResourceScope,
    ) -> String {
        let build_mode = agent_mode.trim().eq_ignore_ascii_case("build");
        match name {
            "read_file" => {
                self.run_read_file_tool(session_id, call_id, name, args, app_id, resource_scope)
            }
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
}
