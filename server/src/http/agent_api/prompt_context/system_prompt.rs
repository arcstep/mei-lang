use crate::{
    agent_runtime::bridge::BridgePromptRequest,
    mei_agent::agent_scope_profile::resolve_resource_visibility,
    mei_agent::mode_policy::AgentModePolicy, AppState,
};
use mei_lang_toolchain::{ai_profile_policy_lines, mcp_surface_descriptor};

fn profile_policy_block(profile_id: &str, heading: &str) -> String {
    let mut blocks = vec![heading.to_string()];
    blocks.extend(ai_profile_policy_lines(profile_id));
    if let Some(surface) = mcp_surface_descriptor(profile_id) {
        if let Some(tools) = surface.get("tools").and_then(|value| value.as_array()) {
            let tool_names = tools
                .iter()
                .filter_map(|item| item.get("name").and_then(|name| name.as_str()))
                .collect::<Vec<_>>();
            if !tool_names.is_empty() {
                blocks.push(format!(
                    "Catalog surface tools: {}.",
                    tool_names.join(", ")
                ));
            }
        }
    }
    blocks.join("\n")
}

pub(crate) fn build_meilang_system_prompt(
    _state: &AppState,
    existing: Option<&str>,
    request: &BridgePromptRequest,
    session_context: Option<&str>,
) -> Option<String> {
    let mode = request
        .mode
        .as_deref()
        .or(request.agent.as_deref())
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "build".to_string());
    let ask_mode = mode == "ask";
    let mut blocks = Vec::new();
    if let Some(system) = existing.map(str::trim).filter(|value| !value.is_empty()) {
        blocks.push(system.to_string());
    }
    if ask_mode {
        blocks.push(
            "You are a MeiLang visitor-facing Q&A assistant. Treat `.mei` as MeiLang scene-first DSL hosted on restricted Starlark, not Music Encoding Initiative XML.".to_string(),
        );
    } else {
        blocks.push(
            "You are assisting a MeiLang authoring workflow from a host runtime that no longer owns the authoring mainline. Treat `.mei` as MeiLang scene-first DSL hosted on restricted Starlark, not Music Encoding Initiative XML.".to_string(),
        );
    }
    blocks.push(
        "Unified ref/clone: xxx_ref(...) is always a pure reference value; xxx(base=xxx_ref(...), ...) clones then field-overrides (scalars replace, props deep-merge, lists replace when explicit). Applies to scene/world/flow/frame/panel/dataset/metric/resource/entity/component. Owner slots bind pure refs only: scene.world/flow/frame, frame.panels, world collections. component_ref(use/id, scene_file) + component(base=component_ref(...)). panel_ref never embeds in blocks. props consume dataset_ref/resource_ref/metric_ref with local ids only.".to_string(),
    );
    blocks.push(
        "Default to Chinese (Simplified Chinese) for all responses, plans, progress updates, and explanations unless the user explicitly requests another language.".to_string(),
    );
    blocks.push(
        "When presenting a plan, keep the execution-oriented content in Chinese and avoid switching to English by default.".to_string(),
    );
    if ask_mode {
        blocks.push(profile_policy_block(
            "access",
            "Tool-first information policy (ask mode, from capability catalog):",
        ));
        blocks.push(
            concat!(
                "Additional access runtime constraints:\n",
                "- Treat injected `[World — catalog]` / runtime summaries as the primary truth for business Q&A.\n",
                "- Treat injected `[Browser — context]` (active query_state / tab / overlay hints) plus `[Access — default eval scope]` as request-time runtime truth for UI state; when they change, recompute answer scope.\n",
                "- Respect injected `[Host — protocol]` and `host_contract_schema` as runtime contract metadata; do not infer capabilities beyond that envelope.\n",
                "- Do not guess resource ids, dataset fields, or `.mei` source you have not read.\n",
                "- Do not generate or suggest direct `.mei` rewrite plans in ask mode.\n",
                "- Ask mode intentionally disables authoring skill tools; avoid author-time scaffolding unless the user explicitly asks for verbatim DSL."
            )
            .to_string(),
        );
    } else {
        blocks.push(profile_policy_block(
            "author",
            "External authoring guidance (legacy non-ask mode, from capability catalog):",
        ));
        blocks.push(
            concat!(
                "Additional authoring constraints:\n",
                "- Host-side build mode is no longer the mainline for MeiLang authoring; prefer external dev tools plus `mei-toolchain` CLI / `mei-lsp` for source edits.\n",
                "- Treat current `.mei` source, syntax docs, component references, examples, and `mei check` diagnostics as the primary truth for authoring questions.\n",
                "- Treat `inspect summary` / `workspace summary` as routing/index hints, not as a replacement for reading the target source files.\n",
                "- Do not suggest or rely on `skill_list`, `skill_read`, `rewrite_current_mei`, or other host-only authoring loops.\n",
                "- If a question depends on runtime values rather than source structure, explicitly switch to access-style dataset/metric tooling instead of pretending the source already contains the answer.\n",
                "- Read workspace text files with `read_file` only when you need verbatim DSL evidence under the current allowed path scope.\n",
                "- Prefer compact, source-grounded guidance over host-runtime scaffolding."
            )
            .to_string(),
        );
    }
    if let Some(context) = session_context {
        blocks.push(format!("[MeiLang Session Context]\n{context}"));
    }
    let route = request.route_mode.as_deref().unwrap_or("");
    let policy = AgentModePolicy::from_request(request);
    let vis_eff = resolve_resource_visibility(request, policy).as_slug();
    blocks.push(format!(
        "[Agent request scope hints] route_mode={route} resource_visibility_effective={vis_eff}"
    ));
    if blocks.is_empty() {
        None
    } else {
        Some(blocks.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use super::build_meilang_system_prompt;
    use crate::agent_runtime::bridge::BridgePromptRequest;
    use crate::agent_runtime::ManagedOpencodeRuntime;
    use crate::AppState;

    #[test]
    fn system_prompt_has_tool_policy_and_no_inlined_companion_bodies() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("server crate parent")
            .to_path_buf();
        let source_root = std::env::temp_dir().join("mei_system_prompt_test_root");
        let _ = std::fs::create_dir_all(&source_root);
        let native_agent = Arc::new(
            crate::mei_agent::NativeAgent::open_with_resource_tools(
                source_root.clone(),
                Arc::new(crate::mei_agent::resource_tools::NoopResourceToolExecutor::default()),
            )
            .expect("native"),
        );
        let state = AppState {
            package_root: Arc::new(package_root),
            source_root: Arc::new(source_root),
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            auth_enforcement: crate::auth::AuthEnforcement::Disabled,
            agent_runtime: Arc::new(Mutex::new(ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
        };
        let sys = build_meilang_system_prompt(
            &state,
            None,
            &BridgePromptRequest {
                text: String::new(),
                app_id: None,
                scene_id: None,
                target_file: None,
                system: None,
                mode: Some("build".into()),
                route_mode: Some("manage".into()),
                agent: None,
                model: None,
                resource_visibility: None,
                browser_context: None,
                host_protocol: None,
                host_contract_schema: None,
            },
            Some("compact-session-ctx"),
        )
        .expect("system");
        assert!(sys.contains("External authoring guidance (legacy non-ask mode, from capability catalog)"));
        assert!(sys.contains("Catalog surface tools:"));
        assert!(sys.contains("[MeiLang Session Context]"));
        assert!(sys.contains("compact-session-ctx"));
        assert!(!sys.contains("[MeiLang Author Skill — index]"));
        assert!(
            !sys.contains("## 阅读顺序"),
            "companion-only headings should not be inlined: {}",
            sys.len()
        );
    }

    #[test]
    fn ask_mode_system_prompt_uses_access_catalog_tools() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("server crate parent")
            .to_path_buf();
        let source_root = std::env::temp_dir().join("mei_system_prompt_ask_test_root");
        let _ = std::fs::create_dir_all(&source_root);
        let native_agent = Arc::new(
            crate::mei_agent::NativeAgent::open_with_resource_tools(
                source_root.clone(),
                Arc::new(crate::mei_agent::resource_tools::NoopResourceToolExecutor::default()),
            )
            .expect("native"),
        );
        let state = AppState {
            package_root: Arc::new(package_root),
            source_root: Arc::new(source_root),
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            auth_enforcement: crate::auth::AuthEnforcement::Disabled,
            agent_runtime: Arc::new(Mutex::new(ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
        };
        let sys = build_meilang_system_prompt(
            &state,
            None,
            &BridgePromptRequest {
                text: String::new(),
                app_id: None,
                scene_id: None,
                target_file: None,
                system: None,
                mode: Some("ask".into()),
                route_mode: Some("access".into()),
                agent: None,
                model: None,
                resource_visibility: None,
                browser_context: None,
                host_protocol: None,
                host_contract_schema: None,
            },
            None,
        )
        .expect("system");
        assert!(sys.contains("Tool-first information policy (ask mode, from capability catalog)"));
        assert!(sys.contains("dataset_query"));
        assert!(sys.contains("dataset_metric"));
    }
}
