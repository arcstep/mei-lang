//! 将 `http::scene_api` 的只读 world 查询接到 `mei_agent::ResourceToolExecutor`，打破 `mei_agent -> http` 循环依赖。

use std::path::Path;

use serde_json::Value;
use std::collections::BTreeMap;

use crate::http::scene_api::{
    query_resource_dataset, query_resource_dataset_metric, query_resource_get, query_resource_list,
    query_resource_runtime_peek, WorldScope,
};
use crate::mei_agent::agent_scope_profile::validate_dataset_world_scope_merge;
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
        match tool_name {
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
                let kind = args
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let limit = args
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|u| u as usize);
                Self::json_result(query_resource_list(
                    source_root,
                    app,
                    scope_ref,
                    kind,
                    limit,
                ))
            }
            "resource_get" => {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if id.is_empty() {
                    return "error: resource_get requires non-empty id".to_string();
                }
                Self::json_result(query_resource_get(source_root, app, scope_ref, id))
            }
            "resource_runtime_peek" => {
                let trace_limit = args
                    .get("trace_limit")
                    .and_then(Value::as_u64)
                    .map(|u| u as usize);
                Self::json_result(query_resource_runtime_peek(
                    source_root,
                    app,
                    scope_ref,
                    trace_limit,
                ))
            }
            other => format!("error: unknown resource tool `{other}`"),
        }
    }
}
