use super::prelude::*;

pub(crate) fn declared_layout() -> Vec<EditorRuntimePathDescriptor> {
    vec![
        EditorRuntimePathDescriptor {
            id: "mei_toolchain_bin".to_string(),
            rel_path: "bin/mei-toolchain".to_string(),
            purpose:
                "Headless toolchain entrypoint for check, inspect, query, and workspace setup."
                    .to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "mei_lsp_bin".to_string(),
            rel_path: "bin/mei-lsp".to_string(),
            purpose: "Language server entrypoint for IDE integrations.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "mei_host_web_bin".to_string(),
            rel_path: "bin/mei-host-web".to_string(),
            purpose: "Workspace-local browser host entrypoint.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "author_mcp_adapter".to_string(),
            rel_path: "bin/author-mcp-adapter".to_string(),
            purpose: "stdio MCP adapter for author-side AI tools.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "access_mcp_adapter".to_string(),
            rel_path: "bin/access-mcp-adapter".to_string(),
            purpose: "stdio MCP adapter for access-side AI tools.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "capability_catalog".to_string(),
            rel_path: "share/mei/catalog/capability-catalog.json".to_string(),
            purpose: "Single-source capability catalog projection.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "author_surface".to_string(),
            rel_path: "share/mei/catalog/author-surface.json".to_string(),
            purpose: "Author MCP surface descriptor.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "knowledge_bundle".to_string(),
            rel_path: "share/mei/knowledge/author".to_string(),
            purpose: "Authoring knowledge bundle: skill, docs, examples, recipes.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "platform_assets".to_string(),
            rel_path: "share/mei/platform-assets/stock".to_string(),
            purpose: "Built-in component and template assets.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "tooling_templates".to_string(),
            rel_path: "share/mei/tooling-templates".to_string(),
            purpose: "Per-tool glue templates and onboarding stubs.".to_string(),
        },
    ]
}

pub(crate) fn current_source_layout() -> Vec<EditorRuntimePathDescriptor> {
    vec![
        EditorRuntimePathDescriptor {
            id: "author_mcp_adapter".to_string(),
            rel_path: "scripts/mcp/mei-author-stdio-adapter.mjs".to_string(),
            purpose: "Current source-backed author MCP adapter.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "access_mcp_adapter".to_string(),
            rel_path: "scripts/mcp/mei-access-stdio-adapter.mjs".to_string(),
            purpose: "Current source-backed access MCP adapter.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "author_skill".to_string(),
            rel_path: "agent/guides/author-skills/SKILL.md".to_string(),
            purpose: "Current authoring skill entrypoint in the source tree.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "author_profile".to_string(),
            rel_path: "agent/guides/author-profile.md".to_string(),
            purpose: "Canonical author profile guidance shipped with the package.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "access_profile".to_string(),
            rel_path: "agent/guides/access-profile.md".to_string(),
            purpose: "Canonical access profile guidance shipped with the package.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "access_skill".to_string(),
            rel_path: "agent/guides/access-skills/SKILL.md".to_string(),
            purpose: "Canonical access skill entrypoint in the source tree.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "stock_components".to_string(),
            rel_path: "stock/components".to_string(),
            purpose: "Built-in component packs in the source tree.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "stock_templates".to_string(),
            rel_path: "stock/templates".to_string(),
            purpose: "Built-in template packs in the source tree.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "standalone_example".to_string(),
            rel_path: "agent/knowledge/editor-runtime/minimal-app-main.mei".to_string(),
            purpose: "Minimal standalone app example shipped inside the source package."
                .to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "editor_runtime_docs".to_string(),
            rel_path: "agent/knowledge/editor-runtime".to_string(),
            purpose: "Package-contained editor runtime docs.".to_string(),
        },
    ]
}

pub(crate) fn tooling_templates() -> Vec<EditorRuntimeTemplateDescriptor> {
    vec![
        EditorRuntimeTemplateDescriptor {
            tool: "cursor".to_string(),
            description: "Cursor rule + MCP configuration scaffold.".to_string(),
            files: vec![
                ".cursor/rules/meilang-authoring.mdc".to_string(),
                ".cursor/mcp.json".to_string(),
            ],
        },
        EditorRuntimeTemplateDescriptor {
            tool: "vscode".to_string(),
            description: "VS Code settings/tasks scaffold and onboarding notes.".to_string(),
            files: vec![
                ".vscode/settings.json".to_string(),
                ".vscode/tasks.json".to_string(),
                "runtime/platform/tooling/vscode/README.md".to_string(),
            ],
        },
        EditorRuntimeTemplateDescriptor {
            tool: "trae".to_string(),
            description: "Trae authoring notes and MCP config scaffold.".to_string(),
            files: vec![
                ".trae/rules/meilang-authoring.md".to_string(),
                ".trae/mcp.json".to_string(),
            ],
        },
        EditorRuntimeTemplateDescriptor {
            tool: "codex".to_string(),
            description: "Codex MCP and knowledge bridge notes.".to_string(),
            files: vec![
                "runtime/platform/tooling/codex/README.md".to_string(),
                "runtime/platform/tooling/codex/mcp.json".to_string(),
            ],
        },
        EditorRuntimeTemplateDescriptor {
            tool: "claude-code".to_string(),
            description: "Claude Code MCP and authoring prompt notes.".to_string(),
            files: vec![
                "runtime/platform/tooling/claude-code/README.md".to_string(),
                "runtime/platform/tooling/claude-code/mcp.json".to_string(),
            ],
        },
        EditorRuntimeTemplateDescriptor {
            tool: "opencode".to_string(),
            description: "OpenCode MCP and workflow bridge notes.".to_string(),
            files: vec![
                "runtime/platform/tooling/opencode/README.md".to_string(),
                "runtime/platform/tooling/opencode/mcp.json".to_string(),
            ],
        },
    ]
}
