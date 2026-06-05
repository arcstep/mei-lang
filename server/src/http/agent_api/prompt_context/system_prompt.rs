use crate::{
    agent_runtime::bridge::BridgePromptRequest,
    agent_runtime::runtime::load_managed_agent_skill_meta,
    mei_agent::agent_scope_profile::resolve_resource_visibility,
    mei_agent::mode_policy::AgentModePolicy, AppState,
};

pub(crate) fn build_meilang_system_prompt(
    state: &AppState,
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
            "You are a MeiLang authoring assistant. Treat `.mei` as MeiLang scene-first DSL hosted on restricted Starlark, not Music Encoding Initiative XML.".to_string(),
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
        blocks.push(
            concat!(
                "Tool-first information policy (ask mode):\n",
                "- Ask mode is **world-first**: treat injected `[World — catalog]` / runtime summaries as the primary truth for business Q&A.\n",
                "- Do not guess resource ids, dataset fields, or `.mei` source you have not read.\n",
                "- For dataset resources, use `dataset_query` for schema/rows and `dataset_metric` for aggregated asks (count/rate/trend/summary-card values); do not read spreadsheets with `read_file`.\n",
                "- Use `read_file` only for small, scoped evidence paths allowed by the current resource visibility (usually under the active app folder).\n",
                "- Do not generate or suggest direct `.mei` rewrite plans in ask mode.\n",
                "- Ask mode intentionally disables authoring skill tools; avoid author-time scaffolding unless the user explicitly asks for verbatim DSL."
            )
            .to_string(),
        );
    } else {
        blocks.push(
            concat!(
                "Tool-first information policy (build mode):\n",
                "- Build mode is **scene-first** with an optional **source-focus file**: anchor on the current scene; inline only the active source-focus `.mei` body; treat other session context as structured scene index, not the full app.\n",
                "- Do not guess resource ids, component keys, dataset fields, or `.mei` source you have not read.\n",
                "- The session injects a **[World — catalog]** block first: treat it as the authoritative index of `world.resources` (datasets, sources, metric ids) plus query-tool contracts.\n",
                "- For routine **authoring** tasks, prefer `read_file` / `skill_*` over repeatedly calling `dataset_query` unless you need live data samples to tune bindings.\n",
                "- For dataset resources, call **`dataset_query` once** in the first tool round when the question needs schema/filters/sample rows; it returns bounded (`schema + filters + metric ids + first 10 rows + first 10 columns`, cell text truncated).\n",
                "- For aggregated questions like '多少/占比/趋势/卡片值', prefer **`dataset_metric` once** when the dataset id is known/implied and metric ids are available from the world catalog or dataset query output.\n",
                "- **Do not** chain `read_file` on the active `.mei` after successful `dataset_query` / `dataset_metric` unless the user wants **verbatim DSL** or file edits. **Never** `read_file` `.xlsx` / spreadsheets (binary).\n",
                "- **Do not** call `resource_list` / `resource_get` / `resource_runtime_peek` for routine dataset Q&A after a successful dataset tool call; only use runtime peek when user explicitly asks phase/trace.\n",
                "- Read workspace **text** files with `read_file` (path relative to workspace root, no `..`; app-owned `.mei` / `.md` paths almost always start with `<app_id>/`, e.g. `spbjw/data/...`).\n",
                "- Query datasets with `dataset_query` / `dataset_metric` (optional overrides: scene_id, target_file) within the allowed resource visibility scope.\n",
                "- Read MeiLang author skill docs with `skill_list` then `skill_read` (path relative to skill root, no `..`).\n",
                "- Only pull large sources when the user asks for edits/audits/reviews or you need evidence to answer correctly.",
            )
            .to_string(),
        );
        match load_managed_agent_skill_meta(state) {
            Ok(Some(meta)) => {
                let mut block = String::new();
                block.push_str("[MeiLang Author Skill — index]\n");
                block.push_str(&format!(
                    "source_kind: {}\nskill_home: {}\n",
                    meta.source_kind, meta.skill_home
                ));
                block.push_str(
                    "Load authoring rules on demand: call `skill_list`, then `skill_read` for e.g. `syntax-rules.md` or `authoring.md`.\n",
                );
                if !meta.companion_files.is_empty() {
                    block.push_str("companion_md:\n");
                    for item in meta.companion_files.iter().take(24) {
                        block.push_str(&format!("- {item}\n"));
                    }
                    if meta.companion_files.len() > 24 {
                        block.push_str(&format!(
                            "... and {} more (see skill_list)\n",
                            meta.companion_files.len() - 24
                        ));
                    }
                }
                blocks.push(block.trim().to_string());
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "failed to load mei-lang skill meta");
            }
        }
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
            agent_runtime: Arc::new(Mutex::new(ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
            gis_tiles: Arc::new(crate::gis_config::GisTilesConfig::resolve()),
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
            },
            Some("compact-session-ctx"),
        )
        .expect("system");
        assert!(sys.contains("Tool-first information policy (build mode)"));
        assert!(sys.contains("[MeiLang Session Context]"));
        assert!(sys.contains("compact-session-ctx"));
        assert!(
            !sys.contains("## 阅读顺序"),
            "companion-only headings should not be inlined: {}",
            sys.len()
        );
    }
}
