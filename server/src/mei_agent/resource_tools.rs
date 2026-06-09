//! LLM 可调用的只读资源工具定义与执行桥接（实现放在 `resource_tool_bridge`，避免 `mei_agent` 依赖 `http`）。

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use mei_lang_kernel::{FilterIntent, QueryState};
use mei_lang_toolchain::access_host_bound_tool_descriptors;
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
    /// 浏览器访问态当前合并后的 query_state；供 dataset/metric 工具作为默认求值 scope。
    pub browser_query_state: Option<QueryState>,
    /// 与 `browser_query_state` 同源的语义过滤意图；无显式意图时可由 query_state.filters 派生。
    pub browser_filter_intents: Vec<FilterIntent>,
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
            browser_query_state: None,
            browser_filter_intents: Vec::new(),
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
    let mut defs = vec![llm::read_file_tool_definition()];
    defs.extend(access_tool_definitions_for_visibility(vis));
    if normalized == "ask" || normalized == "plan" {
        defs.push(propose_session_patch_tool_definition());
    }
    defs
}

fn access_tool_definitions_for_visibility(vis: ResourceVisibility) -> Vec<Value> {
    access_host_bound_tool_descriptors()
        .into_iter()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?.to_string();
            let description = tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let mut parameters = tool.get("input_schema").cloned().unwrap_or_else(|| {
                json!({
                    "type": "object",
                    "properties": {}
                })
            });
            if vis == ResourceVisibility::LocalOnly {
                strip_scope_overrides(&mut parameters);
            }
            Some(json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": description,
                    "parameters": parameters,
                }
            }))
        })
        .collect()
}

fn strip_scope_overrides(parameters: &mut Value) {
    let Some(props) = parameters
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    props.remove("scene_id");
    props.remove("target_file");
    if let Some(required) = parameters.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|item| {
            item.as_str() != Some("scene_id") && item.as_str() != Some("target_file")
        });
    }
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
            "resource_runtime_trace_export",
            "resource_business_summary",
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
        assert!(names.contains(&"resource_runtime_trace_export".to_string()));
        assert!(names.contains(&"resource_business_summary".to_string()));
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
        assert!(names.contains(&"resource_runtime_trace_export".to_string()));
        assert!(names.contains(&"resource_business_summary".to_string()));
        assert!(!names.contains(&"propose_session_patch".to_string()));
        assert!(!names.contains(&"skill_list".to_string()));
        assert!(!names.contains(&"skill_read".to_string()));
        assert!(!names.contains(&"rewrite_current_mei".to_string()));
    }

    #[test]
    fn access_tools_follow_catalog_host_bound_names() {
        let names = tool_names("ask")
            .into_iter()
            .filter(|name| name != "read_file" && name != "propose_session_patch")
            .collect::<Vec<_>>();
        assert_eq!(names, mei_lang_toolchain::access_host_bound_tool_names());
    }
}
