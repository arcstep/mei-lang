use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use mei_lang_kernel::{
    locate_dataset_resource, resolve_app_root, CompileOptions, RuntimeWarmupApp,
    RuntimeWarmupDatasetRequest, RuntimeWarmupManifest, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};
use serde::Serialize;

use crate::http::compile_cache::compile_app_with_cache;
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostReadyResponse {
    pub ready: bool,
    pub phase: String,
    #[serde(rename = "manifestPath")]
    pub manifest_path: String,
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
    warmed_apps: Vec<String>,
    failed_apps: Vec<String>,
    error_summary: Vec<String>,
}

impl Default for HostWarmupStatus {
    fn default() -> Self {
        Self {
            phase: "starting".to_string(),
            manifest_path: String::new(),
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
        warmed_apps: snapshot.warmed_apps,
        failed_apps: snapshot.failed_apps,
        error_summary: snapshot.error_summary,
    }
}

fn manifest_path_for(source_root: &Path) -> PathBuf {
    source_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL)
}

fn load_runtime_warmup_manifest(source_root: &Path) -> Result<Option<RuntimeWarmupManifest>, String> {
    let manifest_path = manifest_path_for(source_root);
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read warmup manifest {}: {error}", manifest_path.display()))?;
    let manifest = serde_json::from_str::<RuntimeWarmupManifest>(&raw).map_err(|error| {
        format!(
            "parse warmup manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    Ok(Some(manifest))
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
    let compile_outcome = compile_app_with_cache(
        state,
        app_id,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: None,
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
    let warm_query = super::datasets::DatasetQueryOptions {
        page: 1,
        page_size: 20,
        search: None,
        filters: Default::default(),
        collect_all: false,
        ..Default::default()
    };
    if let Some(metric_id) = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        super::datasets::query_metric_dataframe(
            &compile_outcome.compiled,
            &app_root,
            dataset_id,
            metric_id,
            Some(scene_id),
            None,
            &compile_outcome.compile_revision,
            warm_query,
            None,
            Vec::new(),
        )
        .map_err(|error| error.to_string())?;
    } else {
        super::datasets::query_dataset_rows(&app_root, dataset, warm_query)
            .map_err(|error| error.to_string())?;
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
    for request in &app.datasets {
        warmup_dataset_request(
            state,
            app.app_id.as_str(),
            app.default_scene.as_deref(),
            request,
            components_root,
        )?;
    }
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
            warmed_apps: Vec::new(),
            failed_apps: Vec::new(),
            error_summary: Vec::new(),
        });
        return;
    }

    let manifest = match load_runtime_warmup_manifest(state.source_root.as_path()) {
        Ok(Some(manifest)) => manifest,
        Ok(None) => {
            replace_status(HostWarmupStatus {
                phase: "skipped".to_string(),
                manifest_path: manifest_path_display,
                warmed_apps: Vec::new(),
                failed_apps: Vec::new(),
                error_summary: vec!["warmup manifest missing; startup warmup skipped".to_string()],
            });
            return;
        }
        Err(error) => {
            replace_status(HostWarmupStatus {
                phase: "failed".to_string(),
                manifest_path: manifest_path_display,
                warmed_apps: Vec::new(),
                failed_apps: Vec::new(),
                error_summary: vec![error],
            });
            return;
        }
    };

    if !manifest.enabled {
        replace_status(HostWarmupStatus {
            phase: "skipped".to_string(),
            manifest_path: manifest_path_display,
            warmed_apps: Vec::new(),
            failed_apps: Vec::new(),
            error_summary: Vec::new(),
        });
        return;
    }

    replace_status(HostWarmupStatus {
        phase: "running".to_string(),
        manifest_path: manifest_path_display,
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
