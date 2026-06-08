use std::path::{Path, PathBuf};

use anyhow::Result;
use mei_lang_kernel::{CompileOptions, CompileRevisionPlan};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifact_store::{
    write_json_artifact, ArtifactStoreWriteResult, ArtifactWatchedFile, ArtifactWriteContext,
};
use crate::types::WorldScope;

pub const HEADLESS_EXPORT_SCHEMA_VERSION: &str = "mei-headless-export-v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessArtifactKind {
    InventorySnapshot,
    SemanticDag,
    AnalysisContracts,
    EvalPlan,
    RuntimeTrace,
}

impl HeadlessArtifactKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::InventorySnapshot => "inventory_snapshot",
            Self::SemanticDag => "semantic_dag",
            Self::AnalysisContracts => "analysis_contracts",
            Self::EvalPlan => "eval_plan",
            Self::RuntimeTrace => "runtime_trace",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HeadlessExportOptions {
    pub write_store: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessArtifactEnvelope {
    pub schema_version: String,
    pub artifact_kind: HeadlessArtifactKind,
    pub app_id: String,
    pub scope: WorldScope,
    pub revision_token: String,
    pub components_revision: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub artifact: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<ArtifactStoreWriteResult>,
}

fn compile_options_from_scope(scope: &WorldScope) -> CompileOptions {
    CompileOptions {
        scene: scope.scene_id.clone(),
        preview_target: scope.target_file.clone(),
    }
}

fn app_root(source_root: &Path, app_id: &str) -> Result<PathBuf> {
    let trimmed = app_id.trim();
    if trimmed.is_empty() {
        anyhow::bail!("--app is required");
    }
    Ok(source_root.join(trimmed))
}

pub(crate) fn export_context(
    source_root: &Path,
    app_id: &str,
    scope: &WorldScope,
) -> Result<(PathBuf, CompileRevisionPlan)> {
    let app_root = app_root(source_root, app_id)?;
    let revision = mei_lang_kernel::compile_revision_plan_from_root_with_options(
        source_root,
        &app_root,
        &compile_options_from_scope(scope),
    )?;
    Ok((app_root, revision))
}

pub(crate) fn finalize_envelope(
    app_root: &Path,
    revision: &CompileRevisionPlan,
    options: HeadlessExportOptions,
    artifact_kind: HeadlessArtifactKind,
    artifact_name: String,
    app_id: &str,
    scope: &WorldScope,
    active_scene_id: Option<String>,
    active_target_file: String,
    artifact: Value,
) -> Result<HeadlessArtifactEnvelope> {
    let store = if options.write_store {
        Some(write_json_artifact(
            app_root,
            &ArtifactWriteContext {
                app_id: app_id.to_string(),
                artifact_kind: artifact_kind.as_str().to_string(),
                artifact_name,
                scope: scope.clone(),
                active_scene_id: active_scene_id.clone(),
                active_target_file: active_target_file.clone(),
                revision_token: revision.token.clone(),
                components_revision: revision.components_revision,
                watched_files: revision
                    .watched_files
                    .iter()
                    .map(ArtifactWatchedFile::from)
                    .collect(),
            },
            &artifact,
        )?)
    } else {
        None
    };
    Ok(HeadlessArtifactEnvelope {
        schema_version: HEADLESS_EXPORT_SCHEMA_VERSION.to_string(),
        artifact_kind,
        app_id: app_id.to_string(),
        scope: scope.clone(),
        revision_token: revision.token.clone(),
        components_revision: revision.components_revision,
        active_scene_id,
        active_target_file,
        artifact,
        store,
    })
}
