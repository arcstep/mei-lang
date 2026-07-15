use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mei_host_core::{
    BuildAppArtifact, BuildRequest, BuildResult, BundleRef, ConfigSnapshot, DesiredInstance,
    DesiredState, InstanceSpec, LastSuccessfulApply, SCHEMA_INSTANCE_SPEC_V1,
};
use mei_lang_kernel::{
    attach_build_generation, read_links_state, resolve_app_build_generation_from_current,
    resolve_app_root, write_links_state, RuntimeMode, RuntimePlan, WorkspaceConfig,
    WorkspaceProfileDocument, WorkspaceProfileDryRun, WorkspaceProfileError,
    WorkspaceProfileService,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::build_ops::{
    begin_profile_ops_job, finish_ops_job_failure, finish_ops_job_success, update_ops_app_progress,
    update_ops_job_generation, update_ops_job_phase,
};
use crate::build_worker::{build_request_from_profile, run_build_request};
use crate::state::{HostEvent, SharedState};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyProfileRequest {
    pub profile_id: String,
    pub expected_revision: String,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyProfilePlan {
    pub profile_id: String,
    pub profile_revision: String,
    pub apps: Vec<ApplyProfileAppPlan>,
    pub snapshot_policy: String,
    /// Human-readable apply pipeline for control-center dry-run copy.
    pub pipeline: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyProfileAppPlan {
    pub app_id: String,
    pub compile: bool,
    pub import: bool,
    pub publish_snapshots: bool,
    pub warm: bool,
    pub hot_targets: Vec<String>,
    pub hot_metrics: Vec<String>,
    /// `reuse` when a sealed current generation already exists; otherwise `build`.
    pub bundle_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    pub launch_instance: bool,
    pub cutover_route: bool,
}

#[derive(Debug)]
pub struct PreparedProfileApply {
    pub document: WorkspaceProfileDocument,
    pub runtime_plan: RuntimePlan,
    pub plan: ApplyProfilePlan,
    pub config_path: PathBuf,
}

pub fn prepare_profile_apply(
    workspace: &Path,
    request: &ApplyProfileRequest,
) -> Result<PreparedProfileApply, WorkspaceProfileError> {
    let service = WorkspaceProfileService::new(workspace);
    let document = service.read(request.profile_id.as_str())?;
    if document.revision != request.expected_revision {
        return Err(WorkspaceProfileError::RevisionConflict {
            expected: Some(request.expected_revision.clone()),
            current: Some(document.revision),
        });
    }
    let dry_run = service.dry_run(request.profile_id.as_str())?;
    let config: WorkspaceConfig = serde_json::from_value(document.config.clone())
        .map_err(|error| WorkspaceProfileError::InvalidJson(error.to_string()))?;
    let runtime_plan = config.deploy.effective_runtime_plan();
    let plan = build_apply_plan(workspace, &dry_run, &runtime_plan);
    let config_path = workspace.join(document.file.as_str());
    Ok(PreparedProfileApply {
        document,
        runtime_plan,
        plan,
        config_path,
    })
}

pub fn build_apply_plan(
    workspace: &Path,
    dry_run: &WorkspaceProfileDryRun,
    runtime_plan: &RuntimePlan,
) -> ApplyProfilePlan {
    let apps = dry_run
        .discovered_apps
        .iter()
        .map(|app_id| {
            let app_plan = runtime_plan
                .apps
                .get(app_id)
                .or_else(|| runtime_plan.apps.get("*"));
            let hot_targets = app_plan
                .into_iter()
                .flat_map(|app| app.targets.iter())
                .filter(|target| target.mode == RuntimeMode::Hot)
                .map(|target| target.scope.clone())
                .collect::<Vec<_>>();
            let hot_metrics = app_plan
                .into_iter()
                .flat_map(|app| app.metric_overrides.iter())
                .filter(|(_, mode)| **mode == RuntimeMode::Hot)
                .map(|(metric_id, _)| metric_id.clone())
                .collect::<Vec<_>>();
            let warm = runtime_plan.default_mode == RuntimeMode::Hot
                || !hot_targets.is_empty()
                || !hot_metrics.is_empty();
            let generation = resolve_app_build_generation_from_current(
                resolve_app_root(workspace, app_id.as_str()).as_path(),
            )
            .ok();
            let bundle_action = if generation.is_some() {
                "reuse".to_string()
            } else {
                "build".to_string()
            };
            ApplyProfileAppPlan {
                app_id: app_id.clone(),
                compile: true,
                import: true,
                // This phase intentionally uses the conservative policy permitted by the
                // contract: publish each target app's snapshots after a successful import.
                publish_snapshots: true,
                warm,
                hot_targets,
                hot_metrics,
                bundle_action,
                generation,
                launch_instance: true,
                cutover_route: true,
            }
        })
        .collect();
    ApplyProfilePlan {
        profile_id: dry_run.profile.id.clone(),
        profile_revision: dry_run.profile.revision.clone(),
        apps,
        snapshot_policy: "conservative-per-target-app".to_string(),
        pipeline: "build worker → launch instances → cutover route".to_string(),
    }
}

pub fn start_profile_apply(
    state: SharedState,
    managed_plug_slot: Arc<Mutex<Option<crate::managed_plug::ManagedPlugDsPool>>>,
    app_runtime_slot: crate::app_runtime_supervisor::SharedAppRuntime,
    prepared: PreparedProfileApply,
) -> Result<(), String> {
    let app_ids = prepared
        .plan
        .apps
        .iter()
        .map(|app| app.app_id.clone())
        .collect::<Vec<_>>();
    let selected_profile = crate::workspace_profile_api::ResolvedRuntimeProfile {
        id: prepared.document.id.clone(),
        file: prepared.document.file.clone(),
        revision: prepared.document.revision.clone(),
        source: "last_successful".to_string(),
        path: prepared.config_path.clone(),
    };
    let default_app_id =
        serde_json::from_value::<WorkspaceConfig>(prepared.document.config.clone())
            .ok()
            .and_then(|config| config.workspace.default_app)
            .or_else(|| app_ids.first().cloned());
    {
        let mut guard = state.write().expect("state lock");
        begin_profile_ops_job(
            &mut guard,
            prepared.document.id.as_str(),
            prepared.document.revision.as_str(),
            app_ids.as_slice(),
        )?;
    }
    tokio::spawn(async move {
        let task_state = state.clone();
        let result = tokio::task::spawn_blocking(move || run_profile_apply(&task_state, prepared))
            .await
            .map_err(|error| format!("apply-profile task join failed: {error}"))
            .and_then(|result| result.map_err(|error| error.to_string()));
        let result = match result {
            Ok((message, instance_specs)) => {
                match crate::route_lifecycle::cutover_after_apply(
                    &state,
                    &app_runtime_slot,
                    instance_specs.as_slice(),
                )
                .await
                {
                    Ok(()) => start_applied_data_plane(
                        &state,
                        managed_plug_slot,
                        app_ids.as_slice(),
                        default_app_id.as_deref(),
                    )
                    .await
                    .map(|()| message)
                    .map_err(|error| error.to_string()),
                    Err(error) => {
                        // Do not leave half-cut routes; stop failed candidates.
                        let http = crate::state::HostHttpState::with_defaults(
                            state.clone(),
                            mei_host_auth::AuthServeState::new(
                                {
                                    let guard = state.read().expect("state lock");
                                    guard.ctx.workspace_root.clone()
                                },
                                mei_host_auth::AuthEnforcement::Disabled,
                            ),
                            Arc::new(Mutex::new(None)),
                            app_runtime_slot.clone(),
                        );
                        let _ = crate::route_lifecycle::stop_instances(
                            &http,
                            instance_specs.iter().map(|spec| spec.instance_id.as_str()),
                        )
                        .await;
                        Err(error.to_string())
                    }
                }
            }
            Err(error) => Err(error),
        };
        let mut guard = state.write().expect("state lock");
        match result {
            Ok(message) => {
                guard.selected_profile_id = Some(selected_profile.id);
                guard.selected_profile_file = Some(selected_profile.file);
                guard.selected_profile_revision = Some(selected_profile.revision);
                guard.selected_profile_source = Some(selected_profile.source);
                guard.set_default_app(default_app_id);
                guard.data_plane_enabled = true;
                crate::build_ops::refresh_materialization_flags(&mut guard);
                guard.startup_phase = "ready".to_string();
                guard.startup_detail = Some("配置档已应用，Access 数据面已就绪".to_string());
                guard.startup_error = None;
                finish_ops_job_success(&mut guard, message);
            }
            Err(error) => finish_ops_job_failure(&mut guard, error),
        }
    });
    Ok(())
}

async fn start_applied_data_plane(
    state: &SharedState,
    managed_plug_slot: Arc<Mutex<Option<crate::managed_plug::ManagedPlugDsPool>>>,
    app_ids: &[String],
    default_app_id: Option<&str>,
) -> anyhow::Result<()> {
    let (workspace, ceiling) = {
        let guard = state.read().expect("state lock");
        (guard.ctx.workspace_root.clone(), guard.data_mode_ceiling)
    };
    if !ceiling.requires_plug_ds() {
        return Ok(());
    }
    let default_app_id = default_app_id
        .or_else(|| app_ids.first().map(String::as_str))
        .ok_or_else(|| anyhow::anyhow!("applied profile did not discover an app"))?;
    let default_ctx =
        mei_host_core::HostContext::new(workspace.clone(), default_app_id.to_string());
    let external = crate::plug_proxy::configured_plug_ds_endpoint(&default_ctx);
    let covered_by_runtime = {
        let guard = state.read().expect("state lock");
        crate::legacy_compat::apps_covered_by_desired_runtime(&guard.launch_manifest)
    };
    let (endpoints, replacement) = if let Some(endpoint) = external {
        (
            BTreeMap::from([(default_app_id.to_string(), endpoint)]),
            None,
        )
    } else {
        let pool = crate::managed_plug::spawn_managed_plug_ds_pool(
            workspace.as_path(),
            app_ids,
            &covered_by_runtime,
        )
        .await?;
        (pool.endpoints.clone(), Some(pool))
    };
    let old_pool = {
        let mut slot = managed_plug_slot
            .lock()
            .map_err(|_| anyhow::anyhow!("managed plug-ds slot poisoned"))?;
        std::mem::replace(&mut *slot, replacement)
    };
    if let Some(mut old_pool) = old_pool {
        old_pool.shutdown().await?;
    }
    let mut guard = state.write().expect("state lock");
    guard.plug_ds_by_app = endpoints;
    guard.plug_ds_managed = managed_plug_slot
        .lock()
        .map(|slot| slot.is_some())
        .unwrap_or(false);
    guard.set_default_app(Some(default_app_id.to_string()));
    Ok(())
}

fn run_profile_apply(
    state: &SharedState,
    prepared: PreparedProfileApply,
) -> anyhow::Result<(String, Vec<InstanceSpec>)> {
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    set_phase(state, "validating", "profile revision and schema validated");
    let app_ids = prepared
        .plan
        .apps
        .iter()
        .map(|app| app.app_id.clone())
        .collect::<Vec<_>>();
    let old_links = read_links_state(workspace.as_path()).unwrap_or_default();
    let old_generations = app_ids
        .iter()
        .map(|app_id| {
            let app_root = resolve_app_root(workspace.as_path(), app_id.as_str());
            (
                app_id.clone(),
                resolve_app_build_generation_from_current(app_root.as_path()).ok(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    match run_candidate_pipeline(
        state,
        workspace.as_path(),
        &prepared,
        app_ids.as_slice(),
        &old_links,
        &old_generations,
    ) {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            restore_active_environment(
                workspace.as_path(),
                app_ids.as_slice(),
                &old_generations,
                &old_links,
            )
            .map_err(|restore_error| {
                anyhow::anyhow!(
                    "{error}; additionally failed to restore active generation: {restore_error}"
                )
            })?;
            Err(error)
        }
    }
}

fn run_candidate_pipeline(
    state: &SharedState,
    workspace: &Path,
    prepared: &PreparedProfileApply,
    app_ids: &[String],
    old_links: &mei_lang_kernel::LinksState,
    old_generations: &BTreeMap<String, Option<String>>,
) -> anyhow::Result<(String, Vec<InstanceSpec>)> {
    set_phase(
        state,
        "building",
        "spawning Build Worker for compile/import/snapshot",
    );
    let build_request = build_request_from_profile(
        prepared.document.id.as_str(),
        prepared.document.revision.as_str(),
        prepared.document.file.as_str(),
        app_ids,
    );
    for app_id in app_ids {
        set_app(
            state,
            app_id.as_str(),
            "building",
            false,
            "waiting for Build Worker",
        );
    }

    let build_result = match run_build_request(workspace, &build_request) {
        Ok(result) => result,
        Err(error) => {
            let message = error.to_string();
            for app_id in app_ids {
                set_app(state, app_id.as_str(), "building", false, message.as_str());
            }
            return Err(error);
        }
    };
    apply_build_phases_to_ops(state, &build_result);
    let generation = build_result
        .generation
        .clone()
        .ok_or_else(|| anyhow::anyhow!("BuildResult missing generation"))?;
    {
        let mut guard = state.write().expect("state lock");
        update_ops_job_generation(&mut guard, generation.as_str());
    }
    for app in &build_result.apps {
        set_app(
            state,
            app.app_id.as_str(),
            "building",
            true,
            "Build Worker complete",
        );
    }

    // Build Worker may attach env/current for artifact sealing; Host no longer treats
    // symlink flip as the primary cutover. Route CAS happens after candidate launch.
    set_phase(
        state,
        "activating",
        "registering candidate instances after Build Worker seal",
    );
    let mut links = old_links.clone();
    // Prefer reading toolchain from links after worker finish_prebuild.
    if let Ok(after) = read_links_state(workspace) {
        links.toolchain.active = after.toolchain.active.or(links.toolchain.active.clone());
    }
    if links.toolchain.active.is_none() {
        links.toolchain.active = Some(mei_lang_kernel::resolve_toolchain_version_with_hint(
            workspace,
            Some(crate::build_ops::toolchain_hint()),
        ));
    }
    links.build.candidate = Some(generation.clone());
    links.build.previous = old_generations.values().find_map(Clone::clone);
    write_links_state(workspace, &links)?;

    set_phase(
        state,
        "publishing",
        "creating InstanceSpecs and LaunchManifest candidates",
    );
    let instance_specs = instance_specs_from_build(prepared, &build_result)?;
    write_host_control_state(
        workspace,
        prepared,
        generation.as_str(),
        instance_specs.as_slice(),
    )?;
    {
        let mut guard = state.write().expect("state lock");
        if let Some(control) = mei_host_core::read_host_control_state(workspace) {
            guard.install_launch_manifest(control.launch_manifest);
        }
    }
    crate::dev_eval_scope::install_runtime_plan(prepared.runtime_plan.clone());

    emit(
        state,
        "profile-applied",
        json!({
            "profileId": prepared.document.id,
            "profileRevision": prepared.document.revision,
            "apps": app_ids,
            "envVersion": generation,
            "runtimePlan": prepared.runtime_plan,
            "instanceIds": instance_specs
                .iter()
                .map(|spec| spec.instance_id.clone())
                .collect::<Vec<_>>(),
            "buildWorker": true,
            "pendingCutover": true,
        }),
    );
    for app in &build_result.apps {
        emit(
            state,
            "revision-published",
            json!({
                "appId": app.app_id,
                "profileId": prepared.document.id,
                "profileRevision": prepared.document.revision,
                "bundlePath": app.bundle_path,
                "digest": app.digest,
            }),
        );
    }

    Ok((
        format!(
            "profile {} applied via Build Worker to {} app(s) (envVersion={})",
            prepared.document.id,
            app_ids.len(),
            generation
        ),
        instance_specs,
    ))
}

fn apply_build_phases_to_ops(state: &SharedState, result: &BuildResult) {
    for phase in &result.phases {
        let message = phase
            .message
            .clone()
            .unwrap_or_else(|| format!("{} ({} ms)", phase.name, phase.ms));
        set_phase(state, phase.name.as_str(), message.as_str());
    }
}

pub fn instance_specs_from_build(
    prepared: &PreparedProfileApply,
    build: &BuildResult,
) -> anyhow::Result<Vec<InstanceSpec>> {
    let generation = build
        .generation
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("BuildResult missing generation"))?;
    let default_app = serde_json::from_value::<WorkspaceConfig>(prepared.document.config.clone())
        .ok()
        .and_then(|config| config.workspace.default_app);
    let mut specs = Vec::new();
    for artifact in &build.apps {
        specs.push(instance_spec_for_artifact(
            prepared,
            generation,
            artifact,
            default_app.as_deref(),
        ));
    }
    if specs.is_empty() {
        for app in &prepared.plan.apps {
            specs.push(instance_spec_for_artifact(
                prepared,
                generation,
                &BuildAppArtifact {
                    app_id: app.app_id.clone(),
                    bundle_path: format!("apps/{}/env/{generation}", app.app_id),
                    digest: None,
                    config_digest: Some(prepared.document.revision.clone()),
                },
                default_app.as_deref(),
            ));
        }
    }
    Ok(specs)
}

fn instance_spec_for_artifact(
    prepared: &PreparedProfileApply,
    generation: &str,
    artifact: &BuildAppArtifact,
    default_app: Option<&str>,
) -> InstanceSpec {
    let instance_id = format!(
        "{}@{}@{}",
        artifact.app_id, generation, prepared.document.revision
    );
    InstanceSpec {
        schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
        instance_id,
        app_id: artifact.app_id.clone(),
        bundle: BundleRef {
            generation: generation.to_string(),
            bundle_path: artifact.bundle_path.clone(),
            digest: artifact.digest.clone(),
            toolchain_version: Some(crate::build_ops::toolchain_hint().to_string()),
            config_digest: artifact
                .config_digest
                .clone()
                .or_else(|| Some(prepared.document.revision.clone())),
        },
        config_snapshot: ConfigSnapshot {
            profile_id: prepared.document.id.clone(),
            profile_revision: prepared.document.revision.clone(),
            profile_file: prepared.document.file.clone(),
            runtime_plan: prepared.runtime_plan.clone(),
            default_app: default_app.map(str::to_string),
            ..Default::default()
        },
        runtime_abi: env!("CARGO_PKG_VERSION").to_string(),
        data_mode_ceiling: None,
    }
}

fn restore_active_environment(
    workspace: &Path,
    app_ids: &[String],
    old_generations: &BTreeMap<String, Option<String>>,
    old_links: &mei_lang_kernel::LinksState,
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

fn write_host_control_state(
    workspace: &Path,
    prepared: &PreparedProfileApply,
    env_version: &str,
    instance_specs: &[InstanceSpec],
) -> anyhow::Result<()> {
    let instance_ids = instance_specs
        .iter()
        .map(|spec| spec.instance_id.clone())
        .collect::<Vec<_>>();
    let last_successful_apply = LastSuccessfulApply {
        profile_id: prepared.document.id.clone(),
        profile_revision: prepared.document.revision.clone(),
        env_version: Some(env_version.to_string()),
        applied_at_ms: crate::state::current_time_ms(),
        instance_ids: instance_ids.clone(),
        apps: prepared
            .plan
            .apps
            .iter()
            .map(|app| app.app_id.clone())
            .collect(),
    };
    let mut state = mei_host_core::read_host_control_state(workspace)
        .unwrap_or_else(mei_host_core::HostControlState::empty);
    state.active_profile = Some(mei_host_core::ActiveProfileRef {
        id: prepared.document.id.clone(),
        revision: prepared.document.revision.clone(),
        file: prepared.document.file.clone(),
    });
    state.runtime_plan = Some(prepared.runtime_plan.clone());

    let mut manifest = state.launch_manifest.clone();
    if manifest.workspace_root.is_none() {
        manifest.workspace_root = Some(workspace.display().to_string());
    }
    for spec in instance_specs {
        let _ = mei_host_core::write_instance_spec(workspace, spec);
        manifest.instances.insert(
            spec.instance_id.clone(),
            DesiredInstance {
                spec_ref: spec.spec_digest(),
                desired_state: DesiredState::Running,
            },
        );
        let route = manifest
            .routes
            .entry(spec.app_id.clone())
            .or_insert_with(|| mei_host_core::RouteBinding {
                active: None,
                candidate: None,
                previous: None,
            });
        // Register as candidate; atomic cutover happens after launch ready.
        route.candidate = Some(spec.instance_id.clone());
    }
    manifest.last_successful_apply = Some(last_successful_apply);
    state.launch_manifest = manifest.with_recomputed_revision();
    state.sync_compat_fields();
    mei_host_core::write_host_control_state(workspace, &state)
}

fn set_phase(state: &SharedState, phase: &str, message: &str) {
    {
        let mut guard = state.write().expect("state lock");
        update_ops_job_phase(&mut guard, phase, message);
    }
    crate::instance_api::emit_builder_phase(state, phase, message);
}

fn set_app(state: &SharedState, app_id: &str, phase: &str, completed: bool, message: &str) {
    let mut guard = state.write().expect("state lock");
    update_ops_app_progress(&mut guard, app_id, phase, completed, message);
}

fn emit(state: &SharedState, event_type: &str, payload: serde_json::Value) {
    let guard = state.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(event_type, payload));
}

/// Submit a BuildRequest through the same ops lock / worker path used by apply-profile.
pub fn start_build_request_job(
    state: SharedState,
    request: BuildRequest,
) -> Result<String, String> {
    request.validate()?;
    let job_id = format!("build-{}", crate::state::current_time_ms());
    {
        let mut guard = state.write().expect("state lock");
        begin_profile_ops_job(
            &mut guard,
            request.profile_id.as_str(),
            request.profile_revision.as_str(),
            request.apps.as_slice(),
        )?;
        if let Some(job) = guard.ops_job.as_mut() {
            job.kind = "build".to_string();
            job.append_log(format!("job id: {job_id}"));
        }
    }
    let job_id_for_task = job_id.clone();
    tokio::spawn(async move {
        let workspace = {
            let guard = state.read().expect("state lock");
            guard.ctx.workspace_root.clone()
        };
        let result =
            tokio::task::spawn_blocking(move || run_build_request(workspace.as_path(), &request))
                .await
                .map_err(|error| format!("build task join failed: {error}"))
                .and_then(|inner| inner.map_err(|error| error.to_string()));
        let mut guard = state.write().expect("state lock");
        match result {
            Ok(build) => {
                if let Some(generation) = build.generation.as_deref() {
                    update_ops_job_generation(&mut guard, generation);
                }
                for phase in &build.phases {
                    let message = format!("{} ({} ms)", phase.name, phase.ms);
                    update_ops_job_phase(&mut guard, phase.name.as_str(), message.as_str());
                }
                drop(guard);
                for phase in &build.phases {
                    crate::instance_api::emit_builder_phase(
                        &state,
                        phase.name.as_str(),
                        format!("{} ({} ms)", phase.name, phase.ms),
                    );
                }
                let mut guard = state.write().expect("state lock");
                finish_ops_job_success(
                    &mut guard,
                    format!(
                        "build {job_id_for_task} ok generation={}",
                        build.generation.as_deref().unwrap_or("-")
                    ),
                );
            }
            Err(error) => finish_ops_job_failure(&mut guard, error),
        }
    });
    Ok(job_id)
}

/// Synchronous BuildRequest execution (still uses worker subprocess unless in-process env set).
pub fn run_build_request_sync(
    state: &SharedState,
    request: &BuildRequest,
) -> Result<BuildResult, String> {
    request.validate()?;
    {
        let mut guard = state.write().expect("state lock");
        begin_profile_ops_job(
            &mut guard,
            request.profile_id.as_str(),
            request.profile_revision.as_str(),
            request.apps.as_slice(),
        )?;
        if let Some(job) = guard.ops_job.as_mut() {
            job.kind = "build".to_string();
        }
    }
    let workspace = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let result = run_build_request(workspace.as_path(), request);
    let mut guard = state.write().expect("state lock");
    match &result {
        Ok(build) => {
            if let Some(generation) = build.generation.as_deref() {
                update_ops_job_generation(&mut guard, generation);
            }
            finish_ops_job_success(
                &mut guard,
                format!(
                    "build ok generation={}",
                    build.generation.as_deref().unwrap_or("-")
                ),
            );
        }
        Err(error) => finish_ops_job_failure(&mut guard, error.to_string()),
    }
    result.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_targets_only_discovered_apps_and_warms_hot_entries() {
        let dry_run: WorkspaceProfileDryRun = serde_json::from_value(json!({
            "profile": {
                "id": "local",
                "file": "configs/local.json",
                "revision": "r1",
                "valid": true,
                "issues": [],
                "label": null,
                "defaultApp": "hot-app",
                "defaultMode": "frozen",
                "configuredAppCount": 4
            },
            "defaultApp": "hot-app",
            "discoveredApps": ["hot-app", "lazy-app", "frozen-app", "metric-app"],
            "apps": [],
            "unresolvedScopes": [],
            "unresolvedMetrics": [],
            "deferred": []
        }))
        .expect("dry run");
        let plan: RuntimePlan = serde_json::from_value(json!({
            "defaultMode": "frozen",
            "apps": {
                "hot-app": {
                    "targets": [{"scope": "home/hot", "mode": "hot"}],
                    "metricOverrides": {}
                },
                "lazy-app": {
                    "targets": [{"scope": "home/lazy", "mode": "lazy"}],
                    "metricOverrides": {}
                },
                "frozen-app": {
                    "targets": [],
                    "metricOverrides": {}
                },
                "metric-app": {
                    "targets": [],
                    "metricOverrides": {"hot_metric": "hot"}
                },
                "excluded-app": {
                    "targets": [{"scope": "home", "mode": "hot"}],
                    "metricOverrides": {}
                }
            }
        }))
        .expect("runtime plan");
        let apply = build_apply_plan(
            std::path::Path::new("/tmp/nonexistent-mei-workspace"),
            &dry_run,
            &plan,
        );
        assert_eq!(
            apply
                .apps
                .iter()
                .map(|app| app.app_id.as_str())
                .collect::<Vec<_>>(),
            vec!["hot-app", "lazy-app", "frozen-app", "metric-app"]
        );
        assert!(apply.apps[0].warm);
        assert!(!apply.apps[1].warm);
        assert!(!apply.apps[2].warm);
        assert!(apply.apps[3].warm);
        assert_eq!(apply.apps[3].hot_metrics, vec!["hot_metric"]);
        assert_eq!(
            apply.pipeline,
            "build worker → launch instances → cutover route"
        );
        assert!(apply
            .apps
            .iter()
            .all(|app| app.launch_instance && app.cutover_route));
        assert!(apply.apps.iter().all(|app| app.bundle_action == "build"));
    }

    #[test]
    fn instance_specs_from_build_result() {
        let prepared = PreparedProfileApply {
            document: WorkspaceProfileDocument {
                id: "local".to_string(),
                file: "configs/local.json".to_string(),
                revision: "r1".to_string(),
                config: json!({
                    "workspace": {"defaultApp": "mini-data"}
                }),
                validation: mei_lang_kernel::WorkspaceProfileValidation {
                    valid: true,
                    issues: Vec::new(),
                },
            },
            runtime_plan: RuntimePlan {
                default_mode: RuntimeMode::Lazy,
                apps: BTreeMap::new(),
            },
            plan: ApplyProfilePlan {
                profile_id: "local".to_string(),
                profile_revision: "r1".to_string(),
                apps: vec![ApplyProfileAppPlan {
                    app_id: "mini-data".to_string(),
                    compile: true,
                    import: true,
                    publish_snapshots: true,
                    warm: false,
                    hot_targets: Vec::new(),
                    hot_metrics: Vec::new(),
                    bundle_action: "build".to_string(),
                    generation: None,
                    launch_instance: true,
                    cutover_route: true,
                }],
                snapshot_policy: "conservative-per-target-app".to_string(),
                pipeline: "build worker → launch instances → cutover route".to_string(),
            },
            config_path: PathBuf::from("configs/local.json"),
        };
        let build = BuildResult::success(
            "WS-1",
            vec![BuildAppArtifact {
                app_id: "mini-data".to_string(),
                bundle_path: "apps/mini-data/env/WS-1".to_string(),
                digest: Some("d".to_string()),
                config_digest: Some("r1".to_string()),
            }],
        );
        let specs = instance_specs_from_build(&prepared, &build).expect("specs");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].app_id, "mini-data");
        assert_eq!(specs[0].bundle.generation, "WS-1");
        assert!(specs[0].instance_id.contains("mini-data@WS-1@r1"));
    }

    #[test]
    fn apply_profile_uses_worker_stub_not_in_process_compile() {
        let _env_guard = crate::build_worker::BUILD_WORKER_ENV_LOCK
            .lock()
            .expect("worker env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        fs::create_dir_all(workspace.join("configs")).expect("configs");
        fs::write(
            workspace.join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"t","defaultApp":"mini-data"}}"#,
        )
        .expect("workspace");
        fs::write(
            workspace.join("configs/local.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"t","defaultApp":"mini-data"},"deploy":{"runtimePlan":{"defaultMode":"lazy","apps":{}}}}"#,
        )
        .expect("profile");
        fs::create_dir_all(workspace.join("apps/mini-data/src")).expect("app");
        fs::write(
            workspace.join("apps/mini-data/app.config.json"),
            r#"{"schemaVersion":1,"app":{"id":"mini-data"}}"#,
        )
        .expect("app config");

        let stub = tmp.path().join("fake-worker");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = r#"#!/bin/sh
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
mkdir -p "$(dirname "$out")"
cat > "$out" <<'EOF'
{
  "schemaVersion": "mei-build-result-v1",
  "ok": true,
  "generation": "WS-apply-stub.1",
  "apps": [{"appId":"mini-data","bundlePath":"apps/mini-data/env/WS-apply-stub.1","digest":"abc","configDigest":"r1"}],
  "error": null,
  "phases": [{"name":"compiling","ok":true,"ms":1},{"name":"importing","ok":true,"ms":1},{"name":"snapshotting","ok":true,"ms":1},{"name":"sealing","ok":true,"ms":1}]
}
EOF
"#;
            fs::write(&stub, script).expect("stub");
            let mut perms = fs::metadata(&stub).expect("meta").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&stub, perms).expect("chmod");
        }

        // Ensure we do not take the in-process path; stub binary must be used.
        std::env::remove_var("MEI_BUILD_WORKER_IN_PROCESS");
        std::env::set_var("MEI_BUILD_WORKER_BIN", &stub);

        let request = build_request_from_profile(
            "local",
            "ignored-by-stub",
            "configs/local.json",
            &["mini-data".to_string()],
        );
        let result = run_build_request(workspace, &request).expect("worker stub");
        std::env::remove_var("MEI_BUILD_WORKER_BIN");
        assert!(result.ok);
        assert_eq!(result.generation.as_deref(), Some("WS-apply-stub.1"));
        // Stub never creates a real meibundle — proves Host did not compile in-process.
        assert!(!workspace
            .join("apps/mini-data/env/WS-apply-stub.1/build/exchange/mini-data.meibundle")
            .exists());
    }
}
