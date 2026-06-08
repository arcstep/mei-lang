use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
    Json,
};
use mei_lang_kernel::{host_runtime_capabilities_catalog, host_runtime_contract_descriptor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    agent_runtime::bridge::BridgePromptRequest,
    mei_agent::{
        agent_scope_profile::resolve_resource_visibility, mode_policy::AgentModePolicy,
        resource_tools,
    },
    AppState,
};

use crate::http::agent_api::prompt_context::{
    build_dynamic_session_context_preview, enrich_prompt_request, prepare_prompt_request,
    AgentScopeBundle,
};
use crate::http::error_response;
use crate::http::scene_api::{default_resource_query_tools, RESOURCE_QUERY_SCHEMA_VERSION};

/// 为 preview 的 `resource_inventory` 增加 `reach_tier`（direct | scene | other），供前端按边界语义分组。
fn enrich_resource_inventory_preview_value(
    inv: &crate::http::scene_api::ResourceInventorySnapshot,
    rs: &resource_tools::AgentResourceScope,
    app_id: &str,
) -> Value {
    use crate::http::scene_api::ResourceInventoryItem;
    use crate::mei_agent::agent_scope_profile::resource_inventory_reach_tier;

    let mut root = match serde_json::to_value(inv) {
        Ok(v) => v,
        Err(_) => return Value::Null,
    };
    if let Some(arr) = root.get_mut("items").and_then(|x| x.as_array_mut()) {
        for elem in arr.iter_mut() {
            if let Some(obj) = elem.as_object_mut() {
                let item = ResourceInventoryItem {
                    id: obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    resource_type: obj
                        .get("resource_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    title: obj
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    summary: obj
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    source_path: obj
                        .get("source_path")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    references: Vec::new(),
                    related_to_target: obj
                        .get("related_to_target")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                };
                let tier = resource_inventory_reach_tier(&item, rs, app_id);
                obj.insert("reach_tier".into(), json!(tier));
            }
        }
    }
    root
}

#[derive(Debug, Deserialize)]
pub struct OpencodeContextPreviewQuery {
    pub app_id: String,
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default, alias = "routeMode")]
    pub route_mode: Option<String>,
    #[serde(default, alias = "resourceVisibility")]
    pub resource_visibility: Option<String>,
    #[serde(default, alias = "browserContext")]
    pub browser_context: Option<String>,
    #[serde(default, alias = "hostProtocol")]
    pub host_protocol: Option<String>,
    #[serde(default, alias = "hostContractSchema")]
    pub host_contract_schema: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScopeBoundaryView {
    /// `scene`：带 scene_id 的会话绑定；`file`：以目标文件为主、无 scene 约束。
    pub binding_scope: String,
    /// `local_only` | `allow_direct_refs` | `allow_scene_reachable`
    pub resource_visibility: String,
    /// `read_only`（ask）或 `rewrite_target_only`（build）。
    pub edit_scope: String,
}

#[derive(Debug, Serialize)]
pub struct OpencodeContextPreviewResponse {
    pub app_id: String,
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
    pub session_context: String,
    pub system_prompt: String,
    pub query_schema_version: String,
    #[serde(default)]
    pub query_tools: Vec<Value>,
    #[serde(default)]
    pub runtime_capabilities: Vec<String>,
    pub host_contract: Value,
    pub resource_inventory: Value,
    #[serde(default)]
    pub preview_error: Option<String>,
    #[serde(default)]
    pub profile_summary: String,
    #[serde(default)]
    pub native_tool_names: Vec<String>,
    #[serde(default)]
    pub scope_digest: String,
    pub scope_boundary: ScopeBoundaryView,
    #[serde(default)]
    pub skill_status: Option<Value>,
    #[serde(default)]
    pub browser_context_echo: Option<Value>,
    #[serde(default)]
    pub host_protocol_echo: Option<Value>,
    #[serde(default)]
    pub host_contract_schema_echo: Option<String>,
}

fn parse_browser_context_query(raw: Option<&str>) -> Option<Value> {
    let text = raw.map(str::trim).filter(|value| !value.is_empty())?;
    serde_json::from_str::<Value>(text).ok()
}

fn parse_host_protocol_query(raw: Option<&str>) -> Option<Value> {
    let text = raw.map(str::trim).filter(|value| !value.is_empty())?;
    serde_json::from_str::<Value>(text).ok()
}

pub async fn api_agent_context_preview(
    State(state): State<AppState>,
    Query(query): Query<OpencodeContextPreviewQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return error_response("query parameter `app_id` is required");
    }
    let mut request = BridgePromptRequest {
        text: String::new(),
        app_id: Some(app_id.to_string()),
        scene_id: query.scene_id.clone(),
        target_file: query.target_file.clone(),
        system: None,
        mode: query.mode.clone(),
        route_mode: query.route_mode.clone(),
        agent: None,
        model: None,
        resource_visibility: query.resource_visibility.clone(),
        browser_context: parse_browser_context_query(query.browser_context.as_deref()),
        host_protocol: parse_host_protocol_query(query.host_protocol.as_deref()),
        host_contract_schema: query.host_contract_schema.clone(),
    };
    let policy = AgentModePolicy::from_request(&request);
    if let Err(error) = policy.validate() {
        return error_response(error);
    }
    policy.apply_to_request(&mut request);
    if let Err(error) = prepare_prompt_request(&state, &mut request) {
        return error.into_response();
    }
    let bundle = AgentScopeBundle::resolve(&state, &request);
    let snapshot_ref = bundle.as_ref().and_then(|b| b.snapshot.as_ref());
    let preview_error = bundle.as_ref().and_then(|b| b.snapshot_error.as_deref());
    let preview_error_owned = preview_error.map(|e| {
        tracing::debug!(
            app_id = %app_id,
            scene_id = ?query.scene_id,
            target_file = ?query.target_file,
            error = %e,
            "degraded context preview snapshot"
        );
        e.to_string()
    });
    let session_context =
        build_dynamic_session_context_preview(&state, &request, snapshot_ref, preview_error)
            .unwrap_or_else(|| String::new());
    request = enrich_prompt_request(&state, Some(&session_context), request);
    let profile_summary = bundle
        .as_ref()
        .map(|b| b.profile.summary_line())
        .unwrap_or_default();
    let scope_digest = bundle
        .as_ref()
        .map(|b| b.scope_digest_token())
        .unwrap_or_default();
    let pol2 = AgentModePolicy::from_request(&request);
    let vis = bundle
        .as_ref()
        .map(|b| b.profile.resource_visibility)
        .unwrap_or_else(|| resolve_resource_visibility(&request, pol2));
    let mode_for_tools = request
        .mode
        .as_deref()
        .or(request.agent.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("ask");
    let native_tool_names: Vec<String> =
        resource_tools::tool_definitions_for_profile(mode_for_tools, vis)
            .into_iter()
            .filter_map(|item| {
                item.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(str::to_string)
            })
            .collect();
    let (tools, resource_inventory) = match bundle.as_ref() {
        Some(b) => {
            if let Some(snapshot) = b.snapshot.as_ref() {
                let tools = if snapshot.query_tools.is_empty() {
                    default_resource_query_tools()
                } else {
                    snapshot.query_tools.clone()
                };
                let inv = enrich_resource_inventory_preview_value(
                    &snapshot.resource_inventory,
                    &b.resource_scope,
                    b.app_id.as_str(),
                );
                (tools, inv)
            } else {
                (default_resource_query_tools(), Value::Null)
            }
        }
        None => (default_resource_query_tools(), Value::Null),
    };
    let query_tools = tools
        .into_iter()
        .map(|item| serde_json::to_value(item).unwrap_or(Value::Null))
        .collect::<Vec<_>>();
    let runtime_capabilities = host_runtime_capabilities_catalog()
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let skill_status = None;
    let binding_scope = "scene".to_string();
    let edit_scope = "read_only".to_string();
    let scope_boundary = ScopeBoundaryView {
        binding_scope,
        resource_visibility: vis.as_slug().to_string(),
        edit_scope,
    };
    Json(OpencodeContextPreviewResponse {
        app_id: app_id.to_string(),
        scene_id: query.scene_id,
        target_file: query.target_file,
        session_context,
        system_prompt: request.system.unwrap_or_default(),
        query_schema_version: RESOURCE_QUERY_SCHEMA_VERSION.to_string(),
        query_tools,
        runtime_capabilities,
        host_contract: host_runtime_contract_descriptor(),
        resource_inventory,
        preview_error: preview_error_owned,
        profile_summary,
        native_tool_names,
        scope_digest,
        scope_boundary,
        skill_status,
        browser_context_echo: request.browser_context.clone(),
        host_protocol_echo: request.host_protocol.clone(),
        host_contract_schema_echo: request.host_contract_schema.clone(),
    })
    .into_response()
}

