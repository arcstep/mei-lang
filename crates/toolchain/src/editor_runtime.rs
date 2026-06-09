use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::knowledge_bundle_descriptor_for_package_root;

pub const EDITOR_RUNTIME_SCHEMA_VERSION: &str = "mei-editor-runtime-v1";

#[derive(Debug, Clone, Serialize)]
pub struct EditorRuntimePathDescriptor {
    pub id: String,
    pub rel_path: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorRuntimeTemplateDescriptor {
    pub tool: String,
    pub description: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorRuntimeDescriptor {
    pub schema_version: String,
    pub package_root: String,
    pub declared_layout: Vec<EditorRuntimePathDescriptor>,
    pub current_source_layout: Vec<EditorRuntimePathDescriptor>,
    pub package_root_resolution: Vec<String>,
    pub standalone_flow: Vec<String>,
    pub tooling_templates: Vec<EditorRuntimeTemplateDescriptor>,
    pub editor_knowledge_bundle: crate::KnowledgeBundleDescriptor,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorRuntimeCheck {
    pub id: String,
    pub ok: bool,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorRuntimeDoctorReport {
    pub schema_version: String,
    pub ok: bool,
    pub package_root: String,
    pub checks: Vec<EditorRuntimeCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorRuntimeScaffoldFile {
    pub rel_path: String,
    pub overwritten: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditorRuntimeScaffoldReport {
    pub schema_version: String,
    pub target_root: String,
    pub tools: Vec<String>,
    pub files: Vec<EditorRuntimeScaffoldFile>,
}

fn declared_layout() -> Vec<EditorRuntimePathDescriptor> {
    vec![
        EditorRuntimePathDescriptor {
            id: "mei_toolchain_bin".to_string(),
            rel_path: "bin/mei-toolchain".to_string(),
            purpose: "Headless toolchain entrypoint for check, inspect, query, and workspace setup."
                .to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "mei_lsp_bin".to_string(),
            rel_path: "bin/mei-lsp".to_string(),
            purpose: "Language server entrypoint for IDE integrations.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "compat_bin".to_string(),
            rel_path: "bin/mei".to_string(),
            purpose: "Compatibility entrypoint for legacy consumers.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "editor_mcp_adapter".to_string(),
            rel_path: "bin/editor-mcp-adapter".to_string(),
            purpose: "stdio MCP adapter for editor-side AI tools.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "capability_catalog".to_string(),
            rel_path: "share/mei/catalog/capability-catalog.json".to_string(),
            purpose: "Single-source capability catalog projection.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "editor_surface".to_string(),
            rel_path: "share/mei/catalog/editor-surface.json".to_string(),
            purpose: "Editor MCP surface descriptor.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "knowledge_bundle".to_string(),
            rel_path: "share/mei/knowledge/editor".to_string(),
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

fn current_source_layout() -> Vec<EditorRuntimePathDescriptor> {
    vec![
        EditorRuntimePathDescriptor {
            id: "editor_mcp_adapter".to_string(),
            rel_path: "scripts/mcp/mei-editor-stdio-adapter.mjs".to_string(),
            purpose: "Current source-backed editor MCP adapter.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "author_skill".to_string(),
            rel_path: "guides/claude-skills/SKILL.md".to_string(),
            purpose: "Current authoring skill entrypoint in the source tree.".to_string(),
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
            rel_path: "knowledge/editor-runtime/minimal-app-main.mei".to_string(),
            purpose: "Minimal standalone app example shipped inside the source package."
                .to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "editor_runtime_docs".to_string(),
            rel_path: "knowledge/editor-runtime".to_string(),
            purpose: "Package-contained editor runtime docs.".to_string(),
        },
    ]
}

fn tooling_templates() -> Vec<EditorRuntimeTemplateDescriptor> {
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
                ".mei/tooling/vscode/README.md".to_string(),
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
                ".mei/tooling/codex/README.md".to_string(),
                ".mei/tooling/codex/mcp.json".to_string(),
            ],
        },
        EditorRuntimeTemplateDescriptor {
            tool: "claude-code".to_string(),
            description: "Claude Code MCP and authoring prompt notes.".to_string(),
            files: vec![
                ".mei/tooling/claude-code/README.md".to_string(),
                ".mei/tooling/claude-code/mcp.json".to_string(),
            ],
        },
        EditorRuntimeTemplateDescriptor {
            tool: "opencode".to_string(),
            description: "OpenCode MCP and workflow bridge notes.".to_string(),
            files: vec![
                ".mei/tooling/opencode/README.md".to_string(),
                ".mei/tooling/opencode/mcp.json".to_string(),
            ],
        },
    ]
}

pub fn editor_runtime_descriptor_for_package_root(package_root: &Path) -> EditorRuntimeDescriptor {
    let editor_knowledge_bundle =
        knowledge_bundle_descriptor_for_package_root(package_root, "editor").expect("editor bundle");
    EditorRuntimeDescriptor {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        package_root: package_root.display().to_string(),
        declared_layout: declared_layout(),
        current_source_layout: current_source_layout(),
        package_root_resolution: vec![
            "Prefer MEI_PACKAGE_ROOT when explicitly provided.".to_string(),
            "Otherwise infer from the current executable and prefer a sibling share/mei layout when present.".to_string(),
            "Fallback to source-tree package root for local development builds.".to_string(),
        ],
        standalone_flow: vec![
            "Run `mei-toolchain workspace init --standalone --source-root <dir>` to create a standalone workspace.".to_string(),
            "Run `mei-toolchain workspace materialize --source-root <dir>` to materialize .stock assets.".to_string(),
            "Run `mei-toolchain editor-runtime scaffold --target-root <dir> --tool <tool>` to write tool glue files.".to_string(),
            "Use `mei-lsp` for IDE semantics and `node scripts/mcp/mei-editor-stdio-adapter.mjs` for agent-side tools.".to_string(),
        ],
        tooling_templates: tooling_templates(),
        editor_knowledge_bundle,
    }
}

pub fn doctor_editor_runtime_for_package_root(package_root: &Path) -> EditorRuntimeDoctorReport {
    let paths = vec![
        ("skill_entry", package_root.join("guides/claude-skills/SKILL.md")),
        ("editor_adapter", package_root.join("scripts/mcp/mei-editor-stdio-adapter.mjs")),
        ("stock_components", package_root.join("stock/components")),
        ("stock_templates", package_root.join("stock/templates")),
        (
            "editor_runtime_docs",
            package_root.join("knowledge/editor-runtime/authoring-overview.md"),
        ),
        (
            "standalone_example",
            package_root.join("knowledge/editor-runtime/minimal-app-main.mei"),
        ),
    ];
    let checks = paths
        .into_iter()
        .map(|(id, path)| EditorRuntimeCheck {
            id: id.to_string(),
            ok: path.exists(),
            path: path.display().to_string(),
            message: if path.exists() {
                "ok".to_string()
            } else {
                "missing runtime asset".to_string()
            },
        })
        .collect::<Vec<_>>();
    let ok = checks.iter().all(|check| check.ok);
    EditorRuntimeDoctorReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        ok,
        package_root: package_root.display().to_string(),
        checks,
    }
}

fn write_file(path: &Path, content: &str, force: bool) -> Result<EditorRuntimeScaffoldFile> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create scaffold dir {}", parent.display()))?;
    }
    let existed = path.exists();
    if existed && !force {
        return Ok(EditorRuntimeScaffoldFile {
            rel_path: path.to_string_lossy().to_string(),
            overwritten: false,
        });
    }
    fs::write(path, content).with_context(|| format!("write scaffold file {}", path.display()))?;
    Ok(EditorRuntimeScaffoldFile {
        rel_path: path.to_string_lossy().to_string(),
        overwritten: existed,
    })
}

fn render_common_runtime_json(package_root: &Path) -> Result<String> {
    let descriptor = editor_runtime_descriptor_for_package_root(package_root);
    serde_json::to_string_pretty(&descriptor).map_err(Into::into)
}

fn render_mcp_json(package_root: &Path) -> Result<String> {
    let adapter = package_root.join("scripts/mcp/mei-editor-stdio-adapter.mjs");
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "meilang-editor": {
                "command": "node",
                "args": [adapter.display().to_string()],
                "env": {
                    "MEI_TOOLCHAIN_BIN": "mei-toolchain",
                    "MEI_HOST_WEB_BIN": "mei-host-web"
                }
            }
        }
    }))
    .map_err(Into::into)
}

fn render_cursor_rule() -> String {
    r#"---
description: MeiLang authoring rule for standalone editor runtime workspaces.
globs: ["**/*.mei", ".mei/**"]
alwaysApply: false
---

- Treat `mei-toolchain`, `mei-lsp`, and the local `.mei/editor-runtime.json` as the canonical runtime entrypoints.
- Prefer `mei-toolchain knowledge export --surface editor --include-content --json` when you need bundled authoring docs or examples.
- Use `mei-toolchain check --app <app> --source-root <workspace>` for compile diagnostics.
- Use `mei-lsp` for symbol, hover, completion, definition, and in-editor diagnostics.
"#
    .to_string()
}

fn render_vscode_settings() -> Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "files.associations": {
            "*.mei": "python"
        },
        "editor.quickSuggestions": {
            "strings": true
        }
    }))
    .map_err(Into::into)
}

fn render_vscode_tasks() -> Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "version": "2.0.0",
        "tasks": [
            {
                "label": "mei: check",
                "type": "shell",
                "command": "mei-toolchain",
                "args": ["check", "--app", "${input:meiApp}", "--source-root", "${workspaceFolder}", "--json"]
            },
            {
                "label": "mei: doctor",
                "type": "shell",
                "command": "mei-toolchain",
                "args": ["editor-runtime", "doctor", "--json"]
            }
        ],
        "inputs": [
            {
                "id": "meiApp",
                "type": "promptString",
                "description": "MeiLang app id"
            }
        ]
    }))
    .map_err(Into::into)
}

fn render_tool_readme(tool: &str) -> String {
    format!(
        "# MeiLang {tool} integration\n\n\
Use the local `.mei/editor-runtime.json` as the runtime descriptor.\n\n\
Recommended commands:\n\n\
- `mei-toolchain editor-runtime doctor --json`\n\
- `mei-toolchain knowledge export --surface editor --include-content --json`\n\
- `mei-toolchain check --app <app> --source-root <workspace> --json`\n\
- `mei-toolchain mcp describe --surface editor --json`\n"
    )
}

pub fn scaffold_editor_runtime_tooling(
    target_root: &Path,
    package_root: &Path,
    tools: &[String],
    force: bool,
) -> Result<EditorRuntimeScaffoldReport> {
    let tools = if tools.is_empty() {
        vec!["cursor".to_string()]
    } else {
        tools.iter()
            .map(|tool| tool.trim().to_ascii_lowercase())
            .filter(|tool| !tool.is_empty())
            .collect::<Vec<_>>()
    };
    fs::create_dir_all(target_root)
        .with_context(|| format!("create target root {}", target_root.display()))?;
    let mut files = Vec::new();
    files.push(write_file(
        &target_root.join(".mei/editor-runtime.json"),
        &render_common_runtime_json(package_root)?,
        force,
    )?);
    files.push(write_file(
        &target_root.join(".mei/knowledge/editor-runtime.json"),
        &serde_json::to_string_pretty(&crate::export_knowledge_bundle_for_package_root(
            package_root,
            "editor",
            None,
            false,
        )?)?,
        force,
    )?);
    for tool in &tools {
        match tool.as_str() {
            "cursor" => {
                files.push(write_file(
                    &target_root.join(".cursor/rules/meilang-authoring.mdc"),
                    &render_cursor_rule(),
                    force,
                )?);
                files.push(write_file(
                    &target_root.join(".cursor/mcp.json"),
                    &render_mcp_json(package_root)?,
                    force,
                )?);
            }
            "vscode" => {
                files.push(write_file(
                    &target_root.join(".vscode/settings.json"),
                    &render_vscode_settings()?,
                    force,
                )?);
                files.push(write_file(
                    &target_root.join(".vscode/tasks.json"),
                    &render_vscode_tasks()?,
                    force,
                )?);
                files.push(write_file(
                    &target_root.join(".mei/tooling/vscode/README.md"),
                    &render_tool_readme("VS Code"),
                    force,
                )?);
            }
            "trae" => {
                files.push(write_file(
                    &target_root.join(".trae/rules/meilang-authoring.md"),
                    &render_tool_readme("Trae"),
                    force,
                )?);
                files.push(write_file(
                    &target_root.join(".trae/mcp.json"),
                    &render_mcp_json(package_root)?,
                    force,
                )?);
            }
            "codex" => {
                files.push(write_file(
                    &target_root.join(".mei/tooling/codex/README.md"),
                    &render_tool_readme("Codex"),
                    force,
                )?);
                files.push(write_file(
                    &target_root.join(".mei/tooling/codex/mcp.json"),
                    &render_mcp_json(package_root)?,
                    force,
                )?);
            }
            "claude-code" => {
                files.push(write_file(
                    &target_root.join(".mei/tooling/claude-code/README.md"),
                    &render_tool_readme("Claude Code"),
                    force,
                )?);
                files.push(write_file(
                    &target_root.join(".mei/tooling/claude-code/mcp.json"),
                    &render_mcp_json(package_root)?,
                    force,
                )?);
            }
            "opencode" => {
                files.push(write_file(
                    &target_root.join(".mei/tooling/opencode/README.md"),
                    &render_tool_readme("OpenCode"),
                    force,
                )?);
                files.push(write_file(
                    &target_root.join(".mei/tooling/opencode/mcp.json"),
                    &render_mcp_json(package_root)?,
                    force,
                )?);
            }
            other => {
                anyhow::bail!("unsupported scaffold tool `{other}`");
            }
        }
    }
    let files = files
        .into_iter()
        .map(|mut item| {
            if let Ok(rel) = PathBuf::from(&item.rel_path).strip_prefix(target_root) {
                item.rel_path = rel.to_string_lossy().replace('\\', "/");
            }
            item
        })
        .collect::<Vec<_>>();
    Ok(EditorRuntimeScaffoldReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        target_root: target_root.display().to_string(),
        tools,
        files,
    })
}
