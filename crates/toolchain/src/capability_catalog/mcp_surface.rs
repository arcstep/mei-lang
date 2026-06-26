use super::prelude::*;
use super::*;

fn workspace_relative_path(path: &str) -> String {
    path.to_string()
}

fn workspace_runtime_bin_path(file_name: &str) -> String {
    workspace_relative_path(&format!("toolchain/bin/{file_name}"))
}

pub(crate) fn mcp_surface_descriptor_for_roots(
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
                "knowledge_root": workspace_root.map(|_| workspace_relative_path("runtime/platform/knowledge"))
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
        "access" => Some(access_mcp_surface_descriptor(
            package_root,
            workspace_root,
            access_adapter_reference,
            access_adapter_entrypoint,
        )),
        _ => None,
    }
}

pub fn mcp_surface_descriptor(surface: &str) -> Option<Value> {
    mcp_surface_descriptor_for_roots(
        surface,
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .as_path(),
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
