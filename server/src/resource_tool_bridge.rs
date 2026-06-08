//! 将 `http::scene_api` 的只读 world 查询接到 `mei_agent::ResourceToolExecutor`，打破 `mei_agent -> http` 循环依赖。

use std::path::Path;

use mei_lang_toolchain as toolchain;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;

use crate::http::scene_api::{
    query_resource_dataset, query_resource_dataset_metric, WorldAssetListResponse, WorldScope,
};
use crate::mei_agent::agent_scope_profile::{
    resource_world_tools_precheck, validate_dataset_world_scope_merge,
};
use crate::mei_agent::resource_tools::{AgentResourceScope, ResourceToolExecutor};

#[derive(Debug, Default)]
pub struct SceneResourceToolExecutor;

impl SceneResourceToolExecutor {
    fn world_scope(base: &AgentResourceScope, args: &Value) -> WorldScope {
        fn pick(args: &Value, key: &str, fallback: Option<&String>) -> Option<String> {
            args.get(key)
                .and_then(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| fallback.cloned())
        }
        WorldScope {
            scene_id: pick(args, "scene_id", base.scene_id.as_ref()),
            target_file: pick(args, "target_file", base.target_file.as_ref()),
        }
    }

    fn json_result<T: serde::Serialize>(result: anyhow::Result<T>) -> String {
        match result {
            Ok(v) => match serde_json::to_string(&v) {
                Ok(s) if s.len() > 120_000 => format!(
                    "{{\"truncated\":true,\"preview\":{}}}",
                    serde_json::to_string(&s.chars().take(2000).collect::<String>())
                        .unwrap_or_else(|_| "\"\"".into())
                ),
                Ok(s) => s,
                Err(e) => format!("error: failed to serialize tool result: {e}"),
            },
            Err(e) => format!("error: {e}"),
        }
    }

    fn parse_filters(args: &Value) -> BTreeMap<String, String> {
        let mut filters = BTreeMap::new();
        let Some(map) = args.get("filters").and_then(Value::as_object) else {
            return filters;
        };
        for (k, v) in map {
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            let val = match v {
                Value::Null => String::new(),
                Value::String(s) => s.trim().to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                other => other.to_string(),
            };
            if !val.is_empty() {
                filters.insert(key.to_string(), val);
            }
        }
        filters
    }

    fn parse_columns(args: &Value) -> Vec<String> {
        args.get("columns")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }
}

impl ResourceToolExecutor for SceneResourceToolExecutor {
    fn run_resource_tool(
        &self,
        source_root: &Path,
        app_id: Option<&str>,
        scope: &AgentResourceScope,
        tool_name: &str,
        args_json: &str,
    ) -> String {
        let app = match app_id.map(str::trim).filter(|s| !s.is_empty()) {
            Some(a) => a,
            None => {
                return "error: app_id is required for resource tools (set app on author page)"
                    .to_string()
            }
        };
        let args: Value = match serde_json::from_str(args_json) {
            Ok(v) => v,
            Err(e) => return format!("error: invalid tool arguments JSON: {e}"),
        };
        let base_ws = WorldScope {
            scene_id: scope.scene_id.clone(),
            target_file: scope.target_file.clone(),
        };
        let ws = Self::world_scope(scope, &args);
        if let Err(e) = validate_dataset_world_scope_merge(
            &base_ws,
            &ws,
            scope.resource_visibility,
            Some(scope),
            Some(app),
        ) {
            return format!("error: {e}");
        }
        let scope_ref = Some(&ws);
        let tool_started = Instant::now();
        tracing::info!(
            app_id = %app,
            tool_name = %tool_name,
            scene_id = %ws.scene_id.as_deref().unwrap_or("-"),
            target_file = %ws.target_file.as_deref().unwrap_or("-"),
            "resource tool started"
        );
        let output = match tool_name {
            "dataset_query" => {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if id.is_empty() {
                    return "error: dataset_query requires non-empty id".to_string();
                }
                let search = args
                    .get("search")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let limit = args
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|u| u as usize);
                let filters = Self::parse_filters(&args);
                let columns = Self::parse_columns(&args);
                let columns_ref = if columns.is_empty() {
                    None
                } else {
                    Some(columns.as_slice())
                };
                Self::json_result(query_resource_dataset(
                    source_root,
                    app,
                    scope_ref,
                    id,
                    search,
                    &filters,
                    columns_ref,
                    limit,
                ))
            }
            "dataset_metric" => {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if id.is_empty() {
                    return "error: dataset_metric requires non-empty id".to_string();
                }
                let search = args
                    .get("search")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let filters = Self::parse_filters(&args);
                let metric_ids = args
                    .get("metric_ids")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Self::json_result(query_resource_dataset_metric(
                    source_root,
                    app,
                    scope_ref,
                    id,
                    &metric_ids,
                    search,
                    &filters,
                ))
            }
            "resource_list" => {
                if let Err(e) = resource_world_tools_precheck(scope) {
                    return format!("error: {e}");
                }
                let allowed = scope
                    .world_injection_allowed_ids
                    .as_ref()
                    .expect("precheck ensures Some");
                let kind = args
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let limit = args
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|u| u as usize);
                let mut response =
                    match toolchain::query_world_assets(source_root, app, scope_ref, kind, limit) {
                        Ok(r) => r,
                        Err(e) => return Self::json_result::<WorldAssetListResponse>(Err(e)),
                    };
                response.items.retain(|it| allowed.contains(&it.id));
                response.total = response.items.len();
                Self::json_result(Ok(response))
            }
            "resource_get" => {
                if let Err(e) = resource_world_tools_precheck(scope) {
                    return format!("error: {e}");
                }
                let allowed = scope
                    .world_injection_allowed_ids
                    .as_ref()
                    .expect("precheck ensures Some");
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if id.is_empty() {
                    return "error: resource_get requires non-empty id".to_string();
                }
                if !allowed.contains(id) {
                    return format!(
                        "error: scope_denied: resource_get id `{id}` is not in the current `{}` reachable inventory (aligned with /world asset)",
                        scope.resource_visibility.as_slug()
                    );
                }
                Self::json_result(toolchain::query_world_asset(
                    source_root,
                    app,
                    scope_ref,
                    id,
                ))
            }
            "resource_runtime_peek" => {
                if let Err(e) = resource_world_tools_precheck(scope) {
                    return format!("error: {e}");
                }
                let trace_limit = args
                    .get("trace_limit")
                    .and_then(Value::as_u64)
                    .map(|u| u as usize);
                Self::json_result(toolchain::query_world_runtime(
                    source_root,
                    app,
                    scope_ref,
                    trace_limit,
                ))
            }
            other => format!("error: unknown resource tool `{other}`"),
        };
        tracing::info!(
            app_id = %app,
            tool_name = %tool_name,
            scene_id = %ws.scene_id.as_deref().unwrap_or("-"),
            target_file = %ws.target_file.as_deref().unwrap_or("-"),
            elapsed_ms = tool_started.elapsed().as_millis() as u64,
            result_bytes = output.len(),
            result_is_error = output.starts_with("error:"),
            "resource tool finished"
        );
        output
    }
}

#[cfg(test)]
mod resource_tool_bridge_tests {
    use super::SceneResourceToolExecutor;
    use crate::agent_runtime::bridge::BridgePromptRequest;
    use crate::http::agent_api::prompt_context::scope_bundle::AgentScopeBundle;
    use crate::mei_agent::mode_policy::AgentModePolicy;
    use crate::mei_agent::resource_tools::ResourceToolExecutor;
    use crate::test_support;

    #[test]
    fn resource_list_smoke_under_workspace_app() {
        let state = test_support::test_app_state().expect("state");
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_list",
            "{}",
        );
        assert!(
            out.starts_with('{') || out.starts_with("error:"),
            "unexpected output: {}",
            &out[..out.len().min(120)]
        );
    }

    #[test]
    fn resource_get_scope_denied_for_unknown_id() {
        let state = test_support::test_app_state().expect("state");
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_get",
            r#"{"id":"__definitely_not_in_inventory__"}"#,
        );
        assert!(
            out.contains("scope_denied"),
            "expected scope_denied, got {}",
            &out[..out.len().min(200)]
        );
    }

    #[test]
    fn resource_list_denied_when_world_snapshot_missing() {
        let state = test_support::test_app_state().expect("state");
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/_invalid/07-app-missing-scene".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/_invalid/07-app-missing-scene"),
            &bundle.resource_scope,
            "resource_list",
            "{}",
        );
        assert!(
            out.contains("missing world snapshot"),
            "{}",
            &out[..out.len().min(200)]
        );
    }

    #[test]
    fn resource_world_tools_rejected_under_local_only() {
        let state = test_support::test_app_state().expect("state");
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("local_only".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_runtime_peek",
            "{}",
        );
        assert!(out.contains("local_only"), "{}", &out[..out.len().min(200)]);
    }

    #[test]
    fn resource_runtime_peek_ok_with_valid_snapshot_scope() {
        let state = test_support::test_app_state().expect("state");
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_runtime_peek",
            "{}",
        );
        assert!(
            out.starts_with('{'),
            "expected JSON runtime peek, got {}",
            &out[..out.len().min(160)]
        );
    }
}
