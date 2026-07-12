use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mei_lang_kernel::{
    app_env_dir, attach_build_generation, clean_env_generations, discover_apps,
    load_workspace_config, read_build_manifest, read_links_state,
    resolve_app_build_generation_from_current, resolve_app_root, write_links_state, BuildManifest,
    LinksState, RuntimeMode, RuntimePlan,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::build_ops::{
    begin_ops_job, finish_ops_job_failure, finish_ops_job_success, import_with_options,
    update_ops_app_progress, update_ops_job_generation, update_ops_job_phase,
};
use crate::route_lifecycle::{
    cleanup_policy_from_manifest, execute_rollback, garbage_collect_instances,
    launch_candidates_and_cutover, register_candidates_on_manifest, RollbackRequest,
};
use crate::state::{CleanupPreviewState, HostEvent, HostHttpState, SharedState};

const CLEANUP_PREVIEW_TTL_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBuildsView {
    pub revision: String,
    pub apps: Vec<String>,
    pub active: Option<String>,
    pub candidate: Option<String>,
    pub previous: Option<String>,
    pub retain_build_generations: Option<u32>,
    pub generations: Vec<GenerationView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationView {
    pub generation: String,
    pub coherent: bool,
    pub active: bool,
    pub candidate: bool,
    pub previous: bool,
    pub bytes: u64,
    pub created_at: Option<String>,
    pub toolchain_digest: Option<String>,
    pub config_digest: Option<String>,
    pub protected_reasons: Vec<String>,
    pub apps: Vec<GenerationAppView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationAppView {
    pub app_id: String,
    pub available: bool,
    pub current: bool,
    pub valid: bool,
    pub path: String,
    pub bytes: u64,
    pub created_at: Option<String>,
    pub manifest: Option<BuildManifest>,
    pub error: Option<String>,
}

pub fn workspace_builds_view(
    workspace: &Path,
    running_generation: Option<&str>,
) -> anyhow::Result<WorkspaceBuildsView> {
    let apps = lifecycle_app_ids(workspace);
    let links = read_links_state(workspace).unwrap_or_default();
    let current_by_app = current_generations(workspace, apps.as_slice());
    let mut generation_ids = BTreeSet::new();
    for app_id in &apps {
        let env_root = resolve_app_root(workspace, app_id.as_str()).join("env");
        let Ok(entries) = fs::read_dir(env_root) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let generation = entry.file_name().to_string_lossy().to_string();
            if generation.starts_with("WS-") {
                generation_ids.insert(generation);
            }
        }
    }
    if let Some(candidate) = links.build.candidate.as_deref() {
        generation_ids.insert(candidate.to_string());
    }
    if let Some(previous) = links.build.previous.as_deref() {
        generation_ids.insert(previous.to_string());
    }

    let mut generations = generation_ids
        .into_iter()
        .map(|generation| {
            generation_view(
                workspace,
                apps.as_slice(),
                &current_by_app,
                &links,
                running_generation,
                generation,
            )
        })
        .collect::<Vec<_>>();
    generations.sort_by(|left, right| right.generation.cmp(&left.generation));
    let active = generations
        .iter()
        .find(|generation| generation.active)
        .map(|generation| generation.generation.clone());
    let retain_build_generations = load_workspace_config(workspace)
        .build
        .retain_build_generations;
    let mut view = WorkspaceBuildsView {
        revision: String::new(),
        apps,
        active,
        candidate: links.build.candidate,
        previous: links.build.previous,
        retain_build_generations,
        generations,
    };
    view.revision = view_revision(&view);
    Ok(view)
}

fn lifecycle_app_ids(workspace: &Path) -> Vec<String> {
    let mut apps = discover_apps(workspace)
        .unwrap_or_default()
        .into_iter()
        .map(|app| app.id)
        .collect::<BTreeSet<_>>();
    let control_path = workspace.join("deploy/state/host-control.json");
    if let Ok(bytes) = fs::read(control_path) {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(control_apps) = value
                .get("lastSuccessfulApply")
                .and_then(|entry| entry.get("apps"))
                .and_then(|entry| entry.as_array())
            {
                apps.extend(
                    control_apps
                        .iter()
                        .filter_map(|app| app.as_str())
                        .map(str::to_string),
                );
            }
        }
    }
    apps.into_iter().collect()
}

fn current_generations(workspace: &Path, apps: &[String]) -> BTreeMap<String, Option<String>> {
    apps.iter()
        .map(|app_id| {
            let app_root = resolve_app_root(workspace, app_id.as_str());
            (
                app_id.clone(),
                resolve_app_build_generation_from_current(app_root.as_path()).ok(),
            )
        })
        .collect()
}

fn generation_view(
    workspace: &Path,
    app_ids: &[String],
    current_by_app: &BTreeMap<String, Option<String>>,
    links: &LinksState,
    running_generation: Option<&str>,
    generation: String,
) -> GenerationView {
    let mut bytes = 0;
    let mut created_at = None::<String>;
    let mut toolchains = BTreeSet::new();
    let mut config_digests = BTreeSet::new();
    let apps = app_ids
        .iter()
        .map(|app_id| {
            let path = app_env_dir(
                resolve_app_root(workspace, app_id.as_str()).as_path(),
                generation.as_str(),
            );
            let available = path.is_dir();
            let app_bytes = if available {
                directory_bytes(path.as_path())
            } else {
                0
            };
            bytes += app_bytes;
            let current =
                current_by_app.get(app_id).and_then(Clone::clone).as_deref() == Some(&generation);
            let (manifest, error) = if available {
                match read_build_manifest(path.as_path()) {
                    Ok(Some(manifest)) => (Some(manifest), None),
                    Ok(None) => (None, Some("BUILD.json missing".to_string())),
                    Err(error) => (None, Some(format!("BUILD.json invalid: {error}"))),
                }
            } else {
                (None, Some("generation directory missing".to_string()))
            };
            let bundle_path = path
                .join("build/exchange")
                .join(format!("{app_id}.meibundle"));
            let valid = manifest.as_ref().is_some_and(|manifest| {
                manifest.env_version == generation
                    && manifest.app_id == *app_id
                    && path.join("build").is_dir()
                    && bundle_path.is_file()
            });
            if let Some(manifest) = manifest.as_ref() {
                created_at = Some(
                    created_at
                        .as_deref()
                        .map(|current| current.max(manifest.finished_at.as_str()).to_string())
                        .unwrap_or_else(|| manifest.finished_at.clone()),
                );
                toolchains.insert(manifest.toolchain_version.clone());
                if let Some(digest) = manifest.config_digest.as_ref() {
                    config_digests.insert(digest.clone());
                }
            }
            GenerationAppView {
                app_id: app_id.clone(),
                available,
                current,
                valid,
                path: path.to_string_lossy().to_string(),
                bytes: app_bytes,
                created_at: manifest
                    .as_ref()
                    .map(|manifest| manifest.finished_at.clone()),
                manifest,
                error: if valid {
                    None
                } else if error.is_some() {
                    error
                } else if !bundle_path.is_file() {
                    Some(format!("bundle missing: {}", bundle_path.display()))
                } else {
                    Some("BUILD.json metadata mismatch".to_string())
                },
            }
        })
        .collect::<Vec<_>>();
    let active = !apps.is_empty() && apps.iter().all(|app| app.current);
    let coherent = !apps.is_empty() && apps.iter().all(|app| app.available && app.valid);
    let mut protected_reasons = apps
        .iter()
        .filter(|app| app.current)
        .map(|app| format!("current:{}", app.app_id))
        .collect::<Vec<_>>();
    if links.build.candidate.as_deref() == Some(generation.as_str()) {
        protected_reasons.push("candidate".to_string());
    }
    if links.build.previous.as_deref() == Some(generation.as_str()) {
        protected_reasons.push("previous".to_string());
    }
    if running_generation == Some(generation.as_str()) {
        protected_reasons.push("ops-job".to_string());
    }
    if let Some(control) = mei_host_core::read_host_control_state(workspace) {
        let route_reasons = crate::route_lifecycle::collect_bundle_protections(
            workspace,
            &control.launch_manifest,
            running_generation,
        );
        if let Some(extra) = route_reasons.get(generation.as_str()) {
            for reason in extra {
                if !protected_reasons.contains(reason) {
                    protected_reasons.push(reason.clone());
                }
            }
        }
    }
    GenerationView {
        generation: generation.clone(),
        coherent,
        active,
        candidate: links.build.candidate.as_deref() == Some(generation.as_str())
            || protected_reasons.iter().any(|r| r == "route:candidate"),
        previous: links.build.previous.as_deref() == Some(generation.as_str())
            || protected_reasons.iter().any(|r| r == "route:previous"),
        bytes,
        created_at,
        toolchain_digest: single_value(toolchains),
        config_digest: single_value(config_digests),
        protected_reasons,
        apps,
    }
}

fn single_value(values: BTreeSet<String>) -> Option<String> {
    (values.len() == 1)
        .then(|| values.into_iter().next())
        .flatten()
}

fn view_revision(view: &WorkspaceBuildsView) -> String {
    let bytes = serde_json::to_vec(&json!({
        "apps": view.apps,
        "active": view.active,
        "candidate": view.candidate,
        "previous": view.previous,
        "retainBuildGenerations": view.retain_build_generations,
        "generations": view.generations,
    }))
    .unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

fn directory_bytes(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return metadata.len();
    }
    if !metadata.is_dir() {
        return 0;
    }
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| directory_bytes(entry.path().as_path()))
        .sum()
}

pub async fn api_host_builds(State(state): State<SharedState>) -> Response {
    let (workspace, running_generation) = {
        let guard = state.read().expect("state lock");
        (
            guard.ctx.workspace_root.clone(),
            guard
                .ops_job
                .as_ref()
                .and_then(|job| job.generation.clone()),
        )
    };
    match workspace_builds_view(workspace.as_path(), running_generation.as_deref()) {
        Ok(view) => Json(view).into_response(),
        Err(error) => lifecycle_error(StatusCode::INTERNAL_SERVER_ERROR, "builds_failed", error),
    }
}

pub async fn api_host_build_activate(
    State(http): State<HostHttpState>,
    AxumPath(generation): AxumPath<String>,
) -> Response {
    start_activation(http, generation, false).await
}

pub async fn api_host_build_rollback(
    State(http): State<HostHttpState>,
    AxumPath(generation): AxumPath<String>,
) -> Response {
    start_activation(http, generation, true).await
}

async fn start_activation(
    http: HostHttpState,
    generation: String,
    rollback: bool,
) -> Response {
    let state = http.shell.clone();
    let (workspace, running_generation) = {
        let guard = state.read().expect("state lock");
        if guard
            .ops_job
            .as_ref()
            .is_some_and(crate::build_ops::OpsJobState::is_running)
        {
            return lifecycle_conflict("another host-shell ops job is already running");
        }
        (
            guard.ctx.workspace_root.clone(),
            guard
                .ops_job
                .as_ref()
                .and_then(|job| job.generation.clone()),
        )
    };
    let view = match workspace_builds_view(workspace.as_path(), running_generation.as_deref()) {
        Ok(view) => view,
        Err(error) => {
            return lifecycle_error(StatusCode::INTERNAL_SERVER_ERROR, "builds_failed", error);
        }
    };
    if rollback {
        // Prefer LaunchManifest routes.previous; fall back to links.previous generation check.
        let has_route_previous = {
            let guard = state.read().expect("state lock");
            guard.launch_manifest.routes.values().any(|route| {
                route.previous.as_ref().is_some_and(|id| id.contains(&generation))
            })
        };
        if !has_route_previous && view.previous.as_deref() != Some(generation.as_str()) {
            return lifecycle_conflict(
                "rollback target is not route.previous / links.previous",
            );
        }
    }
    let Some(target) = view
        .generations
        .iter()
        .find(|entry| entry.generation == generation)
    else {
        return lifecycle_error_message(
            StatusCode::NOT_FOUND,
            "generation_not_found",
            "generation not found",
        );
    };
    if !target.coherent {
        return lifecycle_conflict(
            "generation is not coherent for all active profile/discover apps",
        );
    }
    {
        let mut guard = state.write().expect("state lock");
        let kind = if rollback {
            "generation-rollback"
        } else {
            "generation-activate"
        };
        if let Err(error) = begin_ops_job(&mut guard, kind) {
            return lifecycle_conflict(error.as_str());
        }
        update_ops_job_generation(&mut guard, generation.as_str());
    }
    let response_generation = generation.clone();
    let kind = if rollback { "rollback" } else { "activate" };
    tokio::spawn(async move {
        let result = if rollback {
            run_generation_rollback(&http, generation.as_str()).await
        } else {
            run_generation_activate(&http, generation.as_str()).await
        };
        let mut guard = http.shell.write().expect("state lock");
        match result {
            Ok(message) => finish_ops_job_success(&mut guard, message),
            Err(error) => finish_ops_job_failure(&mut guard, error.to_string()),
        }
    });
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "accepted": true,
            "kind": format!("generation-{kind}"),
            "generation": response_generation,
        })),
    )
        .into_response()
}

async fn run_generation_activate(
    http: &HostHttpState,
    generation: &str,
) -> anyhow::Result<String> {
    let state = &http.shell;
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    set_phase(
        state,
        "validating",
        "validating coherent workspace generation",
    );
    let view = workspace_builds_view(workspace.as_path(), Some(generation))?;
    let target = view
        .generations
        .iter()
        .find(|entry| entry.generation == generation)
        .ok_or_else(|| anyhow::anyhow!("generation `{generation}` not found"))?;
    if !target.coherent {
        anyhow::bail!("generation `{generation}` is not coherent");
    }
    let app_ids = view.apps.clone();
    let runtime_plan = crate::dev_eval_scope::applied_runtime_plan(workspace.as_path())
        .or_else(|| {
            mei_host_core::read_host_control_state(workspace.as_path())
                .and_then(|control| control.runtime_plan)
        })
        .unwrap_or_else(|| {
            load_workspace_config(workspace.as_path())
                .deploy
                .effective_runtime_plan()
        });
    let profile = mei_host_core::read_host_control_state(workspace.as_path())
        .and_then(|control| control.active_profile)
        .unwrap_or(mei_host_core::ActiveProfileRef {
            id: "runtime".to_string(),
            revision: "0".to_string(),
            file: String::new(),
        });

    set_phase(
        state,
        "creating-instances",
        "creating/reusing InstanceSpecs for candidate launch",
    );
    let mut specs = Vec::new();
    for app_id in &app_ids {
        set_app(
            state,
            app_id.as_str(),
            "creating-instances",
            false,
            "preparing InstanceSpec",
        );
        let spec = instance_spec_for_generation(
            workspace.as_path(),
            app_id.as_str(),
            generation,
            &runtime_plan,
            &profile,
        );
        let _ = mei_host_core::write_instance_spec(workspace.as_path(), &spec);
        specs.push(spec);
        set_app(
            state,
            app_id.as_str(),
            "creating-instances",
            true,
            "InstanceSpec ready",
        );
    }

    let mut control = mei_host_core::read_host_control_state(workspace.as_path())
        .unwrap_or_else(mei_host_core::HostControlState::empty);
    let registered =
        register_candidates_on_manifest(control.launch_manifest.clone(), workspace.as_path(), &specs)?;
    let expected_revision = registered.revision.clone();
    control.launch_manifest = registered.clone();
    control.sync_compat_fields();
    mei_host_core::write_host_control_state(workspace.as_path(), &control)?;
    {
        let mut guard = state.write().expect("state lock");
        guard.install_launch_manifest(registered);
    }

    set_phase(
        state,
        "launching",
        "launching candidate app-runtime instances",
    );
    match launch_candidates_and_cutover(http, &specs, expected_revision.as_str()).await {
        Ok(results) => {
            // Optional compat symlink update without import/warm.
            for app_id in &app_ids {
                let _ = attach_build_generation(
                    workspace.as_path(),
                    std::slice::from_ref(app_id),
                    generation,
                );
            }
            let mut links = read_links_state(workspace.as_path()).unwrap_or_default();
            links.build.previous = view.active.clone().filter(|prev| prev != generation);
            if links.build.candidate.as_deref() == Some(generation) {
                links.build.candidate = None;
            }
            links.toolchain.active = target.toolchain_digest.clone();
            let _ = write_links_state(workspace.as_path(), &links);

            emit(
                state,
                "generation-activated",
                json!({
                    "generation": generation,
                    "apps": app_ids,
                    "cutovers": results,
                }),
            );
            Ok(format!(
                "workspace generation {generation} activated via route cutover for {} app(s)",
                app_ids.len()
            ))
        }
        Err(error) => {
            // Failure: do not cut routes (cutover itself rolls back); stop candidates.
            let _ = crate::route_lifecycle::stop_instances(
                http,
                specs.iter().map(|spec| spec.instance_id.as_str()),
            )
            .await;
            Err(anyhow::anyhow!(
                "generation activate failed before/during cutover: {}",
                error.message()
            ))
        }
    }
}

async fn run_generation_rollback(
    http: &HostHttpState,
    generation: &str,
) -> anyhow::Result<String> {
    let state = &http.shell;
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    set_phase(state, "rolling-back", "rolling routes back to previous");
    let app_ids = {
        let guard = state.read().expect("state lock");
        guard
            .launch_manifest
            .routes
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    let app_ids = if app_ids.is_empty() {
        lifecycle_app_ids(workspace.as_path())
    } else {
        app_ids
    };
    let mut rolled = Vec::new();
    for app_id in &app_ids {
        let body = RollbackRequest {
            expected_manifest_revision: None,
        };
        match execute_rollback(http, app_id.as_str(), &body).await {
            Ok(result) => {
                if let Some(spec) =
                    mei_host_core::read_instance_spec(workspace.as_path(), result.active.as_str())
                {
                    if spec.bundle.generation == generation {
                        rolled.push(app_id.clone());
                    }
                } else if result.active.contains(generation) {
                    rolled.push(app_id.clone());
                }
            }
            Err(error) => {
                tracing::warn!(
                    app_id = %app_id,
                    error = %error.message(),
                    "route rollback skipped for app"
                );
            }
        }
    }
    if rolled.is_empty() {
        // Fall back to legacy symlink rollback when no LaunchManifest previous exists.
        return run_legacy_symlink_activation(state, generation, true);
    }
    emit(
        state,
        "generation-rolled-back",
        json!({"generation": generation, "apps": rolled}),
    );
    Ok(format!(
        "workspace generation {generation} rolled back via routes for {} app(s)",
        rolled.len()
    ))
}

fn instance_spec_for_generation(
    workspace: &Path,
    app_id: &str,
    generation: &str,
    runtime_plan: &RuntimePlan,
    profile: &mei_host_core::ActiveProfileRef,
) -> mei_host_core::InstanceSpec {
    let instance_id = format!("{app_id}@{generation}@{}", profile.revision);
    if let Some(existing) = mei_host_core::read_instance_spec(workspace, instance_id.as_str()) {
        if existing.bundle.generation == generation && existing.app_id == app_id {
            return existing;
        }
    }
    mei_host_core::InstanceSpec {
        schema_version: mei_host_core::SCHEMA_INSTANCE_SPEC_V1.to_string(),
        instance_id,
        app_id: app_id.to_string(),
        bundle: mei_host_core::BundleRef {
            generation: generation.to_string(),
            bundle_path: format!("apps/{app_id}/env/{generation}"),
            digest: None,
            toolchain_version: None,
            config_digest: Some(profile.revision.clone()),
        },
        config_snapshot: mei_host_core::ConfigSnapshot {
            profile_id: profile.id.clone(),
            profile_revision: profile.revision.clone(),
            profile_file: profile.file.clone(),
            runtime_plan: runtime_plan.clone(),
            default_app: Some(app_id.to_string()),
        },
        runtime_abi: env!("CARGO_PKG_VERSION").to_string(),
        data_mode_ceiling: None,
    }
}

/// Legacy symlink+import path kept for generation-rollback when LaunchManifest has no previous.
fn run_legacy_symlink_activation(
    state: &SharedState,
    generation: &str,
    rollback: bool,
) -> anyhow::Result<String> {
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    set_phase(
        state,
        "validating",
        "legacy symlink activation (no LaunchManifest previous)",
    );
    let running = Some(generation);
    let view = workspace_builds_view(workspace.as_path(), running)?;
    let target = view
        .generations
        .iter()
        .find(|entry| entry.generation == generation)
        .ok_or_else(|| anyhow::anyhow!("generation `{generation}` not found"))?;
    if !target.coherent {
        anyhow::bail!("generation `{generation}` is not coherent");
    }
    if rollback && view.previous.as_deref() != Some(generation) {
        anyhow::bail!("rollback target `{generation}` is no longer links.previous");
    }
    let app_ids = view.apps;
    let old_links = read_links_state(workspace.as_path()).unwrap_or_default();
    let old_generations = current_generations(workspace.as_path(), app_ids.as_slice());

    set_phase(
        state,
        "switching",
        "switching all workspace generation pointers",
    );
    if let Err(error) = switch_generation_pointers(
        workspace.as_path(),
        app_ids.as_slice(),
        generation,
        |_index, _app_id| Ok(()),
    ) {
        restore_generation_pointers(
            workspace.as_path(),
            app_ids.as_slice(),
            &old_generations,
            &old_links,
        )?;
        return Err(error);
    }

    let runtime_plan = crate::dev_eval_scope::applied_runtime_plan(workspace.as_path())
        .unwrap_or_else(|| {
            load_workspace_config(workspace.as_path())
                .deploy
                .effective_runtime_plan()
        });
    let result = refresh_generation_runtime(
        state,
        workspace.as_path(),
        app_ids.as_slice(),
        generation,
        &runtime_plan,
    )
    .and_then(|revisions| {
        let mut links = old_links.clone();
        links.build.previous =
            common_generation(&old_generations).filter(|previous| previous.as_str() != generation);
        if links.build.candidate.as_deref() == Some(generation) {
            links.build.candidate = None;
        }
        links.toolchain.active = target.toolchain_digest.clone();
        write_links_state(workspace.as_path(), &links)?;
        Ok(revisions)
    });

    let revisions = match result {
        Ok(revisions) => revisions,
        Err(error) => {
            set_phase(
                state,
                "restoring",
                "activation failed; restoring old pointers and runtime",
            );
            let restore = restore_generation_pointers(
                workspace.as_path(),
                app_ids.as_slice(),
                &old_generations,
                &old_links,
            )
            .and_then(|_| {
                refresh_restored_runtime(
                    workspace.as_path(),
                    app_ids.as_slice(),
                    &old_generations,
                    &runtime_plan,
                )
            });
            return match restore {
                Ok(()) => Err(error),
                Err(restore_error) => Err(anyhow::anyhow!(
                    "{error}; additionally failed to restore runtime: {restore_error}"
                )),
            };
        }
    };

    set_phase(state, "publishing", "publishing generation revision events");
    for (app_id, revision) in revisions {
        emit(
            state,
            "revision-published",
            json!({
                "appId": app_id,
                "generation": generation,
                "revision": revision,
            }),
        );
    }
    emit(
        state,
        if rollback {
            "generation-rolled-back"
        } else {
            "generation-activated"
        },
        json!({"generation": generation, "apps": app_ids}),
    );
    Ok(format!(
        "workspace generation {generation} {} for {} app(s)",
        if rollback { "rolled back" } else { "activated" },
        app_ids.len()
    ))
}

fn switch_generation_pointers<F>(
    workspace: &Path,
    app_ids: &[String],
    generation: &str,
    mut before_switch: F,
) -> anyhow::Result<()>
where
    F: FnMut(usize, &str) -> anyhow::Result<()>,
{
    for (index, app_id) in app_ids.iter().enumerate() {
        before_switch(index, app_id.as_str())?;
        attach_build_generation(workspace, std::slice::from_ref(app_id), generation)?;
    }
    Ok(())
}

fn restore_generation_pointers(
    workspace: &Path,
    app_ids: &[String],
    old_generations: &BTreeMap<String, Option<String>>,
    old_links: &LinksState,
) -> anyhow::Result<()> {
    for app_id in app_ids {
        if let Some(Some(generation)) = old_generations.get(app_id) {
            attach_build_generation(workspace, std::slice::from_ref(app_id), generation)?;
        } else {
            let current = resolve_app_root(workspace, app_id.as_str()).join("env/current");
            if current.is_symlink() || current.exists() {
                fs::remove_file(&current).or_else(|_| fs::remove_dir_all(&current))?;
            }
        }
    }
    write_links_state(workspace, old_links)?;
    Ok(())
}

fn refresh_generation_runtime(
    state: &SharedState,
    workspace: &Path,
    app_ids: &[String],
    generation: &str,
    runtime_plan: &RuntimePlan,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut revisions = BTreeMap::new();
    set_phase(
        state,
        "refreshing",
        "refreshing registries, caches, bootstrap and warm runtime targets",
    );
    for app_id in app_ids {
        set_app(
            state,
            app_id,
            "refreshing",
            false,
            "importing activated bundle",
        );
        let report = import_with_options(workspace, app_id.as_str(), None)?;
        refresh_cache_and_warm(workspace, app_id.as_str(), runtime_plan)?;
        revisions.insert(app_id.clone(), report.registry_revision);
        set_app(
            state,
            app_id,
            "refreshing",
            true,
            format!("{generation} registry/cache/bootstrap refreshed").as_str(),
        );
    }
    Ok(revisions)
}

fn refresh_restored_runtime(
    workspace: &Path,
    app_ids: &[String],
    old_generations: &BTreeMap<String, Option<String>>,
    runtime_plan: &RuntimePlan,
) -> anyhow::Result<()> {
    for app_id in app_ids {
        if old_generations.get(app_id).and_then(Clone::clone).is_none() {
            continue;
        }
        import_with_options(workspace, app_id.as_str(), None)?;
        refresh_cache_and_warm(workspace, app_id.as_str(), runtime_plan)?;
    }
    Ok(())
}

fn refresh_cache_and_warm(
    workspace: &Path,
    app_id: &str,
    runtime_plan: &RuntimePlan,
) -> anyhow::Result<()> {
    mei_host_graph::clear_assemble_cache_for_app(app_id);
    let _ = mei_host_graph::invalidate_app_eval_cache(workspace, app_id, false)?;
    if app_requires_warm(runtime_plan, app_id) {
        crate::tool_exec::run_mei_plug_ds_warmup_with_plan(
            workspace,
            app_id,
            "home",
            "all",
            None,
            Some(runtime_plan),
        )?;
    }
    Ok(())
}

fn app_requires_warm(plan: &RuntimePlan, app_id: &str) -> bool {
    if plan.default_mode == RuntimeMode::Hot {
        return true;
    }
    plan.apps
        .get(app_id)
        .or_else(|| plan.apps.get("*"))
        .is_some_and(|app| {
            app.targets
                .iter()
                .any(|target| target.mode == RuntimeMode::Hot)
                || app
                    .metric_overrides
                    .values()
                    .any(|mode| *mode == RuntimeMode::Hot)
        })
}

fn common_generation(generations: &BTreeMap<String, Option<String>>) -> Option<String> {
    let mut values = generations.values();
    let first = values.next()?.clone()?;
    values
        .all(|generation| generation.as_deref() == Some(first.as_str()))
        .then_some(first)
}

pub async fn api_host_builds_cleanup_preview(State(state): State<SharedState>) -> Response {
    let (workspace, running_generation, manifest) = {
        let guard = state.read().expect("state lock");
        if guard
            .ops_job
            .as_ref()
            .is_some_and(crate::build_ops::OpsJobState::is_running)
        {
            return lifecycle_conflict("another host-shell ops job is already running");
        }
        (
            guard.ctx.workspace_root.clone(),
            guard
                .ops_job
                .as_ref()
                .and_then(|job| job.generation.clone()),
            guard.launch_manifest.clone(),
        )
    };
    let view = match workspace_builds_view(workspace.as_path(), running_generation.as_deref()) {
        Ok(view) => view,
        Err(error) => {
            return lifecycle_error(StatusCode::INTERNAL_SERVER_ERROR, "builds_failed", error);
        }
    };
    let manifest = if manifest.revision.is_empty() {
        mei_host_core::read_host_control_state(workspace.as_path())
            .map(|control| control.launch_manifest)
            .unwrap_or(manifest)
    } else {
        manifest
    };
    let policy = cleanup_policy_from_manifest(
        workspace.as_path(),
        &manifest,
        view.retain_build_generations,
        running_generation.as_deref(),
        true,
    );
    let mut report = match clean_env_generations(workspace.as_path(), view.apps.as_slice(), &policy)
    {
        Ok(report) => report,
        Err(error) => {
            return lifecycle_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "cleanup_preview_failed",
                error,
            );
        }
    };
    let instance_removed =
        garbage_collect_instances(workspace.as_path(), &manifest, true);
    for instance_id in &instance_removed {
        report.removed.push(format!("instance:{instance_id}"));
    }
    let generated_at_ms = crate::state::current_time_ms();
    let token = format!(
        "{:x}",
        Sha256::digest(format!("{}:{generated_at_ms}", view.revision).as_bytes())
    );
    {
        let mut guard = state.write().expect("state lock");
        guard.cleanup_preview = Some(CleanupPreviewState {
            token: token.clone(),
            revision: view.revision.clone(),
            generated_at_ms,
            report: report.clone(),
        });
    }
    Json(json!({
        "previewToken": token,
        "revision": view.revision,
        "expiresAtMs": generated_at_ms + CLEANUP_PREVIEW_TTL_MS,
        "report": report,
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CleanupExecuteRequest {
    pub preview_token: String,
    pub revision: String,
}

pub async fn api_host_builds_cleanup(
    State(state): State<SharedState>,
    Json(body): Json<CleanupExecuteRequest>,
) -> Response {
    let (workspace, preview, running_generation, manifest) = {
        let guard = state.read().expect("state lock");
        if guard
            .ops_job
            .as_ref()
            .is_some_and(crate::build_ops::OpsJobState::is_running)
        {
            return lifecycle_conflict("another host-shell ops job is already running");
        }
        (
            guard.ctx.workspace_root.clone(),
            guard.cleanup_preview.clone(),
            guard
                .ops_job
                .as_ref()
                .and_then(|job| job.generation.clone()),
            guard.launch_manifest.clone(),
        )
    };
    let Some(preview) = preview else {
        return lifecycle_conflict("cleanup preview is required");
    };
    let now = crate::state::current_time_ms();
    if preview.token != body.preview_token
        || preview.revision != body.revision
        || now.saturating_sub(preview.generated_at_ms) > CLEANUP_PREVIEW_TTL_MS
    {
        return lifecycle_conflict("cleanup preview token is invalid or expired");
    }
    let current_view =
        match workspace_builds_view(workspace.as_path(), running_generation.as_deref()) {
            Ok(view) => view,
            Err(error) => {
                return lifecycle_error(StatusCode::INTERNAL_SERVER_ERROR, "builds_failed", error);
            }
        };
    if current_view.revision != preview.revision {
        return lifecycle_conflict("build generation state changed after preview");
    }
    {
        let mut guard = state.write().expect("state lock");
        if let Err(error) = begin_ops_job(&mut guard, "generation-cleanup") {
            return lifecycle_conflict(error.as_str());
        }
        guard.cleanup_preview = None;
    }
    let task_state = state.clone();
    let manifest = if manifest.revision.is_empty() {
        mei_host_core::read_host_control_state(workspace.as_path())
            .map(|control| control.launch_manifest)
            .unwrap_or(manifest)
    } else {
        manifest
    };
    tokio::spawn(async move {
        let app_ids = current_view.apps.clone();
        let policy = cleanup_policy_from_manifest(
            workspace.as_path(),
            &manifest,
            current_view.retain_build_generations,
            None,
            false,
        );
        let workspace_for_gc = workspace.clone();
        let manifest_for_gc = manifest.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut report =
                clean_env_generations(workspace.as_path(), app_ids.as_slice(), &policy)?;
            let removed_instances =
                garbage_collect_instances(workspace_for_gc.as_path(), &manifest_for_gc, false);
            for instance_id in removed_instances {
                report.removed.push(format!("instance:{instance_id}"));
            }
            Ok::<_, anyhow::Error>(report)
        })
        .await
        .map_err(|error| format!("generation cleanup task join failed: {error}"))
        .and_then(|result| result.map_err(|error| error.to_string()));
        let mut guard = task_state.write().expect("state lock");
        match result {
            Ok(report) => finish_ops_job_success(
                &mut guard,
                format!(
                    "generation cleanup removed {} directory(s), {} protected",
                    report.removed.len(),
                    report.retained.len()
                ),
            ),
            Err(error) => finish_ops_job_failure(&mut guard, error),
        }
    });
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "accepted": true,
            "kind": "generation-cleanup",
            "previewedRemoveCount": preview.report.removed.len(),
        })),
    )
        .into_response()
}

fn set_phase(state: &SharedState, phase: &str, message: &str) {
    let mut guard = state.write().expect("state lock");
    update_ops_job_phase(&mut guard, phase, message);
}

fn set_app(state: &SharedState, app_id: &str, phase: &str, completed: bool, message: &str) {
    let mut guard = state.write().expect("state lock");
    if !guard
        .ops_job
        .as_ref()
        .is_some_and(|job| job.apps.iter().any(|app| app.app_id == app_id))
    {
        if let Some(job) = guard.ops_job.as_mut() {
            job.apps.push(crate::build_ops::OpsAppProgress {
                app_id: app_id.to_string(),
                phase: "queued".to_string(),
                completed: false,
                message: None,
            });
        }
    }
    update_ops_app_progress(&mut guard, app_id, phase, completed, message);
}

fn emit(state: &SharedState, event_type: &str, payload: serde_json::Value) {
    let guard = state.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(event_type, payload));
}

fn lifecycle_conflict(message: &str) -> Response {
    lifecycle_error_message(StatusCode::CONFLICT, "generation_conflict", message)
}

fn lifecycle_error(status: StatusCode, code: &str, error: impl std::fmt::Display) -> Response {
    lifecycle_error_message(status, code, error.to_string().as_str())
}

fn lifecycle_error_message(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "code": code,
                "message": message,
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{write_build_manifest, BUILD_MANIFEST_SCHEMA};

    fn fixture_apps(workspace: &Path) -> Vec<String> {
        fs::write(
            workspace.join("workspace.json"),
            r#"{"workspace":{"id":"test","defaultApp":"app-a"}}"#,
        )
        .expect("workspace config");
        for app_id in ["app-a", "app-b"] {
            let app_root = workspace.join("apps").join(app_id);
            fs::create_dir_all(&app_root).expect("app dir");
            fs::write(
                app_root.join("app.config.json"),
                format!(r#"{{"app":{{"id":"{app_id}"}}}}"#),
            )
            .expect("app config");
        }
        vec!["app-a".to_string(), "app-b".to_string()]
    }

    fn write_generation(workspace: &Path, app_id: &str, generation: &str) {
        let env_dir = workspace
            .join("apps")
            .join(app_id)
            .join("env")
            .join(generation);
        fs::create_dir_all(env_dir.join("build/exchange")).expect("generation build");
        fs::write(
            env_dir
                .join("build/exchange")
                .join(format!("{app_id}.meibundle")),
            b"fixture",
        )
        .expect("bundle fixture");
        fs::create_dir_all(env_dir.join("var")).expect("generation var");
        write_build_manifest(
            env_dir.as_path(),
            &BuildManifest {
                schema_version: BUILD_MANIFEST_SCHEMA.to_string(),
                env_version: generation.to_string(),
                app_id: app_id.to_string(),
                toolchain_version: "test-toolchain".to_string(),
                build_generation: Some(generation.to_string()),
                workspace_version: Some("20260712".to_string()),
                config_digest: Some("config-r1".to_string()),
                source_revision: None,
                stock_revision: None,
                finished_at: "2026-07-12T00:00:00Z".to_string(),
            },
        )
        .expect("manifest");
    }

    #[test]
    fn coherent_requires_every_target_app_generation() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let apps = fixture_apps(tmp.path());
        for app_id in &apps {
            write_generation(tmp.path(), app_id, "WS-20260712.0");
        }
        write_generation(tmp.path(), "app-a", "WS-20260711.0");
        let view = workspace_builds_view(tmp.path(), None).expect("view");
        assert!(view
            .generations
            .iter()
            .find(|entry| entry.generation == "WS-20260712.0")
            .is_some_and(|entry| entry.coherent));
        assert!(
            !view
                .generations
                .iter()
                .find(|entry| entry.generation == "WS-20260711.0")
                .expect("missing generation row")
                .coherent
        );
    }

    #[test]
    fn failed_batch_switch_can_restore_all_old_pointers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let apps = fixture_apps(tmp.path());
        for app_id in &apps {
            write_generation(tmp.path(), app_id, "WS-20260711.0");
            write_generation(tmp.path(), app_id, "WS-20260712.0");
        }
        attach_build_generation(tmp.path(), apps.as_slice(), "WS-20260711.0").expect("attach old");
        let old_links = LinksState::default();
        let old = current_generations(tmp.path(), apps.as_slice());
        let result =
            switch_generation_pointers(tmp.path(), apps.as_slice(), "WS-20260712.0", |index, _| {
                if index == 1 {
                    anyhow::bail!("injected switch failure");
                }
                Ok(())
            });
        assert!(result.is_err());
        restore_generation_pointers(tmp.path(), apps.as_slice(), &old, &old_links)
            .expect("restore");
        assert!(apps.iter().all(|app_id| {
            resolve_app_build_generation_from_current(
                tmp.path().join("apps").join(app_id).as_path(),
            )
            .ok()
            .as_deref()
                == Some("WS-20260711.0")
        }));
    }

    #[test]
    fn lifecycle_view_protects_current_candidate_previous_and_running_generations() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let apps = fixture_apps(tmp.path());
        for generation in ["WS-20260710.0", "WS-20260711.0", "WS-20260712.0"] {
            for app_id in &apps {
                write_generation(tmp.path(), app_id, generation);
            }
        }
        attach_build_generation(tmp.path(), apps.as_slice(), "WS-20260711.0").expect("current");
        write_links_state(
            tmp.path(),
            &LinksState {
                build: mei_lang_kernel::BuildLinks {
                    candidate: Some("WS-20260712.0".to_string()),
                    previous: Some("WS-20260710.0".to_string()),
                },
                ..LinksState::default()
            },
        )
        .expect("links");

        let view = workspace_builds_view(tmp.path(), Some("WS-20260712.0")).expect("builds view");
        let current = view
            .generations
            .iter()
            .find(|entry| entry.generation == "WS-20260711.0")
            .expect("current row");
        assert!(current
            .protected_reasons
            .iter()
            .all(|reason| reason.starts_with("current:")));
        let candidate = view
            .generations
            .iter()
            .find(|entry| entry.generation == "WS-20260712.0")
            .expect("candidate row");
        assert!(candidate
            .protected_reasons
            .contains(&"candidate".to_string()));
        assert!(candidate.protected_reasons.contains(&"ops-job".to_string()));
        let previous = view
            .generations
            .iter()
            .find(|entry| entry.generation == "WS-20260710.0")
            .expect("previous row");
        assert!(previous.protected_reasons.contains(&"previous".to_string()));
    }
}
