use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::capability_catalog::CAPABILITY_CATALOG_SCHEMA_VERSION;
use crate::{knowledge_bundle::package_root_hint, knowledge_bundle_descriptor_for_package_root};
use mei_lang_kernel::{
    apply_toolchain_store_symlinks, build_runtime_warmup_manifest, record_toolchain_install_links,
    resolve_toolchain_root, resolve_workspace_runtime_root, toolchain_store_dir,
    RuntimeWarmupManifest, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};

pub const EDITOR_RUNTIME_SCHEMA_VERSION: &str = "mei-editor-runtime-v1";
pub const WORKSPACE_RUNTIME_VERSION_SCHEMA_VERSION: &str = "mei-runtime-version-v1";
pub const WORKSPACE_RUNTIME_MANIFEST_SCHEMA_VERSION: &str = "mei-runtime-manifest-v1";
#[allow(unused_imports)]
pub use mei_lang_kernel::WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION;
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
pub struct EnsureAuthorSkillReport {
    pub installed: bool,
    pub installed_now: bool,
    pub install_dir: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceRuntimeStatusReport {
    pub schema_version: String,
    pub source_root: String,
    pub runtime_root: String,
    pub package_root: String,
    pub installed: bool,
    pub fallback_to_source_tree: bool,
    pub version_path: String,
    pub manifest_path: String,
    pub catalog_path: String,
    pub author_skill_dir: String,
    pub author_profile_path: String,
    pub doctor: EditorRuntimeDoctorReport,
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
    pub mei_host_web: String,
    pub author_mcp_adapter: String,
    pub access_mcp_adapter: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeManifestContentDescriptor {
    pub capability_catalog: String,
    pub author_surface: String,
    pub access_surface: String,
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

fn workspace_platform_dir(workspace_root: &Path) -> PathBuf {
    resolve_workspace_runtime_root(workspace_root).join("platform")
}

fn workspace_runtime_bin_dir(workspace_root: &Path) -> PathBuf {
    resolve_toolchain_root(workspace_root).join("bin")
}

fn workspace_store_bin_dir(workspace_root: &Path, toolchain_version: &str) -> PathBuf {
    toolchain_store_dir(workspace_root, toolchain_version).join("bin")
}

fn workspace_catalog_dir(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("catalog")
}

fn workspace_profiles_dir(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("profiles")
}

fn workspace_knowledge_dir(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("knowledge")
}

fn workspace_author_skill_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(crate::meilang_author_skill_package().install_dir_rel)
}

fn workspace_access_skill_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(crate::meilang_access_skill_package().install_dir_rel)
}

fn declared_layout() -> Vec<EditorRuntimePathDescriptor> {
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

fn current_source_layout() -> Vec<EditorRuntimePathDescriptor> {
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
            rel_path: "guides/author-skills/SKILL.md".to_string(),
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
            id: "access_skill".to_string(),
            rel_path: "guides/access-skills/SKILL.md".to_string(),
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

fn binary_file_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn current_exe_candidates(base: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        let file_name = binary_file_name(base);
        let current_name = current_exe
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if current_name == file_name {
            candidates.push(current_exe.clone());
        }
        if let Some(bin_dir) = current_exe.parent() {
            candidates.push(bin_dir.join(&file_name));
            if bin_dir.file_name().and_then(|value| value.to_str()) == Some("deps") {
                if let Some(parent) = bin_dir.parent() {
                    candidates.push(parent.join(&file_name));
                }
            }
        }
    }
    candidates
}

fn package_root_binary_candidates(package_root: &Path, base: &str) -> Vec<PathBuf> {
    let file_name = binary_file_name(base);
    let mut candidates = Vec::new();
    if package_root.ends_with(Path::new("share/mei")) {
        if let Some(prefix) = package_root.parent().and_then(|path| path.parent()) {
            candidates.push(prefix.join("bin").join(&file_name));
        }
    }
    candidates.push(package_root.join("target/debug").join(&file_name));
    candidates.push(package_root.join("target/release").join(&file_name));
    candidates
}

fn try_resolve_runtime_binary(package_root: &Path, env_key: &str, base: &str) -> Option<PathBuf> {
    if let Ok(raw) = std::env::var(env_key) {
        let candidate = PathBuf::from(raw);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    current_exe_candidates(base)
        .into_iter()
        .chain(package_root_binary_candidates(package_root, base))
        .find(|candidate| candidate.is_file())
}

fn build_runtime_binary_set_for_package_root(
    package_root: &Path,
    target_root: &Path,
) -> Result<Vec<(&'static str, PathBuf, PathBuf)>> {
    let version = TOOLCHAIN_VERSION;
    let store_bin = workspace_store_bin_dir(target_root, version);
    let mut binaries = vec![
        (
            "mei-toolchain",
            PathBuf::new(),
            store_bin.join(binary_file_name("mei-toolchain")),
        ),
        (
            "mei-lsp",
            PathBuf::new(),
            store_bin.join(binary_file_name("mei-lsp")),
        ),
        (
            "mei-host-web",
            PathBuf::new(),
            store_bin.join(binary_file_name("mei-host-web")),
        ),
    ];
    let env_keys = [
        ("MEI_TOOLCHAIN_BIN", "mei-toolchain"),
        ("MEI_LSP_BIN", "mei-lsp"),
        ("MEI_HOST_WEB_BIN", "mei-host-web"),
    ];
    let missing = env_keys
        .iter()
        .filter_map(|(env_key, base)| {
            try_resolve_runtime_binary(package_root, env_key, base)
                .map(|path| ((*base).to_string(), path))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if missing.len() != env_keys.len() && package_root.join("Cargo.toml").is_file() {
        let status = Command::new("cargo")
            .arg("build")
            .arg("-p")
            .arg("mei-lang-server")
            .arg("-p")
            .arg("mei-lang-lsp")
            .arg("--bin")
            .arg("mei-toolchain")
            .arg("--bin")
            .arg("mei-host-web")
            .arg("--bin")
            .arg("mei-lsp")
            .current_dir(package_root)
            .status()
            .with_context(|| format!("build runtime binaries under {}", package_root.display()))?;
        if !status.success() {
            anyhow::bail!(
                "failed to build workspace-local runtime binaries from {}",
                package_root.display()
            );
        }
    }
    for (name, source, destination) in &mut binaries {
        let env_key = match *name {
            "mei-toolchain" => "MEI_TOOLCHAIN_BIN",
            "mei-lsp" => "MEI_LSP_BIN",
            "mei-host-web" => "MEI_HOST_WEB_BIN",
            _ => unreachable!(),
        };
        *source = try_resolve_runtime_binary(package_root, env_key, name).ok_or_else(|| {
            anyhow::anyhow!(
                "cannot locate required runtime binary `{}`; checked current executable siblings, {} and {}",
                name,
                package_root.join("target/debug").display(),
                package_root.join("target/release").display()
            )
        })?;
        *destination = store_bin.join(binary_file_name(name));
    }
    Ok(binaries)
}

fn finalize_toolchain_store_layout(target_root: &Path) -> Result<()> {
    apply_toolchain_store_symlinks(target_root, TOOLCHAIN_VERSION)?;
    record_toolchain_install_links(target_root, TOOLCHAIN_VERSION)?;
    Ok(())
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

pub fn workspace_runtime_manifest_for_package_root(
    package_root: &Path,
) -> WorkspaceRuntimeManifest {
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
            mei_host_web: "bin/mei-host-web".to_string(),
            author_mcp_adapter: "bin/author-mcp-adapter".to_string(),
            access_mcp_adapter: "bin/access-mcp-adapter".to_string(),
        },
        content: RuntimeManifestContentDescriptor {
            capability_catalog: "share/mei/catalog/capability-catalog.json".to_string(),
            author_surface: "share/mei/catalog/author-surface.json".to_string(),
            access_surface: "share/mei/catalog/access-surface.json".to_string(),
            knowledge_path: "share/mei/knowledge/author".to_string(),
            platform_assets_path: "share/mei/platform-assets/stock".to_string(),
            tooling_templates_path: "share/mei/tooling-templates".to_string(),
        },
        provenance: RuntimeManifestProvenance {
            built_at: BUILD_TIMESTAMP_UTC.to_string(),
            built_from: "source-tree-bootstrap".to_string(),
            package_root: package_root_hint(package_root),
        },
    }
}

pub fn editor_runtime_descriptor_for_package_root(package_root: &Path) -> EditorRuntimeDescriptor {
    let editor_knowledge_bundle =
        knowledge_bundle_descriptor_for_package_root(package_root, "author")
            .expect("author bundle");
    EditorRuntimeDescriptor {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        package_root: package_root_hint(package_root),
        declared_layout: declared_layout(),
        current_source_layout: current_source_layout(),
        package_root_resolution: vec![
            "Prefer MEI_PACKAGE_ROOT when explicitly provided.".to_string(),
            "Otherwise infer from the current executable and prefer a sibling share/mei layout when present.".to_string(),
            "Fallback to source-tree package root for local development builds.".to_string(),
        ],
        standalone_flow: vec![
            "Run `mei-toolchain workspace init --standalone --source-root <dir>` to create a source workspace skeleton.".to_string(),
            "Run `mei-toolchain workspace bootstrap --source-root <dir>` to create a workspace (stock is copied automatically).".to_string(),
            "Run `mei-toolchain workspace runtime install --source-root <dir>` to install workspace-local .mei runtime assets and `./start.sh`.".to_string(),
            "Run `./start.sh` from the workspace root to launch the MeiLang host.".to_string(),
            "Run `mei-toolchain editor-runtime scaffold --target-root <dir> --tool <tool>` to write tool glue files only.".to_string(),
            "Run `mei-toolchain knowledge --surface author --include-content --json` to export packaged authoring docs/examples.".to_string(),
            "Use `mei-lsp` for IDE semantics and `node scripts/mcp/mei-author-stdio-adapter.mjs` for agent-side tools.".to_string(),
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
    checks.extend(editor_knowledge_bundle.assets.into_iter().map(|asset| {
        let path = package_root.join(&asset.relative_path);
        EditorRuntimeCheck {
            id: format!("knowledge_asset:{}", asset.id),
            ok: path.exists(),
            path: path.display().to_string(),
            message: if path.exists() {
                format!(
                    "packaged knowledge asset present for topic `{}`",
                    asset.topic
                )
            } else {
                format!(
                    "missing packaged knowledge asset for topic `{}`",
                    asset.topic
                )
            },
        }
    }));
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
                        format!(
                            "{message_prefix}: metadata does not match expected toolchain identity"
                        )
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

fn workspace_version_path(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("version.json")
}

fn workspace_manifest_path(workspace_root: &Path) -> PathBuf {
    resolve_toolchain_root(workspace_root).join("MANIFEST.json")
}

fn workspace_editor_runtime_path(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("editor-runtime.json")
}

fn workspace_author_knowledge_path(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root)
        .join("knowledge")
        .join("author-runtime.json")
}

pub fn doctor_editor_runtime_for_workspace_root(
    package_root: &Path,
    workspace_root: &Path,
) -> EditorRuntimeDoctorReport {
    let version_path = workspace_version_path(workspace_root);
    let manifest_path = workspace_manifest_path(workspace_root);
    let editor_runtime_path = workspace_editor_runtime_path(workspace_root);
    let knowledge_path = workspace_author_knowledge_path(workspace_root);
    let catalog_path = workspace_catalog_dir(workspace_root).join("capability-catalog.json");
    let author_surface_path = workspace_catalog_dir(workspace_root).join("author-surface.json");
    let access_surface_path = workspace_catalog_dir(workspace_root).join("access-surface.json");
    let author_profile_path = workspace_profiles_dir(workspace_root).join("author.md");
    let access_profile_path = workspace_profiles_dir(workspace_root).join("access.md");
    let author_skill_entry = workspace_author_skill_dir(workspace_root).join("SKILL.md");
    let access_skill_entry = workspace_access_skill_dir(workspace_root).join("SKILL.md");
    let toolchain_bin =
        workspace_runtime_bin_dir(workspace_root).join(binary_file_name("mei-toolchain"));
    let lsp_bin = workspace_runtime_bin_dir(workspace_root).join(binary_file_name("mei-lsp"));
    let host_web_bin =
        workspace_runtime_bin_dir(workspace_root).join(binary_file_name("mei-host-web"));
    let author_runtime_adapter =
        workspace_runtime_bin_dir(workspace_root).join("author-mcp-adapter");
    let access_runtime_adapter =
        workspace_runtime_bin_dir(workspace_root).join("access-mcp-adapter");
    let expected_version = workspace_runtime_version_descriptor();
    let expected_manifest = workspace_runtime_manifest_for_package_root(package_root);
    let checks = vec![
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
            id: "workspace_author_knowledge_bundle".to_string(),
            ok: knowledge_path.is_file(),
            path: knowledge_path.display().to_string(),
            message: if knowledge_path.is_file() {
                "workspace author knowledge bundle present".to_string()
            } else {
                "missing workspace author knowledge bundle".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_capability_catalog".to_string(),
            ok: catalog_path.is_file(),
            path: catalog_path.display().to_string(),
            message: if catalog_path.is_file() {
                "workspace-local capability catalog present".to_string()
            } else {
                "missing workspace-local capability catalog".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_surface".to_string(),
            ok: author_surface_path.is_file(),
            path: author_surface_path.display().to_string(),
            message: if author_surface_path.is_file() {
                "workspace-local author MCP surface descriptor present".to_string()
            } else {
                "missing workspace-local author MCP surface descriptor".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_access_surface".to_string(),
            ok: access_surface_path.is_file(),
            path: access_surface_path.display().to_string(),
            message: if access_surface_path.is_file() {
                "workspace-local access MCP surface descriptor present".to_string()
            } else {
                "missing workspace-local access MCP surface descriptor".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_profile".to_string(),
            ok: author_profile_path.is_file(),
            path: author_profile_path.display().to_string(),
            message: if author_profile_path.is_file() {
                "workspace-local author profile present".to_string()
            } else {
                "missing workspace-local author profile".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_access_profile".to_string(),
            ok: access_profile_path.is_file(),
            path: access_profile_path.display().to_string(),
            message: if access_profile_path.is_file() {
                "workspace-local access profile present".to_string()
            } else {
                "missing workspace-local access profile".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_skill".to_string(),
            ok: author_skill_entry.is_file(),
            path: author_skill_entry.display().to_string(),
            message: if author_skill_entry.is_file() {
                "workspace-local author skill package present".to_string()
            } else {
                "missing workspace-local author skill package".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_access_skill".to_string(),
            ok: access_skill_entry.is_file(),
            path: access_skill_entry.display().to_string(),
            message: if access_skill_entry.is_file() {
                "workspace-local access skill package present".to_string()
            } else {
                "missing workspace-local access skill package".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_mei_toolchain_bin".to_string(),
            ok: toolchain_bin.is_file(),
            path: toolchain_bin.display().to_string(),
            message: if toolchain_bin.is_file() {
                "workspace-local mei-toolchain binary present".to_string()
            } else {
                "missing workspace-local mei-toolchain binary".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_mei_lsp_bin".to_string(),
            ok: lsp_bin.is_file(),
            path: lsp_bin.display().to_string(),
            message: if lsp_bin.is_file() {
                "workspace-local mei-lsp binary present".to_string()
            } else {
                "missing workspace-local mei-lsp binary".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_mei_host_web_bin".to_string(),
            ok: host_web_bin.is_file(),
            path: host_web_bin.display().to_string(),
            message: if host_web_bin.is_file() {
                "workspace-local mei-host-web binary present".to_string()
            } else {
                "missing workspace-local mei-host-web binary".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_mcp_adapter".to_string(),
            ok: author_runtime_adapter.is_file(),
            path: author_runtime_adapter.display().to_string(),
            message: if author_runtime_adapter.is_file() {
                "workspace-local author MCP adapter present".to_string()
            } else {
                "missing workspace-local author MCP adapter".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_access_mcp_adapter".to_string(),
            ok: access_runtime_adapter.is_file(),
            path: access_runtime_adapter.display().to_string(),
            message: if access_runtime_adapter.is_file() {
                "workspace-local access MCP adapter present".to_string()
            } else {
                "missing workspace-local access MCP adapter".to_string()
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
                    && value["artifacts"]["mei_toolchain"]
                        == expected_manifest.artifacts.mei_toolchain
                    && value["artifacts"]["mei_lsp"] == expected_manifest.artifacts.mei_lsp
                    && value["artifacts"]["mei_host_web"]
                        == expected_manifest.artifacts.mei_host_web
            },
        ),
    ];
    let ok = checks.iter().all(|check| check.ok);
    EditorRuntimeDoctorReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        ok,
        package_root: package_root.display().to_string(),
        workspace_root: Some(workspace_root.display().to_string()),
        checks,
    }
}

pub fn workspace_runtime_status_for_workspace_root(
    package_root: &Path,
    workspace_root: &Path,
) -> WorkspaceRuntimeStatusReport {
    let doctor = doctor_editor_runtime_for_workspace_root(package_root, workspace_root);
    let version_path = workspace_version_path(workspace_root);
    let manifest_path = workspace_manifest_path(workspace_root);
    let catalog_path = workspace_catalog_dir(workspace_root).join("capability-catalog.json");
    let author_skill_dir = workspace_author_skill_dir(workspace_root);
    let access_skill_dir = workspace_access_skill_dir(workspace_root);
    let author_profile_path = workspace_profiles_dir(workspace_root).join("author.md");
    let runtime_bin_dir = workspace_runtime_bin_dir(workspace_root);
    let installed = version_path.is_file()
        && manifest_path.is_file()
        && catalog_path.is_file()
        && author_skill_dir.join("SKILL.md").is_file()
        && access_skill_dir.join("SKILL.md").is_file()
        && runtime_bin_dir
            .join(binary_file_name("mei-toolchain"))
            .is_file()
        && runtime_bin_dir.join(binary_file_name("mei-lsp")).is_file()
        && runtime_bin_dir
            .join(binary_file_name("mei-host-web"))
            .is_file()
        && runtime_bin_dir.join("author-mcp-adapter").is_file()
        && runtime_bin_dir.join("access-mcp-adapter").is_file();
    let fallback_to_source_tree = false;
    WorkspaceRuntimeStatusReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        source_root: workspace_root.display().to_string(),
        runtime_root: resolve_workspace_runtime_root(workspace_root)
            .display()
            .to_string(),
        package_root: package_root.display().to_string(),
        installed,
        fallback_to_source_tree,
        version_path: version_path.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        catalog_path: catalog_path.display().to_string(),
        author_skill_dir: author_skill_dir.display().to_string(),
        author_profile_path: author_profile_path.display().to_string(),
        doctor,
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

fn write_executable_file(
    path: &Path,
    content: &str,
    force: bool,
) -> Result<EditorRuntimeScaffoldFile> {
    let report = write_file(path, content, force)?;
    set_executable_permissions(path)?;
    Ok(report)
}

fn set_executable_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    if path.is_file() {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .with_context(|| format!("read permissions for {}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .with_context(|| format!("set executable permissions for {}", path.display()))?;
    }
    Ok(())
}

fn copy_runtime_binary(
    target_root: &Path,
    source_path: &Path,
    destination_path: &Path,
    force: bool,
) -> Result<EditorRuntimeScaffoldFile> {
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create runtime binary dir {}", parent.display()))?;
    }
    let existed = destination_path.exists();
    if existed && !force {
        return Ok(normalize_scaffold_files(
            target_root,
            vec![EditorRuntimeScaffoldFile {
                rel_path: destination_path.display().to_string(),
                overwritten: false,
            }],
        )
        .into_iter()
        .next()
        .expect("normalized runtime binary"));
    }
    fs::copy(source_path, destination_path).with_context(|| {
        format!(
            "copy runtime binary {} -> {}",
            source_path.display(),
            destination_path.display()
        )
    })?;
    set_executable_permissions(destination_path)?;
    Ok(normalize_scaffold_files(
        target_root,
        vec![EditorRuntimeScaffoldFile {
            rel_path: destination_path.display().to_string(),
            overwritten: existed,
        }],
    )
    .into_iter()
    .next()
    .expect("normalized runtime binary"))
}

fn render_workspace_start_script() -> &'static str {
    r#"#!/usr/bin/env bash
# MeiLang workspace host launcher (generated by `mei-toolchain workspace runtime install`).
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
HOST="${MEI_HOST:-127.0.0.1}"
PORT="${MEI_PORT:-9527}"
TOOLCHAIN_MODE="${MEI_TOOLCHAIN_MODE:-cargo}"
MEI_LANG_ROOT="${MEI_LANG_ROOT:-${WORKSPACE_ROOT}/../../mei-lang}"
INSTALLED_HOST_BIN="${WORKSPACE_ROOT}/toolchain/bin/mei-host-web"
LINKS_JSON="${WORKSPACE_ROOT}/deploy/state/links.json"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --toolchain-mode)
      TOOLCHAIN_MODE="$2"
      shift 2
      ;;
    --toolchain-mode=*)
      TOOLCHAIN_MODE="${1#*=}"
      shift
      ;;
    --port)
      PORT="$2"
      shift 2
      ;;
    --port=*)
      PORT="${1#*=}"
      shift
      ;;
    --host)
      HOST="$2"
      shift 2
      ;;
    --host=*)
      HOST="${1#*=}"
      shift
      ;;
    *)
      break
      ;;
  esac
done

URL="http://${HOST}:${PORT}"

echo "MeiLang workspace: ${WORKSPACE_ROOT}"
echo "Toolchain mode: ${TOOLCHAIN_MODE}"
echo "Listen: ${HOST}:${PORT}"
echo "Open: ${URL}"
if [[ -f "${LINKS_JSON}" && "${TOOLCHAIN_MODE}" == "installed" ]]; then
  echo "Links: ${LINKS_JSON} (toolchain.active should match toolchain/MANIFEST.json#version)"
fi
echo ""

if [[ "${TOOLCHAIN_MODE}" == "cargo" ]]; then
  if [[ ! -f "${MEI_LANG_ROOT}/Cargo.toml" ]]; then
    echo "error: MEI_LANG_ROOT=${MEI_LANG_ROOT} is not a mei-lang checkout" >&2
    exit 1
  fi
  echo "cargo mode: incremental compile; stock and prebuild run automatically at startup when needed."
  exec cargo run --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
    -p mei-lang-server --bin mei-host-web -- serve \
    --source-root "${WORKSPACE_ROOT}" \
    --toolchain-mode cargo \
    --host "${HOST}" \
    --port "${PORT}" \
    "$@"
fi

if [[ -x "${INSTALLED_HOST_BIN}" ]]; then
  exec "${INSTALLED_HOST_BIN}" serve \
    --source-root "${WORKSPACE_ROOT}" \
    --toolchain-mode installed \
    --host "${HOST}" \
    --port "${PORT}" \
    "$@"
fi

if [[ -n "${MEI_HOST_WEB_BIN:-}" && -x "${MEI_HOST_WEB_BIN}" ]]; then
  exec "${MEI_HOST_WEB_BIN}" serve \
    --source-root "${WORKSPACE_ROOT}" \
    --toolchain-mode installed \
    --host "${HOST}" \
    --port "${PORT}" \
    "$@"
fi

cat >&2 <<EOF
error: cannot find mei-host-web for installed mode.

Try one of:
  1. ./deploy/start.sh --toolchain-mode cargo --port ${PORT}
  2. mei-toolchain workspace runtime install --source-root "${WORKSPACE_ROOT}" --force
  3. export MEI_HOST_WEB_BIN=/path/to/mei-host-web

EOF
exit 1
"#
}

fn render_common_runtime_json(package_root: &Path) -> Result<String> {
    let mut descriptor = editor_runtime_descriptor_for_package_root(package_root);
    descriptor.package_root = "workspace-local-runtime".to_string();
    descriptor.package_root_resolution = vec![
        "Prefer the workspace-local runtime under `.mei/runtime/bin/`.".to_string(),
        "Use explicit `MEI_TOOLCHAIN_BIN` / `MEI_HOST_WEB_BIN` only as a recovery override."
            .to_string(),
        "A runtime-installed workspace must not require a sibling `mei-lang` checkout.".to_string(),
    ];
    descriptor.standalone_flow = vec![
        "Run `mei-toolchain workspace bootstrap --source-root <dir> [--app <app>] [--tool <tool>] --json` when creating a brand new source workspace.".to_string(),
        "If the source workspace already exists, run `workspace runtime install` or `workspace runtime update` to refresh `.mei/`.".to_string(),
        "If you need the staged flow, run `workspace init`, `workspace runtime install`, then `editor-runtime scaffold`.".to_string(),
        "Use `./start.sh` to launch the workspace-local `mei-host-web` binary.".to_string(),
        "Use `.mei/runtime/bin/mei-toolchain` and `.mei/runtime/bin/mei-lsp` as the canonical local binaries.".to_string(),
        "Run `mei-toolchain knowledge --surface author --include-content --json` to export packaged authoring docs/examples.".to_string(),
    ];
    serde_json::to_string_pretty(&descriptor).map_err(Into::into)
}

fn render_workspace_runtime_version_json() -> Result<String> {
    serde_json::to_string_pretty(&workspace_runtime_version_descriptor()).map_err(Into::into)
}

fn render_workspace_runtime_manifest_json(package_root: &Path) -> Result<String> {
    serde_json::to_string_pretty(&workspace_runtime_manifest_for_package_root(package_root))
        .map_err(Into::into)
}

fn render_workspace_catalog_json(workspace_root: &Path, package_root: &Path) -> Result<String> {
    serde_json::to_string_pretty(
        &crate::capability_catalog::capability_catalog_descriptor_for_workspace_root(
            workspace_root,
            package_root,
        ),
    )
    .map_err(Into::into)
}

fn render_workspace_surface_json(
    workspace_root: &Path,
    package_root: &Path,
    surface: &str,
) -> Result<String> {
    let descriptor = crate::capability_catalog::mcp_surface_descriptor_for_workspace_root(
        workspace_root,
        package_root,
        surface,
    )
    .ok_or_else(|| anyhow::anyhow!("unsupported mcp surface `{surface}`"))?;
    serde_json::to_string_pretty(&descriptor).map_err(Into::into)
}

fn copy_runtime_file(
    target_root: &Path,
    source_path: &Path,
    destination_path: &Path,
    force: bool,
) -> Result<EditorRuntimeScaffoldFile> {
    let content = fs::read_to_string(source_path)
        .with_context(|| format!("read runtime asset {}", source_path.display()))?;
    write_file(destination_path, content.as_str(), force).map(|mut file| {
        file.rel_path = destination_path.display().to_string();
        normalize_scaffold_files(target_root, vec![file])
            .into_iter()
            .next()
            .expect("normalized runtime file")
    })
}

fn copy_runtime_tree(
    target_root: &Path,
    source_dir: &Path,
    destination_dir: &Path,
    force: bool,
) -> Result<Vec<EditorRuntimeScaffoldFile>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(source_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let source_path = entry.path();
        let rel = source_path
            .strip_prefix(source_dir)
            .with_context(|| format!("strip runtime asset prefix {}", source_dir.display()))?;
        if rel.as_os_str().is_empty() || entry.file_type().is_dir() {
            continue;
        }
        files.push(copy_runtime_file(
            target_root,
            source_path,
            &destination_dir.join(rel),
            force,
        )?);
    }
    Ok(files)
}

fn write_runtime_projection_files(
    target_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<Vec<EditorRuntimeScaffoldFile>> {
    let mut files = Vec::new();
    for (_, source_path, destination_path) in
        build_runtime_binary_set_for_package_root(package_root, target_root)?
    {
        files.push(copy_runtime_binary(
            target_root,
            &source_path,
            &destination_path,
            force,
        )?);
    }
    let catalog_dir = workspace_catalog_dir(target_root);
    files.push(write_file(
        &catalog_dir.join("capability-catalog.json"),
        &render_workspace_catalog_json(target_root, package_root)?,
        force,
    )?);
    files.push(write_file(
        &catalog_dir.join("author-surface.json"),
        &render_workspace_surface_json(target_root, package_root, "author")?,
        force,
    )?);
    files.push(write_file(
        &catalog_dir.join("access-surface.json"),
        &render_workspace_surface_json(target_root, package_root, "access")?,
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("guides/author-profile.md"),
        &workspace_profiles_dir(target_root).join("author.md"),
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("guides/access-profile.md"),
        &workspace_profiles_dir(target_root).join("access.md"),
        force,
    )?);
    files.extend(copy_runtime_tree(
        target_root,
        &package_root.join("guides/author-skills"),
        &workspace_author_skill_dir(target_root),
        force,
    )?);
    files.extend(copy_runtime_tree(
        target_root,
        &package_root.join("guides/access-skills"),
        &workspace_access_skill_dir(target_root),
        force,
    )?);
    files.extend(copy_runtime_tree(
        target_root,
        &package_root.join("knowledge/editor-runtime"),
        &workspace_knowledge_dir(target_root).join("author"),
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("scripts/mcp/mei-author-stdio-adapter.mjs"),
        &workspace_store_bin_dir(target_root, TOOLCHAIN_VERSION).join("author-mcp-adapter"),
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("scripts/mcp/mei-access-stdio-adapter.mjs"),
        &workspace_store_bin_dir(target_root, TOOLCHAIN_VERSION).join("access-mcp-adapter"),
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("scripts/mcp/mcp-adapter-common.mjs"),
        &workspace_store_bin_dir(target_root, TOOLCHAIN_VERSION).join("mcp-adapter-common.mjs"),
        force,
    )?);
    finalize_toolchain_store_layout(target_root)?;
    Ok(files)
}

fn normalize_scaffold_files(
    target_root: &Path,
    files: Vec<EditorRuntimeScaffoldFile>,
) -> Vec<EditorRuntimeScaffoldFile> {
    files
        .into_iter()
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
    let platform = workspace_platform_dir(target_root);
    fs::create_dir_all(&platform).ok();
    files.push(write_file(
        &platform.join("editor-runtime.json"),
        &render_common_runtime_json(package_root)?,
        force,
    )?);
    files.push(write_file(
        &platform.join("knowledge/author-runtime.json"),
        &serde_json::to_string_pretty(&crate::export_knowledge_bundle_for_package_root(
            package_root,
            "author",
            None,
            false,
        )?)?,
        force,
    )?);
    files.push(write_file(
        &platform.join("version.json"),
        &render_workspace_runtime_version_json()?,
        force,
    )?);
    let manifest_json = render_workspace_runtime_manifest_json(package_root)?;
    files.push(write_file(
        &toolchain_store_dir(target_root, TOOLCHAIN_VERSION).join("MANIFEST.json"),
        &manifest_json,
        force,
    )?);
    files.push(write_file(
        &resolve_toolchain_root(target_root).join("MANIFEST.json"),
        &manifest_json,
        force,
    )?);
    files.push(write_file(
        &target_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL),
        &render_workspace_runtime_warmup_manifest_json(target_root)?,
        force,
    )?);
    files.extend(write_runtime_projection_files(
        target_root,
        package_root,
        force,
    )?);
    fs::create_dir_all(target_root.join("deploy")).ok();
    files.push(write_executable_file(
        &target_root.join("deploy/start.sh"),
        render_workspace_start_script(),
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
    crate::workspace_stock::ensure_workspace_stock_materialized(target_root, package_root)?;
    let files = normalize_scaffold_files(
        target_root,
        write_common_runtime_files(target_root, package_root, force)?,
    );
    Ok(EditorRuntimeInstallReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        target_root: target_root.display().to_string(),
        files,
    })
}

/// Bootstrap only the workspace-local author skill tree when missing.
/// Does not install binaries, MCP adapters, or other runtime projection files.
pub fn ensure_workspace_author_skill_package(
    workspace_root: &Path,
    package_root: &Path,
) -> Result<EnsureAuthorSkillReport> {
    let install_dir = workspace_author_skill_dir(workspace_root);
    let entry_file = install_dir.join("SKILL.md");
    if entry_file.is_file() {
        return Ok(EnsureAuthorSkillReport {
            installed: true,
            installed_now: false,
            install_dir: install_dir.display().to_string(),
            file_count: count_markdown_files(&install_dir),
        });
    }
    let source_dir = package_root.join("guides/author-skills");
    anyhow::ensure!(
        source_dir.is_dir(),
        "author skill source tree missing at {}",
        source_dir.display()
    );
    copy_runtime_tree(workspace_root, &source_dir, &install_dir, false)?;
    anyhow::ensure!(
        entry_file.is_file(),
        "author skill install incomplete at {}",
        entry_file.display()
    );
    Ok(EnsureAuthorSkillReport {
        installed: true,
        installed_now: true,
        install_dir: install_dir.display().to_string(),
        file_count: count_markdown_files(&install_dir),
    })
}

fn count_markdown_files(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count()
}

fn render_workspace_runtime_warmup_manifest_json(target_root: &Path) -> Result<String> {
    serde_json::to_string_pretty(&build_workspace_runtime_warmup_manifest(target_root)?)
        .context("serialize workspace warmup manifest")
}

fn build_workspace_runtime_warmup_manifest(target_root: &Path) -> Result<RuntimeWarmupManifest> {
    build_runtime_warmup_manifest(target_root)
}

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
globs: ["**/*.mei", ".mei/**"]
alwaysApply: false
---

- Treat `workspace runtime status/install/update`, `mei-toolchain`, `mei-lsp`, and the local `.mei/editor-runtime.json` as the canonical workspace-local environment entrypoints.
- Treat checked-in workspace files as the source-of-truth layer, and treat `.mei/` as the installed local runtime layer.
- Prefer `workspace bootstrap` when creating a brand new workspace; prefer `workspace runtime install` or `workspace runtime update` when the source workspace already exists.
- Prefer the workspace-local `.mei/runtime/bin/mei-toolchain`, `.mei/runtime/bin/mei-lsp`, and `./start.sh` over sibling source checkouts or global PATH assumptions.
- Prefer `mei-toolchain knowledge --surface author --include-content --json --source-root <workspace>` when you need bundled authoring docs, profile guidance, or examples.
- Use `mei-toolchain knowledge --surface access --include-content --json --source-root <workspace>` for world-first access guidance and query-state-aware runtime questions.
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
Use the local `.mei/editor-runtime.json` as the runtime descriptor. Treat the checked-in workspace files as the source-of-truth layer, and `.mei/` as locally installed runtime output.\n\n\
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use mei_lang_kernel::{RuntimeWarmupManifest, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL};

    use super::*;

    fn temp_workspace_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("mei-editor-runtime-{name}-{nanos}"))
    }

    #[test]
    fn install_runtime_writes_warmup_manifest() {
        let workspace_root = temp_workspace_root("warmup");
        let app_root = workspace_root.join("demo");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::write(
            app_root.join("main.mei"),
            "app(id=\"demo\")\nscene(id=\"home\", target=\"home.mei\")\n",
        )
        .expect("write main");
        fs::write(app_root.join("home.mei"), "frame()").expect("write scene");
        fs::write(
            workspace_root.join(".mei-workspace.json"),
            r#"{
  "warmup": {
    "apps": {
      "demo": {
        "hotScenes": ["command-center"],
        "datasets": [
          {
            "sceneId": "home",
            "datasetId": "warning_list",
            "metricId": "case_total"
          }
        ]
      }
    }
  }
}"#,
        )
        .expect("write workspace config");

        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("mei-lang package root")
            .to_path_buf();
        install_editor_runtime_support_files(&workspace_root, &package_root, true)
            .expect("install runtime");

        let manifest_path = workspace_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL);
        let raw = fs::read_to_string(&manifest_path).expect("read manifest");
        let manifest: RuntimeWarmupManifest =
            serde_json::from_str(&raw).expect("parse warmup manifest");
        assert!(manifest.enabled);
        assert_eq!(manifest.apps.len(), 1);
        assert_eq!(manifest.apps[0].app_id, "demo");
        assert_eq!(
            manifest.apps[0].hot_scenes,
            vec!["command-center".to_string()]
        );
        assert!(
            manifest.apps[0]
                .scenes
                .contains(&"command-center".to_string()),
            "expected hot scene to be included in merged warmup scenes"
        );
        assert_eq!(manifest.apps[0].datasets.len(), 1);
        assert_eq!(manifest.apps[0].datasets[0].dataset_id, "warning_list");
        assert_eq!(
            manifest.apps[0].datasets[0].metric_id.as_deref(),
            Some("case_total")
        );
        assert_eq!(manifest.apps[0].focuses, vec!["main.mei".to_string()]);
        assert!(manifest.apps[0].datasets[0].focus.is_none());

        let _ = fs::remove_dir_all(&workspace_root);
    }

    #[test]
    fn ensure_author_skill_installs_when_missing_without_full_runtime_install() {
        let workspace_root = temp_workspace_root("ensure-author-skill");
        fs::create_dir_all(&workspace_root).expect("create workspace root");
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("mei-lang package root")
            .to_path_buf();
        assert!(
            !workspace_root
                .join("runtime/platform/skills/meilang-author/SKILL.md")
                .is_file(),
            "fixture should start without author skill"
        );
        let report = ensure_workspace_author_skill_package(&workspace_root, &package_root)
            .expect("ensure author skill");
        assert!(report.installed);
        assert!(report.installed_now);
        assert!(report.file_count > 0);
        assert!(
            workspace_root
                .join("runtime/platform/skills/meilang-author/SKILL.md")
                .is_file()
        );
        let again = ensure_workspace_author_skill_package(&workspace_root, &package_root)
            .expect("ensure author skill again");
        assert!(again.installed);
        assert!(!again.installed_now);
        let _ = fs::remove_dir_all(&workspace_root);
    }
}
