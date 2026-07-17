use super::prelude::*;
use super::*;

fn render_mcp_json(target_root: &Path) -> Result<String> {
    let author_adapter = workspace_runtime_bin_dir(target_root).join("author-mcp-adapter");
    let access_adapter = workspace_runtime_bin_dir(target_root).join("access-mcp-adapter");
    let toolchain_bin =
        workspace_runtime_bin_dir(target_root).join(binary_file_name("mei-toolchain"));
    let host_web_bin =
        workspace_runtime_bin_dir(target_root).join(binary_file_name("mei-host-web"));
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "meilang-author": {
                "command": "node",
                "args": [author_adapter.display().to_string()],
                "env": {
                    "MEI_TOOLCHAIN_BIN": toolchain_bin.display().to_string(),
                    "MEI_HOST_WEB_BIN": host_web_bin.display().to_string(),
                    "MEI_SOURCE_ROOT": target_root.display().to_string()
                }
            },
            "meilang-access": {
                "command": "node",
                "args": [access_adapter.display().to_string()],
                "env": {
                    "MEI_TOOLCHAIN_BIN": toolchain_bin.display().to_string(),
                    "MEI_HOST_WEB_BIN": host_web_bin.display().to_string(),
                    "MEI_SOURCE_ROOT": target_root.display().to_string()
                }
            }
        }
    }))
    .map_err(Into::into)
}

fn render_cursor_rule() -> String {
    r#"---
description: MeiLang authoring rule for source workspaces with local runtime.
globs: ["**/*.mei", ".mei/**", "**/app.toml"]
alwaysApply: false
---

- Treat `workspace runtime status/install/update`, `mei-toolchain`, `mei-lsp`, and the local `.mei/editor-runtime.json` as the canonical workspace-local environment entrypoints.
- Treat checked-in workspace files as the source-of-truth layer, and treat `.mei/` as the installed local runtime layer.
- Prefer `workspace bootstrap` when creating a brand new workspace; prefer `workspace runtime install` or `workspace runtime update` when the source workspace already exists.
- Prefer the workspace-local `.mei/runtime/bin/mei-toolchain`, `.mei/runtime/bin/mei-lsp`, and `./start.sh` over sibling source checkouts or global PATH assumptions.
- Install the MeiLang VS Code/Cursor extension (`mei-lang.mei-lang`) for language id `mei`, highlighting, and LSP. Do not long-term remap `*.mei` via `files.associations` to python/starlark.
- For `app.toml`, install Even Better TOML (`tamasfe.even-better-toml`); MeiLang contributes the app.toml JSON Schema.
- Prefer `mei-toolchain knowledge --surface author --include-content --json --source-root <workspace>` when you need bundled authoring docs, profile guidance, or examples.
- Use `mei-toolchain knowledge --surface access --include-content --json --source-root <workspace>` for world-first access guidance and query-state-aware runtime questions.
- Use `mei-toolchain check --app <app> --source-root <workspace>` for compile diagnostics.
- Use `mei-lsp` for symbol, hover, completion, definition, and in-editor diagnostics.
"#
    .to_string()
}

fn render_vscode_settings() -> Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "editor.quickSuggestions": {
            "strings": true
        }
    }))
    .map_err(Into::into)
}

fn render_vscode_extensions() -> Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "recommendations": [
            "mei-lang.mei-lang",
            "tamasfe.even-better-toml"
        ]
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
Use the local `.mei/editor-runtime.json` as the runtime descriptor. Treat the checked-in workspace files as the source-of-truth layer, and `.mei/` as locally installed runtime output.\n\n\
Prefer the MeiLang editor extension (`mei-lang.mei-lang`) for `.mei` language id, highlighting, and `mei-lsp`. Do not long-term remap `*.mei` with `files.associations` to python/starlark. For `app.toml`, install Even Better TOML (`tamasfe.even-better-toml`).\n\n\
Recommended commands:\n\n\
- `mei-toolchain workspace bootstrap --source-root <workspace> [--app <app>] --tool <tool> --json`\n\
- `mei-toolchain workspace runtime install --source-root <workspace> --force --json`\n\
- `mei-toolchain workspace runtime update --source-root <workspace> --json`\n\
- `mei-toolchain workspace runtime status --source-root <workspace> --json`\n\
- `mei-toolchain editor-runtime doctor --source-root <workspace> --json`\n\
- `mei-toolchain knowledge --surface author --source-root <workspace> --include-content --json`\n\
- `mei-toolchain knowledge --surface access --source-root <workspace> --include-content --json`\n\
- `mei-toolchain knowledge --surface author --source-root <workspace> --topic author_profile --include-content --json`\n\
- `mei-toolchain mcp describe --surface access --json`\n\
- `mei-toolchain check --app <app> --source-root <workspace> --json`\n\
- `mei-toolchain mcp describe --surface author --json`\n"
    )
}

pub fn scaffold_editor_runtime_tooling(
    target_root: &Path,
    _package_root: &Path,
    tools: &[String],
    force: bool,
) -> Result<EditorRuntimeScaffoldReport> {
    let tools = if tools.is_empty() {
        vec!["cursor".to_string()]
    } else {
        tools
            .iter()
            .map(|tool| tool.trim().to_ascii_lowercase())
            .filter(|tool| !tool.is_empty())
            .collect::<Vec<_>>()
    };
    fs::create_dir_all(target_root)
        .with_context(|| format!("create target root {}", target_root.display()))?;
    let mut files = Vec::new();
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
                    &render_mcp_json(target_root)?,
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
                    &target_root.join(".vscode/extensions.json"),
                    &render_vscode_extensions()?,
                    force,
                )?);
                files.push(write_file(
                    &target_root.join(".vscode/tasks.json"),
                    &render_vscode_tasks()?,
                    force,
                )?);
                files.push(write_file(
                    &target_root.join("runtime/platform/tooling/vscode/README.md"),
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
                    &render_mcp_json(target_root)?,
                    force,
                )?);
            }
            "codex" => {
                files.push(write_file(
                    &target_root.join("runtime/platform/tooling/codex/README.md"),
                    &render_tool_readme("Codex"),
                    force,
                )?);
                files.push(write_file(
                    &target_root.join("runtime/platform/tooling/codex/mcp.json"),
                    &render_mcp_json(target_root)?,
                    force,
                )?);
            }
            "claude-code" => {
                files.push(write_file(
                    &target_root.join("runtime/platform/tooling/claude-code/README.md"),
                    &render_tool_readme("Claude Code"),
                    force,
                )?);
                files.push(write_file(
                    &target_root.join("runtime/platform/tooling/claude-code/mcp.json"),
                    &render_mcp_json(target_root)?,
                    force,
                )?);
            }
            "opencode" => {
                files.push(write_file(
                    &target_root.join("runtime/platform/tooling/opencode/README.md"),
                    &render_tool_readme("OpenCode"),
                    force,
                )?);
                files.push(write_file(
                    &target_root.join("runtime/platform/tooling/opencode/mcp.json"),
                    &render_mcp_json(target_root)?,
                    force,
                )?);
            }
            other => {
                anyhow::bail!("unsupported scaffold tool `{other}`");
            }
        }
    }
    let files = normalize_scaffold_files(target_root, files);
    Ok(EditorRuntimeScaffoldReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        target_root: target_root.display().to_string(),
        tools,
        files,
    })
}
