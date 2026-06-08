use anyhow::Result;
use mei_lang_kernel::{host_runtime_capabilities_catalog, host_runtime_contract_descriptor};
use serde_json::json;

use super::super::args::{McpArgs, McpCommand, McpDescribeArgs};
use super::super::util::print_json_output;

pub fn mcp_command(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommand::Describe(args) => mcp_describe_command(args),
    }
}

pub fn mcp_describe_command(args: McpDescribeArgs) -> Result<()> {
    let surface = args.surface.trim().to_ascii_lowercase();
    let descriptor = match surface.as_str() {
        "editor" => json!({
            "schema_version": "mei-mcp-surface-v1",
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
        }),
        "access" => json!({
            "schema_version": "mei-mcp-surface-v1",
            "surface": "access",
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
        }),
        _ => anyhow::bail!("unsupported MCP surface `{surface}`"),
    };
    print_json_output(&descriptor, args.json)
}
