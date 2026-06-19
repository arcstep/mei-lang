use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use mei_lang_datasets::{
    collect_all_query_options, evaluate_runtime_metrics_from_plan,
    metric_request_revision_fingerprint_for_compiled, metric_response_cache_scope_key,
    normalize_query_filters, normalize_query_search, plan_access_metric_eval_for_ids,
    query_state_from_request, runtime_metric_workset, store_cached_metric_response,
    store_metric_response_result_artifact, RuntimeMetricEvalMode,
};
use mei_lang_kernel::{
    cached_load_xlsx_table_snapshot, locate_dataset_resource, resolve_app_root,
    resolve_data_snapshot_import_entry,
    resolve_runtime_warmup_manifest, CompileOptions, RuntimeWarmupApp, RuntimeWarmupDatasetRequest,
    RuntimeWarmupManifest, RuntimeWarmupXlsxSource, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};
use std::collections::BTreeMap;
use serde::Serialize;

use crate::http::compile_cache::compile_app_with_cache;
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostReadyResponse {
    pub ready: bool,
    pub phase: String,
    #[serde(rename = "manifestPath")]
    pub manifest_path: String,
    #[serde(rename = "manifestSource")]
    pub manifest_source: String,
    #[serde(rename = "warmedApps")]
    pub warmed_apps: Vec<String>,
    #[serde(rename = "failedApps")]
    pub failed_apps: Vec<String>,
    #[serde(rename = "errorSummary")]
    pub error_summary: Vec<String>,
}

#[derive(Debug, Clone)]
struct HostWarmupStatus {
    phase: String,
    manifest_path: String,
    manifest_source: String,
    warmed_apps: Vec<String>,
    failed_apps: Vec<String>,
    error_summary: Vec<String>,
}

impl Default for HostWarmupStatus {
    fn default() -> Self {
        Self {
            phase: "starting".to_string(),
            manifest_path: String::new(),
            manifest_source: String::new(),
            warmed_apps: Vec::new(),
            failed_apps: Vec::new(),
            error_summary: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
struct HostWarmupExecution {
    warmed_apps: Vec<String>,
    failed_apps: Vec<String>,
    error_summary: Vec<String>,
}

fn host_warmup_status() -> &'static Mutex<HostWarmupStatus> {
    static STATUS: OnceLock<Mutex<HostWarmupStatus>> = OnceLock::new();
    STATUS.get_or_init(|| Mutex::new(HostWarmupStatus::default()))
}

fn replace_status(status: HostWarmupStatus) {
    if let Ok(mut guard) = host_warmup_status().lock() {
        *guard = status;
    }
}

fn update_status(update: impl FnOnce(&mut HostWarmupStatus)) {
    if let Ok(mut guard) = host_warmup_status().lock() {
        update(&mut guard);
    }
}

fn status_snapshot() -> HostReadyResponse {
    let snapshot = host_warmup_status()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    HostReadyResponse {
        ready: matches!(snapshot.phase.as_str(), "ready" | "skipped"),
        phase: snapshot.phase,
        manifest_path: snapshot.manifest_path,
        manifest_source: snapshot.manifest_source,
        warmed_apps: snapshot.warmed_apps,
        failed_apps: snapshot.failed_apps,
        error_summary: snapshot.error_summary,
    }
}

fn manifest_path_for(source_root: &Path) -> PathBuf {
    source_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL)
}

fn manifest_source_label(source_root: &Path) -> &'static str {
    if manifest_path_for(source_root).is_file() {
        "runtime_manifest"
    } else {
        "workspace_config_fallback"
    }
}

fn warmup_compile(
    state: &AppState,
    app_id: &str,
    scene: Option<String>,
    preview_target: Option<String>,
    components_root: &Path,
) -> Result<(), String> {
    compile_app_with_cache(
        state,
        app_id,
        CompileOptions {
            scene,
            preview_target,
        },
        components_root,
    )
    .map(|_| ())
    .map_err(|failure| failure.error.to_string())
}

fn warmup_scene(
    state: &AppState,
    app_id: &str,
    scene_id: &str,
    components_root: &Path,
) -> Result<(), String> {
    let scene_id = scene_id.trim();
    if scene_id.is_empty() {
        return Ok(());
    }
    let outcome = compile_app_with_cache(
        state,
        app_id,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: None,
        },
        components_root,
    )
    .map_err(|failure| failure.error.to_string())?;
    let active_scene = outcome
        .compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if active_scene != scene_id {
        return Err(format!(
            "scene `{scene_id}` warmup resolved to `{}`",
            if active_scene.is_empty() {
                "<none>"
            } else {
                active_scene
            }
        ));
    }
    Ok(())
}

fn warmup_focus_targets(
    state: &AppState,
    app: &RuntimeWarmupApp,
    components_root: &Path,
) -> Result<(), String> {
    for focus in &app.focuses {
        let focus = focus.trim();
        if focus.is_empty() {
            continue;
        }
        let preview_target = Some(focus.to_string());
        warmup_compile(
            state,
            app.app_id.as_str(),
            None,
            preview_target.clone(),
            components_root,
        )?;
        for scene in &app.scenes {
            let scene = scene.trim();
            if scene.is_empty() {
                continue;
            }
            warmup_compile(
                state,
                app.app_id.as_str(),
                Some(scene.to_string()),
                preview_target.clone(),
                components_root,
            )?;
        }
    }
    Ok(())
}

fn normalized_warmup_metric_ids(request: &RuntimeWarmupDatasetRequest) -> Vec<String> {
    let mut metric_ids = request
        .metric_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if let Some(metric_id) = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        metric_ids.push(metric_id.to_string());
    }
    metric_ids.sort();
    metric_ids.dedup();
    metric_ids
}

fn require_import_snapshot(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
    label: &str,
) -> Result<(), String> {
    if resolve_data_snapshot_import_entry(app_root, source_path, sheet, header_row).is_some() {
        return Ok(());
    }
    Err(format!(
        "{label} requires published import snapshot for `{source_path}` (sheet=`{}`, header_row={header_row}); run `mei-host-web export data-snapshots --app <app>` before serving",
        sheet.unwrap_or("")
    ))
}

fn validate_dataset_import_hit(
    source_kind: &str,
    perf: &BTreeMap<String, u64>,
    label: &str,
) -> Result<(), String> {
    if !matches!(source_kind.trim(), "xlsx" | "xls") {
        return Ok(());
    }
    if perf.get("dataset_import_artifact_hit").copied().unwrap_or(0) == 1 {
        return Ok(());
    }
    Err(format!(
        "{label} expected dataset_import_artifact_hit=1 for xlsx source during warmup"
    ))
}

fn warmup_metric_group_request(
    app_id: &str,
    request: &RuntimeWarmupDatasetRequest,
    compile_outcome: &crate::http::compile_cache::CompileWithCacheOutcome,
    app_root: &Path,
) -> Result<(), String> {
    let metric_ids = normalized_warmup_metric_ids(request);
    if metric_ids.is_empty() {
        return Ok(());
    }
    let active_scene = compile_outcome
        .compiled
        .active_scene
        .as_deref()
        .unwrap_or_default();
    let active_target = compile_outcome.compiled.active_target_file.as_str();
    let filters = normalize_query_filters(&BTreeMap::new());
    let search = normalize_query_search(None);
    let effective_query_state = query_state_from_request(&filters, search.as_deref(), None);
    let access_plan = plan_access_metric_eval_for_ids(
        &compile_outcome.compiled,
        request.dataset_id.trim(),
        &metric_ids,
    )
    .map_err(|error| error.to_string())?;
    let runtime_workset = runtime_metric_workset(
        &access_plan.owner.id,
        &access_plan.request_metric_ids,
        access_plan.owner_dataset,
    );
    let requested_eval_metric_ids = runtime_workset
        .eval_metric_ids
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        &compile_outcome.compiled,
        &access_plan.owner.id,
        &runtime_workset.defs_for_hydrate,
    );
    let query = collect_all_query_options(&effective_query_state);
    let response_cache_key = metric_response_cache_scope_key(
        app_id,
        active_scene,
        Some(active_target),
        &access_plan.owner.id,
        &query,
        &compile_outcome.compile_revision,
        &dependency_revision_key,
        &[],
    );
    let request_all_metrics = access_plan.request_metric_ids.is_empty();
    let eval_outcome = evaluate_runtime_metrics_from_plan(
        &compile_outcome.compiled,
        app_root,
        &access_plan,
        active_scene,
        Some(active_target),
        &effective_query_state,
        &[],
        RuntimeMetricEvalMode::WithDag,
        request_all_metrics,
    )
    .map_err(|error| error.to_string())?;
    validate_dataset_import_hit(
        access_plan.primary_dataset.source.kind.as_str(),
        &eval_outcome.query_perf,
        &format!(
            "warmup metric group `{}` ({})",
            request.dataset_id,
            metric_ids.join(",")
        ),
    )?;
    store_cached_metric_response(
        response_cache_key.clone(),
        eval_outcome.total_rows,
        &eval_outcome.metrics_map,
        &requested_eval_metric_ids,
        request_all_metrics,
    );
    store_metric_response_result_artifact(
        app_root,
        &response_cache_key,
        eval_outcome.total_rows,
        &eval_outcome.metrics_map,
        &requested_eval_metric_ids,
        request_all_metrics,
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn warmup_dataset_request(
    state: &AppState,
    app_id: &str,
    default_scene: Option<&str>,
    request: &RuntimeWarmupDatasetRequest,
    components_root: &Path,
) -> Result<(), String> {
    let dataset_id = request.dataset_id.trim();
    if dataset_id.is_empty() {
        return Ok(());
    }
    let scene_id = request
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(default_scene.map(str::trim).filter(|value| !value.is_empty()))
        .ok_or_else(|| format!("dataset `{dataset_id}` warmup requires a scene_id"))?;
    let preview_target = request
        .focus
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let compile_outcome = compile_app_with_cache(
        state,
        app_id,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target,
        },
        components_root,
    )
    .map_err(|failure| failure.error.to_string())?;
    let resource = locate_dataset_resource(&compile_outcome.compiled, dataset_id)
        .map_err(|error| error.to_string())?;
    let dataset = resource
        .dataset
        .as_ref()
        .ok_or_else(|| format!("resource `{}` is not a dataset", resource.id))?;
    let app_root = resolve_app_root(state.source_root.as_path(), app_id);
    if matches!(dataset.source.kind.trim(), "xlsx" | "xls") {
        require_import_snapshot(
            app_root.as_path(),
            dataset.source.path.as_str(),
            dataset.source.sheet.as_deref(),
            dataset.source.header_row.unwrap_or(1).max(1) as usize,
            &format!("warmup dataset `{}`", resource.id),
        )?;
    }
    let warm_query = super::datasets::DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: None,
        filters: Default::default(),
        collect_all: true,
        ..Default::default()
    };
    let metric_ids = normalized_warmup_metric_ids(request);
    if metric_ids.len() > 1 {
        warmup_metric_group_request(app_id, request, &compile_outcome, app_root.as_path())?;
    } else if let Some(metric_id) = metric_ids.first() {
        let result = super::datasets::query_metric_dataframe(
            &compile_outcome.compiled,
            &app_root,
            dataset_id,
            metric_id,
            Some(scene_id),
            request.focus.as_deref(),
            &compile_outcome.compile_revision,
            warm_query,
            None,
            Vec::new(),
        )
        .map_err(|error| error.to_string())?;
        validate_dataset_import_hit(
            dataset.source.kind.as_str(),
            &result.perf,
            &format!("warmup metric `{metric_id}` from `{}`", resource.id),
        )?;
    } else {
        let result = super::datasets::query_dataset_rows(&app_root, dataset, warm_query)
            .map_err(|error| error.to_string())?;
        validate_dataset_import_hit(
            dataset.source.kind.as_str(),
            &result.perf,
            &format!("warmup dataset rows `{}`", resource.id),
        )?;
    }
    Ok(())
}

fn warmup_xlsx_sources(app_root: &Path, sources: &[RuntimeWarmupXlsxSource]) -> Result<(), String> {
    for source in sources {
        let path = source.path.trim();
        if path.is_empty() {
            continue;
        }
        let header_row = source.header_row.unwrap_or(1).max(1);
        require_import_snapshot(
            app_root,
            path,
            source.sheet.as_deref(),
            header_row,
            &format!("warmup xlsx `{path}`"),
        )?;
        cached_load_xlsx_table_snapshot(
            app_root,
            path,
            source.sheet.as_deref(),
            header_row,
        )
        .map_err(|error| format!("xlsx warmup `{path}` failed: {error}"))?;
    }
    Ok(())
}

fn warmup_app(
    state: &AppState,
    app: &RuntimeWarmupApp,
    components_root: &Path,
) -> Result<(), String> {
    for scene in &app.scenes {
        warmup_scene(state, app.app_id.as_str(), scene, components_root)?;
    }
    warmup_focus_targets(state, app, components_root)?;
    for request in &app.datasets {
        warmup_dataset_request(
            state,
            app.app_id.as_str(),
            app.default_scene.as_deref(),
            request,
            components_root,
        )?;
    }
    let app_root = resolve_app_root(state.source_root.as_path(), app.app_id.as_str());
    warmup_xlsx_sources(app_root.as_path(), app.xlsx_sources.as_slice())?;
    Ok(())
}

fn run_startup_warmup(state: &AppState, manifest: &RuntimeWarmupManifest) -> HostWarmupExecution {
    let mut execution = HostWarmupExecution::default();
    let components_root = mei_lang_toolchain::resolve_components_root(state.source_root.as_path());
    for app in &manifest.apps {
        match warmup_app(state, app, components_root.as_path()) {
            Ok(()) => execution.warmed_apps.push(app.app_id.clone()),
            Err(error) => {
                execution.failed_apps.push(app.app_id.clone());
                execution
                    .error_summary
                    .push(format!("{}: {error}", app.app_id));
            }
        }
    }
    execution
}

pub(crate) fn schedule_startup_warmup(state: AppState) {
    let manifest_path = manifest_path_for(state.source_root.as_path());
    let manifest_path_display = manifest_path.display().to_string();
    if cfg!(test) || super::compile_cache::env_flag_enabled("MEI_DISABLE_HOST_WARMUP") {
        replace_status(HostWarmupStatus {
            phase: "skipped".to_string(),
            manifest_path: manifest_path_display,
            manifest_source: "disabled".to_string(),
            warmed_apps: Vec::new(),
            failed_apps: Vec::new(),
            error_summary: Vec::new(),
        });
        return;
    }

    let manifest_source = manifest_source_label(state.source_root.as_path()).to_string();
    let manifest = match resolve_runtime_warmup_manifest(state.source_root.as_path()) {
        Ok(Some(manifest)) => manifest,
        Ok(None) => {
            replace_status(HostWarmupStatus {
                phase: "skipped".to_string(),
                manifest_path: manifest_path_display,
                manifest_source,
                warmed_apps: Vec::new(),
                failed_apps: Vec::new(),
                error_summary: vec!["warmup disabled in workspace config".to_string()],
            });
            return;
        }
        Err(error) => {
            replace_status(HostWarmupStatus {
                phase: "failed".to_string(),
                manifest_path: manifest_path_display,
                manifest_source,
                warmed_apps: Vec::new(),
                failed_apps: Vec::new(),
                error_summary: vec![error.to_string()],
            });
            return;
        }
    };

    if !manifest.enabled {
        replace_status(HostWarmupStatus {
            phase: "skipped".to_string(),
            manifest_path: manifest_path_display,
            manifest_source,
            warmed_apps: Vec::new(),
            failed_apps: Vec::new(),
            error_summary: Vec::new(),
        });
        return;
    }

    if manifest_source == "workspace_config_fallback" {
        tracing::info!(
            manifest_path = %manifest_path_display,
            apps = manifest.apps.len(),
            "startup warmup using workspace config fallback (runtime manifest missing)"
        );
    }

    replace_status(HostWarmupStatus {
        phase: "running".to_string(),
        manifest_path: manifest_path_display,
        manifest_source,
        warmed_apps: Vec::new(),
        failed_apps: Vec::new(),
        error_summary: Vec::new(),
    });

    tokio::spawn(async move {
        let warmup = tokio::task::spawn_blocking(move || run_startup_warmup(&state, &manifest)).await;
        match warmup {
            Ok(execution) => {
                let phase = if execution.failed_apps.is_empty() {
                    "ready"
                } else {
                    "failed"
                };
                update_status(|status| {
                    status.phase = phase.to_string();
                    status.warmed_apps = execution.warmed_apps;
                    status.failed_apps = execution.failed_apps;
                    status.error_summary = execution.error_summary;
                });
            }
            Err(error) => {
                update_status(|status| {
                    status.phase = "failed".to_string();
                    status
                        .error_summary
                        .push(format!("warmup task join failed: {error}"));
                });
            }
        }
    });
}

pub async fn api_host_ready() -> impl IntoResponse {
    let response = status_snapshot();
    let status = if response.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(response))
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostHeartbeatResponse {
    #[serde(rename = "buildVersion")]
    pub build_version: String,
    pub ready: bool,
    pub phase: String,
}

pub async fn api_host_heartbeat() -> impl IntoResponse {
    let ready = status_snapshot();
    Json(HostHeartbeatResponse {
        build_version: crate::build_info::BUILD_VERSION.to_string(),
        ready: ready.ready,
        phase: ready.phase,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn host_heartbeat_response_includes_build_version() {
        let ready = status_snapshot();
        let heartbeat = HostHeartbeatResponse {
            build_version: crate::build_info::BUILD_VERSION.to_string(),
            ready: ready.ready,
            phase: ready.phase,
        };
        assert!(!heartbeat.build_version.is_empty());
        assert_eq!(heartbeat.build_version, crate::build_info::BUILD_VERSION);
    }

    #[test]
    fn manifest_source_label_distinguishes_runtime_file_and_fallback() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-warmup-source-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&root).expect("create temp root");
        assert_eq!(
            manifest_source_label(root.as_path()),
            "workspace_config_fallback"
        );

        let runtime_dir = root.join(".mei/runtime");
        fs::create_dir_all(&runtime_dir).expect("create runtime dir");
        fs::write(
            runtime_dir.join("warmup-manifest.json"),
            r#"{"enabled":false,"apps":[]}"#,
        )
        .expect("write manifest");
        assert_eq!(manifest_source_label(root.as_path()), "runtime_manifest");

        let _ = fs::remove_dir_all(root);
    }
}
