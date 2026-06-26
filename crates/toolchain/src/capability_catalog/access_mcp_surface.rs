use super::prelude::*;
use super::*;

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

pub(crate) fn access_mcp_surface_descriptor(
    package_root: &Path,
    workspace_root: Option<&Path>,
    access_adapter_reference: String,
    access_adapter_entrypoint: String,
) -> Value {
    json!({
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
    })
}
