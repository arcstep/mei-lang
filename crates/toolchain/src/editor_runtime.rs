use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::capability_catalog::CAPABILITY_CATALOG_SCHEMA_VERSION;
use crate::knowledge_bundle_descriptor_for_package_root;

pub const EDITOR_RUNTIME_SCHEMA_VERSION: &str = "mei-editor-runtime-v1";
pub const WORKSPACE_RUNTIME_VERSION_SCHEMA_VERSION: &str = "mei-runtime-version-v1";
pub const WORKSPACE_RUNTIME_MANIFEST_SCHEMA_VERSION: &str = "mei-runtime-manifest-v1";
pub const RUNTIME_BUNDLE_SCHEMA_VERSION: &str = "mei-runtime-bundle-v1";

const TOOLCHAIN_VERSION: &str = env!("MEI_CARGO_PACKAGE_VERSION");
const GIT_COMMIT_SHORT: &str = env!("MEI_GIT_COMMIT_SHORT");
const GIT_COMMIT_FULL: &str = env!("MEI_GIT_COMMIT_FULL");
const GIT_DIRTY: &str = env!("MEI_GIT_DIRTY");
const BUILD_VERSION: &str = env!("MEI_BUILD_VERSION");
const BUILD_TIMESTAMP_UTC: &str = env!("MEI_BUILD_TIMESTAMP_UTC");
const TARGET_TRIPLE: &str = env!("MEI_TARGET_TRIPLE");
const COMPATIBILITY_LINE: &str = env!("MEI_COMPATIBILITY_LINE");

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
    pub workspace_root: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
pub struct EditorRuntimeInstallReport {
    pub schema_version: String,
    pub target_root: String,
    pub files: Vec<EditorRuntimeScaffoldFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSourceRevision {
    pub git_commit: String,
    pub git_commit_short: String,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeCompatibilityDescriptor {
    pub line: String,
    pub bundle_schema: String,
    pub catalog_schema: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledRuntimeDescriptor {
    pub runtime_id: String,
    pub target_triple: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceRuntimeVersionDescriptor {
    pub schema_version: String,
    pub toolchain_version: String,
    pub source_revision: RuntimeSourceRevision,
    pub compatibility: RuntimeCompatibilityDescriptor,
    pub installed_runtime: InstalledRuntimeDescriptor,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeManifestArtifactDescriptor {
    pub mei_toolchain: String,
    pub mei_lsp: String,
    pub compat_bin: String,
    pub editor_mcp_adapter: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeManifestContentDescriptor {
    pub capability_catalog: String,
    pub editor_surface: String,
    pub knowledge_path: String,
    pub platform_assets_path: String,
    pub tooling_templates_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeManifestProvenance {
    pub built_at: String,
    pub built_from: String,
    pub package_root: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceRuntimeManifest {
    pub schema_version: String,
    pub bundle_id: String,
    pub toolchain_version: String,
    pub source_revision: RuntimeSourceRevision,
    pub compatibility_line: String,
    pub bundle_schema: String,
    pub target_triple: String,
    pub artifacts: RuntimeManifestArtifactDescriptor,
    pub content: RuntimeManifestContentDescriptor,
    pub provenance: RuntimeManifestProvenance,
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
            id: "access_mcp_adapter".to_string(),
            rel_path: "scripts/mcp/mei-access-stdio-adapter.mjs".to_string(),
            purpose: "Current source-backed access MCP adapter.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "author_skill".to_string(),
            rel_path: "guides/claude-skills/SKILL.md".to_string(),
            purpose: "Current authoring skill entrypoint in the source tree.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "author_profile".to_string(),
            rel_path: "guides/author-profile.md".to_string(),
            purpose: "Canonical author profile guidance shipped with the package.".to_string(),
        },
        EditorRuntimePathDescriptor {
            id: "access_profile".to_string(),
            rel_path: "guides/access-profile.md".to_string(),
            purpose: "Canonical access profile guidance shipped with the package.".to_string(),
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

fn runtime_source_revision() -> RuntimeSourceRevision {
    RuntimeSourceRevision {
        git_commit: GIT_COMMIT_FULL.to_string(),
        git_commit_short: GIT_COMMIT_SHORT.to_string(),
        dirty: GIT_DIRTY == "true",
    }
}

fn runtime_bundle_id() -> String {
    format!("mei-lang-{BUILD_VERSION}")
}

fn now_timestamp_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn workspace_runtime_version_descriptor() -> WorkspaceRuntimeVersionDescriptor {
    WorkspaceRuntimeVersionDescriptor {
        schema_version: WORKSPACE_RUNTIME_VERSION_SCHEMA_VERSION.to_string(),
        toolchain_version: TOOLCHAIN_VERSION.to_string(),
        source_revision: runtime_source_revision(),
        compatibility: RuntimeCompatibilityDescriptor {
            line: COMPATIBILITY_LINE.to_string(),
            bundle_schema: RUNTIME_BUNDLE_SCHEMA_VERSION.to_string(),
            catalog_schema: CAPABILITY_CATALOG_SCHEMA_VERSION.to_string(),
        },
        installed_runtime: InstalledRuntimeDescriptor {
            runtime_id: runtime_bundle_id(),
            target_triple: TARGET_TRIPLE.to_string(),
        },
        generated_at: now_timestamp_utc(),
    }
}

pub fn workspace_runtime_manifest_for_package_root(package_root: &Path) -> WorkspaceRuntimeManifest {
    WorkspaceRuntimeManifest {
        schema_version: WORKSPACE_RUNTIME_MANIFEST_SCHEMA_VERSION.to_string(),
        bundle_id: runtime_bundle_id(),
        toolchain_version: TOOLCHAIN_VERSION.to_string(),
        source_revision: runtime_source_revision(),
        compatibility_line: COMPATIBILITY_LINE.to_string(),
        bundle_schema: RUNTIME_BUNDLE_SCHEMA_VERSION.to_string(),
        target_triple: TARGET_TRIPLE.to_string(),
        artifacts: RuntimeManifestArtifactDescriptor {
            mei_toolchain: "bin/mei-toolchain".to_string(),
            mei_lsp: "bin/mei-lsp".to_string(),
            compat_bin: "bin/mei".to_string(),
            editor_mcp_adapter: "bin/editor-mcp-adapter".to_string(),
        },
        content: RuntimeManifestContentDescriptor {
            capability_catalog: "share/mei/catalog/capability-catalog.json".to_string(),
            editor_surface: "share/mei/catalog/editor-surface.json".to_string(),
            knowledge_path: "share/mei/knowledge/editor".to_string(),
            platform_assets_path: "share/mei/platform-assets/stock".to_string(),
            tooling_templates_path: "share/mei/tooling-templates".to_string(),
        },
        provenance: RuntimeManifestProvenance {
            built_at: BUILD_TIMESTAMP_UTC.to_string(),
            built_from: "source-tree-bootstrap".to_string(),
            package_root: package_root.display().to_string(),
        },
    }
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
            "Run `mei-toolchain knowledge --surface editor --include-content --json` to export packaged authoring docs/examples.".to_string(),
            "Use `mei-lsp` for IDE semantics and `node scripts/mcp/mei-editor-stdio-adapter.mjs` for agent-side tools.".to_string(),
        ],
        tooling_templates: tooling_templates(),
        editor_knowledge_bundle,
    }
}

pub fn doctor_editor_runtime_for_package_root(package_root: &Path) -> EditorRuntimeDoctorReport {
    let EditorRuntimeDescriptor {
        current_source_layout,
        editor_knowledge_bundle,
        ..
    } = editor_runtime_descriptor_for_package_root(package_root);
    let mut checks = current_source_layout
        .into_iter()
        .map(|item| {
            let path = package_root.join(&item.rel_path);
            EditorRuntimeCheck {
                id: item.id,
                ok: path.exists(),
                path: path.display().to_string(),
                message: if path.exists() {
                    format!("source-backed runtime asset present: {}", item.purpose)
                } else {
                    format!("missing source-backed runtime asset: {}", item.purpose)
                },
            }
        })
        .collect::<Vec<_>>();
    checks.extend(
        editor_knowledge_bundle
            .assets
            .into_iter()
            .map(|asset| {
                let path = package_root.join(&asset.relative_path);
                EditorRuntimeCheck {
                    id: format!("knowledge_asset:{}", asset.id),
                    ok: path.exists(),
                    path: path.display().to_string(),
                    message: if path.exists() {
                        format!("packaged knowledge asset present for topic `{}`", asset.topic)
                    } else {
                        format!("missing packaged knowledge asset for topic `{}`", asset.topic)
                    },
                }
            }),
    );
    let ok = checks.iter().all(|check| check.ok);
    EditorRuntimeDoctorReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        ok,
        package_root: package_root.display().to_string(),
        workspace_root: None,
        checks,
    }
}

fn json_value_matches(
    path: &Path,
    id: &str,
    message_prefix: &str,
    predicate: impl Fn(&Value) -> bool,
) -> EditorRuntimeCheck {
    match fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Value>(&contents) {
            Ok(value) => {
                let ok = predicate(&value);
                EditorRuntimeCheck {
                    id: id.to_string(),
                    ok,
                    path: path.display().to_string(),
                    message: if ok {
                        format!("{message_prefix}: metadata matches expected toolchain identity")
                    } else {
                        format!("{message_prefix}: metadata does not match expected toolchain identity")
                    },
                }
            }
            Err(error) => EditorRuntimeCheck {
                id: id.to_string(),
                ok: false,
                path: path.display().to_string(),
                message: format!("{message_prefix}: failed to parse json ({error})"),
            },
        },
        Err(error) => EditorRuntimeCheck {
            id: id.to_string(),
            ok: false,
            path: path.display().to_string(),
            message: format!("{message_prefix}: failed to read file ({error})"),
        },
    }
}

pub fn doctor_editor_runtime_for_workspace_root(
    package_root: &Path,
    workspace_root: &Path,
) -> EditorRuntimeDoctorReport {
    let mut report = doctor_editor_runtime_for_package_root(package_root);
    let version_path = workspace_root.join(".mei/version.json");
    let manifest_path = workspace_root.join(".mei/runtime/MANIFEST.json");
    let editor_runtime_path = workspace_root.join(".mei/editor-runtime.json");
    let knowledge_path = workspace_root.join(".mei/knowledge/editor-runtime.json");
    let expected_version = workspace_runtime_version_descriptor();
    let expected_manifest = workspace_runtime_manifest_for_package_root(package_root);
    report.workspace_root = Some(workspace_root.display().to_string());
    report.checks.extend([
        EditorRuntimeCheck {
            id: "workspace_editor_runtime_descriptor".to_string(),
            ok: editor_runtime_path.is_file(),
            path: editor_runtime_path.display().to_string(),
            message: if editor_runtime_path.is_file() {
                "workspace runtime descriptor present".to_string()
            } else {
                "missing workspace runtime descriptor".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_editor_knowledge_bundle".to_string(),
            ok: knowledge_path.is_file(),
            path: knowledge_path.display().to_string(),
            message: if knowledge_path.is_file() {
                "workspace editor knowledge bundle present".to_string()
            } else {
                "missing workspace editor knowledge bundle".to_string()
            },
        },
        json_value_matches(
            &version_path,
            "workspace_version_descriptor",
            "workspace runtime version descriptor",
            |value| {
                value["schema_version"] == expected_version.schema_version
                    && value["toolchain_version"] == expected_version.toolchain_version
                    && value["compatibility"]["line"] == expected_version.compatibility.line
                    && value["installed_runtime"]["runtime_id"]
                        == expected_version.installed_runtime.runtime_id
                    && value["installed_runtime"]["target_triple"]
                        == expected_version.installed_runtime.target_triple
            },
        ),
        json_value_matches(
            &manifest_path,
            "workspace_runtime_manifest",
            "workspace runtime manifest",
            |value| {
                value["schema_version"] == expected_manifest.schema_version
                    && value["bundle_id"] == expected_manifest.bundle_id
                    && value["toolchain_version"] == expected_manifest.toolchain_version
                    && value["compatibility_line"] == expected_manifest.compatibility_line
                    && value["target_triple"] == expected_manifest.target_triple
            },
        ),
    ]);
    report.ok = report.checks.iter().all(|check| check.ok);
    report
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

fn render_workspace_runtime_version_json() -> Result<String> {
    serde_json::to_string_pretty(&workspace_runtime_version_descriptor()).map_err(Into::into)
}

fn render_workspace_runtime_manifest_json(package_root: &Path) -> Result<String> {
    serde_json::to_string_pretty(&workspace_runtime_manifest_for_package_root(package_root))
        .map_err(Into::into)
}

fn normalize_scaffold_files(
    target_root: &Path,
    files: Vec<EditorRuntimeScaffoldFile>,
) -> Vec<EditorRuntimeScaffoldFile> {
    files.into_iter()
        .map(|mut item| {
            if let Ok(rel) = PathBuf::from(&item.rel_path).strip_prefix(target_root) {
                item.rel_path = rel.to_string_lossy().replace('\\', "/");
            }
            item
        })
        .collect()
}

fn write_common_runtime_files(
    target_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<Vec<EditorRuntimeScaffoldFile>> {
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
    files.push(write_file(
        &target_root.join(".mei/version.json"),
        &render_workspace_runtime_version_json()?,
        force,
    )?);
    files.push(write_file(
        &target_root.join(".mei/runtime/MANIFEST.json"),
        &render_workspace_runtime_manifest_json(package_root)?,
        force,
    )?);
    Ok(files)
}

pub fn install_editor_runtime_support_files(
    target_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<EditorRuntimeInstallReport> {
    fs::create_dir_all(target_root)
        .with_context(|| format!("create target root {}", target_root.display()))?;
    let files = normalize_scaffold_files(target_root, write_common_runtime_files(target_root, package_root, force)?);
    Ok(EditorRuntimeInstallReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        target_root: target_root.display().to_string(),
        files,
    })
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
- Prefer `mei-toolchain knowledge --surface editor --include-content --json` when you need bundled authoring docs, profile guidance, or examples.
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
- `mei-toolchain knowledge --surface editor --include-content --json`\n\
- `mei-toolchain knowledge --surface editor --topic author_profile --include-content --json`\n\
- `mei-toolchain check --app <app> --source-root <workspace> --json`\n\
- `mei-toolchain mcp describe --surface author --json`\n"
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
    let mut files = write_common_runtime_files(target_root, package_root, force)?;
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
    let files = normalize_scaffold_files(target_root, files);
    Ok(EditorRuntimeScaffoldReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        target_root: target_root.display().to_string(),
        tools,
        files,
    })
}
