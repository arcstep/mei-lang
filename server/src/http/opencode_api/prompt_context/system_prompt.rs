use crate::{
    opencode::runtime::load_managed_opencode_skill_prompt,
    AppState,
};

pub(super) fn build_meilang_system_prompt(
    state: &AppState,
    existing: Option<&str>,
    session_context: Option<&str>,
) -> Option<String> {
    let mut blocks = Vec::new();
    if let Some(system) = existing.map(str::trim).filter(|value| !value.is_empty()) {
        blocks.push(system.to_string());
    }
    blocks.push(
        "You are a MeiLang authoring assistant. Treat `.mei` as MeiLang scene-first DSL hosted on restricted Starlark, not Music Encoding Initiative XML.".to_string(),
    );
    blocks.push(
        "Prefer declarative bindings: app(entries=[entry(...)]), scene(world/flow/frame=...), world(id=...), flow(id=...), frame(id=...), frame.add_panel(...).".to_string(),
    );
    blocks.push(
        "Default to Chinese (Simplified Chinese) for all responses, plans, progress updates, and explanations unless the user explicitly requests another language.".to_string(),
    );
    blocks.push(
        "When presenting a plan, keep the execution-oriented content in Chinese and avoid switching to English by default.".to_string(),
    );
    match load_managed_opencode_skill_prompt(state) {
        Ok(Some(skill_prompt)) => {
            let mut block = String::new();
            block.push_str("[MeiLang Claude Skill Entry]\n");
            block.push_str(&skill_prompt.entry_markdown);
            block.push_str("\n\n[Skill Home]\n");
            block.push_str(&format!(
                "source_kind: {}\npath: {}",
                skill_prompt.source_kind, skill_prompt.skill_home
            ));
            block.push_str(
                "\n\n[Important]\nCompanion files are relative to skill_home. Resolve them as `skill_home/<file>` before reading.",
            );
            if !skill_prompt.companion_files.is_empty() {
                block.push_str("\n\n[Companion Files]\n");
                for item in skill_prompt.companion_files {
                    block.push_str(&format!("- rel: {item}\n"));
                }
            }
            blocks.push(block.trim().to_string());
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to load mei-lang skill prompt");
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
