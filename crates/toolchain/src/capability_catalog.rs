use std::path::Path;

use mei_lang_kernel::{host_runtime_capabilities_catalog, host_runtime_contract_descriptor};
use serde::Serialize;
use serde_json::{json, Value};

use crate::platform_assets::platform_asset_catalog_descriptor_for_package_root;

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
    pub primary_inputs: Vec<String>,
    pub recommended_flow: Vec<String>,
    pub preferred_surface: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance_file_rel: Option<String>,
}

pub fn meilang_author_skill_package() -> SkillPackageDescriptor {
    SkillPackageDescriptor {
        id: "meilang-author".to_string(),
        name: "MeiLang Author".to_string(),
        description: "Canonical MeiLang authoring skill package exported from the toolchain capability catalog.".to_string(),
        source_dir_rel: "guides/claude-skills".to_string(),
        install_dir_rel: ".mei/skills/meilang-author".to_string(),
        entry_file: "SKILL.md".to_string(),
        companion_priority: vec![
            "authoring.md".to_string(),
            "syntax-rules.md".to_string(),
            "components-reference.md".to_string(),
            "context.md".to_string(),
        ],
    }
}

pub fn author_profile_descriptor() -> AiProfileDescriptor {
    AiProfileDescriptor {
        id: "author".to_string(),
        name: "MeiLang Author".to_string(),
        description: "Source-first authoring profile: prioritize `.mei` source, syntax knowledge, examples, diagnostics, and external dev tools over host-side runtime callbacks.".to_string(),
        primary_inputs: vec![
            "current_mei_source".to_string(),
            "syntax_rules".to_string(),
            "components_reference".to_string(),
            "examples".to_string(),
            "mei_check_and_lsp_diagnostics".to_string(),
        ],
        recommended_flow: vec![
            "read_target_source".to_string(),
            "read_author_docs_and_examples".to_string(),
            "run_mei_check_or_mei_lsp".to_string(),
            "use_inspect_or_query_only_when_runtime_facts_are_needed".to_string(),
        ],
        preferred_surface: "editor".to_string(),
        skill_package_id: Some("meilang-author".to_string()),
        guidance_file_rel: None,
    }
}

pub fn access_profile_descriptor() -> AiProfileDescriptor {
    AiProfileDescriptor {
        id: "access".to_string(),
        name: "MeiLang Access".to_string(),
        description: "World-first access profile: prioritize runtime/world/dataset/metric facts, bounded query tools, and request-time browser query_state over static source guessing.".to_string(),
        primary_inputs: vec![
            "world_context_snapshot".to_string(),
            "resource_inventory".to_string(),
            "dataset_metric_results".to_string(),
            "dataset_query_results".to_string(),
            "browser_query_state".to_string(),
            "runtime_peek".to_string(),
        ],
        recommended_flow: vec![
            "read_world_catalog_and_runtime_summary".to_string(),
            "merge_browser_query_state_into_eval_scope".to_string(),
            "prefer_preinjected_metric_preview_then_dataset_metric".to_string(),
            "use_read_file_only_for_small_verbatim_evidence".to_string(),
        ],
        preferred_surface: "access".to_string(),
        skill_package_id: None,
        guidance_file_rel: Some("guides/access-profile.md".to_string()),
    }
}

pub fn capability_catalog_descriptor() -> Value {
    json!(capability_catalog_descriptor_for_package_root(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").as_path()
    ))
}

pub fn capability_catalog_descriptor_for_package_root(package_root: &Path) -> Value {
    json!({
        "schema_version": CAPABILITY_CATALOG_SCHEMA_VERSION,
        "toolchain_role": "canonical_truth",
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
            meilang_author_skill_package()
        ],
        "mcp_surfaces": [
            mcp_surface_descriptor("author").expect("author surface"),
            mcp_surface_descriptor("access").expect("access surface")
        ]
    })
}

pub fn mcp_surface_descriptor(surface: &str) -> Option<Value> {
    match surface.trim().to_ascii_lowercase().as_str() {
        "editor" | "author" => Some(json!({
            "schema_version": MCP_SURFACE_SCHEMA_VERSION,
            "surface": "editor",
            "profile_id": "author",
            "profile": "editor_readonly_minimal_v1",
            "transport": {
                "status": "adapter_ready",
                "recommended": "run `npm run mcp:editor-adapter` for stdio MCP and `npm run test:mcp:editor-adapter` for smoke validation"
            },
            "adapter": {
                "reference": "scripts/mcp/mei-editor-stdio-adapter.mjs",
                "entrypoint": "node ./scripts/mcp/mei-editor-stdio-adapter.mjs",
                "smoke_test": "npm run test:mcp:editor-adapter"
            },
            "runtime": {
                "cli_entrypoint": "mei",
                "lsp_entrypoint": "mei-lsp (stdio)",
                "adapter_entrypoint": "node ./scripts/mcp/mei-editor-stdio-adapter.mjs"
            },
            "skill_package": meilang_author_skill_package(),
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
                    "name": "mei_check",
                    "description": "Compile an app and return diagnostics plus revision metadata.",
                    "backed_by": "mei check --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
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
                    "backed_by": "mei compile --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
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
                    "backed_by": "mei host describe --json",
                    "input_schema": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false
                    }
                },
                {
                    "name": "mei_workspace_summary",
                    "description": "Return a workspace-level headless summary, including discovered apps, aliases, menu groups, layout health, and compile-derived app semantics such as app_kind, semantic_tags, and business_explanation.",
                    "backed_by": "mei workspace summary [--source-root <dir>] --json",
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
                    "backed_by": "mei inspect world --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
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
                    "backed_by": "mei inspect inventory --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
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
                    "backed_by": "mei inspect summary --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] --json",
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
                    "backed_by": "mei query dataset --app <app> --source-root <dir> --id <dataset_id> [--scene <scene>] [--filter key=value]... [--column name]... [--limit N] --json",
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
                    "backed_by": "mei query metric --app <app> --source-root <dir> --id <dataset_id> [--metric-id <metric>]... [--scene <scene>] [--filter key=value]... --json",
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
                    "backed_by": "mei runtime peek --app <app> [--source-root <dir>] [--scene <scene>] [--target-file <file>] [--trace-limit N] --json",
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
                    "backed_by": "mei query resource --app <app> --source-root <dir> --id <resource_id> [--scene <scene>] [--target-file <file>] --json",
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
                "note": "Editor-side MCP currently wraps semantic read/check/query surfaces only; file writes stay in the external dev tool."
            },
            "host_contract": host_runtime_contract_descriptor()
        })),
        "access" => Some(json!({
            "schema_version": MCP_SURFACE_SCHEMA_VERSION,
            "surface": "access",
            "profile_id": "access",
            "profile": "access_readonly_world_v1",
            "transport": {
                "status": "descriptor_ready",
                "recommended": "bind these tools to host-side access agents after scope/auth is enforced"
            },
            "context_ir": {
                "primary": "world-first",
                "producer": "mei inspect world --app <app> [--scene <scene>] [--target-file <file>] --json",
                "eval_scope": "merge browser query_state into bounded dataset/metric evaluation before answering"
            },
            "guidance_file_rel": "guides/access-profile.md",
            "tools": [
                {
                    "name": "dataset_query",
                    "description": "Bounded dataset schema/sample-row query for visitor-facing QA.",
                    "backed_by": "mei query dataset --app <app> --id <dataset_id> [--scene <scene>] [--filter key=value]... [--column name]... [--limit N] --json"
                },
                {
                    "name": "dataset_metric",
                    "description": "Bounded aggregate metric query for visitor-facing QA.",
                    "backed_by": "mei query metric --app <app> --id <dataset_id> [--metric-id <metric>]... [--scene <scene>] [--filter key=value]... --json"
                },
                {
                    "name": "resource_list",
                    "description": "List world assets/resources visible in the current scope.",
                    "backed_by": "mei inspect inventory --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "resource_get",
                    "description": "Fetch a single world resource/entity payload.",
                    "backed_by": "mei query resource --app <app> --id <resource_id> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "resource_runtime_peek",
                    "description": "Peek runtime phase/result/actions for the current scope.",
                    "backed_by": "mei runtime peek --app <app> [--scene <scene>] [--target-file <file>] --json"
                },
                {
                    "name": "resource_runtime_trace_export",
                    "description": "Export a bounded runtime trace envelope for the current scope.",
                    "backed_by": "mei export runtime-trace --app <app> [--scene <scene>] [--target-file <file>] [--trace-limit N] --json"
                },
                {
                    "name": "resource_business_summary",
                    "description": "Return a bounded business summary for the current app/scene/world scope.",
                    "backed_by": "mei inspect summary --app <app> [--scene <scene>] [--target-file <file>] --json"
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
