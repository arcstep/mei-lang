//! HTTP APIs for per-app launch configs and start/stop (0537).

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mei_host_core::{
    ensure_default_launch_config, list_launch_configs, read_launch_config, write_launch_config,
    AppLaunchConfig, AppLaunchDocument, DesiredInstance, DesiredState, RouteBinding,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::app_runtime_supervisor::{
    generate_instance_token, instance_spec_from_launch, spawn_into, stop_from,
};
use crate::app_start_inflight::{StartInflightGuard, StartPhase};
use crate::runtime_route_table::RuntimeRoutePhase;
use crate::state::{HostEvent, HostHttpState};

static REPLACEMENT_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAppBody {
    /// Launch config name or relative path under the app (Phase 8.5: only `launch.json`).
    pub config: Option<String>,
    /// Unified runtime mode for this start (`hot` / `lazy` / `frozen`).
    pub mode: Option<String>,
    /// When true, clear ephemeral overlay and follow `launch.json` defaultMode.
    #[serde(default)]
    pub follow_git: bool,
}

pub async fn api_host_apps_overview(State(http): State<HostHttpState>) -> Response {
    Json(crate::shell_chrome::build_apps_overview_payload(&http)).into_response()
}

pub async fn api_host_app_launch_configs(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    match list_launch_configs(workspace.as_path(), app_id.as_str()) {
        Ok(launches) => Json(json!({ "appId": app_id, "launches": launches })).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn api_host_app_start(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
    Json(body): Json<StartAppBody>,
) -> Response {
    let result = if let Some(actor) = http.runtime_actor.as_ref() {
        actor
            .start(
                app_id.as_str(),
                body.config.as_deref(),
                body.mode.as_deref(),
                body.follow_git,
            )
            .await
    } else {
        start_app_with_launch(
            &http,
            app_id.as_str(),
            body.config.as_deref(),
            body.mode.as_deref(),
            body.follow_git,
        )
        .await
    };
    match result {
        Ok(payload) => Json(payload).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_host_app_stop(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let result = if let Some(actor) = http.runtime_actor.as_ref() {
        actor.stop(app_id.as_str()).await
    } else {
        stop_app_runtime(&http, app_id.as_str()).await
    };
    match result {
        Ok(payload) => Json(payload).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_host_app_ensure_default_launch(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    match ensure_default_launch_config(workspace.as_path(), app_id.as_str()) {
        Ok(doc) => Json(json!({ "accepted": true, "launch": doc })).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOverlayBody {
    pub overlay: mei_host_core::RuntimePolicyOverlay,
    pub expected_revision: Option<String>,
}

pub async fn api_host_app_runtime_overlay_get(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let overlay = mei_host_core::read_runtime_overlay(workspace.as_path(), app_id.as_str());
    let base = read_launch_config(workspace.as_path(), app_id.as_str(), "launch")
        .ok()
        .map(|doc| doc.config);
    let effective = base
        .as_ref()
        .map(|config| launch_runtime_plan(workspace.as_path(), app_id.as_str(), config));
    Json(json!({
        "appId": app_id,
        "overlay": overlay,
        "effectiveRuntimePlan": effective,
    }))
    .into_response()
}

pub async fn api_host_app_runtime_overlay_put(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
    Json(body): Json<RuntimeOverlayBody>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    match mei_host_core::write_runtime_overlay(
        workspace.as_path(),
        app_id.as_str(),
        body.overlay,
        body.expected_revision.as_deref(),
    ) {
        Ok(overlay) => {
            if let Ok(launch) = read_launch_config(workspace.as_path(), app_id.as_str(), "launch") {
                apply_launch_runtime_profile(
                    &http,
                    workspace.as_path(),
                    app_id.as_str(),
                    &launch.config,
                );
            }
            Json(json!({ "accepted": true, "overlay": overlay })).into_response()
        }
        Err(mei_host_core::RuntimeOverlayError::Conflict(message)) => {
            (StatusCode::CONFLICT, Json(json!({ "error": message }))).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn api_host_app_runtime_overlay_reset(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    match mei_host_core::clear_runtime_overlay(workspace.as_path(), app_id.as_str()) {
        Ok(()) => {
            if let Ok(launch) = read_launch_config(workspace.as_path(), app_id.as_str(), "launch") {
                apply_launch_runtime_profile(
                    &http,
                    workspace.as_path(),
                    app_id.as_str(),
                    &launch.config,
                );
            }
            Json(json!({ "accepted": true, "overlay": serde_json::Value::Null })).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLaunchBody {
    pub config: AppLaunchConfig,
}

pub async fn api_host_app_save_launch(
    State(http): State<HostHttpState>,
    AxumPath((app_id, name)): AxumPath<(String, String)>,
    Json(body): Json<SaveLaunchBody>,
) -> Response {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    match write_launch_config(
        workspace.as_path(),
        app_id.as_str(),
        name.as_str(),
        &body.config,
    ) {
        Ok(doc) => Json(json!({ "accepted": true, "launch": doc })).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn start_app_with_launch(
    http: &HostHttpState,
    app_id: &str,
    config: Option<&str>,
    mode: Option<&str>,
    follow_git: bool,
) -> Result<serde_json::Value, StartStopError> {
    let _inflight = StartInflightGuard::try_acquire(
        http.start_inflight.as_ref(),
        app_id,
        StartPhase::Prebuilding,
    )
    .map_err(|existing| {
        StartStopError::Conflict(format!(
            "app-start-in-flight: app `{}` is already {}",
            existing.app_id,
            match existing.phase {
                StartPhase::Prebuilding => "prebuilding",
                StartPhase::Spawning => "spawning",
            }
        ))
    })?;

    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    apply_start_mode_policy(workspace.as_path(), app_id, mode, follow_git)?;
    let config_ref = config.unwrap_or("launch").trim();
    // Phase 8.5: each app binds solely to `apps/{app}/launch.json`. Temporary
    // hot/lazy/frozen adjustments use ephemeral runtime overlay APIs instead.
    let allowed = config_ref.is_empty()
        || config_ref == "launch"
        || config_ref == "default"
        || config_ref == "launch.json"
        || config_ref.ends_with("/launch.json");
    if !allowed {
        return Err(StartStopError::BadRequest(format!(
            "Phase 8.5 single-launch policy: only apps/{{app}}/launch.json is allowed (got `{config_ref}`); use ephemeral runtime overlay for temporary policy"
        )));
    }
    let launch = match read_launch_config(workspace.as_path(), app_id, "launch") {
        Ok(doc) => doc,
        Err(_) => ensure_default_launch_config(workspace.as_path(), app_id)
            .map_err(|e| StartStopError::BadRequest(e.to_string()))?,
    };
    apply_launch_runtime_profile(http, workspace.as_path(), app_id, &launch.config);
    prepare_current_launch(http, workspace.as_path(), app_id, &launch).await?;

    _inflight.set_phase(StartPhase::Spawning);

    let old_active = {
        let guard = http.shell.read().expect("state lock");
        guard
            .launch_manifest
            .routes
            .get(app_id)
            .and_then(|route| route.active.clone())
    };
    if old_active.is_none() {
        http.route_table
            .publish_entry(crate::runtime_route_table::RuntimeRouteEntry {
                app_id: app_id.to_string(),
                instance_id: format!("{app_id}:starting"),
                endpoint: "pending://starting".to_string(),
                token: String::new(),
                generation: String::new(),
                spec_digest: String::new(),
                phase: RuntimeRoutePhase::Starting,
            });
    }
    {
        let guard = http.shell.read().expect("state lock");
        let _ = guard.events.send(HostEvent::new(
            "app-starting",
            json!({
                "appId": app_id,
                "launchId": launch.id,
                "phase": "spawning",
                "message": "正在拉起 mei-app-runtime…",
            }),
        ));
    }
    let mut spec = instance_spec_from_launch(workspace.as_path(), app_id, &launch)
        .map_err(|e| StartStopError::BadRequest(e.to_string()))?;
    spec.instance_id = replacement_instance_id(spec.instance_id.as_str());
    let effective_mode = match spec.config_snapshot.runtime_plan.default_mode {
        mei_lang_kernel::RuntimeMode::Hot => "hot",
        mei_lang_kernel::RuntimeMode::Lazy => "lazy",
        mei_lang_kernel::RuntimeMode::Frozen => "frozen",
    };
    tracing::info!(
        target: "mei.host.runtime",
        app = %app_id,
        mode = %effective_mode,
        follow_git = follow_git,
        requested_mode = mode.unwrap_or(""),
        phase = "spawning",
        "starting app with unified runtime mode"
    );
    let token = generate_instance_token(spec.instance_id.as_str());
    let spec_for_state = spec.clone();
    let candidate_revision =
        register_start_candidate(http, workspace.as_path(), &spec_for_state).await?;

    // Spawn and warm the candidate while the route snapshot still targets old_active.
    let observed = match spawn_into(&http.app_runtime, spec, token).await {
        Ok(observed) => observed,
        Err(error) => {
            discard_start_candidate(http, workspace.as_path(), &spec_for_state).await;
            return Err(StartStopError::Unavailable(error.to_string()));
        }
    };

    let endpoint = observed.endpoint.clone().unwrap_or_default();
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.register_app_runtime_endpoint(
            spec_for_state.instance_id.clone(),
            endpoint.clone(),
            Some(crate::state::current_time_ms()),
        );
    }

    let cutover = match crate::route_lifecycle::execute_cutover(
        http,
        app_id,
        &crate::route_lifecycle::CutoverRequest {
            instance_id: spec_for_state.instance_id.clone(),
            expected_manifest_revision: candidate_revision,
        },
        true,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            let _ = stop_from(&http.app_runtime, spec_for_state.instance_id.as_str()).await;
            discard_start_candidate(http, workspace.as_path(), &spec_for_state).await;
            return Err(StartStopError::Conflict(error.message().to_string()));
        }
    };

    {
        let mut guard = http.shell.write().expect("state lock");
        guard.route_plane_ready = true;
        guard.data_plane_enabled = true;
        let _ = guard.events.send(HostEvent::new(
            "app-started",
            crate::shell_chrome::running_event_payload_with_plan(
                workspace.as_path(),
                app_id,
                launch.id.as_str(),
                spec_for_state.instance_id.as_str(),
                Some(&spec_for_state.config_snapshot.runtime_plan),
            ),
        ));
    }

    if let Some(previous) = cutover
        .previous
        .as_deref()
        .filter(|previous| *previous != spec_for_state.instance_id)
    {
        drain_and_retire_previous(http, workspace.as_path(), app_id, previous).await;
    }

    Ok(json!({
        "accepted": true,
        "kind": "app-started",
        "appId": app_id,
        "mode": effective_mode,
        "launch": launch,
        "instance": observed,
        "cutover": cutover,
    }))
}

fn replacement_instance_id(base: &str) -> String {
    let counter = REPLACEMENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{base}@run-{}-{counter}", crate::state::current_time_ms())
}

async fn register_start_candidate(
    http: &HostHttpState,
    workspace: &std::path::Path,
    spec: &mei_host_core::InstanceSpec,
) -> Result<String, StartStopError> {
    mei_host_core::write_instance_spec(workspace, spec)
        .map_err(|error| StartStopError::Unavailable(error.to_string()))?;
    let _mutation = http.manifest_mutation.lock().await;
    let (expected, next) = {
        let guard = http.shell.read().expect("state lock");
        let expected = guard.launch_manifest.revision.clone();
        let mut next = guard.launch_manifest.clone();
        next.instances.insert(
            spec.instance_id.clone(),
            DesiredInstance {
                spec_ref: spec.spec_digest(),
                desired_state: DesiredState::Running,
            },
        );
        let route = next
            .routes
            .entry(spec.app_id.clone())
            .or_insert(RouteBinding {
                active: None,
                candidate: None,
                previous: None,
            });
        route.candidate = Some(spec.instance_id.clone());
        (expected, next.with_recomputed_revision())
    };
    mei_host_core::write_if_revision_matches(workspace, expected.as_str(), next.clone())
        .map_err(|error| StartStopError::Conflict(error.to_string()))?;
    let revision = next.revision.clone();
    http.shell
        .write()
        .expect("state lock")
        .install_launch_manifest(next);
    Ok(revision)
}

async fn discard_start_candidate(
    http: &HostHttpState,
    workspace: &std::path::Path,
    spec: &mei_host_core::InstanceSpec,
) {
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.unregister_app_runtime_endpoint(spec.instance_id.as_str());
    }
    let _mutation = http.manifest_mutation.lock().await;
    let (expected, next) = {
        let guard = http.shell.read().expect("state lock");
        let expected = guard.launch_manifest.revision.clone();
        let mut next = guard.launch_manifest.clone();
        next.instances.remove(spec.instance_id.as_str());
        if let Some(route) = next.routes.get_mut(spec.app_id.as_str()) {
            if route.candidate.as_deref() == Some(spec.instance_id.as_str()) {
                route.candidate = None;
            }
        }
        (expected, next.with_recomputed_revision())
    };
    if mei_host_core::write_if_revision_matches(workspace, expected.as_str(), next.clone()).is_ok()
    {
        http.shell
            .write()
            .expect("state lock")
            .install_launch_manifest(next);
    }
    let _ = mei_host_core::clear_instance_ephemeral_runtime(
        workspace,
        spec.app_id.as_str(),
        spec.instance_id.as_str(),
    );
    http.sync_route_table_from_supervisor().await;
}

async fn drain_and_retire_previous(
    http: &HostHttpState,
    workspace: &std::path::Path,
    app_id: &str,
    previous: &str,
) {
    let drain = std::env::var("MEI_HOST_RUNTIME_DRAIN_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(750);
    if drain > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(drain)).await;
    }
    let _ = stop_from(&http.app_runtime, previous).await;
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.unregister_app_runtime_endpoint(previous);
    }
    let _mutation = http.manifest_mutation.lock().await;
    let (expected, next) = {
        let guard = http.shell.read().expect("state lock");
        let expected = guard.launch_manifest.revision.clone();
        let mut next = guard.launch_manifest.clone();
        if let Some(desired) = next.instances.get_mut(previous) {
            desired.desired_state = DesiredState::Stopped;
        }
        (expected, next.with_recomputed_revision())
    };
    if mei_host_core::write_if_revision_matches(workspace, expected.as_str(), next.clone()).is_ok()
    {
        http.shell
            .write()
            .expect("state lock")
            .install_launch_manifest(next);
    }
    http.sync_route_table_from_supervisor().await;
    tracing::info!(
        target: "mei.host.runtime",
        app = %app_id,
        instance = %previous,
        drain_ms = drain,
        "retired previous app runtime after cutover"
    );
}

async fn prepare_current_launch(
    http: &HostHttpState,
    workspace: &std::path::Path,
    app_id: &str,
    launch: &AppLaunchDocument,
) -> Result<(), StartStopError> {
    if !launch_uses_current_generation(&launch.config) {
        return Ok(());
    }
    {
        let guard = http.shell.read().expect("state lock");
        let _ = guard.events.send(HostEvent::new(
            "app-starting",
            json!({
                "appId": app_id,
                "launchId": launch.id,
                "phase": "prebuilding",
                "message": if crate::startup::defer_warmup_to_prebuild() {
                    "正在等待后台 prebuild 完成…"
                } else {
                    "正在编译应用并准备数据快照…"
                },
            }),
        ));
    }
    // `start.sh --app` runs deploy/prebuild.sh in the background while host
    // autostarts. Both used to call prepare→replace_env_generation and race on
    // env/{ver} (macOS ENOTEMPTY / "Directory not empty"). When defer is on,
    // wait for that background job for apps it covers; if the job finished
    // without this app (or the app is already imported), fall through to inline
    // prebuild instead of spinning on a shared prebuild.pid.
    if crate::startup::defer_warmup_to_prebuild() {
        match wait_for_deferred_app_prebuild(workspace, app_id).await? {
            DeferredPrebuildWait::Ready => return Ok(()),
            DeferredPrebuildWait::RunInline => {}
        }
    }
    let workspace = workspace.to_path_buf();
    let app_id = app_id.to_string();
    let scenes = launch_warmup_scenes(workspace.as_path(), &launch.config, app_id.as_str());
    tokio::task::spawn_blocking(move || {
        crate::build_ops::prebuild_pipeline(workspace.as_path(), app_id.as_str(), &scenes)
    })
    .await
    .map_err(|error| StartStopError::Unavailable(format!("app prebuild task failed: {error}")))?
    .map_err(|error| StartStopError::Unavailable(format!("app prebuild failed: {error}")))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredPrebuildWait {
    Ready,
    RunInline,
}

async fn wait_for_deferred_app_prebuild(
    workspace: &std::path::Path,
    app_id: &str,
) -> Result<DeferredPrebuildWait, StartStopError> {
    // Already imported: do not block on another app's background prebuild PID.
    if crate::landing::app_has_prebuilt_access_entry(workspace, app_id) {
        tracing::info!(
            app = %app_id,
            "deferred prebuild skip — app already imported"
        );
        return Ok(DeferredPrebuildWait::Ready);
    }

    const POLL: std::time::Duration = std::time::Duration::from_millis(500);
    const MAX_WAIT: std::time::Duration = std::time::Duration::from_secs(600);
    let started = std::time::Instant::now();
    let mut polls: u32 = 0;
    loop {
        polls = polls.saturating_add(1);
        let pid_alive = deferred_prebuild_pid_alive(workspace);
        let imported = crate::landing::app_has_prebuilt_access_entry(workspace, app_id);
        if imported {
            tracing::info!(
                app = %app_id,
                waited_ms = started.elapsed().as_millis() as u64,
                "deferred prebuild ready for autostart"
            );
            return Ok(DeferredPrebuildWait::Ready);
        }
        if !pid_alive {
            tracing::warn!(
                app = %app_id,
                waited_ms = started.elapsed().as_millis() as u64,
                "deferred prebuild ended without this app — falling back to inline prebuild"
            );
            return Ok(DeferredPrebuildWait::RunInline);
        }
        if started.elapsed() > MAX_WAIT {
            return Err(StartStopError::Unavailable(format!(
                "timed out waiting for deferred prebuild of `{app_id}` (pid_alive={pid_alive}, imported={imported})"
            )));
        }
        if polls == 1 || polls.is_multiple_of(10) {
            tracing::info!(
                app = %app_id,
                pid_alive,
                imported,
                "waiting for deferred prebuild before app start"
            );
        }
        tokio::time::sleep(POLL).await;
    }
}

fn deferred_prebuild_pid_alive(workspace: &std::path::Path) -> bool {
    let pid_path = workspace.join("deploy/state/prebuild.pid");
    let Ok(raw) = std::fs::read_to_string(&pid_path) else {
        return false;
    };
    let Ok(pid) = raw.trim().parse::<i32>() else {
        let _ = std::fs::remove_file(&pid_path);
        return false;
    };
    if pid <= 0 {
        let _ = std::fs::remove_file(&pid_path);
        return false;
    }
    let alive = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !alive {
        // Stale pid left after background prebuild exits — clear so later
        // starts do not treat a recycled PID as "still compiling".
        let _ = std::fs::remove_file(&pid_path);
    }
    alive
}

fn launch_uses_current_generation(config: &AppLaunchConfig) -> bool {
    let generation = config.generation.trim();
    generation.is_empty() || generation.eq_ignore_ascii_case("current")
}

pub(crate) fn base_launch_runtime_plan(
    workspace: &std::path::Path,
    config: &AppLaunchConfig,
) -> mei_lang_kernel::RuntimePlan {
    use mei_lang_kernel::{RuntimeMode, RuntimePlan};
    if let Some(value) = config.runtime_plan.as_ref() {
        return serde_json::from_value(value.clone()).unwrap_or(RuntimePlan {
            default_mode: RuntimeMode::Lazy,
            apps: Default::default(),
        });
    }
    let path = workspace.join("deploy/applied/runtime-plan.json");
    if path.is_file() {
        if let Ok(raw) = std::fs::read_to_string(path) {
            if let Ok(plan) = serde_json::from_str::<RuntimePlan>(&raw) {
                return plan;
            }
        }
    }
    RuntimePlan {
        default_mode: RuntimeMode::Lazy,
        apps: Default::default(),
    }
}

pub(crate) fn launch_runtime_plan(
    workspace: &std::path::Path,
    app_id: &str,
    config: &AppLaunchConfig,
) -> mei_lang_kernel::RuntimePlan {
    let base = base_launch_runtime_plan(workspace, config);
    let overlay = mei_host_core::read_runtime_overlay(workspace, app_id);
    mei_host_core::effective_runtime_plan(&base, app_id, overlay.as_ref())
}

fn apply_launch_data_mode_ceiling(http: &HostHttpState, app_id: &str, config: &AppLaunchConfig) {
    let ceiling = match config
        .data_mode_ceiling
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None => mei_lang_kernel::DataModeCeiling::Eval,
        Some(raw) => match mei_lang_kernel::DataModeCeiling::parse(raw) {
            Some(ceiling) => ceiling,
            None => {
                tracing::warn!(
                    ceiling = %raw,
                    app_id = %app_id,
                    "launch config dataModeCeiling ignored: unknown value"
                );
                mei_lang_kernel::DataModeCeiling::Eval
            }
        },
    };
    let mut guard = http.shell.write().expect("state lock");
    guard.set_data_mode_ceiling_for(app_id, ceiling);
}

pub(crate) fn apply_launch_runtime_profile(
    http: &HostHttpState,
    workspace: &std::path::Path,
    app_id: &str,
    config: &AppLaunchConfig,
) {
    let runtime_plan = launch_runtime_plan(workspace, app_id, config);
    crate::dev_eval_scope::install_runtime_plan_for_app(app_id, runtime_plan);
    apply_launch_data_mode_ceiling(http, app_id, config);
}

fn launch_warmup_enabled(config: &AppLaunchConfig) -> bool {
    config
        .warmup
        .as_ref()
        .and_then(|warmup| warmup.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

/// All `hotScenes` from launch warmup config (not just the first).
/// When enabled but empty, fall back to the app's `default_scene` (not blind `home`).
pub(crate) fn launch_warmup_scenes(
    workspace: &std::path::Path,
    config: &AppLaunchConfig,
    app_id: &str,
) -> Vec<String> {
    if !launch_warmup_enabled(config) {
        return Vec::new();
    }
    let scenes = config
        .warmup
        .as_ref()
        .and_then(|warmup| warmup.get("apps"))
        .and_then(|apps| apps.get(app_id))
        .and_then(|app| app.get("hotScenes"))
        .and_then(serde_json::Value::as_array)
        .map(|scenes| {
            scenes
                .iter()
                .filter_map(|scene| scene.as_str())
                .map(str::trim)
                .filter(|scene| !scene.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if scenes.is_empty() {
        vec![crate::shell_chrome::default_access_scene(workspace, app_id)]
    } else {
        scenes
    }
}

pub async fn stop_app_runtime(
    http: &HostHttpState,
    app_id: &str,
) -> Result<serde_json::Value, StartStopError> {
    let active_id = {
        let guard = http.shell.read().expect("state lock");
        guard
            .launch_manifest
            .routes
            .get(app_id)
            .and_then(|r| r.active.clone())
    };
    let mut instance_ids = {
        let supervisor = http.app_runtime.lock().await;
        supervisor
            .runtimes
            .iter()
            .filter(|(_, runtime)| runtime.spec.app_id == app_id)
            .map(|(instance_id, _)| instance_id.clone())
            .collect::<Vec<_>>()
    };
    if let Some(active) = active_id.as_ref() {
        if !instance_ids.contains(active) {
            instance_ids.push(active.clone());
        }
    }
    if instance_ids.is_empty() {
        http.remove_route(app_id);
        return Ok(json!({
            "accepted": true,
            "kind": "app-stopped",
            "appId": app_id,
            "alreadyStopped": true,
        }));
    }

    // Drain route table / manifest first so proxy stops targeting a dying process.
    http.mark_route_draining(app_id);
    {
        let _mutation = http.manifest_mutation.lock().await;
        let (expected, next) = {
            let guard = http.shell.read().expect("state lock");
            let expected = guard.launch_manifest.revision.clone();
            let mut manifest = guard.launch_manifest.clone();
            if let Some(route) = manifest.routes.get_mut(app_id) {
                route.previous = route.active.take();
                route.candidate = None;
            }
            for instance_id in &instance_ids {
                if let Some(desired) = manifest.instances.get_mut(instance_id.as_str()) {
                    desired.desired_state = DesiredState::Stopped;
                }
            }
            (expected, manifest.with_recomputed_revision())
        };
        let workspace_root = {
            let guard = http.shell.read().expect("state lock");
            guard.ctx.workspace_root.clone()
        };
        mei_host_core::write_if_revision_matches(
            workspace_root.as_path(),
            expected.as_str(),
            next.clone(),
        )
        .map_err(|error| StartStopError::Conflict(error.to_string()))?;
        let mut guard = http.shell.write().expect("state lock");
        for instance_id in &instance_ids {
            guard.unregister_app_runtime_endpoint(instance_id.as_str());
        }
        guard.install_launch_manifest(next);
    }
    http.remove_route(app_id);

    let mut stops = tokio::task::JoinSet::new();
    for instance_id in instance_ids.iter().cloned() {
        let supervisor = http.app_runtime.clone();
        stops.spawn(async move {
            let result = stop_from(&supervisor, instance_id.as_str()).await;
            (instance_id, result)
        });
    }
    while let Some(result) = stops.join_next().await {
        if let Ok((instance_id, Err(error))) = result {
            tracing::warn!(
                app = %app_id,
                instance = %instance_id,
                error = %error,
                "failed to stop app runtime instance"
            );
        }
    }

    let instance_id = active_id.unwrap_or_else(|| instance_ids[0].clone());
    {
        let guard = http.shell.write().expect("state lock");
        let workspace = guard.ctx.workspace_root.clone();
        let _ = guard.events.send(HostEvent::new(
            "app-stopped",
            json!({
                "appId": app_id,
                "instanceId": instance_id,
                "instanceIds": instance_ids,
                "href": crate::shell_chrome::app_access_href(workspace.as_path(), app_id),
            }),
        ));
        let _ = mei_host_core::clear_runtime_overlay(workspace.as_path(), app_id);
    }

    Ok(json!({
        "accepted": true,
        "kind": "app-stopped",
        "appId": app_id,
        "instanceId": instance_id,
        "instanceIds": instance_ids,
    }))
}

#[derive(Debug)]
pub enum StartStopError {
    BadRequest(String),
    Conflict(String),
    Unavailable(String),
}

fn error_response(error: StartStopError) -> Response {
    let (status, message, kind) = match &error {
        StartStopError::BadRequest(message) => {
            (StatusCode::BAD_REQUEST, message.clone(), "bad_request")
        }
        StartStopError::Conflict(message) => {
            let kind = if message.starts_with("app-start-in-flight") {
                "app-start-in-flight"
            } else {
                "conflict"
            };
            (StatusCode::CONFLICT, message.clone(), kind)
        }
        StartStopError::Unavailable(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            message.clone(),
            "unavailable",
        ),
    };
    let mut body = json!({
        "error": message,
        "kind": kind,
    });
    if kind == "app-start-in-flight" {
        if let Some(app_id) = message
            .strip_prefix("app-start-in-flight: app `")
            .and_then(|rest| rest.split('`').next())
        {
            body["appId"] = json!(app_id);
        }
        if message.contains("prebuilding") {
            body["phase"] = json!("prebuilding");
        } else if message.contains("spawning") {
            body["phase"] = json!("spawning");
        }
        tracing::info!(
            target: "mei.host.runtime",
            kind = "app-start-in-flight",
            inflight_rejected = true,
            app_id = body.get("appId").and_then(|v| v.as_str()).unwrap_or(""),
            phase = body.get("phase").and_then(|v| v.as_str()).unwrap_or(""),
            "rejected duplicate app start"
        );
    }
    (status, Json(body)).into_response()
}

fn apply_start_mode_policy(
    workspace: &std::path::Path,
    app_id: &str,
    mode: Option<&str>,
    follow_git: bool,
) -> Result<(), StartStopError> {
    if follow_git {
        mei_host_core::clear_runtime_overlay(workspace, app_id)
            .map_err(|e| StartStopError::BadRequest(e.to_string()))?;
        return Ok(());
    }
    let Some(mode) = mode.map(str::trim).filter(|m| !m.is_empty()) else {
        return Ok(());
    };
    let mode = mode.to_ascii_lowercase();
    if !matches!(mode.as_str(), "hot" | "lazy" | "frozen") {
        return Err(StartStopError::BadRequest(format!(
            "invalid runtime mode `{mode}`; expected hot|lazy|frozen"
        )));
    }
    let overlay = mei_host_core::RuntimePolicyOverlay {
        schema_version: mei_host_core::SCHEMA_RUNTIME_OVERLAY_V1.to_string(),
        app_id: app_id.to_string(),
        default_mode: Some(mode),
        targets: Vec::new(),
        metric_overrides: Default::default(),
        revision: String::new(),
    };
    mei_host_core::write_runtime_overlay(workspace, app_id, overlay, None)
        .map_err(|e| StartStopError::BadRequest(e.to_string()))?;
    Ok(())
}

/// Autostart targets collected at serve time.
pub async fn autostart_launch_targets(
    http: &HostHttpState,
    targets: &[crate::launch_targets::LaunchTarget],
) {
    for target in targets {
        match start_app_with_launch(
            http,
            target.app_id.as_str(),
            Some(target.document.id.as_str()),
            target.mode_override.as_deref(),
            target.clear_overlay,
        )
        .await
        {
            Ok(_) => tracing::info!(
                app = %target.app_id,
                launch = %target.document.id,
                mode = target.mode_override.as_deref().unwrap_or("launch.json"),
                "autostarted app"
            ),
            Err(error) => tracing::warn!(
                app = %target.app_id,
                launch = %target.document.id,
                detail = %match error {
                    StartStopError::BadRequest(m)
                    | StartStopError::Conflict(m)
                    | StartStopError::Unavailable(m) => m,
                },
                "autostart failed"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{launch_uses_current_generation, launch_warmup_scenes};
    use mei_host_core::AppLaunchConfig;
    use std::path::Path;

    #[test]
    fn current_generation_launch_requires_prebuild() {
        let mut config = AppLaunchConfig::default_for_app("zhifa");
        config.generation = "current".to_string();
        assert!(launch_uses_current_generation(&config));

        config.generation = "WS-20260713.0".to_string();
        assert!(!launch_uses_current_generation(&config));
    }

    #[test]
    fn warmup_scenes_include_all_hot_scenes() {
        let mut config = AppLaunchConfig::default_for_app("zhifa");
        config.warmup = Some(serde_json::json!({
            "enabled": true,
            "apps": {
                "zhifa": {
                    "hotScenes": ["home/t1", "home"]
                }
            }
        }));
        let workspace = Path::new("/tmp/mei-missing-workspace");
        assert_eq!(
            launch_warmup_scenes(workspace, &config, "zhifa"),
            vec!["home/t1".to_string(), "home".to_string()]
        );
        // Missing hotScenes for this app → default_scene fallback (home when app.mei missing).
        assert_eq!(
            launch_warmup_scenes(workspace, &config, "other-app"),
            vec!["home".to_string()]
        );
    }

    #[test]
    fn warmup_disabled_skips_all_scenes() {
        let mut config = AppLaunchConfig::default_for_app("zhifa");
        config.warmup = Some(serde_json::json!({
            "enabled": false,
            "apps": {
                "zhifa": {
                    "hotScenes": ["home"]
                }
            }
        }));
        assert!(launch_warmup_scenes(Path::new("/tmp"), &config, "zhifa").is_empty());
    }
}
