//! Atomic route cutover / rollback and safe instance+bundle GC.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mei_host_core::{
    read_host_control_state, read_instance_spec, write_if_revision_matches, write_instance_spec,
    DesiredInstance, DesiredState, HostControlConflict, InstanceSpec, LaunchManifest, RouteBinding,
};
use mei_lang_kernel::{attach_build_generation, CleanEnvPolicy};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app_runtime_supervisor::AppRuntimeSupervisor;
use crate::state::{HostEvent, HostHttpState, SharedState};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutoverRequest {
    pub instance_id: String,
    pub expected_manifest_revision: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RollbackRequest {
    #[serde(default)]
    pub expected_manifest_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutoverResult {
    pub app_id: String,
    pub active: String,
    pub previous: Option<String>,
    pub manifest_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteLifecycleError {
    Conflict(String),
    NotFound(String),
    NotReady(String),
    Other(String),
}

impl RouteLifecycleError {
    fn status(&self) -> StatusCode {
        match self {
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::NotReady(_) => StatusCode::CONFLICT,
            Self::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::Conflict(_) => "route_conflict",
            Self::NotFound(_) => "route_not_found",
            Self::NotReady(_) => "instance_not_ready",
            Self::Other(_) => "route_lifecycle_failed",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Conflict(msg) | Self::NotFound(msg) | Self::NotReady(msg) | Self::Other(msg) => {
                msg.as_str()
            }
        }
    }
}

/// Pure CAS cutover of one app route. Does not touch Host processes.
pub fn cutover_route_in_manifest(
    mut manifest: LaunchManifest,
    app_id: &str,
    target_instance_id: &str,
) -> Result<LaunchManifest, RouteLifecycleError> {
    if !manifest.instances.contains_key(target_instance_id) {
        return Err(RouteLifecycleError::NotFound(format!(
            "instance `{target_instance_id}` is not in LaunchManifest"
        )));
    }
    let route = manifest
        .routes
        .entry(app_id.to_string())
        .or_insert(RouteBinding {
            active: None,
            candidate: None,
            previous: None,
        });
    let old_active = route.active.clone();
    if old_active.as_deref() == Some(target_instance_id) {
        route.candidate = None;
        return Ok(manifest.with_recomputed_revision());
    }
    route.previous = old_active.clone();
    route.active = Some(target_instance_id.to_string());
    route.candidate = None;

    if let Some(desired) = manifest.instances.get_mut(target_instance_id) {
        desired.desired_state = DesiredState::Running;
    }
    if let Some(old_id) = old_active {
        if old_id != target_instance_id {
            if let Some(desired) = manifest.instances.get_mut(old_id.as_str()) {
                desired.desired_state = DesiredState::Standby;
            }
        }
    }
    Ok(manifest.with_recomputed_revision())
}

/// Rollback active ← previous for one app.
pub fn rollback_route_in_manifest(
    manifest: LaunchManifest,
    app_id: &str,
) -> Result<(LaunchManifest, String), RouteLifecycleError> {
    let route = manifest.routes.get(app_id).cloned().ok_or_else(|| {
        RouteLifecycleError::NotFound(format!("no route binding for app `{app_id}`"))
    })?;
    let previous = route.previous.clone().ok_or_else(|| {
        RouteLifecycleError::Conflict(format!(
            "app `{app_id}` has no previous instance to rollback to"
        ))
    })?;
    if !manifest.instances.contains_key(previous.as_str()) {
        return Err(RouteLifecycleError::NotFound(format!(
            "previous instance `{previous}` is not in LaunchManifest"
        )));
    }
    let next = cutover_route_in_manifest(manifest, app_id, previous.as_str())?;
    Ok((next, previous))
}

/// Persist cutover with revision CAS. Optionally refresh compat symlink (no import/warm).
pub fn persist_cutover(
    workspace: &Path,
    expected_revision: &str,
    new_manifest: LaunchManifest,
    app_id: &str,
    target_instance_id: &str,
    update_compat_symlink: bool,
) -> Result<CutoverResult, RouteLifecycleError> {
    let revision = new_manifest.revision.clone();
    let previous = new_manifest
        .routes
        .get(app_id)
        .and_then(|route| route.previous.clone());
    match write_if_revision_matches(workspace, expected_revision, new_manifest) {
        Ok(()) => {}
        Err(HostControlConflict::Conflict { expected, current }) => {
            return Err(RouteLifecycleError::Conflict(format!(
                "launch manifest revision conflict: expected {expected}, current {current}"
            )));
        }
        Err(HostControlConflict::Io(error)) => {
            return Err(RouteLifecycleError::Other(error.to_string()));
        }
    }
    if update_compat_symlink {
        if let Some(spec) = read_instance_spec(workspace, target_instance_id) {
            let _ = attach_build_generation(
                workspace,
                std::slice::from_ref(&spec.app_id),
                spec.bundle.generation.as_str(),
            );
        }
    }
    Ok(CutoverResult {
        app_id: app_id.to_string(),
        active: target_instance_id.to_string(),
        previous,
        manifest_revision: revision,
    })
}

pub fn instance_is_ready(
    supervisor: &Option<AppRuntimeSupervisor>,
    shell_ready_ids: &BTreeSet<String>,
    instance_id: &str,
) -> bool {
    if shell_ready_ids.contains(instance_id) {
        return true;
    }
    supervisor
        .as_ref()
        .and_then(|sup| sup.runtime_for(instance_id))
        .is_some()
}

pub async fn api_host_route_cutover(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
    Json(body): Json<CutoverRequest>,
) -> Response {
    match execute_cutover(&http, app_id.as_str(), &body, true).await {
        Ok(result) => {
            emit_route_event(&http.shell, "route-cutover", &result);
            Json(json!({
                "accepted": true,
                "kind": "route-cutover",
                "result": result,
            }))
            .into_response()
        }
        Err(error) => route_error(error),
    }
}

pub async fn api_host_route_rollback(
    State(http): State<HostHttpState>,
    AxumPath(app_id): AxumPath<String>,
    Json(body): Json<RollbackRequest>,
) -> Response {
    match execute_rollback(&http, app_id.as_str(), &body).await {
        Ok(result) => {
            emit_route_event(&http.shell, "route-rollback", &result);
            Json(json!({
                "accepted": true,
                "kind": "route-rollback",
                "result": result,
            }))
            .into_response()
        }
        Err(error) => route_error(error),
    }
}

pub async fn execute_cutover(
    http: &HostHttpState,
    app_id: &str,
    body: &CutoverRequest,
    update_compat_symlink: bool,
) -> Result<CutoverResult, RouteLifecycleError> {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let instance_id = body.instance_id.trim();
    if instance_id.is_empty() {
        return Err(RouteLifecycleError::Conflict(
            "instanceId must not be empty".to_string(),
        ));
    }
    let (manifest, shell_ready) = {
        let guard = http.shell.read().expect("state lock");
        let manifest = if guard.launch_manifest.revision.is_empty() {
            read_host_control_state(workspace.as_path())
                .map(|state| state.launch_manifest)
                .unwrap_or_else(LaunchManifest::empty)
        } else {
            guard.launch_manifest.clone()
        };
        let ready = guard
            .app_runtime_by_instance
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        (manifest, ready)
    };
    if manifest.revision != body.expected_manifest_revision {
        return Err(RouteLifecycleError::Conflict(format!(
            "launch manifest revision conflict: expected {}, current {}",
            body.expected_manifest_revision, manifest.revision
        )));
    }
    {
        let supervisor = http
            .app_runtime
            .lock()
            .map_err(|_| RouteLifecycleError::Other("app-runtime supervisor poisoned".into()))?;
        if !instance_is_ready(&supervisor, &shell_ready, instance_id) {
            return Err(RouteLifecycleError::NotReady(format!(
                "instance `{instance_id}` is not Ready"
            )));
        }
    }
    let expected = manifest.revision.clone();
    let next = cutover_route_in_manifest(manifest, app_id, instance_id)?;
    let result = persist_cutover(
        workspace.as_path(),
        expected.as_str(),
        next.clone(),
        app_id,
        instance_id,
        update_compat_symlink,
    )?;
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.install_launch_manifest(next);
    }
    Ok(result)
}

pub async fn execute_rollback(
    http: &HostHttpState,
    app_id: &str,
    body: &RollbackRequest,
) -> Result<CutoverResult, RouteLifecycleError> {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let manifest = read_host_control_state(workspace.as_path())
        .map(|state| state.launch_manifest)
        .unwrap_or_else(|| {
            http.shell
                .read()
                .expect("state lock")
                .launch_manifest
                .clone()
        });
    if let Some(expected) = body.expected_manifest_revision.as_deref() {
        if manifest.revision != expected {
            return Err(RouteLifecycleError::Conflict(format!(
                "launch manifest revision conflict: expected {expected}, current {}",
                manifest.revision
            )));
        }
    }
    let previous = manifest
        .routes
        .get(app_id)
        .and_then(|route| route.previous.clone())
        .ok_or_else(|| {
            RouteLifecycleError::Conflict(format!(
                "app `{app_id}` has no previous instance to rollback to"
            ))
        })?;

    ensure_instance_ready(http, workspace.as_path(), previous.as_str()).await?;

    let expected = manifest.revision.clone();
    let (next, target) = rollback_route_in_manifest(manifest, app_id)?;
    let result = persist_cutover(
        workspace.as_path(),
        expected.as_str(),
        next.clone(),
        app_id,
        target.as_str(),
        true,
    )?;
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.install_launch_manifest(next);
    }
    Ok(result)
}

async fn ensure_instance_ready(
    http: &HostHttpState,
    workspace: &Path,
    instance_id: &str,
) -> Result<(), RouteLifecycleError> {
    let shell_ready = {
        let guard = http.shell.read().expect("state lock");
        guard.app_runtime_by_instance.contains_key(instance_id)
    };
    {
        let supervisor = http
            .app_runtime
            .lock()
            .map_err(|_| RouteLifecycleError::Other("app-runtime supervisor poisoned".into()))?;
        if instance_is_ready(&supervisor, &BTreeSet::new(), instance_id) || shell_ready {
            return Ok(());
        }
    }
    // Restart from persisted spec when previous is not ready.
    let spec = read_instance_spec(workspace, instance_id).ok_or_else(|| {
        RouteLifecycleError::NotReady(format!(
            "previous instance `{instance_id}` is not ready and has no persisted InstanceSpec"
        ))
    })?;
    let mut supervisor = {
        let mut slot = http
            .app_runtime
            .lock()
            .map_err(|_| RouteLifecycleError::Other("app-runtime supervisor poisoned".into()))?;
        slot.take()
            .unwrap_or_else(|| AppRuntimeSupervisor::new(workspace.to_path_buf()))
    };
    let restart_result = if supervisor.runtime_for(instance_id).is_some() {
        supervisor.restart_with_backoff(instance_id, 3).await
    } else {
        let token = crate::app_runtime_supervisor::generate_instance_token(instance_id);
        supervisor.spawn_instance(spec, token).await
    };
    let endpoints = supervisor.endpoint_map();
    let started_at = supervisor.started_at_map();
    {
        let mut slot = http
            .app_runtime
            .lock()
            .map_err(|_| RouteLifecycleError::Other("app-runtime supervisor poisoned".into()))?;
        *slot = Some(supervisor);
    }
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.sync_app_runtime_endpoints_with_started(endpoints, started_at);
    }
    restart_result
        .map(|_| ())
        .map_err(|error| RouteLifecycleError::NotReady(error.to_string()))
}

/// Launch candidates, wait ready, then cut over each app. On failure stop candidates and
/// leave routes unchanged.
pub async fn launch_candidates_and_cutover(
    http: &HostHttpState,
    specs: &[InstanceSpec],
    expected_revision: &str,
) -> Result<Vec<CutoverResult>, RouteLifecycleError> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    for spec in specs {
        let _ = write_instance_spec(workspace.as_path(), spec);
    }

    let launched_ids = match launch_specs(http, specs).await {
        Ok(ids) => ids,
        Err(error) => {
            let _ = stop_instances(http, specs.iter().map(|s| s.instance_id.as_str())).await;
            return Err(error);
        }
    };

    let mut results = Vec::new();
    let mut revision = expected_revision.to_string();
    for spec in specs {
        let body = CutoverRequest {
            instance_id: spec.instance_id.clone(),
            expected_manifest_revision: revision.clone(),
        };
        match execute_cutover(http, spec.app_id.as_str(), &body, true).await {
            Ok(result) => {
                revision = result.manifest_revision.clone();
                results.push(result);
            }
            Err(error) => {
                let _ = stop_instances(http, launched_ids.iter().map(String::as_str)).await;
                return Err(error);
            }
        }
    }
    Ok(results)
}

async fn launch_specs(
    http: &HostHttpState,
    specs: &[InstanceSpec],
) -> Result<Vec<String>, RouteLifecycleError> {
    let mut launched = Vec::new();
    let mut supervisor = {
        let mut slot = http
            .app_runtime
            .lock()
            .map_err(|_| RouteLifecycleError::Other("app-runtime supervisor poisoned".into()))?;
        match slot.take() {
            Some(supervisor) => supervisor,
            None => {
                drop(slot);
                let mut guard = http.shell.write().expect("state lock");
                for spec in specs {
                    guard.register_app_runtime_endpoint(
                        spec.instance_id.clone(),
                        format!("pending://{}", spec.instance_id),
                        Some(crate::state::current_time_ms()),
                    );
                    launched.push(spec.instance_id.clone());
                }
                return Ok(launched);
            }
        }
    };
    for spec in specs {
        if supervisor.runtime_for(spec.instance_id.as_str()).is_some() {
            launched.push(spec.instance_id.clone());
            continue;
        }
        crate::instance_api::emit_instance_id_event(
            &http.shell,
            "instance-phase",
            spec.instance_id.as_str(),
            mei_host_core::InstancePhase::Launching,
            None,
        );
        let token =
            crate::app_runtime_supervisor::generate_instance_token(spec.instance_id.as_str());
        match supervisor.spawn_instance(spec.clone(), token).await {
            Ok(observed) => {
                crate::instance_api::emit_instance_event(
                    &http.shell,
                    "instance-ready",
                    &observed,
                    None,
                );
                launched.push(spec.instance_id.clone());
            }
            Err(error) => {
                crate::instance_api::emit_instance_id_event(
                    &http.shell,
                    "instance-failed",
                    spec.instance_id.as_str(),
                    mei_host_core::InstancePhase::Failed,
                    Some(error.to_string().as_str()),
                );
                let endpoints = supervisor.endpoint_map();
                let started_at = supervisor.started_at_map();
                {
                    let mut slot = http.app_runtime.lock().map_err(|_| {
                        RouteLifecycleError::Other("app-runtime supervisor poisoned".into())
                    })?;
                    *slot = Some(supervisor);
                }
                {
                    let mut guard = http.shell.write().expect("state lock");
                    guard.sync_app_runtime_endpoints_with_started(endpoints, started_at);
                }
                return Err(RouteLifecycleError::NotReady(format!(
                    "failed to launch candidate {}: {error}",
                    spec.instance_id
                )));
            }
        }
    }
    let endpoints = supervisor.endpoint_map();
    let started_at = supervisor.started_at_map();
    {
        let mut slot = http
            .app_runtime
            .lock()
            .map_err(|_| RouteLifecycleError::Other("app-runtime supervisor poisoned".into()))?;
        *slot = Some(supervisor);
    }
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.sync_app_runtime_endpoints_with_started(endpoints, started_at);
    }
    Ok(launched)
}

pub async fn stop_instances(
    http: &HostHttpState,
    instance_ids: impl IntoIterator<Item = &str>,
) -> Result<(), RouteLifecycleError> {
    let ids = instance_ids
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut supervisor = {
        let mut slot = http
            .app_runtime
            .lock()
            .map_err(|_| RouteLifecycleError::Other("app-runtime supervisor poisoned".into()))?;
        match slot.take() {
            Some(supervisor) => supervisor,
            None => {
                drop(slot);
                let mut guard = http.shell.write().expect("state lock");
                for id in &ids {
                    guard.unregister_app_runtime_endpoint(id.as_str());
                }
                return Ok(());
            }
        }
    };
    for id in &ids {
        let _ = supervisor.stop_instance(id.as_str()).await;
    }
    let endpoints = supervisor.endpoint_map();
    let started_at = supervisor.started_at_map();
    {
        let mut slot = http
            .app_runtime
            .lock()
            .map_err(|_| RouteLifecycleError::Other("app-runtime supervisor poisoned".into()))?;
        *slot = Some(supervisor);
    }
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.sync_app_runtime_endpoints_with_started(endpoints, started_at);
    }
    Ok(())
}

/// Instance GC: delete stopped instance private dirs that are not referenced by routes.
pub fn garbage_collect_instances(
    workspace: &Path,
    manifest: &LaunchManifest,
    dry_run: bool,
) -> Vec<String> {
    let protected = protected_instance_ids(manifest);
    let mut removed = Vec::new();

    // Legacy instance dirs under deploy/runtime/instances/{id}.
    let legacy_root = workspace.join("deploy/runtime/instances");
    if let Ok(entries) = fs::read_dir(&legacy_root) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let instance_id = entry.file_name().to_string_lossy().to_string();
            if protected.contains(instance_id.as_str()) {
                continue;
            }
            let desired = manifest.instances.get(instance_id.as_str());
            let is_stopped = desired
                .map(|entry| entry.desired_state == DesiredState::Stopped)
                .unwrap_or(true);
            if !is_stopped {
                continue;
            }
            let path = entry.path();
            if !dry_run {
                let _ = fs::remove_dir_all(&path);
            }
            removed.push(instance_id);
        }
    }

    // App ephemeral roots: clear when no protected route still references the app.
    let apps_root = workspace.join("deploy/runtime/apps");
    if let Ok(entries) = fs::read_dir(&apps_root) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let app_id = entry.file_name().to_string_lossy().to_string();
            let route = manifest.routes.get(app_id.as_str());
            let still_referenced = route.is_some_and(|route| {
                [&route.active, &route.candidate, &route.previous]
                    .into_iter()
                    .flatten()
                    .any(|id| protected.contains(id.as_str()))
            });
            if still_referenced {
                continue;
            }
            let path = entry.path();
            let marker = format!("app:{app_id}");
            if !dry_run {
                let _ = fs::remove_dir_all(&path);
            }
            if !removed.iter().any(|id| id == &marker) {
                removed.push(marker);
            }
        }
    }

    removed
}

pub fn protected_instance_ids(manifest: &LaunchManifest) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for route in manifest.routes.values() {
        for slot in [&route.active, &route.candidate, &route.previous] {
            if let Some(id) = slot {
                ids.insert(id.clone());
            }
        }
    }
    for (id, desired) in &manifest.instances {
        if matches!(
            desired.desired_state,
            DesiredState::Running | DesiredState::Standby
        ) {
            ids.insert(id.clone());
        }
    }
    ids
}

/// Bundle GC protection keyed by generation → reasons.
pub fn collect_bundle_protections(
    workspace: &Path,
    manifest: &LaunchManifest,
    running_generation: Option<&str>,
) -> BTreeMap<String, Vec<String>> {
    let mut protected = BTreeMap::<String, Vec<String>>::new();
    if let Some(generation) = running_generation {
        protected
            .entry(generation.to_string())
            .or_default()
            .push("ops-job".to_string());
    }
    for (app_id, route) in &manifest.routes {
        for (slot, label) in [
            (&route.active, "route:active"),
            (&route.candidate, "route:candidate"),
            (&route.previous, "route:previous"),
        ] {
            let Some(instance_id) = slot.as_deref() else {
                continue;
            };
            if let Some(generation) = generation_for_instance(workspace, manifest, instance_id) {
                let entry = protected.entry(generation).or_default();
                entry.push(label.to_string());
                entry.push(format!("instance:{instance_id}"));
                entry.push(format!("app:{app_id}"));
            }
        }
    }
    for (instance_id, desired) in &manifest.instances {
        if !matches!(
            desired.desired_state,
            DesiredState::Running | DesiredState::Standby
        ) {
            continue;
        }
        if let Some(generation) = generation_for_instance(workspace, manifest, instance_id) {
            let entry = protected.entry(generation).or_default();
            entry.push(format!("instance:{instance_id}"));
            match desired.desired_state {
                DesiredState::Running => entry.push("desired:running".to_string()),
                DesiredState::Standby => entry.push("desired:standby".to_string()),
                DesiredState::Stopped => {}
            }
        }
    }
    // Specs on disk protect generations only while the instance is still referenced as
    // running/standby or by a route slot (not merely Stopped leftovers).
    let route_refs = protected_instance_ids(manifest);
    for instance_id in route_refs {
        if let Some(spec) = read_instance_spec(workspace, instance_id.as_str()) {
            protected
                .entry(spec.bundle.generation.clone())
                .or_default()
                .push(format!("spec:{instance_id}"));
        }
    }
    for reasons in protected.values_mut() {
        reasons.sort();
        reasons.dedup();
    }
    protected
}

pub fn generation_for_instance(
    workspace: &Path,
    _manifest: &LaunchManifest,
    instance_id: &str,
) -> Option<String> {
    if let Some(spec) = read_instance_spec(workspace, instance_id) {
        return Some(spec.bundle.generation);
    }
    // Fallback: instanceId format `{app}@{generation}@{revision}`
    let parts = instance_id.split('@').collect::<Vec<_>>();
    if parts.len() >= 2 && parts[1].starts_with("WS-") {
        return Some(parts[1].to_string());
    }
    None
}

pub fn cleanup_policy_from_manifest(
    workspace: &Path,
    manifest: &LaunchManifest,
    retain_build_generations: Option<u32>,
    running_generation: Option<&str>,
    dry_run: bool,
) -> CleanEnvPolicy {
    CleanEnvPolicy {
        dry_run,
        retain_generations: retain_build_generations.map(|value| value as usize),
        protected_generations: collect_bundle_protections(workspace, manifest, running_generation),
    }
}

/// Register candidate instances on LaunchManifest without switching active.
pub fn register_candidates_on_manifest(
    mut manifest: LaunchManifest,
    workspace: &Path,
    specs: &[InstanceSpec],
) -> Result<LaunchManifest, anyhow::Error> {
    if manifest.workspace_root.is_none() {
        manifest.workspace_root = Some(workspace.display().to_string());
    }
    for spec in specs {
        write_instance_spec(workspace, spec)?;
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
            .or_insert(RouteBinding {
                active: None,
                candidate: None,
                previous: None,
            });
        route.candidate = Some(spec.instance_id.clone());
    }
    Ok(manifest.with_recomputed_revision())
}

fn emit_route_event(shell: &SharedState, event_type: &str, result: &CutoverResult) {
    let guard = shell.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(
        event_type,
        json!({
            "appId": result.app_id,
            "active": result.active,
            "previous": result.previous,
            "manifestRevision": result.manifest_revision,
        }),
    ));
}

fn route_error(error: RouteLifecycleError) -> Response {
    (
        error.status(),
        Json(json!({
            "error": {
                "code": error.code(),
                "message": error.message(),
            }
        })),
    )
        .into_response()
}

/// Helper used by apply-profile after Build Worker success.
pub async fn cutover_after_apply(
    state: &SharedState,
    app_runtime_slot: &Arc<Mutex<Option<AppRuntimeSupervisor>>>,
    specs: &[InstanceSpec],
) -> anyhow::Result<()> {
    let http = HostHttpState {
        shell: state.clone(),
        auth: mei_host_auth::AuthServeState::new(
            {
                let guard = state.read().expect("state lock");
                guard.ctx.workspace_root.clone()
            },
            mei_host_auth::AuthEnforcement::Disabled,
        ),
        managed_plug: Arc::new(Mutex::new(None)),
        app_runtime: app_runtime_slot.clone(),
    };
    let expected = {
        let guard = state.read().expect("state lock");
        guard.launch_manifest.revision.clone()
    };
    launch_candidates_and_cutover(&http, specs, expected.as_str())
        .await
        .map(|_| ())
        .map_err(|error| anyhow::anyhow!("{}", error.message()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_host_core::{
        instance_runtime_root, legacy_instance_runtime_root, write_host_control_state, BundleRef,
        ConfigSnapshot, HostControlState, SCHEMA_INSTANCE_SPEC_V1,
    };
    use mei_lang_kernel::{RuntimeMode, RuntimePlan};

    fn sample_spec(instance_id: &str, app_id: &str, generation: &str) -> InstanceSpec {
        InstanceSpec {
            schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
            instance_id: instance_id.to_string(),
            app_id: app_id.to_string(),
            bundle: BundleRef {
                generation: generation.to_string(),
                bundle_path: format!("apps/{app_id}/env/{generation}"),
                digest: None,
                toolchain_version: None,
                config_digest: None,
            },
            config_snapshot: ConfigSnapshot {
                profile_id: "local".to_string(),
                profile_revision: "r1".to_string(),
                profile_file: "configs/local.json".to_string(),
                runtime_plan: RuntimePlan {
                    default_mode: RuntimeMode::Lazy,
                    apps: Default::default(),
                },
                default_app: None,
                ..Default::default()
            },
            runtime_abi: "1".to_string(),
            data_mode_ceiling: None,
        }
    }

    fn seed_manifest(workspace: &Path) -> LaunchManifest {
        let mut manifest = LaunchManifest::empty();
        for (id, gen) in [("inst-old", "WS-20260711.0"), ("inst-new", "WS-20260712.0")] {
            let spec = sample_spec(id, "mini-data", gen);
            // Keep per-instance specs under legacy dirs so multi-slot tests can resolve generations.
            let legacy = legacy_instance_runtime_root(workspace, id);
            fs::create_dir_all(&legacy).expect("legacy");
            fs::write(
                legacy.join("spec.json"),
                serde_json::to_vec_pretty(&spec).expect("ser"),
            )
            .expect("write legacy spec");
            // Active write path still uses app ephemeral root (last wins for same app).
            write_instance_spec(workspace, &spec).expect("spec");
            manifest.instances.insert(
                id.to_string(),
                DesiredInstance {
                    spec_ref: spec.spec_digest(),
                    desired_state: DesiredState::Running,
                },
            );
        }
        manifest.routes.insert(
            "mini-data".to_string(),
            RouteBinding {
                active: Some("inst-old".to_string()),
                candidate: Some("inst-new".to_string()),
                previous: None,
            },
        );
        let manifest = manifest.with_recomputed_revision();
        let control = HostControlState::new(manifest.clone());
        write_host_control_state(workspace, &control).expect("write control");
        manifest
    }

    #[test]
    fn cutover_cas_conflict() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest = seed_manifest(tmp.path());
        let next =
            cutover_route_in_manifest(manifest.clone(), "mini-data", "inst-new").expect("cutover");
        let err = persist_cutover(
            tmp.path(),
            "stale-revision",
            next,
            "mini-data",
            "inst-new",
            false,
        )
        .expect_err("must conflict");
        assert!(matches!(err, RouteLifecycleError::Conflict(_)));
        let loaded = read_host_control_state(tmp.path())
            .expect("control")
            .launch_manifest;
        assert_eq!(loaded.revision, manifest.revision);
        assert_eq!(
            loaded.routes["mini-data"].active.as_deref(),
            Some("inst-old")
        );
    }

    #[test]
    fn cutover_then_rollback_to_previous() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest = seed_manifest(tmp.path());
        let expected = manifest.revision.clone();
        let next =
            cutover_route_in_manifest(manifest, "mini-data", "inst-new").expect("cutover plan");
        let result = persist_cutover(
            tmp.path(),
            expected.as_str(),
            next.clone(),
            "mini-data",
            "inst-new",
            false,
        )
        .expect("persist");
        assert_eq!(result.active, "inst-new");
        assert_eq!(result.previous.as_deref(), Some("inst-old"));
        assert_eq!(
            next.instances["inst-old"].desired_state,
            DesiredState::Standby
        );

        let after = read_host_control_state(tmp.path())
            .expect("control")
            .launch_manifest;
        let (rolled, target) =
            rollback_route_in_manifest(after.clone(), "mini-data").expect("rollback plan");
        assert_eq!(target, "inst-old");
        let expected = after.revision.clone();
        persist_cutover(
            tmp.path(),
            expected.as_str(),
            rolled.clone(),
            "mini-data",
            "inst-old",
            false,
        )
        .expect("rollback persist");
        assert_eq!(
            rolled.routes["mini-data"].active.as_deref(),
            Some("inst-old")
        );
        assert_eq!(
            rolled.routes["mini-data"].previous.as_deref(),
            Some("inst-new")
        );
    }

    #[test]
    fn cleanup_does_not_delete_protected_bundle_or_instance() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        let manifest = seed_manifest(workspace);

        // Extra stopped instance under legacy instances path + orphan generation dir.
        let orphan = sample_spec("inst-orphan", "mini-data", "WS-20260710.0");
        let legacy = legacy_instance_runtime_root(workspace, "inst-orphan");
        fs::create_dir_all(&legacy).expect("legacy dir");
        fs::write(
            legacy.join("spec.json"),
            serde_json::to_vec_pretty(&orphan).expect("ser"),
        )
        .expect("orphan legacy spec");
        fs::create_dir_all(workspace.join("apps/mini-data/env/WS-20260710.0")).expect("gen");
        fs::create_dir_all(workspace.join("apps/mini-data/env/WS-20260711.0")).expect("gen");
        fs::create_dir_all(workspace.join("apps/mini-data/env/WS-20260712.0")).expect("gen");

        let mut manifest = manifest;
        manifest.instances.insert(
            "inst-orphan".to_string(),
            DesiredInstance {
                spec_ref: orphan.spec_digest(),
                desired_state: DesiredState::Stopped,
            },
        );
        manifest = manifest.with_recomputed_revision();

        let protections = collect_bundle_protections(workspace, &manifest, Some("WS-job"));
        assert!(protections
            .get("WS-20260711.0")
            .is_some_and(|r| r.iter().any(|x| x == "route:active")));
        assert!(protections
            .get("WS-20260712.0")
            .is_some_and(|r| r.iter().any(|x| x == "route:candidate")));
        assert!(protections
            .get("WS-job")
            .is_some_and(|r| r.iter().any(|x| x == "ops-job")));
        assert!(
            !protections.contains_key("WS-20260710.0")
                || !protections["WS-20260710.0"]
                    .iter()
                    .any(|r| r.starts_with("route:"))
        );

        let removed = garbage_collect_instances(workspace, &manifest, false);
        assert!(removed.contains(&"inst-orphan".to_string()));
        assert!(!removed.contains(&"inst-old".to_string()));
        assert!(!removed.contains(&"inst-new".to_string()));
        assert!(!legacy_instance_runtime_root(workspace, "inst-orphan").exists());
        assert!(instance_runtime_root(workspace, "mini-data").exists());
    }

    #[test]
    fn activate_failure_does_not_cut_route() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest = seed_manifest(tmp.path());
        let before = manifest.routes["mini-data"].clone();
        // Simulate activate failure: candidates launched but cutover rejected by CAS.
        let planned =
            cutover_route_in_manifest(manifest.clone(), "mini-data", "inst-new").expect("plan");
        let err = persist_cutover(tmp.path(), "wrong", planned, "mini-data", "inst-new", false)
            .expect_err("cas fail");
        assert!(matches!(err, RouteLifecycleError::Conflict(_)));
        let after = read_host_control_state(tmp.path())
            .expect("control")
            .launch_manifest;
        assert_eq!(after.routes["mini-data"].active, before.active);
        assert_eq!(after.routes["mini-data"].candidate, before.candidate);
    }
}
