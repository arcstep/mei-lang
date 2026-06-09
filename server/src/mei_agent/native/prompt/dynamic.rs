use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use rusqlite::params;
use serde_json::json;
use uuid::Uuid;

use crate::agent_runtime::{
    bridge::{BridgePromptRequest, BridgePromptSummary},
    events::HostOpencodeEvent,
};

use super::super::super::{
    llm, llm_config,
    resource_tools::{self, AgentResourceScope},
    workspace_snapshot_git::WorkspaceSnapshotGit,
};
use super::super::{model_from_env, now_ms, NativeAgent};
impl NativeAgent {
    pub async fn send_prompt(
        &self,
        session_id: &str,
        request: BridgePromptRequest,
        resource_scope: AgentResourceScope,
        scope_meta: Option<(String, String)>,
    ) -> Result<BridgePromptSummary> {
        let this = self.clone();
        let sid = session_id.to_string();
        tokio::task::spawn_blocking(move || {
            this.send_prompt_blocking(&sid, request, resource_scope, scope_meta)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn prompt: {e}"))?
    }

    fn send_prompt_blocking(
        &self,
        session_id: &str,
        request: BridgePromptRequest,
        resource_scope: AgentResourceScope,
        scope_meta: Option<(String, String)>,
    ) -> Result<BridgePromptSummary> {
        let (_user_msg_id, assistant_msg_id, part_id) =
            self.insert_user_and_assistant_placeholder(session_id, &request, &resource_scope)?;

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
        let agent_mode = request
            .mode
            .as_deref()
            .or(request.agent.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("build")
            .to_string();
        let tools = resource_tools::tool_definitions_for_profile(
            &agent_mode,
            resource_scope.resource_visibility,
        );
        let resource_scope_tool = resource_scope.clone();
        let app_id = request.app_id.clone();
        let agent_mode_tool = agent_mode.clone();
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
                    &agent_mode_tool,
                    app_id.as_deref(),
                    &resource_scope_tool,
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
                if let Err(e) =
                    self.capture_session_diff_snapshot(&sid, &assistant_msg_id, diff_rel.as_deref())
                {
                    tracing::warn!(%e, "session diff snapshot failed");
                }
                match WorkspaceSnapshotGit::new(self.inner.source_root.clone())
                    .and_then(|snap_git| snap_git.track())
                {
                    Ok(hash) => {
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
                    }
                    Err(e) => {
                        tracing::warn!(%e, "workspace tree track skipped after assistant");
                    }
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
        let mut summary = self.read_prompt_summary(&sid, &assistant_msg_id)?;
        if let Some((digest, profile)) = scope_meta {
            summary.scope_digest = Some(digest);
            summary.profile_summary = Some(profile);
        }
        Ok(summary)
    }

    fn insert_user_and_assistant_placeholder(
        &self,
        session_id: &str,
        request: &BridgePromptRequest,
        resource_scope: &AgentResourceScope,
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
        let normalized_mode = request
            .mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("build");
        let normalized_route_mode = request
            .route_mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("manage");
        let normalized_agent = request
            .agent
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(normalized_mode);
        let user_info = json!({
            "id": user_msg_id,
            "sessionID": session_id,
            "role": "user",
            "time": { "created": t, "updated": t },
            "agent": normalized_agent,
            "mode": normalized_mode,
            "routeMode": normalized_route_mode,
            "resourceVisibility": resource_scope.resource_visibility.as_slug(),
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
            part: crate::agent_runtime::events::HostOpencodePartSummary {
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
            part: crate::agent_runtime::events::HostOpencodePartSummary {
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
}
