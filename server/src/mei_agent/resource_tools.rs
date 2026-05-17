//! LLM 可调用的只读资源工具定义与执行桥接（实现放在 `resource_tool_bridge`，避免 `mei_agent` 依赖 `http`）。

use std::path::Path;

use serde_json::{json, Value};

use super::{llm, skill_tools};

/// 与 [`crate::agent_runtime::bridge::BridgePromptRequest`](crate::agent_runtime::bridge::BridgePromptRequest) 对齐的 scope 快照。
#[derive(Debug, Clone, Default)]
pub struct AgentResourceScope {
    pub scene_id: Option<String>,
    pub entry_id: Option<String>,
    pub target_file: Option<String>,
}

pub trait ResourceToolExecutor: Send + Sync {
    /// 执行资源查询工具（不含 `read_file`），当前主要用于 `dataset_query`。
    fn run_resource_tool(
        &self,
        source_root: &Path,
        app_id: Option<&str>,
        scope: &AgentResourceScope,
        tool_name: &str,
        args_json: &str,
    ) -> String;
}

/// 默认不注册场景查询（测试与无 `http` 桥接时）；由 [`crate::mei_agent::NativeAgent::open`] 构造。
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct NoopResourceToolExecutor;

impl ResourceToolExecutor for NoopResourceToolExecutor {
    fn run_resource_tool(
        &self,
        _source_root: &Path,
        _app_id: Option<&str>,
        _scope: &AgentResourceScope,
        tool_name: &str,
        _args_json: &str,
    ) -> String {
        format!("error: resource tool `{tool_name}` is not available in this build (noop executor)")
    }
}

pub(crate) fn tool_definitions_for_mode(mode: &str) -> Vec<Value> {
    let normalized = mode.trim().to_ascii_lowercase();
    let mut tools = vec![llm::read_file_tool_definition(), dataset_query_tool_definition()];
    if normalized == "build" {
        tools.push(rewrite_current_mei_tool_definition());
        tools.push(skill_tools::skill_list_tool_definition());
        tools.push(skill_tools::skill_read_tool_definition());
    }
    tools
}

fn dataset_query_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "dataset_query",
            "description": "Query one dataset resource by id via host Mei dataset engine (not raw xlsx reads). Returns bounded result: dataset schema preview + filters + metric ids + sample rows. Defaults keep output small.",
            "parameters": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Dataset resource id in world, e.g. typical_cases" },
                    "search": { "type": "string", "description": "Optional global text search" },
                    "filters": {
                        "type": "object",
                        "description": "Optional field filter map, e.g. {\"涉及单位\":\"某单位\"}",
                        "additionalProperties": { "type": "string" }
                    },
                    "columns": {
                        "type": "array",
                        "description": "Optional preferred columns. If omitted, returns first 10 columns by schema order.",
                        "items": { "type": "string" }
                    },
                    "limit": { "type": "integer", "description": "Optional row count (default 10, max 50)" },
                    "scene_id": { "type": "string", "description": "Override scene id (optional)" },
                    "entry_id": { "type": "string", "description": "Override entry id (optional)" },
                    "target_file": { "type": "string", "description": "Override target .mei path (optional)" }
                },
                "required": ["id"]
            }
        }
    })
}

fn rewrite_current_mei_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "rewrite_current_mei",
            "description": "Rewrite current target `.mei` file with full new content. Build mode only. This tool is restricted to the active target file from the current request scope.",
            "parameters": {
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Full file content to write into current target `.mei`." },
                    "target_file": { "type": "string", "description": "Optional; must equal current target_file if provided." },
                    "reason": { "type": "string", "description": "Short reason for the rewrite." }
                },
                "required": ["content"]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::tool_definitions_for_mode;

    fn tool_names(mode: &str) -> Vec<String> {
        tool_definitions_for_mode(mode)
            .into_iter()
            .filter_map(|item| {
                item.get("function")
                    .and_then(|func| func.get("name"))
                    .and_then(|name| name.as_str())
                    .map(str::to_string)
            })
            .collect()
    }

    #[test]
    fn ask_mode_hides_authoring_tools() {
        let names = tool_names("ask");
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"dataset_query".to_string()));
        assert!(!names.contains(&"skill_list".to_string()));
        assert!(!names.contains(&"skill_read".to_string()));
        assert!(!names.contains(&"rewrite_current_mei".to_string()));
    }

    #[test]
    fn build_mode_includes_authoring_tools() {
        let names = tool_names("build");
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"dataset_query".to_string()));
        assert!(names.contains(&"skill_list".to_string()));
        assert!(names.contains(&"skill_read".to_string()));
        assert!(names.contains(&"rewrite_current_mei".to_string()));
    }
}
