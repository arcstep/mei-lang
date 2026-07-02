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
    pub(crate) assemble_only: bool,
    pub(crate) compile_revision: String,
    pub(crate) cache_lookup_ms: u64,
    pub(crate) artifact_load_ms: u64,
    pub(crate) compile_ms: u64,
    /// Heavy `CompiledApp` dropped after MCG persist; hydrate before artifact eval.
    pub(crate) handle_only: bool,
    /// MCG assembly metadata when `handle_only`.
    pub(crate) assembly_handle: Option<crate::graph::mcg::handle::AssemblyViewHandle>,
}

impl SharedCompileOutcome {
    pub(crate) fn from_shared(outcome: toolchain::CompileWithCacheOutcomeShared) -> Self {
        Self {
            compiled: outcome.compiled,
            cache_hit: outcome.cache_hit,
            artifact_cache_hit: outcome.artifact_cache_hit,
            assemble_only: false,
            compile_revision: outcome.compile_revision,
            cache_lookup_ms: outcome.cache_lookup_ms,
            artifact_load_ms: outcome.artifact_load_ms,
            compile_ms: outcome.compile_ms,
            handle_only: false,
            assembly_handle: None,
        }
    }
}

pub(crate) fn projection_handle_outcome(
    scope: &CompileScope,
    base: &SharedCompileOutcome,
    diagnostics: Option<&PrebuildDiagnostics>,
) -> SharedCompileOutcome {
    let canonical = scope.canonicalized();
    let scene = canonical.requested_scene_id.as_deref();
    let target = canonical
        .requested_target_file
        .as_deref()
        .filter(|value| !value.is_empty());
    let mut stub = mei_lang_kernel::CompiledApp {
        app_id: base.compiled.app_id.clone(),
        title: String::new(),
        app_root: base.compiled.app_root.clone(),
        active_scene: scene.map(str::to_string),
        active_target_file: target.unwrap_or_default().to_string(),
        file_tree: Vec::new(),
        scene_routes: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: BTreeMap::new(),
        scene_bindings_by_id: BTreeMap::new(),
        scene_examples_by_id: BTreeMap::new(),
        scene_projection_assembly_by_id: BTreeMap::new(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    crate::graph::mcg::assemble::apply_scope_to_compiled_app(
        &mut stub,
        scene,
        target,
    );
    if let Some(diag) = diagnostics {
        diag.mcg_assemble_only_count.fetch_add(1, Ordering::Relaxed);
        diag.compile_target_overlay_reuse_hits
            .fetch_add(1, Ordering::Relaxed);
    }
    SharedCompileOutcome {
        compiled: Arc::new(stub),
        cache_hit: true,
        artifact_cache_hit: base.artifact_cache_hit,
        assemble_only: true,
        compile_revision: base.compile_revision.clone(),
        cache_lookup_ms: base.cache_lookup_ms,
        artifact_load_ms: base.artifact_load_ms,
        compile_ms: 0,
        handle_only: true,
        assembly_handle: None,
    }
}

pub(crate) fn shrink_outcome_to_handle(
    outcome: &mut SharedCompileOutcome,
    source_root: Option<&Path>,
    app_id: Option<&str>,
) {
    if outcome.handle_only {
        return;
    }
    let assembly_handle = source_root
        .zip(app_id)
        .map(|(root, id)| {
            crate::graph::mcg::handle::AssemblyViewHandle::from_mcg_registry(
                root,
                id,
                outcome.compiled.as_ref(),
                outcome.compile_revision.as_str(),
            )
        });
    let stub = CompiledApp {
        app_id: outcome.compiled.app_id.clone(),
        title: String::new(),
        app_root: outcome.compiled.app_root.clone(),
        scene_routes: Vec::new(),
        active_scene: outcome.compiled.active_scene.clone(),
        active_target_file: outcome.compiled.active_target_file.clone(),
        file_tree: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: BTreeMap::new(),
        scene_bindings_by_id: BTreeMap::new(),
        scene_examples_by_id: BTreeMap::new(),
        scene_projection_assembly_by_id: BTreeMap::new(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    outcome.compiled = Arc::new(stub);
    outcome.handle_only = true;
    outcome.assembly_handle = assembly_handle;
}

pub(crate) fn hydrate_outcome_for_artifacts(
    source_root: &Path,
    app_id: &str,
    outcome: &SharedCompileOutcome,
) -> Result<SharedCompileOutcome> {
    if !outcome.handle_only {
        return Ok(outcome.clone());
    }
    if let Some(handle) = outcome.assembly_handle.as_ref() {
        let compiled = crate::graph::mcg::handle::hydrate_handle_for_eval(source_root, handle)?;
        return Ok(SharedCompileOutcome {
            compiled: Arc::new(compiled),
            cache_hit: outcome.cache_hit,
            artifact_cache_hit: outcome.artifact_cache_hit,
            assemble_only: outcome.assemble_only,
            compile_revision: outcome.compile_revision.clone(),
            cache_lookup_ms: outcome.cache_lookup_ms,
            artifact_load_ms: outcome.artifact_load_ms,
            compile_ms: outcome.compile_ms,
            handle_only: false,
            assembly_handle: None,
        });
    }
    let scene = outcome.compiled.active_scene.as_deref();
    let target = outcome.compiled.active_target_file.as_str();
    if let Some((mut compiled, compile_revision)) =
        crate::graph::try_assemble_scope_from_scene_payload(source_root, app_id, scene, target)
    {
        let _ = crate::graph::hydrate_compiled_for_prebuild_eval(
            source_root,
            app_id,
            &mut compiled,
            &[],
            &[],
        );
        return Ok(SharedCompileOutcome {
            compiled: Arc::new(compiled),
            cache_hit: outcome.cache_hit,
            artifact_cache_hit: outcome.artifact_cache_hit,
            assemble_only: outcome.assemble_only,
            compile_revision,
            cache_lookup_ms: outcome.cache_lookup_ms,
            artifact_load_ms: outcome.artifact_load_ms,
            compile_ms: outcome.compile_ms,
            handle_only: false,
            assembly_handle: None,
        });
    }
    anyhow::bail!(
        "hydrate artifact handle failed for target `{}` revision `{}`",
        target,
        outcome.compile_revision
    )
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

/// Compile target + revision — shared by all scene overlays on the same `.mei` / board file.
pub(crate) fn compiled_target_identity(outcome: &SharedCompileOutcome) -> String {
    let target = outcome.compiled.active_target_file.trim();
    if target.is_empty() {
        return String::new();
    }
    format!("{}|{}", target, outcome.compile_revision)
}

/// Artifact / RSS dedup key: collapse board scene aliases onto one ScenePayload base.
pub(crate) fn compiled_artifact_identity(outcome: &SharedCompileOutcome) -> String {
    let target_identity = compiled_target_identity(outcome);
    if target_identity.is_empty() {
        compiled_scope_identity(outcome)
    } else {
        target_identity
    }
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

