//! 将 `http::scene_api` 的只读 world 查询接到 `mei_agent::ResourceToolExecutor`，打破 `mei_agent -> http` 循环依赖。


use serde_json::Value;
use std::collections::BTreeMap;

use crate::http::scene_api::WorldScope;
use crate::mei_agent::resource_tools::AgentResourceScope;

#[derive(Debug, Default)]
pub struct SceneResourceToolExecutor;

impl SceneResourceToolExecutor {
    pub(super) fn world_scope(base: &AgentResourceScope, args: &Value) -> WorldScope {
        fn pick(args: &Value, keys: &[&str], fallback: Option<&String>) -> Option<String> {
            for key in keys {
                if let Some(value) = args
                    .get(*key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    return Some(value.to_string());
                }
            }
            fallback.cloned()
        }
        WorldScope {
            scene_id: pick(args, &["scene_id", "scene"], base.scene_id.as_ref()),
            target_file: pick(args, &["target_file"], base.target_file.as_ref()),
        }
    }

    pub(super) fn first_non_empty_arg<'a>(args: &'a Value, keys: &[&str]) -> Option<&'a str> {
        for key in keys {
            if let Some(value) = args
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return Some(value);
            }
        }
        None
    }

    pub(super) fn json_result<T: serde::Serialize>(result: anyhow::Result<T>) -> String {
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

    pub(super) fn parse_filters(args: &Value) -> BTreeMap<String, String> {
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

    pub(super) fn parse_columns(args: &Value) -> Vec<String> {
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

