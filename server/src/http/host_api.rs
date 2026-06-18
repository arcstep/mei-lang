use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use mei_lang_kernel::{
    locate_dataset_resource, resolve_app_root, resolve_runtime_warmup_manifest, CompileOptions,
    RuntimeWarmupApp, RuntimeWarmupDatasetRequest, RuntimeWarmupManifest,
    WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
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
            request.focus.as_deref(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
