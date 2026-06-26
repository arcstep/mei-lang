use super::prelude::*;
use super::*;

pub(crate) fn compile_scopes_for_app(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> Vec<CompileScope> {
    let mut scopes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_scope = |scope: CompileScope| {
        let scope = scope.canonicalized();
        if seen.insert(scope.key()) {
            scopes.push(scope);
        }
    };
    push_scope(CompileScope::default_scope());
    let scene_ids = scene_ids_for_profile(app, scope_profile);
    let focus_targets = focus_targets_for_profile(app, scope_profile);
    for scene_id in &scene_ids {
        push_scope(CompileScope {
            requested_scene_id: Some(scene_id.clone()),
            requested_target_file: None,
        });
    }
    for focus in &focus_targets {
        push_scope(CompileScope {
            requested_scene_id: None,
            requested_target_file: Some(focus.clone()),
        });
    }
    for scene_id in hot_scene_ids(app) {
        for focus in &focus_targets {
            push_scope(CompileScope {
                requested_scene_id: Some(scene_id.clone()),
                requested_target_file: Some(focus.clone()),
            });
        }
    }
    for request in app
        .datasets
        .iter()
        .filter(|request| warmup_dataset_request_in_profile(app, request, scope_profile))
    {
        push_scope(warmup_request_scope(request));
    }
    scopes
}

pub(crate) fn build_prebuild_manifest_plan(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> PrebuildManifestPlan {
    let warmup_requests = aggregate_warmup_requests(app, scope_profile);
    let default_scope = CompileScope::default_scope();
    let all_scopes = compile_scopes_for_app(app, scope_profile);
    let initial_scope_count = all_scopes.len();
    let hot_scope_keys = hot_scene_ids(app)
        .into_iter()
        .map(|scene| format!("{}|", scene.trim()))
        .filter(|key| key != "|")
        .collect::<BTreeSet<_>>();
    let (hot_scopes, deferred_scopes): (Vec<_>, Vec<_>) = all_scopes
        .into_iter()
        .filter(|scope| scope.key() != default_scope.key())
        .partition(|scope| hot_scope_keys.contains(&scope.key()));
    PrebuildManifestPlan {
        initial_scope_count,
        hot_scopes,
        deferred_scopes,
        warmup_requests,
    }
}

pub(crate) fn hot_scene_ids(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut scene_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for scene_id in app.default_scene.iter().chain(app.hot_scenes.iter()) {
        let scene_id = scene_id.trim();
        if scene_id.is_empty() || !seen.insert(scene_id.to_string()) {
            continue;
        }
        scene_ids.push(scene_id.to_string());
    }
    scene_ids
}

pub(crate) fn explicit_scene_ids(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut scene_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for scene_id in app
        .default_scene
        .iter()
        .chain(app.hot_scenes.iter())
        .chain(app.scenes.iter())
    {
        let scene_id = scene_id.trim();
        if scene_id.is_empty() || !seen.insert(scene_id.to_string()) {
            continue;
        }
        scene_ids.push(scene_id.to_string());
    }
    scene_ids
}

pub(crate) fn scene_ids_for_profile(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> Vec<String> {
    match scope_profile {
        PrebuildScopeProfile::Full => explicit_scene_ids(app),
        PrebuildScopeProfile::HotOnly => hot_scene_ids(app),
    }
}

pub(crate) fn explicit_focus_targets(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut targets = Vec::new();
    let mut seen = BTreeSet::new();
    for focus in &app.focuses {
        let focus = focus.trim();
        if focus.is_empty() || !seen.insert(focus.to_string()) {
            continue;
        }
        targets.push(focus.to_string());
    }
    targets
}

pub(crate) fn focus_targets_from_warmup_datasets(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut targets = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push = |target: &str| {
        let target = target.trim();
        if target.is_empty() || !is_script_target(target) || !seen.insert(target.to_string()) {
            return;
        }
        targets.push(target.to_string());
    };
    for request in &app.datasets {
        if let Some(target) = warmup_request_target_file(request) {
            push(target.as_str());
        }
    }
    targets
}

pub(crate) fn warmup_dataset_selector_target_file(dataset_selector: &str) -> Option<String> {
    dataset_selector
        .split("::")
        .map(str::trim)
        .find(|segment| {
            (segment.starts_with("scenes/") || segment.starts_with("src/scenes/"))
                && segment.ends_with(".mei")
        })
        .map(mei_lang_kernel::canonical_app_source_rel_path)
}

pub(crate) fn warmup_request_target_file(request: &RuntimeWarmupDatasetRequest) -> Option<String> {
    request
        .focus
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| warmup_dataset_selector_target_file(request.dataset_id.as_str()))
}

pub(crate) fn warmup_request_scope(request: &RuntimeWarmupDatasetRequest) -> CompileScope {
    CompileScope {
        requested_scene_id: request.scene_id.clone(),
        requested_target_file: warmup_request_target_file(request),
    }
    .canonicalized()
}

pub(crate) fn all_focus_targets(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut targets = explicit_focus_targets(app);
    let mut seen = targets.iter().cloned().collect::<BTreeSet<_>>();
    for focus in focus_targets_from_warmup_datasets(app) {
        if seen.insert(focus.clone()) {
            targets.push(focus);
        }
    }
    targets
}

pub(crate) fn focus_targets_for_profile(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> Vec<String> {
    match scope_profile {
        PrebuildScopeProfile::Full => all_focus_targets(app),
        // Hot path should keep the explicit entry/main focus, but skip dataset-derived expansions.
        PrebuildScopeProfile::HotOnly => explicit_focus_targets(app),
    }
}

pub(crate) fn aggregate_warmup_requests(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> Vec<AggregatedWarmupRequest> {
    let mut aggregated = BTreeMap::<String, AggregatedWarmupRequest>::new();
    for request in app
        .datasets
        .iter()
        .filter(|request| warmup_dataset_request_in_profile(app, request, scope_profile))
    {
        let scope = warmup_request_scope(request);
        let priority = warmup_request_priority(app, request);
        let metric_ids = requested_metric_ids(request);
        let request_all_metrics = metric_ids.is_empty();
        let key = format!("{}|{}", scope.key(), request.dataset_id.trim());
        if let Some(entry) = aggregated.get_mut(&key) {
            entry.priority = entry.priority.min(priority);
            if request_all_metrics || entry.metric_ids.is_empty() {
                entry.metric_ids.clear();
            } else {
                entry.metric_ids.extend(metric_ids);
                entry.metric_ids.sort();
                entry.metric_ids.dedup();
            }
            continue;
        }
        aggregated.insert(
            key,
            AggregatedWarmupRequest {
                scope,
                dataset_id: request.dataset_id.trim().to_string(),
                priority,
                metric_ids,
            },
        );
    }
    aggregated.into_values().collect()
}

pub(crate) fn explicit_warmup_request_priority(
    request: &RuntimeWarmupDatasetRequest,
) -> Option<WarmupRequestPriority> {
    match request.priority.as_deref().map(str::trim) {
        Some("critical" | "hot") => Some(WarmupRequestPriority::Critical),
        Some("deferred" | "heavy" | "full") => Some(WarmupRequestPriority::Deferred),
        _ => None,
    }
}

pub(crate) fn warmup_request_priority(
    app: &RuntimeWarmupApp,
    request: &RuntimeWarmupDatasetRequest,
) -> WarmupRequestPriority {
    if let Some(priority) = explicit_warmup_request_priority(request) {
        return priority;
    }
    let hot_scenes = hot_scene_ids(app);
    let explicit_focuses = explicit_focus_targets(app);
    let request_scene = request
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(scene_id) = request_scene {
        return if hot_scenes.iter().any(|value| value == scene_id) {
            WarmupRequestPriority::Critical
        } else {
            WarmupRequestPriority::Deferred
        };
    }
    if let Some(focus) = warmup_request_target_file(request) {
        return if explicit_focuses.iter().any(|value| value == &focus) {
            WarmupRequestPriority::Critical
        } else {
            WarmupRequestPriority::Deferred
        };
    }
    WarmupRequestPriority::Critical
}

pub(crate) fn warmup_dataset_request_in_profile(
    app: &RuntimeWarmupApp,
    request: &RuntimeWarmupDatasetRequest,
    scope_profile: PrebuildScopeProfile,
) -> bool {
    if scope_profile == PrebuildScopeProfile::Full {
        return true;
    }
    warmup_request_priority(app, request) == WarmupRequestPriority::Critical
}

pub(crate) fn app_has_deferred_warmup_work(app: &RuntimeWarmupApp) -> bool {
    let full = build_prebuild_manifest_plan(app, PrebuildScopeProfile::Full);
    let hot = build_prebuild_manifest_plan(app, PrebuildScopeProfile::HotOnly);
    (full.hot_scopes.len() + full.deferred_scopes.len())
        > (hot.hot_scopes.len() + hot.deferred_scopes.len())
        || full.warmup_requests.len() > hot.warmup_requests.len()
}

pub(crate) fn warmup_request_matches_outcome(
    request: &AggregatedWarmupRequest,
    outcome: &SharedCompileOutcome,
) -> bool {
    let req_scope = request.scope.canonicalized();
    let active_scene = outcome
        .compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let active_target = outcome.compiled.active_target_file.as_str();
    if let Some(req_scene) = req_scope
        .requested_scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if req_scene != active_scene {
            return false;
        }
    }
    if let Some(req_target) = req_scope
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if req_target != active_target {
            return false;
        }
    }
    if !mei_lang_kernel::locate_dataset_resource(&outcome.compiled, request.dataset_id.as_str()).is_ok()
        || !dataset_can_materialize_metric_artifacts(
            &outcome.compiled,
            request.dataset_id.as_str(),
        )
    {
        return false;
    }
    if request.metric_ids.is_empty() {
        return true;
    }
    request.metric_ids.iter().all(|metric_id| {
        locate_runtime_metric_resource(
            &outcome.compiled,
            request.dataset_id.as_str(),
            metric_id.as_str(),
        )
        .is_ok()
    })
}

pub(crate) fn matching_warmup_requests_for_outcome<'a>(
    requests: &'a [AggregatedWarmupRequest],
    outcome: &SharedCompileOutcome,
) -> Vec<&'a AggregatedWarmupRequest> {
    requests
        .iter()
        .filter(|request| warmup_request_matches_outcome(request, outcome))
        .collect()
}

pub(crate) fn group_warmup_requests_by_scope<'a>(
    requests: &[&'a AggregatedWarmupRequest],
) -> Vec<WarmupScopeBatch<'a>> {
    let mut grouped = BTreeMap::<String, WarmupScopeBatch<'a>>::new();
    for request in requests {
        grouped
            .entry(request.scope.key())
            .and_modify(|batch| batch.requests.push(*request))
            .or_insert_with(|| WarmupScopeBatch {
                scope: request.scope.clone(),
                requests: vec![*request],
            });
    }
    grouped.into_values().collect()
}

pub(crate) fn run_warmup_request_batch(
    source_root: &Path,
    app_id: &str,
    app_root: &Path,
    mode: PrebuildMode,
    components_root: &Path,
    coverage_state: &CoverageState,
    requests: &[&AggregatedWarmupRequest],
    max_parallelism: usize,
) -> Vec<(CompileScope, Vec<(String, Result<()>)>, PrebuildCoverageReport)> {
    let grouped_requests = group_warmup_requests_by_scope(requests);
    run_limited_parallel_ordered(grouped_requests, max_parallelism, |batch| {
        let scope = batch.scope.clone();
        let mut local_coverage = PrebuildCoverageReport::default();
        let mut results = Vec::with_capacity(batch.requests.len());
        let compiled = ensure_compile_scope(source_root, app_id, &scope, mode, components_root);
        match compiled {
            Ok(outcome) => {
                for request in batch.requests {
                    let result = ensure_request_artifacts_for_compiled(
                        app_id,
                        app_root,
                        &outcome,
                        request.dataset_id.as_str(),
                        request.metric_ids.as_slice(),
                        mode,
                        &mut local_coverage,
                        coverage_state,
                    );
                    results.push((request.dataset_id.clone(), result));
                }
            }
            Err(error) => {
                let error_text = error.to_string();
                for request in batch.requests {
                    results.push((
                        request.dataset_id.clone(),
                        Err(anyhow::anyhow!(error_text.clone())),
                    ));
                }
            }
        }
        (scope, results, local_coverage)
    })
}

