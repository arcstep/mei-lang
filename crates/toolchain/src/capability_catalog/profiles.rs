use super::prelude::*;

pub fn meilang_author_skill_package() -> SkillPackageDescriptor {
    SkillPackageDescriptor {
        id: "meilang-author".to_string(),
        name: "MeiLang Author".to_string(),
        description: "Canonical MeiLang authoring skill package exported from the toolchain capability catalog.".to_string(),
        source_dir_rel: "guides/author-skills".to_string(),
        install_dir_rel: "runtime/platform/skills/meilang-author".to_string(),
        entry_file: "SKILL.md".to_string(),
        companion_priority: vec![
            "authoring.md".to_string(),
            "syntax-rules.md".to_string(),
            "dsl-reference.md".to_string(),
            "namespace-reference.md".to_string(),
            "components-reference.md".to_string(),
            "context.md".to_string(),
        ],
    }
}

pub fn meilang_access_skill_package() -> SkillPackageDescriptor {
    SkillPackageDescriptor {
        id: "meilang-access".to_string(),
        name: "MeiLang Access".to_string(),
        description:
            "Canonical MeiLang access skill package exported from the toolchain capability catalog."
                .to_string(),
        source_dir_rel: "guides/access-skills".to_string(),
        install_dir_rel: "runtime/platform/skills/meilang-access".to_string(),
        entry_file: "SKILL.md".to_string(),
        companion_priority: vec!["workflow.md".to_string()],
    }
}

pub fn author_profile_descriptor() -> AiProfileDescriptor {
    AiProfileDescriptor {
        id: "author".to_string(),
        name: "MeiLang Author".to_string(),
        description: "Source-first authoring profile: prioritize `.mei` source, syntax knowledge, examples, diagnostics, and external dev tools over host-side runtime callbacks.".to_string(),
        aliases: Vec::new(),
        context_strategy: "source_first".to_string(),
        authority_chain: vec![
            "current_mei_source".to_string(),
            "knowledge_bundle".to_string(),
            "compile_and_lsp_diagnostics".to_string(),
            "runtime_queries_only_when_needed".to_string(),
        ],
        primary_inputs: vec![
            "current_mei_source".to_string(),
            "syntax_rules".to_string(),
            "components_reference".to_string(),
            "examples".to_string(),
            "mei_check_and_lsp_diagnostics".to_string(),
        ],
        recommended_flow: vec![
            "read_target_source".to_string(),
            "read_author_profile".to_string(),
            "read_author_docs_and_examples".to_string(),
            "run_mei_check_or_mei_lsp".to_string(),
            "use_inspect_or_query_only_when_runtime_facts_are_needed".to_string(),
        ],
        preferred_surface: "author".to_string(),
        knowledge_surface: "author".to_string(),
        skill_package_id: Some("meilang-author".to_string()),
        guidance_file_rel: Some("guides/author-profile.md".to_string()),
        guidance_bundle_asset_id: Some("author_profile".to_string()),
    }
}

pub fn access_profile_descriptor() -> AiProfileDescriptor {
    AiProfileDescriptor {
        id: "access".to_string(),
        name: "MeiLang Access".to_string(),
        description: "World-first access profile: prioritize runtime/world/dataset/metric facts, bounded query tools, and request-time browser query_state over static source guessing.".to_string(),
        aliases: Vec::new(),
        context_strategy: "world_first_eval_first".to_string(),
        authority_chain: vec![
            "world_snapshot".to_string(),
            "inventory".to_string(),
            "browser_query_state".to_string(),
            "dataset_metric_query".to_string(),
            "runtime_trace".to_string(),
        ],
        primary_inputs: vec![
            "world_context_snapshot".to_string(),
            "resource_inventory".to_string(),
            "dataset_metric_results".to_string(),
            "dataset_query_results".to_string(),
            "browser_query_state".to_string(),
            "runtime_peek".to_string(),
        ],
        recommended_flow: vec![
            "read_access_profile".to_string(),
            "read_world_catalog_and_runtime_summary".to_string(),
            "merge_browser_query_state_into_eval_scope".to_string(),
            "prefer_preinjected_metric_preview_then_dataset_metric".to_string(),
            "use_read_file_only_for_small_verbatim_evidence".to_string(),
        ],
        preferred_surface: "access".to_string(),
        knowledge_surface: "access".to_string(),
        skill_package_id: Some("meilang-access".to_string()),
        guidance_file_rel: Some("guides/access-profile.md".to_string()),
        guidance_bundle_asset_id: Some("access_profile".to_string()),
    }
}

pub fn ai_profile_descriptor(profile_id: &str) -> Option<AiProfileDescriptor> {
    match profile_id.trim().to_ascii_lowercase().as_str() {
        "author" => Some(author_profile_descriptor()),
        "access" => Some(access_profile_descriptor()),
        _ => None,
    }
}

pub fn ai_profile_policy_lines(profile_id: &str) -> Vec<String> {
    let Some(profile) = ai_profile_descriptor(profile_id) else {
        return Vec::new();
    };
    let mut lines = vec![format!(
        "Profile `{}` ({}) is {}.",
        profile.id, profile.name, profile.description
    )];
    if !profile.primary_inputs.is_empty() {
        lines.push(format!(
            "Primary inputs: {}.",
            profile.primary_inputs.join(", ")
        ));
    }
    lines.push(format!(
        "Preferred surface: `{}`; knowledge surface: `{}`; context strategy: `{}`.",
        profile.preferred_surface, profile.knowledge_surface, profile.context_strategy
    ));
    if !profile.recommended_flow.is_empty() {
        lines.push("Recommended flow:".to_string());
        for (index, step) in profile.recommended_flow.iter().enumerate() {
            lines.push(format!("{}. {}", index + 1, humanize_flow_step(step)));
        }
    }
    if let Some(guidance) = profile.guidance_file_rel.as_deref() {
        lines.push(format!("Guidance file: `{guidance}`."));
    }
    lines
}

fn humanize_flow_step(step: &str) -> String {
    match step {
        "read_target_source" => "Read the target `.mei` source before runtime queries.".to_string(),
        "read_author_profile" => {
            "Read the canonical author profile before companion docs and examples.".to_string()
        }
        "read_author_docs_and_examples" => {
            "Read author docs, examples, and component references.".to_string()
        }
        "run_mei_check_or_mei_lsp" => {
            "Run `mei-toolchain check` or `mei-lsp` for diagnostics.".to_string()
        }
        "use_inspect_or_query_only_when_runtime_facts_are_needed" => {
            "Use inspect/query only when runtime facts are needed.".to_string()
        }
        "read_access_profile" => {
            "Read the canonical access profile and companion workflow before runtime questions."
                .to_string()
        }
        "read_world_catalog_and_runtime_summary" => {
            "Read world catalog and runtime summary for the active app/scene scope.".to_string()
        }
        "merge_browser_query_state_into_eval_scope" => {
            "Merge browser `query_state` into bounded eval scope before answering.".to_string()
        }
        "prefer_preinjected_metric_preview_then_dataset_metric" => {
            "Prefer injected metric previews, then call `dataset_metric` / `dataset_query`."
                .to_string()
        }
        "use_read_file_only_for_small_verbatim_evidence" => {
            "Use `read_file` only for small verbatim DSL evidence.".to_string()
        }
        other => other.replace('_', " "),
    }
}
