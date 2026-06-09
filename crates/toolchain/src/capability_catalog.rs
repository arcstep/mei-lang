use std::path::Path;

use mei_lang_kernel::{
    host_extension_registry_descriptor, host_requirements_descriptor,
    host_runtime_capabilities_catalog, host_runtime_contract_descriptor,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::knowledge_bundle::knowledge_bundle_descriptor_for_package_root;
use crate::platform_assets::platform_asset_catalog_descriptor_for_package_root;
use crate::types::ResourceQueryToolSpec;

pub const CAPABILITY_CATALOG_SCHEMA_VERSION: &str = "mei-capability-catalog-v1";
pub const MCP_SURFACE_SCHEMA_VERSION: &str = "mei-mcp-surface-v1";

#[derive(Debug, Clone, Serialize)]
pub struct SkillPackageDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_dir_rel: String,
    pub install_dir_rel: String,
    pub entry_file: String,
    pub companion_priority: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiProfileDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub context_strategy: String,
    pub authority_chain: Vec<String>,
    pub primary_inputs: Vec<String>,
    pub recommended_flow: Vec<String>,
    pub preferred_surface: String,
    pub knowledge_surface: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance_file_rel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance_bundle_asset_id: Option<String>,
}

pub fn meilang_author_skill_package() -> SkillPackageDescriptor {
    SkillPackageDescriptor {
        id: "meilang-author".to_string(),
        name: "MeiLang Author".to_string(),
        description: "Canonical MeiLang authoring skill package exported from the toolchain capability catalog.".to_string(),
        source_dir_rel: "guides/author-skills".to_string(),
        install_dir_rel: ".mei/skills/meilang-author".to_string(),
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
        description: "Canonical MeiLang access skill package exported from the toolchain capability catalog.".to_string(),
        source_dir_rel: "guides/access-skills".to_string(),
        install_dir_rel: ".mei/skills/meilang-access".to_string(),
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
        "run_mei_check_or_mei_lsp" => "Run `mei-toolchain check` or `mei-lsp` for diagnostics.".to_string(),
        "use_inspect_or_query_only_when_runtime_facts_are_needed" => {
            "Use inspect/query only when runtime facts are needed.".to_string()
        }
        "read_access_profile" => {
            "Read the canonical access profile and companion workflow before runtime questions.".to_string()
        }
        "read_world_catalog_and_runtime_summary" => {
            "Read world catalog and runtime summary for the active app/scene scope.".to_string()
        }
        "merge_browser_query_state_into_eval_scope" => {
            "Merge browser `query_state` into bounded eval scope before answering.".to_string()
        }
        "prefer_preinjected_metric_preview_then_dataset_metric" => {
            "Prefer injected metric previews, then call `dataset_metric` / `dataset_query`.".to_string()
        }
        "use_read_file_only_for_small_verbatim_evidence" => {
            "Use `read_file` only for small verbatim DSL evidence.".to_string()
        }
        other => other.replace('_', " "),
    }
}

pub fn capability_catalog_descriptor() -> Value {
    json!(capability_catalog_descriptor_for_package_root(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").as_path()
    ))
}

fn capability_catalog_descriptor_for_roots(
    package_root: &Path,
    workspace_root: Option<&Path>,
) -> Value {
    let workspace_root_marker = workspace_root.map(|_| ".".to_string());
    json!({
        "schema_version": CAPABILITY_CATALOG_SCHEMA_VERSION,
        "toolchain_role": "canonical_truth",
        "workspace_root": workspace_root_marker,
        "principles": [
            "toolchain_is_canonical_truth",
            "host_is_canonical_consumer",
            "ai_capability_catalog_is_single_source",
            "platform_assets_are_first_class",
            "host_specific_capability_must_register_before_export"
        ],
        "ai_profiles": [
            author_profile_descriptor(),
            access_profile_descriptor()
        ],
        "platform_assets": platform_asset_catalog_descriptor_for_package_root(package_root),
        "skill_packages": [
            meilang_author_skill_package(),
            meilang_access_skill_package()
        ],
        "knowledge_bundles": [
            knowledge_bundle_descriptor_for_package_root(package_root, "author")
                .expect("author knowledge bundle"),
            knowledge_bundle_descriptor_for_package_root(package_root, "access")
                .expect("access knowledge bundle")
        ],
        "host_extensions": host_extension_registry_descriptor(),
        "host_requirements": [
            host_requirements_descriptor("mei-host-web").expect("mei-host-web requirements")
        ],
        "mcp_surfaces": [
            mcp_surface_descriptor_for_roots("author", package_root, workspace_root)
                .expect("author surface"),
            mcp_surface_descriptor_for_roots("access", package_root, workspace_root)
                .expect("access surface")
        ]
    })
}

pub fn capability_catalog_descriptor_for_package_root(package_root: &Path) -> Value {
    capability_catalog_descriptor_for_roots(package_root, None)
}

pub fn capability_catalog_descriptor_for_workspace_root(
    workspace_root: &Path,
    package_root: &Path,
) -> Value {
    capability_catalog_descriptor_for_roots(package_root, Some(workspace_root))
}

fn access_host_overlay_descriptor() -> Value {
    json!({
        "binding_source": "host_runtime",
        "bound_arguments": ["app", "source_root"],
        "renamed_arguments": {
            "scene": "scene_id"
        },
        "visibility_overlays": [
            {
                "when": "resource_visibility=local_only",
                "hide_arguments": ["scene_id", "target_file"]
            }
        ],
        "host_only_tools": ["read_file", "propose_session_patch"],
        "note": "Host runtimes bind access tools to the current app/session scope, then apply visibility-based parameter trimming and optional host-only tools."
    })
}

fn workspace_relative_path(path: &str) -> String {
    path.to_string()
}

fn workspace_runtime_bin_path(file_name: &str) -> String {
    workspace_relative_path(&format!(".mei/runtime/bin/{file_name}"))
}

fn mcp_surface_descriptor_for_roots(
    surface: &str,
    package_root: &Path,
    workspace_root: Option<&Path>,
) -> Option<Value> {
    let author_adapter_reference = workspace_root
        .map(|_| workspace_runtime_bin_path("author-mcp-adapter"))
        .unwrap_or_else(|| "scripts/mcp/mei-author-stdio-adapter.mjs".to_string());
    let author_adapter_entrypoint = if workspace_root.is_some() {
        format!("node {}", author_adapter_reference)
    } else {
        "node ./scripts/mcp/mei-author-stdio-adapter.mjs".to_string()
    };
    let access_adapter_reference = workspace_root
        .map(|_| workspace_runtime_bin_path("access-mcp-adapter"))
        .unwrap_or_else(|| "scripts/mcp/mei-access-stdio-adapter.mjs".to_string());
    let access_adapter_entrypoint = if workspace_root.is_some() {
        format!("node {}", access_adapter_reference)
    } else {
        "node ./scripts/mcp/mei-access-stdio-adapter.mjs".to_string()
    };
    match surface.trim().to_ascii_lowercase().as_str() {
        "author" => Some(json!({
            "schema_version": MCP_SURFACE_SCHEMA_VERSION,
            "surface": "author",
            "profile_id": "author",
            "profile": "author_readonly_minimal_v1",
            "workspace_root": workspace_root.map(|_| ".".to_string()),
            "transport": {
                "status": "adapter_ready",
                "recommended": if workspace_root.is_some() {
                    "run the workspace-local author MCP adapter under `.mei/runtime/bin/` and keep `MEI_SOURCE_ROOT` pointed at the workspace root"
                } else {
                    "run `npm run mcp:author-adapter` for stdio MCP and `npm run test:mcp:author-adapter` for smoke validation"
                }
            },
            "adapter": {
                "reference": author_adapter_reference,
                "entrypoint": author_adapter_entrypoint,
                "smoke_test": "npm run test:mcp:author-adapter"
            },
            "runtime": {
                "cli_entrypoint": "mei-toolchain",
                "lsp_entrypoint": "mei-lsp (stdio)",
                "adapter_entrypoint": author_adapter_entrypoint,
                "catalog_root": workspace_root.map(|_| workspace_relative_path(".mei/catalog")),
                "knowledge_root": workspace_root.map(|_| workspace_relative_path(".mei/knowledge"))
            },
            "skill_package": meilang_author_skill_package(),
            "knowledge_bundle": knowledge_bundle_descriptor_for_package_root(
                package_root,
                "author"
            ).expect("author knowledge bundle"),
            "authoring_mode": {
                "strategy": "source_first",
                "guidance": [
                    "read_target_mei_before_runtime_queries",
                    "use_docs_examples_and_lsp_for_language_help",
                    "treat_summary_as_routing_hint_not_source_substitute"
                ]
            },
            "tools": [
                {
                    "name": "mei_author_knowledge",
                    "description": "Return packaged authoring docs, rules, and examples for standalone editor runtime consumers.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain knowledge --surface author [--topic <topic>] [--include-content] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "topic": { "type": "string" },
                            "include_content": { "type": "boolean" }
                        },
                        "additionalProperties": false
                    }
                },
                {
                    "name": "mei_author_runtime_describe",
                    "description": "Describe the standalone MeiLang author runtime layout, paths, and tool scaffolding contracts.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain editor-runtime describe --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false
                    }
                },
                {
                    "name": "mei_author_runtime_doctor",
                    "description": "Run readonly checks for the standalone author runtime package layout and bundled assets.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain editor-runtime doctor --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false
                    }
                },
                {
                    "name": "mei_check",
                    "description": "Compile an app and return diagnostics plus revision metadata.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain check --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_compile",
                    "description": "Compile an app and return the same JSON contract as check for scripted consumers.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain compile --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_host_describe",
                    "description": "Return machine-readable host runtime contract descriptor.",
                    "capability_origin": "host",
                    "backed_by": "mei-host-web host describe --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false
                    }
                },
                {
                    "name": "mei_workspace_summary",
                    "description": "Return a workspace-level headless summary, including discovered apps, aliases, menu groups, layout health, and compile-derived app semantics such as app_kind, semantic_tags, and business_explanation.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain workspace summary [--source-root <dir>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "source_root": { "type": "string" }
                        },
                        "additionalProperties": false
                    }
                },
                {
                    "name": "mei_inspect_world",
                    "description": "Return the structured world/runtime snapshot for the selected app scope.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain inspect world --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_inspect_inventory",
                    "description": "Return the app inventory/resource index for the selected scope.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain inspect inventory --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_inspect_summary",
                    "description": "Return a bounded business-oriented summary for the selected app/scene scope, including compile-derived routes/resources/components/diagnostics plus semantic narrative like app_kind, scene profile, flow/topology signals, and business_explanation.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain inspect summary --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_query_dataset",
                    "description": "Run bounded dataset row/schema queries.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain query dataset --app <app> --source-root <dir> --id <dataset_id> [--scene <scene>] [--filter key=value]... [--column name]... [--limit N] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "dataset_id": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "search": { "type": "string" },
                            "filters": {
                                "type": "object",
                                "additionalProperties": { "type": "string" }
                            },
                            "columns": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "limit": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["app", "dataset_id"]
                    }
                },
                {
                    "name": "mei_query_metric",
                    "description": "Run bounded runtime metric queries for a dataset.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain query metric --app <app> --source-root <dir> --id <dataset_id> [--metric-id <metric>]... [--scene <scene>] [--filter key=value]... --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "dataset_id": { "type": "string" },
                            "metric_ids": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "search": { "type": "string" },
                            "filters": {
                                "type": "object",
                                "additionalProperties": { "type": "string" }
                            }
                        },
                        "required": ["app", "dataset_id"]
                    }
                },
                {
                    "name": "mei_runtime_peek",
                    "description": "Peek current runtime phase/result/actions for the selected scope.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain runtime peek --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] [--trace-limit N] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "trace_limit": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "mei_query_resource",
                    "description": "Fetch a single world resource/entity payload.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain query resource --app <app> --source-root <dir> --id <resource_id> [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "resource_id": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app", "resource_id"]
                    }
                }
            ],
            "write_policy": {
                "default": "read_only",
                "note": "Author-side MCP currently wraps semantic read/check/query surfaces only; file writes stay in the external dev tool."
            },
            "host_contract": host_runtime_contract_descriptor()
        })),
        "access" => Some(json!({
            "schema_version": MCP_SURFACE_SCHEMA_VERSION,
            "surface": "access",
            "profile_id": "access",
            "profile": "access_readonly_world_v1",
            "workspace_root": workspace_root.map(|_| ".".to_string()),
            "transport": {
                "status": "adapter_ready",
                "recommended": if workspace_root.is_some() {
                    "run the workspace-local access MCP adapter under `.mei/runtime/bin/`; host-side agents should bind the same access tools after scope/auth is enforced"
                } else {
                    "run `npm run mcp:access-adapter` for stdio MCP; host-side agents should bind the same access surface tools after scope/auth is enforced"
                }
            },
            "adapter": {
                "reference": access_adapter_reference,
                "entrypoint": access_adapter_entrypoint,
                "smoke_test": "npm run test:mcp:access-adapter"
            },
            "context_ir": {
                "primary": "world-first",
                "producer": "mei-toolchain inspect world --app <app> [--scene <scene>] [--target-file <file>] --json",
                "eval_scope": "merge browser query_state into bounded dataset/metric evaluation before answering"
            },
            "guidance_file_rel": "guides/access-profile.md",
            "skill_package": meilang_access_skill_package(),
            "knowledge_bundle": knowledge_bundle_descriptor_for_package_root(
                package_root,
                "access"
            ).expect("access knowledge bundle"),
            "host_overlay": access_host_overlay_descriptor(),
            "tools": [
                {
                    "name": "mei_access_knowledge",
                    "description": "Return packaged access profile and companion workflow docs for standalone or host-bound access consumers.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain knowledge --surface access [--topic <topic>] [--include-content] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "topic": { "type": "string" },
                            "include_content": { "type": "boolean" }
                        },
                        "additionalProperties": false
                    }
                },
                {
                    "name": "dataset_query",
                    "description": "Bounded dataset schema/sample-row query for visitor-facing QA.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain query dataset --app <app> --id <dataset_id> [--scene <scene>] [--filter key=value]... [--column name]... [--limit N] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "dataset_id": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "search": { "type": "string" },
                            "filters": {
                                "type": "object",
                                "additionalProperties": { "type": "string" }
                            },
                            "columns": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "limit": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["app", "dataset_id"]
                    }
                },
                {
                    "name": "dataset_metric",
                    "description": "Bounded aggregate metric query for visitor-facing QA.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain query metric --app <app> --id <dataset_id> [--metric-id <metric>]... [--scene <scene>] [--filter key=value]... --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "dataset_id": { "type": "string" },
                            "metric_ids": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "search": { "type": "string" },
                            "filters": {
                                "type": "object",
                                "additionalProperties": { "type": "string" }
                            }
                        },
                        "required": ["app", "dataset_id"]
                    }
                },
                {
                    "name": "resource_list",
                    "description": "List world assets/resources visible in the current scope.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain inspect inventory --app <app> [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "resource_get",
                    "description": "Fetch a single world resource/entity payload.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain query resource --app <app> --id <resource_id> [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "resource_id": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app", "resource_id"]
                    }
                },
                {
                    "name": "resource_runtime_peek",
                    "description": "Peek runtime phase/result/actions for the current scope.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain runtime peek --app <app> [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "trace_limit": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "resource_runtime_trace_export",
                    "description": "Export a bounded runtime trace envelope for the current scope.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain export runtime-trace --app <app> [--scene <scene>] [--target-file <file>] [--trace-limit N] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" },
                            "trace_limit": { "type": "integer", "minimum": 1 }
                        },
                        "required": ["app"]
                    }
                },
                {
                    "name": "resource_business_summary",
                    "description": "Return a bounded business summary for the current app/scene/world scope.",
                    "capability_origin": "toolchain",
                    "backed_by": "mei-toolchain inspect summary --app <app> [--scene <scene>] [--target-file <file>] --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "app": { "type": "string" },
                            "source_root": { "type": "string" },
                            "scene": { "type": "string" },
                            "target_file": { "type": "string" }
                        },
                        "required": ["app"]
                    }
                }
            ],
            "write_policy": {
                "default": "read_only",
                "note": "Access-side MCP is intentionally read-only and should not expose authoring rewrite/diff/revert flows."
            },
            "runtime_capabilities": host_runtime_capabilities_catalog(),
            "host_contract": host_runtime_contract_descriptor()
        })),
        _ => None,
    }
}

pub fn mcp_surface_descriptor(surface: &str) -> Option<Value> {
    mcp_surface_descriptor_for_roots(
        surface,
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").as_path(),
        None,
    )
}

pub fn mcp_surface_descriptor_for_workspace_root(
    workspace_root: &Path,
    package_root: &Path,
    surface: &str,
) -> Option<Value> {
    mcp_surface_descriptor_for_roots(surface, package_root, Some(workspace_root))
}

pub fn access_host_bound_tool_descriptors() -> Vec<Value> {
    let Some(surface) = mcp_surface_descriptor("access") else {
        return Vec::new();
    };
    surface
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(host_bound_access_tool_descriptor)
        .collect()
}

pub fn access_host_bound_tool_names() -> Vec<String> {
    access_host_bound_tool_descriptors()
        .into_iter()
        .filter_map(|tool| {
            tool.get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

pub fn access_host_bound_query_tools() -> Vec<ResourceQueryToolSpec> {
    access_host_bound_tool_descriptors()
        .into_iter()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?.to_string();
            let description = tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let input = format_host_bound_input_summary(
                tool.get("input_schema").unwrap_or(&Value::Null),
            );
            Some(ResourceQueryToolSpec {
                id: name.clone(),
                status: access_query_tool_status(&name).to_string(),
                purpose: description,
                input,
                output: access_query_tool_output(&name).to_string(),
            })
        })
        .collect()
}

fn host_bound_access_tool_descriptor(tool: &Value) -> Option<Value> {
    let name = tool.get("name")?.as_str()?.to_string();
    let description = tool
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let input_schema = host_bound_access_input_schema(tool.get("input_schema")?);
    Some(json!({
        "name": name,
        "description": description,
        "input_schema": input_schema,
    }))
}

fn host_bound_access_input_schema(input_schema: &Value) -> Value {
    let mut schema = match input_schema.as_object() {
        Some(map) => map.clone(),
        None => return json!({ "type": "object", "properties": {} }),
    };
    let mut properties = schema
        .remove("properties")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    properties.remove("app");
    properties.remove("source_root");
    if let Some(scene) = properties.remove("scene") {
        let mut scene_prop = scene;
        if let Some(description) = scene_prop.get("description").and_then(Value::as_str) {
            let normalized = description.replace("scene", "scene id");
            if let Some(obj) = scene_prop.as_object_mut() {
                obj.insert("description".to_string(), Value::String(normalized));
            }
        }
        properties.insert("scene_id".to_string(), scene_prop);
    }
    schema.insert("properties".to_string(), Value::Object(properties));
    let required = schema
        .remove("required")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .filter(|name| name != "app" && name != "source_root")
        .map(|name| {
            if name == "scene" {
                Value::String("scene_id".to_string())
            } else {
                Value::String(name)
            }
        })
        .collect::<Vec<_>>();
    if !required.is_empty() {
        schema.insert("required".to_string(), Value::Array(required));
    }
    Value::Object(schema)
}

fn access_query_tool_status(name: &str) -> &'static str {
    match name {
        "dataset_query" | "dataset_metric" => "phase2_api_ready",
        "resource_list" | "resource_get" | "resource_runtime_peek" => "phase3_native_ready",
        "resource_runtime_trace_export" | "resource_business_summary" => "phase5_native_ready",
        _ => "catalog_bound",
    }
}

fn access_query_tool_output(name: &str) -> &'static str {
    match name {
        "dataset_query" => {
            "bounded: {dataset{schema_preview,filters,metric_ids,analysis_contracts_preview}, sample_rows, truncation, usage_hint}"
        }
        "dataset_metric" => {
            "bounded: {dataset_id, total_rows, metrics, analysis_contracts}; analysis_contracts mirrors host UI explain/popup contract"
        }
        "resource_list" => "bounded: WorldAssetListResponse JSON",
        "resource_get" => "bounded: WorldAssetGetResponse JSON",
        "resource_runtime_peek" => "bounded: WorldRuntimePeekResponse JSON",
        "resource_runtime_trace_export" => {
            "bounded: HeadlessArtifactEnvelope JSON for runtime_trace"
        }
        "resource_business_summary" => "bounded: WorldBusinessSummary JSON",
        _ => "bounded JSON result",
    }
}

fn format_host_bound_input_summary(schema: &Value) -> String {
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return "{}".to_string();
    };
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let preferred_order = [
        "dataset_id",
        "metric_ids",
        "resource_id",
        "kind",
        "search",
        "filters",
        "columns",
        "limit",
        "trace_limit",
        "scene_id",
        "target_file",
    ];
    let mut keys = preferred_order
        .iter()
        .filter(|key| props.contains_key(**key))
        .map(|key| (*key).to_string())
        .collect::<Vec<_>>();
    let mut extras = props
        .keys()
        .filter(|key| !preferred_order.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    extras.sort();
    keys.extend(extras);
    let parts = keys
        .into_iter()
        .filter_map(|name| {
            let ty = props
                .get(&name)
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("value");
            let optional = !required.iter().any(|item| *item == name);
            Some(format!(
                "{}{}: {}",
                name,
                if optional { "?" } else { "" },
                ty
            ))
        })
        .collect::<Vec<_>>();
    format!("{{{}}}", parts.join(", "))
}
