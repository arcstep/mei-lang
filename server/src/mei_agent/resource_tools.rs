//! LLM 可调用的只读资源工具定义与执行桥接（实现放在 `resource_tool_bridge`，避免 `mei_agent` 依赖 `http`）。

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

use super::llm;

/// 业务层资源与工具可见范围（在 workspace 安全边界之内进一步收敛）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceVisibility {
    /// 仅允许使用请求基线 `scene_id + target_file`，禁止工具参数覆盖到其它入口。
    #[default]
    LocalOnly,
    /// 允许在同 scene 下覆盖 `target_file`（例如引用其它 `.mei`），仍必须在 app 目录内。
    AllowDirectRefs,
    /// 与 `AllowDirectRefs` 同级校验 scene；预留未来 inventory 分层差异。
    AllowSceneReachable,
}

impl ResourceVisibility {
    pub(crate) fn parse(raw: Option<&str>) -> Option<Self> {
        let s = raw.map(str::trim).filter(|v| !v.is_empty())?;
        match s.to_ascii_lowercase().as_str() {
            "local_only" | "localonly" | "local" => Some(Self::LocalOnly),
            "allow_direct_refs" | "allowdirectrefs" | "direct_refs" | "refs" => {
                Some(Self::AllowDirectRefs)
            }
            "allow_scene_reachable" | "scene_reachable" | "scene" => {
                Some(Self::AllowSceneReachable)
            }
            _ => None,
        }
    }

    pub(crate) fn as_slug(self) -> &'static str {
        match self {
            ResourceVisibility::LocalOnly => "local_only",
            ResourceVisibility::AllowDirectRefs => "allow_direct_refs",
            ResourceVisibility::AllowSceneReachable => "allow_scene_reachable",
        }
    }
}

/// 与 [`crate::agent_runtime::bridge::BridgePromptRequest`](crate::agent_runtime::bridge::BridgePromptRequest) 对齐的 scope 快照。
#[derive(Debug, Clone)]
pub struct AgentResourceScope {
    pub scene_id: Option<String>,
    pub target_file: Option<String>,
    pub resource_visibility: ResourceVisibility,
    /// 由 world inventory 解析出的「与 target 直接相关」可读路径集合（workspace 相对、已规范化）。
    pub direct_ref_paths: Arc<HashSet<String>>,
    /// 由 world inventory 解析出的「当前 scene 上下文可达」可读路径集合（workspace 相对、已规范化）。
    pub scene_reachable_paths: Arc<HashSet<String>>,
    /// 当 world 快照可用时，与 `/world` 注入一致的「可达 inventory id」集合，用于 `resource_list` / `resource_get` / `resource_runtime_peek`。
    /// `None` 表示无快照（无法按 inventory 校验），非 `local_only` 下应对上述工具拒绝执行。
    pub world_injection_allowed_ids: Option<Arc<HashSet<String>>>,
}

impl Default for AgentResourceScope {
    fn default() -> Self {
        Self {
            scene_id: None,
            target_file: None,
            resource_visibility: ResourceVisibility::LocalOnly,
            direct_ref_paths: Arc::new(HashSet::new()),
            scene_reachable_paths: Arc::new(HashSet::new()),
            world_injection_allowed_ids: None,
        }
    }
}

pub trait ResourceToolExecutor: Send + Sync {
    /// 执行资源查询工具（不含 `read_file`），当前主要用于 dataset 查询/指标查询。
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

#[cfg(test)]
pub(crate) fn tool_definitions_for_mode(mode: &str) -> Vec<Value> {
    tool_definitions_for_profile(mode, ResourceVisibility::AllowSceneReachable)
}

/// 按「模式 + 资源可见性」生成 LLM 可见工具 schema：`local_only` 下 dataset 工具不暴露 scope 覆盖参数。
pub(crate) fn tool_definitions_for_profile(mode: &str, vis: ResourceVisibility) -> Vec<Value> {
    let normalized = mode.trim().to_ascii_lowercase();
    let allow_ds_scope_override = vis != ResourceVisibility::LocalOnly;
    let mut defs = vec![
        llm::read_file_tool_definition(),
        dataset_query_tool_definition(allow_ds_scope_override),
        dataset_metric_tool_definition(allow_ds_scope_override),
        resource_list_tool_definition(allow_ds_scope_override),
        resource_get_tool_definition(allow_ds_scope_override),
        resource_runtime_peek_tool_definition(allow_ds_scope_override),
    ];
    if normalized == "ask" || normalized == "plan" {
        defs.push(propose_session_patch_tool_definition());
    }
    defs
}

fn propose_session_patch_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "propose_session_patch",
            "description": "Propose a session-scoped temporary view patch for access runtime (non-persistent by default).",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Short patch title for operator-readable session history."
                    },
                    "ops": {
                        "type": "array",
                        "description": "Patch operations. Keep this list small (max 8).",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": {
                                    "type": "string",
                                    "enum": ["hide_panel", "highlight_panel", "move_panel_front", "focus_query_state"]
                                },
                                "panel_id": {
                                    "type": "string",
                                    "description": "Target panel id from host runtime metadata."
                                },
                                "query_state_id": {
                                    "type": "string",
                                    "description": "Target query_state id from browser context."
                                },
                                "note": {
                                    "type": "string",
                                    "description": "Optional operator hint."
                                }
                            },
                            "required": ["type"]
                        }
                    }
                },
                "required": ["ops"]
            }
        }
    })
}

fn resource_list_tool_definition(allow_scope_override: bool) -> Value {
    let mut props = serde_json::Map::new();
    props.insert(
        "kind".to_string(),
        json!({ "type": "string", "description": "Optional filter: entity | resource | cell" }),
    );
    props.insert(
        "limit".to_string(),
        json!({ "type": "integer", "description": "Optional max items (bounded by server)" }),
    );
    if allow_scope_override {
        props.insert(
            "scene_id".to_string(),
            json!({ "type": "string", "description": "Override scene id (optional)" }),
        );
        props.insert(
            "target_file".to_string(),
            json!({ "type": "string", "description": "Override target .mei path (optional)" }),
        );
    }
    json!({
        "type": "function",
        "function": {
            "name": "resource_list",
            "description": "List world assets (entities/resources/cells) for the current app with bounded JSON output. Uses the same scope rules as dataset tools.",
            "parameters": {
                "type": "object",
                "properties": props
            }
        }
    })
}

fn resource_get_tool_definition(allow_scope_override: bool) -> Value {
    let mut props = serde_json::Map::new();
    props.insert(
        "id".to_string(),
        json!({ "type": "string", "description": "World asset or entity id" }),
    );
    if allow_scope_override {
        props.insert(
            "scene_id".to_string(),
            json!({ "type": "string", "description": "Override scene id (optional)" }),
        );
        props.insert(
            "target_file".to_string(),
            json!({ "type": "string", "description": "Override target .mei path (optional)" }),
        );
    }
    json!({
        "type": "function",
        "function": {
            "name": "resource_get",
            "description": "Fetch one world asset/entity by id with bounded JSON payload.",
            "parameters": {
                "type": "object",
                "properties": props,
                "required": ["id"]
            }
        }
    })
}

fn resource_runtime_peek_tool_definition(allow_scope_override: bool) -> Value {
    let mut props = serde_json::Map::new();
    props.insert(
        "trace_limit".to_string(),
        json!({ "type": "integer", "description": "Optional trace/event limit" }),
    );
    if allow_scope_override {
        props.insert(
            "scene_id".to_string(),
            json!({ "type": "string", "description": "Override scene id (optional)" }),
        );
        props.insert(
            "target_file".to_string(),
            json!({ "type": "string", "description": "Override target .mei path (optional)" }),
        );
    }
    json!({
        "type": "function",
        "function": {
            "name": "resource_runtime_peek",
            "description": "Peek bounded world runtime state (sim traces, etc.) for the current scope.",
            "parameters": {
                "type": "object",
                "properties": props
            }
        }
    })
}

fn dataset_query_tool_definition(allow_scope_override: bool) -> Value {
    let mut props = serde_json::Map::new();
    props.insert(
        "id".to_string(),
        json!({ "type": "string", "description": "Dataset resource id in world, e.g. typical_cases" }),
    );
    props.insert(
        "search".to_string(),
        json!({ "type": "string", "description": "Optional global text search" }),
    );
    props.insert(
        "filters".to_string(),
        json!({
            "type": "object",
            "description": "Optional field filter map, e.g. {\"涉及单位\":\"某单位\"}",
            "additionalProperties": { "type": "string" }
        }),
    );
    props.insert(
        "columns".to_string(),
        json!({
            "type": "array",
            "description": "Optional preferred columns. If omitted, returns first 10 columns by schema order.",
            "items": { "type": "string" }
        }),
    );
    props.insert(
        "limit".to_string(),
        json!({ "type": "integer", "description": "Optional row count (default 10, max 50)" }),
    );
    if allow_scope_override {
        props.insert(
            "scene_id".to_string(),
            json!({ "type": "string", "description": "Override scene id (optional)" }),
        );
        props.insert(
            "target_file".to_string(),
            json!({ "type": "string", "description": "Override target .mei path (optional)" }),
        );
    }
    json!({
        "type": "function",
        "function": {
            "name": "dataset_query",
            "description": "Query one dataset resource by id via host Mei dataset engine (not raw xlsx reads). Returns bounded result: dataset schema preview + filters + metric ids + sample rows. Defaults keep output small.",
            "parameters": {
                "type": "object",
                "properties": props,
                "required": ["id"]
            }
        }
    })
}

fn dataset_metric_tool_definition(allow_scope_override: bool) -> Value {
    let mut props = serde_json::Map::new();
    props.insert(
        "id".to_string(),
        json!({ "type": "string", "description": "Dataset resource id in world, e.g. issue_result_list" }),
    );
    props.insert(
        "metric_ids".to_string(),
        json!({
            "type": "array",
            "description": "Optional metric ids to evaluate. If omitted, returns all runtime metrics on the dataset.",
            "items": { "type": "string" }
        }),
    );
    props.insert(
        "search".to_string(),
        json!({ "type": "string", "description": "Optional global text search before metric evaluation" }),
    );
    props.insert(
        "filters".to_string(),
        json!({
            "type": "object",
            "description": "Optional field filter map applied before metric evaluation",
            "additionalProperties": { "type": "string" }
        }),
    );
    if allow_scope_override {
        props.insert(
            "scene_id".to_string(),
            json!({ "type": "string", "description": "Override scene id (optional)" }),
        );
        props.insert(
            "target_file".to_string(),
            json!({ "type": "string", "description": "Override target .mei path (optional)" }),
        );
    }
    json!({
        "type": "function",
        "function": {
            "name": "dataset_metric",
            "description": "Query runtime metric values for one dataset resource by id via host Mei dataset engine. Best for aggregated asks such as count/rate/trend/summary-card values.",
            "parameters": {
                "type": "object",
                "properties": props,
                "required": ["id"]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{tool_definitions_for_mode, tool_definitions_for_profile, ResourceVisibility};

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
    fn local_only_hides_scope_overrides_on_resource_tools() {
        let defs = tool_definitions_for_profile("ask", ResourceVisibility::LocalOnly);
        for name in [
            "dataset_query",
            "dataset_metric",
            "resource_list",
            "resource_get",
            "resource_runtime_peek",
        ] {
            let dq = defs
                .iter()
                .find(|d| {
                    d.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        == Some(name)
                })
                .unwrap_or_else(|| panic!("missing {name}"));
            let props = dq
                .pointer("/function/parameters/properties")
                .and_then(|v| v.as_object())
                .expect("props");
            assert!(
                !props.contains_key("scene_id"),
                "{name} should hide scene_id in local_only"
            );
            assert!(
                !props.contains_key("target_file"),
                "{name} should hide target_file in local_only"
            );
        }
    }

    #[test]
    fn ask_mode_hides_authoring_tools() {
        let names = tool_names("ask");
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"dataset_query".to_string()));
        assert!(names.contains(&"dataset_metric".to_string()));
        assert!(names.contains(&"resource_list".to_string()));
        assert!(names.contains(&"resource_get".to_string()));
        assert!(names.contains(&"resource_runtime_peek".to_string()));
        assert!(names.contains(&"propose_session_patch".to_string()));
        assert!(!names.contains(&"skill_list".to_string()));
        assert!(!names.contains(&"skill_read".to_string()));
        assert!(!names.contains(&"rewrite_current_mei".to_string()));
    }

    #[test]
    fn build_mode_now_matches_read_only_toolset() {
        let names = tool_names("build");
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"dataset_query".to_string()));
        assert!(names.contains(&"dataset_metric".to_string()));
        assert!(names.contains(&"resource_list".to_string()));
        assert!(names.contains(&"resource_get".to_string()));
        assert!(names.contains(&"resource_runtime_peek".to_string()));
        assert!(!names.contains(&"propose_session_patch".to_string()));
        assert!(!names.contains(&"skill_list".to_string()));
        assert!(!names.contains(&"skill_read".to_string()));
        assert!(!names.contains(&"rewrite_current_mei".to_string()));
    }
}
