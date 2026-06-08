use mei_lang_kernel::{host_runtime_capabilities_catalog, host_runtime_contract_descriptor};
use serde::Serialize;
use serde_json::{json, Value};

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
        ],
    }
}

pub fn capability_catalog_descriptor() -> Value {
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
        "skill_packages": [
            meilang_author_skill_package()
        ],
        "mcp_surfaces": [
            mcp_surface_descriptor("editor").expect("editor surface"),
            mcp_surface_descriptor("access").expect("access surface")
        ]
    })
}

pub fn mcp_surface_descriptor(surface: &str) -> Option<Value> {
    match surface.trim().to_ascii_lowercase().as_str() {
        "editor" => Some(json!({
            "schema_version": MCP_SURFACE_SCHEMA_VERSION,
            "surface": "editor",
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
            "profile": "access_readonly_world_v1",
            "transport": {
                "status": "descriptor_ready",
                "recommended": "bind these tools to host-side access agents after scope/auth is enforced"
            },
            "context_ir": {
                "primary": "world-first",
                "producer": "mei inspect world --app <app> [--scene <scene>] [--target-file <file>] --json"
            },
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
