use serde::Serialize;

pub const EDITOR_RUNTIME_SCHEMA_VERSION: &str = "mei-editor-runtime-v1";
pub const WORKSPACE_RUNTIME_VERSION_SCHEMA_VERSION: &str = "mei-runtime-version-v1";
pub const WORKSPACE_RUNTIME_MANIFEST_SCHEMA_VERSION: &str = "mei-runtime-manifest-v1";
#[allow(unused_imports)]
pub use mei_lang_kernel::WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION;
pub const RUNTIME_BUNDLE_SCHEMA_VERSION: &str = "mei-runtime-bundle-v1";

pub(crate) const TOOLCHAIN_VERSION: &str = env!("MEI_CARGO_PACKAGE_VERSION");
pub(crate) const GIT_COMMIT_SHORT: &str = env!("MEI_GIT_COMMIT_SHORT");
pub(crate) const GIT_COMMIT_FULL: &str = env!("MEI_GIT_COMMIT_FULL");
pub(crate) const GIT_DIRTY: &str = env!("MEI_GIT_DIRTY");
pub(crate) const BUILD_VERSION: &str = env!("MEI_BUILD_VERSION");
pub(crate) const BUILD_TIMESTAMP_UTC: &str = env!("MEI_BUILD_TIMESTAMP_UTC");
pub(crate) const TARGET_TRIPLE: &str = env!("MEI_TARGET_TRIPLE");
pub(crate) const COMPATIBILITY_LINE: &str = env!("MEI_COMPATIBILITY_LINE");

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
