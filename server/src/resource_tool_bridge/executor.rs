//! 将 `http::scene_api` 的只读 world 查询接到 `mei_agent::ResourceToolExecutor`，打破 `mei_agent -> http` 循环依赖。

use std::path::Path;

use mei_lang_toolchain as toolchain;
use serde_json::Value;
use std::time::Instant;

use crate::http::scene_api::{
    query_resource_dataset, query_resource_dataset_metric, WorldAssetListResponse, WorldScope,
};
use crate::mei_agent::agent_scope_profile::{
    resource_world_tools_precheck, validate_dataset_world_scope_merge,
};
use crate::mei_agent::browser_context::{
    effective_filter_intents, merge_browser_query_state_with_args,
};
use crate::mei_agent::resource_tools::{AgentResourceScope, ResourceToolExecutor};

use super::core::SceneResourceToolExecutor;

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
                let dataset_id =
                    Self::first_non_empty_arg(&args, &["dataset_id", "id"]).unwrap_or("");
                if dataset_id.is_empty() {
                    return "error: dataset_query requires non-empty dataset_id".to_string();
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
                let effective_query_state = merge_browser_query_state_with_args(
                    scope.browser_query_state.as_ref(),
                    &filters,
                    search,
                );
                let columns_ref = if columns.is_empty() {
                    None
                } else {
                    Some(columns.as_slice())
                };
                Self::json_result(query_resource_dataset(
                    source_root,
                    app,
                    scope_ref,
                    dataset_id,
                    search,
                    &filters,
                    columns_ref,
                    limit,
                    Some(&effective_query_state),
                ))
            }
            "dataset_metric" => {
                let dataset_id =
                    Self::first_non_empty_arg(&args, &["dataset_id", "id"]).unwrap_or("");
                if dataset_id.is_empty() {
                    return "error: dataset_metric requires non-empty dataset_id".to_string();
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
                let effective_query_state = merge_browser_query_state_with_args(
                    scope.browser_query_state.as_ref(),
                    &filters,
                    search,
                );
                let effective_filter_intents =
                    effective_filter_intents(&scope.browser_filter_intents, &effective_query_state);
                Self::json_result(query_resource_dataset_metric(
                    source_root,
                    app,
                    scope_ref,
                    dataset_id,
                    &metric_ids,
                    search,
                    &filters,
                    Some(&effective_query_state),
                    &effective_filter_intents,
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
                let resource_id =
                    Self::first_non_empty_arg(&args, &["resource_id", "id"]).unwrap_or("");
                if resource_id.is_empty() {
                    return "error: resource_get requires non-empty resource_id".to_string();
                }
                if !allowed.contains(resource_id) {
                    return format!(
                        "error: scope_denied: resource_get resource_id `{resource_id}` is not in the current `{}` reachable inventory (aligned with /world asset)",
                        scope.resource_visibility.as_slug()
                    );
                }
                Self::json_result(toolchain::query_world_asset(
                    source_root,
                    app,
                    scope_ref,
                    resource_id,
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
            "resource_runtime_trace_export" => {
                if let Err(e) = resource_world_tools_precheck(scope) {
                    return format!("error: {e}");
                }
                let trace_limit = args
                    .get("trace_limit")
                    .and_then(Value::as_u64)
                    .map(|u| u as usize);
                Self::json_result(toolchain::export_runtime_trace(
                    source_root,
                    app,
                    &ws,
                    trace_limit,
                    toolchain::HeadlessExportOptions::default(),
                ))
            }
            "resource_business_summary" => Self::json_result(
                toolchain::build_world_business_summary(source_root, app, scope_ref),
            ),
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
