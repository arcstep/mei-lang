use super::prelude::*;
use super::*;

pub(crate) fn compile_scope_specificity(scope: &CompileScope) -> u8 {
    let canonical = scope.canonicalized();
    let mut score = 0u8;
    if canonical.requested_scene_id.is_some() {
        score = score.saturating_add(2);
    }
    if canonical.requested_target_file.is_some() {
        score = score.saturating_add(1);
    }
    score
}

pub(crate) fn discovered_compile_scopes(
    scope: &CompileScope,
    compiled: &mei_lang_kernel::CompiledApp,
) -> Vec<CompileScope> {
    let mut scopes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_scope = |candidate: CompileScope| {
        let candidate = candidate.canonicalized();
        if seen.insert(candidate.key()) {
            scopes.push(candidate);
        }
    };
    let active_scene = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene_id| !scene_id.is_empty())
        .map(str::to_string);
    let active_target = compiled.active_target_file.trim();
    if let Some(active_scene) = active_scene.clone() {
        push_scope(CompileScope {
            requested_scene_id: Some(active_scene.clone()),
            requested_target_file: None,
        });
        let target = scope
            .requested_target_file
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .unwrap_or(active_target);
        if !target.is_empty() {
            push_scope(CompileScope {
                requested_scene_id: Some(active_scene.clone()),
                requested_target_file: Some(target.to_string()),
            });
            if target.ends_with(".board.mei") {
                for export_scene_id in compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .chain(compiled.scene_bindings_by_id.keys())
                {
                    let export_scene_id = export_scene_id.trim();
                    if export_scene_id.is_empty() || export_scene_id == active_scene {
                        continue;
                    }
                    push_scope(CompileScope {
                        requested_scene_id: Some(export_scene_id.to_string()),
                        requested_target_file: Some(target.to_string()),
                    });
                }
            }
        }
    } else if let Some(board_file) = scope
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty() && target.ends_with(".board.mei"))
        .or_else(|| {
            active_target
                .ends_with(".board.mei")
                .then_some(active_target)
        })
    {
        for entry in compiled.build_board_index.boards.values() {
            if entry.board_file.trim() != board_file {
                continue;
            }
            push_scope(CompileScope {
                requested_scene_id: Some(entry.scene_id.clone()),
                requested_target_file: Some(board_file.to_string()),
            });
        }
    }
    scopes
}

#[derive(Clone)]
pub(crate) struct SharedCompileOutcome {
    pub(crate) compiled: Arc<CompiledApp>,
    pub(crate) cache_hit: bool,
    pub(crate) artifact_cache_hit: bool,
    pub(crate) compile_revision: String,
    pub(crate) cache_lookup_ms: u64,
    pub(crate) artifact_load_ms: u64,
    pub(crate) compile_ms: u64,
}

impl SharedCompileOutcome {
    pub(crate) fn from_shared(outcome: toolchain::CompileWithCacheOutcomeShared) -> Self {
        Self {
            compiled: outcome.compiled,
            cache_hit: outcome.cache_hit,
            artifact_cache_hit: outcome.artifact_cache_hit,
            compile_revision: outcome.compile_revision,
            cache_lookup_ms: outcome.cache_lookup_ms,
            artifact_load_ms: outcome.artifact_load_ms,
            compile_ms: outcome.compile_ms,
        }
    }
}

#[derive(Clone)]
pub(crate) struct PreparedCompileOutcome {
    pub(crate) scope: CompileScope,
    pub(crate) outcome: SharedCompileOutcome,
}

pub(crate) fn compiled_scope_identity(outcome: &SharedCompileOutcome) -> String {
    format!(
        "{}|{}|{}",
        outcome.compiled.active_scene.as_deref().unwrap_or_default(),
        outcome.compiled.active_target_file,
        outcome.compile_revision
    )
}

pub(crate) fn compiled_default_target_file(compiled: &CompiledApp) -> Option<&str> {
    compiled
        .scene_routes
        .iter()
        .find(|route| route.is_default)
        .or_else(|| compiled.scene_routes.iter().find(|route| route.scene_id.trim() == "home"))
        .map(|route| route.target_file.trim())
        .filter(|target| !target.is_empty())
}

pub(crate) fn compile_outcome_matches_scope(scope: &CompileScope, compiled: &CompiledApp) -> bool {
    let requested = scope.canonicalized();
    let active_scene = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let active_target = compiled.active_target_file.trim();
    if let Some(scene_id) = requested
        .requested_scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if active_scene != Some(scene_id) {
            return false;
        }
    }
    if let Some(target_file) = requested
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if active_target != target_file {
            return false;
        }
    }
    if requested.requested_scene_id.is_none() && requested.requested_target_file.is_none() {
        if let Some(default_target) = compiled_default_target_file(compiled) {
            return active_target == default_target;
        }
    }
    true
}

