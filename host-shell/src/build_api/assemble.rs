use std::path::Path;

use mei_host_graph::assemble_scope_from_registry;
use mei_host_graph::{enrich_compiled_scope, EnrichCompiledScopeOptions};
use mei_lang_kernel::{
    compile_scene_from_build_node, BuildNodeId, CompiledApp,
};

#[derive(Debug, Clone)]
pub struct EnrichedAssembleOutcome {
    pub compiled: CompiledApp,
    pub compile_revision: String,
}

#[derive(Debug)]
pub enum AssembleBuildError {
    InvalidNode,
    NotAssembled(String),
    AssembleFailed(String),
}

impl AssembleBuildError {
    pub fn message(&self) -> String {
        match self {
            Self::InvalidNode => "invalid or missing scene for node".to_string(),
            Self::NotAssembled(message) => message.clone(),
            Self::AssembleFailed(message) => message.clone(),
        }
    }
}

pub fn enrich_compiled(compiled: CompiledApp, workspace_root: &Path) -> CompiledApp {
    let app_id = compiled.app_id.clone();
    enrich_compiled_scope(
        compiled,
        workspace_root,
        app_id.as_str(),
        EnrichCompiledScopeOptions::default(),
    )
}

pub fn assemble_enriched_for_build_node(
    workspace_root: &Path,
    app_id: &str,
    node_raw: &str,
    scene_fallback: Option<&str>,
) -> Result<EnrichedAssembleOutcome, AssembleBuildError> {
    let scene_id = BuildNodeId::parse(node_raw)
        .and_then(|node| compile_scene_from_build_node(&node))
        .or_else(|| scene_fallback.map(str::to_string))
        .filter(|value| !value.trim().is_empty())
        .ok_or(AssembleBuildError::InvalidNode)?;

    let outcome = assemble_scope_from_registry(workspace_root, app_id, scene_id.as_str())
        .map_err(|error| AssembleBuildError::AssembleFailed(error.to_string()))?
        .ok_or_else(|| {
            AssembleBuildError::NotAssembled(format!(
                "scene `{scene_id}` not assembled for app `{app_id}`; run prebuild"
            ))
        })?;

    let compiled = enrich_compiled(outcome.compiled, workspace_root);
    Ok(EnrichedAssembleOutcome {
        compiled,
        compile_revision: outcome.compile_revision,
    })
}
