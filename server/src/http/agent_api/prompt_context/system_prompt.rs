use crate::{agent_runtime::runtime::load_managed_agent_skill_meta, AppState};

pub(crate) fn build_meilang_system_prompt(
    state: &AppState,
    existing: Option<&str>,
    mode: Option<&str>,
    session_context: Option<&str>,
) -> Option<String> {
    let mode = mode
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
        "Prefer declarative bindings: app(entries=[entry(...)]), scene(world/flow/frame=...), world(id=...), flow(id=...), frame(id=...), frame.add_panel(...).".to_string(),
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
                "- Do not guess resource ids, component keys, dataset fields, or `.mei` source you have not read.\n",
                "- Use only read-only workspace evidence and keep answers scoped to the current page/target.\n",
                "- For dataset resources, call `dataset_query`; do not read spreadsheets with `read_file`.\n",
                "- Do not generate or suggest direct `.mei` rewrite plans in ask mode.\n",
                "- Ask mode intentionally disables authoring skill tools; answer from injected context and read-only files."
            )
            .to_string(),
        );
    } else {
        blocks.push(
            concat!(
                "Tool-first information policy:\n",
                "- Do not guess resource ids, component keys, dataset fields, or `.mei` source you have not read.\n",
                "- The session injects a **[World — catalog]** block first: treat it as the authoritative index of `world.resources` (datasets, sources, metric ids) plus query-tool contracts.\n",
                "- For dataset resources, call **`dataset_query` once** in the first tool round when the id is known/implied; default output is bounded (`schema + filters + metric ids + first 10 rows + first 10 columns`, cell text truncated).\n",
                "- **Do not** chain `read_file` on the entry `.mei` after successful `dataset_query` unless the user wants **verbatim DSL** or file edits. **Never** `read_file` `.xlsx` / spreadsheets (binary).\n",
                "- **Do not** call `resource_list` / `resource_get` / `resource_runtime_peek` for routine dataset Q&A after a successful `dataset_query`; only use runtime peek when user explicitly asks phase/trace.\n",
                "- Session context is still an index, not full app source.\n",
                "- Read workspace **text** files with `read_file` (path relative to workspace root, no `..`; app-owned `.mei` / `.md` paths almost always start with `<app_id>/`, e.g. `spbjw/data/...`).\n",
                "- Query datasets with `dataset_query` (optional overrides: scene_id, entry_id, target_file).\n",
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
                package_root.clone(),
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
        };
        let sys =
            build_meilang_system_prompt(&state, None, Some("build"), Some("compact-session-ctx"))
                .expect("system");
        assert!(sys.contains("Tool-first information policy"));
        assert!(sys.contains("[MeiLang Session Context]"));
        assert!(sys.contains("compact-session-ctx"));
        assert!(
            !sys.contains("## 阅读顺序"),
            "companion-only headings should not be inlined: {}",
            sys.len()
        );
    }
}
