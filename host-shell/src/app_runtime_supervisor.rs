//! Host-side App Runtime supervisor: spawn / stop / reconcile LaunchManifest instances.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mei_host_core::{
    BundleRef, ConfigSnapshot, DesiredState, InstanceHealth, InstancePhase, InstanceResource,
    InstanceRevisions, InstanceSpec, LaunchManifest, ObservedInstance, SCHEMA_INSTANCE_SPEC_V1,
};
use mei_lang_kernel::{RuntimeMode, RuntimePlan};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

const MANAGED_APP_RUNTIME_HOST: &str = "127.0.0.1";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(90);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(200);
const RESTART_BACKOFF_BASE: Duration = Duration::from_millis(500);
const RESTART_BACKOFF_MAX: Duration = Duration::from_secs(8);
const LISTEN_LINE_PREFIX: &str = "MEI_APP_RUNTIME_LISTEN=";

static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// One managed App Runtime child process.
pub struct ManagedRuntime {
    pub child: Child,
    pub endpoint: String,
    pub token: String,
    pub spec: InstanceSpec,
    /// Wall-clock ms when the child became healthy / entered the pool.
    pub started_at_ms: u64,
    /// Child pid / process-group id (Unix `process_group(0)`).
    pub child_pid: Option<u32>,
    /// Workspace used for `runtime.pid` cleanup on stop.
    pub workspace_root: PathBuf,
}

impl ManagedRuntime {
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        crate::stale_runtime_sweep::clear_runtime_pid_file(
            self.workspace_root.as_path(),
            self.spec.app_id.as_str(),
            self.spec.instance_id.as_str(),
        );
        if self.child.try_wait()?.is_some() {
            if let Some(pid) = self.child_pid {
                crate::process_group::kill_process_group_if_alive(pid);
            }
            return Ok(());
        }
        if let Err(error) = self.child.start_kill() {
            if error.kind() != std::io::ErrorKind::InvalidInput {
                return Err(anyhow::anyhow!("stop managed app-runtime: {error}"));
            }
        }
        let _ = timeout(Duration::from_secs(3), self.child.wait()).await;
        if let Some(pid) = self.child_pid {
            crate::process_group::kill_process_group_if_alive(pid);
        }
        Ok(())
    }
}

/// In-memory pool keyed by `instance_id`.
pub struct AppRuntimeSupervisor {
    pub runtimes: BTreeMap<String, ManagedRuntime>,
    pub workspace_root: PathBuf,
}

impl AppRuntimeSupervisor {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            runtimes: BTreeMap::new(),
            workspace_root: workspace_root.into(),
        }
    }

    pub fn endpoint_map(&self) -> BTreeMap<String, String> {
        self.runtimes
            .iter()
            .map(|(id, rt)| (id.clone(), rt.endpoint.clone()))
            .collect()
    }

    pub fn started_at_map(&self) -> BTreeMap<String, u64> {
        self.runtimes
            .iter()
            .map(|(id, rt)| (id.clone(), rt.started_at_ms))
            .collect()
    }

    pub fn runtime_for(&self, instance_id: &str) -> Option<&ManagedRuntime> {
        self.runtimes.get(instance_id)
    }

    /// Spawn one instance and insert into the pool.
    ///
    /// Prefer [`spawn_into`] when calling through a shared mutex so the lock is
    /// not held across the slow process health-check.
    pub async fn spawn_instance(
        &mut self,
        spec: InstanceSpec,
        token: impl Into<String>,
    ) -> anyhow::Result<ObservedInstance> {
        let token = token.into();
        let instance_id = spec.instance_id.clone();
        if self.runtimes.contains_key(instance_id.as_str()) {
            anyhow::bail!("instance `{instance_id}` is already managed");
        }
        let managed =
            spawn_managed_runtime(self.workspace_root.as_path(), &spec, token.as_str()).await?;
        let observed = observed_from_managed(&managed, DesiredState::Running, None);
        self.runtimes.insert(instance_id, managed);
        Ok(observed)
    }

    pub async fn stop_instance(&mut self, instance_id: &str) -> anyhow::Result<()> {
        let Some(mut managed) = self.runtimes.remove(instance_id) else {
            return Ok(());
        };
        managed.shutdown().await
    }

    pub fn token_map(&self) -> BTreeMap<String, String> {
        self.runtimes
            .iter()
            .map(|(id, rt)| (id.clone(), rt.token.clone()))
            .collect()
    }

    pub fn generation_map(&self) -> BTreeMap<String, String> {
        self.runtimes
            .iter()
            .map(|(id, rt)| (id.clone(), rt.spec.bundle.generation.clone()))
            .collect()
    }

    pub fn digest_map(&self) -> BTreeMap<String, String> {
        self.runtimes
            .iter()
            .map(|(id, rt)| (id.clone(), rt.spec.spec_digest()))
            .collect()
    }
}

/// Shared resident supervisor (never taken out of the slot as `None`).
pub type SharedAppRuntime = Arc<tokio::sync::Mutex<AppRuntimeSupervisor>>;

pub fn empty_shared_app_runtime(workspace: impl Into<PathBuf>) -> SharedAppRuntime {
    Arc::new(tokio::sync::Mutex::new(AppRuntimeSupervisor::new(
        workspace,
    )))
}

/// Spawn without holding the shared mutex across process startup (≤30s).
pub async fn spawn_into(
    shared: &SharedAppRuntime,
    spec: InstanceSpec,
    token: impl Into<String>,
) -> anyhow::Result<ObservedInstance> {
    let token = token.into();
    let instance_id = spec.instance_id.clone();
    let workspace = {
        let guard = shared.lock().await;
        if guard.runtimes.contains_key(instance_id.as_str()) {
            anyhow::bail!("instance `{instance_id}` is already managed");
        }
        guard.workspace_root.clone()
    };
    let managed = spawn_managed_runtime(workspace.as_path(), &spec, token.as_str()).await?;
    let observed = observed_from_managed(&managed, DesiredState::Running, None);
    {
        let mut guard = shared.lock().await;
        if guard.runtimes.contains_key(instance_id.as_str()) {
            drop(guard);
            let mut dup = managed;
            let _ = dup.shutdown().await;
            anyhow::bail!("instance `{instance_id}` was inserted concurrently");
        }
        guard.runtimes.insert(instance_id, managed);
    }
    Ok(observed)
}

/// Remove + shutdown without holding the mutex across kill/wait.
pub async fn stop_from(shared: &SharedAppRuntime, instance_id: &str) -> anyhow::Result<()> {
    let mut managed = {
        let mut guard = shared.lock().await;
        match guard.runtimes.remove(instance_id) {
            Some(managed) => managed,
            None => return Ok(()),
        }
    };
    managed.shutdown().await
}

/// Restart with backoff without a global take() window.
pub async fn restart_from(
    shared: &SharedAppRuntime,
    instance_id: &str,
    max_attempts: u32,
) -> anyhow::Result<ObservedInstance> {
    let (spec, token) = {
        let guard = shared.lock().await;
        let managed = guard
            .runtimes
            .get(instance_id)
            .ok_or_else(|| anyhow::anyhow!("instance `{instance_id}` is not managed"))?;
        (managed.spec.clone(), managed.token.clone())
    };
    let _ = stop_from(shared, instance_id).await;
    let mut delay = RESTART_BACKOFF_BASE;
    let mut last_error = None;
    for attempt in 1..=max_attempts.max(1) {
        match spawn_into(shared, spec.clone(), token.clone()).await {
            Ok(observed) => return Ok(observed),
            Err(error) => {
                tracing::warn!(
                    instance_id = %instance_id,
                    attempt,
                    error = %error,
                    "app-runtime restart attempt failed"
                );
                last_error = Some(error);
                sleep(delay).await;
                delay = (delay * 2).min(RESTART_BACKOFF_MAX);
            }
        }
    }
    Err(last_error
        .unwrap_or_else(|| anyhow::anyhow!("app-runtime restart failed for `{instance_id}`")))
}

impl AppRuntimeSupervisor {
    /// Restart with exponential backoff. Returns the new observation.
    pub async fn restart_with_backoff(
        &mut self,
        instance_id: &str,
        max_attempts: u32,
    ) -> anyhow::Result<ObservedInstance> {
        let (spec, token) = {
            let managed = self
                .runtimes
                .get(instance_id)
                .ok_or_else(|| anyhow::anyhow!("instance `{instance_id}` is not managed"))?;
            (managed.spec.clone(), managed.token.clone())
        };
        let _ = self.stop_instance(instance_id).await;
        let mut delay = RESTART_BACKOFF_BASE;
        let mut last_error = None;
        for attempt in 1..=max_attempts.max(1) {
            match spawn_managed_runtime(self.workspace_root.as_path(), &spec, token.as_str()).await
            {
                Ok(managed) => {
                    let observed = observed_from_managed(&managed, DesiredState::Running, None);
                    self.runtimes.insert(instance_id.to_string(), managed);
                    return Ok(observed);
                }
                Err(error) => {
                    tracing::warn!(
                        instance_id = %instance_id,
                        attempt,
                        error = %error,
                        "app-runtime restart attempt failed"
                    );
                    last_error = Some(error);
                    sleep(delay).await;
                    delay = (delay * 2).min(RESTART_BACKOFF_MAX);
                }
            }
        }
        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("app-runtime restart failed for `{instance_id}`")))
    }

    pub async fn shutdown_all(&mut self) -> anyhow::Result<()> {
        let ids: Vec<String> = self.runtimes.keys().cloned().collect();
        for id in ids {
            if let Err(error) = self.stop_instance(id.as_str()).await {
                tracing::warn!(instance_id = %id, detail = %error, "app-runtime shutdown failed");
            }
        }
        // Catch children that escaped the in-memory map (e.g. race / prior orphans).
        let swept = crate::stale_runtime_sweep::sweep_stale_app_runtimes(
            self.workspace_root.as_path(),
            &std::collections::BTreeSet::new(),
        );
        if swept > 0 {
            tracing::info!(
                swept,
                workspace = %self.workspace_root.display(),
                "shutdown_all swept leftover mei-app-runtime processes"
            );
        }
        Ok(())
    }

    /// Reconcile desired Running instances from LaunchManifest.
    ///
    /// Does **not** reuse legacy PIDs — always spawn fresh children for missing ids.
    /// Stops managed instances that are no longer desired Running.
    pub async fn reconcile_launch_manifest(
        &mut self,
        manifest: &LaunchManifest,
    ) -> anyhow::Result<Vec<ObservedInstance>> {
        let desired_running: BTreeMap<String, String> = manifest
            .instances
            .iter()
            .filter(|(_, desired)| desired.desired_state == DesiredState::Running)
            .map(|(id, _)| {
                let app_id = manifest
                    .routes
                    .iter()
                    .find(|(_, route)| route.active.as_deref() == Some(id.as_str()))
                    .map(|(app, _)| app.clone())
                    .unwrap_or_else(|| id.clone());
                (id.clone(), app_id)
            })
            .collect();

        let managed_ids: Vec<String> = self.runtimes.keys().cloned().collect();
        for id in managed_ids {
            if !desired_running.contains_key(id.as_str()) {
                tracing::info!(instance_id = %id, "stopping app-runtime not in desired Running set");
                let _ = self.stop_instance(id.as_str()).await;
            }
        }

        let mut observed = Vec::new();
        for (instance_id, app_id) in &desired_running {
            if let Some(existing) = self.runtimes.get(instance_id.as_str()) {
                observed.push(observed_from_managed(existing, DesiredState::Running, None));
                continue;
            }
            let spec = synthesize_instance_spec(
                self.workspace_root.as_path(),
                app_id.as_str(),
                instance_id.as_str(),
            );
            let token = generate_instance_token(instance_id.as_str());
            match self.spawn_instance(spec, token).await {
                Ok(obs) => {
                    tracing::info!(
                        instance_id = %instance_id,
                        app_id = %app_id,
                        endpoint = ?obs.endpoint,
                        "reconciled app-runtime instance"
                    );
                    observed.push(obs);
                }
                Err(error) => {
                    tracing::warn!(
                        instance_id = %instance_id,
                        app_id = %app_id,
                        error = %error,
                        "failed to reconcile app-runtime instance"
                    );
                    observed.push(ObservedInstance {
                        instance_id: instance_id.clone(),
                        spec_ref: String::new(),
                        observed_at_ms: crate::state::current_time_ms(),
                        phase: InstancePhase::Failed,
                        desired_state: DesiredState::Running,
                        reachable: false,
                        endpoint: None,
                        token_present: false,
                        health: InstanceHealth {
                            process: "failed".to_string(),
                            plug_ds: "unknown".to_string(),
                            warmup: "unknown".to_string(),
                            bootstrap: "unknown".to_string(),
                        },
                        revisions: InstanceRevisions::default(),
                        protected_reasons: Vec::new(),
                        last_error: Some(error.to_string()),
                        resource: InstanceResource::default(),
                    });
                }
            }
        }
        Ok(observed)
    }
}

pub fn generate_instance_token(instance_id: &str) -> String {
    let n = TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(instance_id.as_bytes());
    hasher.update(b"|");
    hasher.update(n.to_le_bytes());
    hasher.update(b"|");
    hasher.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
            .to_le_bytes(),
    );
    hasher.update(std::process::id().to_le_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn synthesize_instance_spec(workspace: &Path, app_id: &str, instance_id: &str) -> InstanceSpec {
    let app_root = mei_lang_kernel::resolve_app_root(workspace, app_id);
    let generation = mei_lang_kernel::resolve_app_build_generation_from_current(app_root.as_path())
        .unwrap_or_else(|_| "current".to_string());
    let runtime_plan = load_runtime_plan(workspace).unwrap_or(RuntimePlan {
        default_mode: RuntimeMode::Lazy,
        apps: Default::default(),
    });
    InstanceSpec {
        schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
        instance_id: instance_id.to_string(),
        app_id: app_id.to_string(),
        bundle: BundleRef {
            generation: generation.clone(),
            bundle_path: format!("apps/{app_id}/env/{generation}"),
            digest: None,
            toolchain_version: None,
            config_digest: None,
        },
        config_snapshot: ConfigSnapshot {
            profile_id: "runtime".to_string(),
            profile_revision: "0".to_string(),
            profile_file: String::new(),
            runtime_plan,
            default_app: Some(app_id.to_string()),
            ..Default::default()
        },
        runtime_abi: env!("CARGO_PKG_VERSION").to_string(),
        data_mode_ceiling: None,
    }
}

/// Build an InstanceSpec from an on-disk App Launch Config (0537).
pub fn instance_spec_from_launch(
    workspace: &Path,
    app_id: &str,
    launch: &mei_host_core::AppLaunchDocument,
) -> anyhow::Result<InstanceSpec> {
    let app_root = mei_lang_kernel::resolve_app_root(workspace, app_id);
    let generation = match launch.config.generation.trim() {
        "" | "current" => {
            mei_lang_kernel::resolve_app_build_generation_from_current(app_root.as_path())
                .unwrap_or_else(|_| "current".to_string())
        }
        other => other.to_string(),
    };
    let runtime_plan = if let Some(value) = launch.config.runtime_plan.as_ref() {
        serde_json::from_value(value.clone()).unwrap_or(RuntimePlan {
            default_mode: RuntimeMode::Lazy,
            apps: Default::default(),
        })
    } else {
        load_runtime_plan(workspace).unwrap_or(RuntimePlan {
            default_mode: RuntimeMode::Lazy,
            apps: Default::default(),
        })
    };
    let overlay = mei_host_core::read_runtime_overlay(workspace, app_id);
    let runtime_plan =
        mei_host_core::effective_runtime_plan(&runtime_plan, app_id, overlay.as_ref());
    let instance_id = format!(
        "{app_id}@{}@{}",
        generation,
        &launch.revision[..8.min(launch.revision.len())]
    );
    Ok(InstanceSpec {
        schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
        instance_id,
        app_id: app_id.to_string(),
        bundle: BundleRef {
            generation: generation.clone(),
            bundle_path: format!("apps/{app_id}/env/{generation}"),
            digest: None,
            toolchain_version: None,
            config_digest: Some(launch.revision.clone()),
        },
        config_snapshot: ConfigSnapshot {
            profile_id: launch.id.clone(),
            profile_revision: launch.revision.clone(),
            profile_file: launch.path.clone(),
            runtime_plan,
            default_app: Some(app_id.to_string()),
            launch_config_id: Some(launch.id.clone()),
            launch_config_revision: Some(launch.revision.clone()),
            launch_config_file: Some(launch.path.clone()),
            warmup: launch.config.warmup.clone(),
        },
        runtime_abi: env!("CARGO_PKG_VERSION").to_string(),
        data_mode_ceiling: launch.config.data_mode_ceiling.clone(),
    })
}

fn load_runtime_plan(workspace: &Path) -> Option<RuntimePlan> {
    let path = workspace.join("deploy/applied/runtime-plan.json");
    if !path.is_file() {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn observed_from_managed(
    managed: &ManagedRuntime,
    desired_state: DesiredState,
    last_error: Option<String>,
) -> ObservedInstance {
    ObservedInstance {
        instance_id: managed.spec.instance_id.clone(),
        spec_ref: managed.spec.spec_digest(),
        observed_at_ms: crate::state::current_time_ms(),
        phase: if last_error.is_some() {
            InstancePhase::Failed
        } else {
            InstancePhase::Ready
        },
        desired_state,
        reachable: last_error.is_none(),
        endpoint: Some(managed.endpoint.clone()),
        token_present: !managed.token.is_empty(),
        health: InstanceHealth {
            process: if last_error.is_none() {
                "ok".to_string()
            } else {
                "failed".to_string()
            },
            plug_ds: "ok".to_string(),
            warmup: "ready".to_string(),
            bootstrap: "ok".to_string(),
        },
        revisions: InstanceRevisions {
            data_generation: Some(managed.spec.bundle.generation.clone()),
            ..InstanceRevisions::default()
        },
        protected_reasons: Vec::new(),
        last_error,
        resource: InstanceResource {
            generation: Some(managed.spec.bundle.generation.clone()),
            ..InstanceResource::default()
        },
    }
}

async fn spawn_managed_runtime(
    workspace_root: &Path,
    spec: &InstanceSpec,
    token: &str,
) -> anyhow::Result<ManagedRuntime> {
    let reserved_port = reserve_loopback_port()?;
    let binary = crate::tool_exec::resolve_mei_app_runtime(Some(workspace_root))?;
    let _ = mei_host_core::write_instance_spec(workspace_root, spec);
    let spec_path = mei_host_core::instance_spec_path(
        workspace_root,
        spec.app_id.as_str(),
        spec.instance_id.as_str(),
    );
    let listen_hint = format!("http://{MANAGED_APP_RUNTIME_HOST}:{reserved_port}");
    let mode_label = match spec.config_snapshot.runtime_plan.default_mode {
        mei_lang_kernel::RuntimeMode::Hot => "hot",
        mei_lang_kernel::RuntimeMode::Lazy => "lazy",
        mei_lang_kernel::RuntimeMode::Frozen => "frozen",
    };
    let start_lines = [
        format!("app={}", spec.app_id),
        format!("mode={mode_label}"),
        format!("generation={}", spec.bundle.generation),
        format!("instance={}", spec.instance_id),
        format!("listen={listen_hint}"),
    ];
    let start_refs: Vec<&str> = start_lines.iter().map(String::as_str).collect();
    crate::startup_banner::emit_app_start_banner(start_refs.as_slice());
    let mut cmd = Command::new(&binary);
    cmd.arg("serve")
        .arg("--workspace")
        .arg(workspace_root)
        .arg("--app")
        .arg(spec.app_id.as_str())
        .arg("--instance-id")
        .arg(spec.instance_id.as_str())
        .arg("--token")
        .arg(token)
        .arg("--generation")
        .arg(spec.bundle.generation.as_str())
        .arg("--instance-spec")
        .arg(&spec_path)
        .arg("--host")
        .arg(MANAGED_APP_RUNTIME_HOST)
        .arg("--port")
        .arg(reserved_port.to_string())
        .env("MEI_APP_RUNTIME_APP_ID", spec.app_id.as_str())
        .env(
            "MEI_APP_RUNTIME_GENERATION",
            spec.bundle.generation.as_str(),
        )
        .env(
            "MEI_APP_RUNTIME_VAR_ROOT",
            mei_host_core::instance_var_dir(
                workspace_root,
                spec.app_id.as_str(),
                spec.instance_id.as_str(),
            ),
        );
    if let Some(ceiling) = spec
        .data_mode_ceiling
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cmd.arg("--data-mode-ceiling").arg(ceiling);
    }
    for (key, value) in mei_lang_kernel::runtime_plan_env_vars(
        &spec.config_snapshot.runtime_plan,
        spec.app_id.as_str(),
    ) {
        cmd.env(key, value);
    }
    crate::process_group::configure_managed_child_process_group(&mut cmd);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| anyhow::anyhow!("spawn {}: {error}", binary.display()))?;
    let child_pid = child.id();
    if let Some(pid) = child_pid {
        if let Err(error) = crate::stale_runtime_sweep::write_runtime_pid_file(
            workspace_root,
            spec.app_id.as_str(),
            spec.instance_id.as_str(),
            pid,
        ) {
            tracing::warn!(
                %error,
                app = %spec.app_id,
                instance = %spec.instance_id,
                "failed to write runtime.pid"
            );
        }
    }

    if let Some(stderr) = child.stderr.take() {
        let app_id = spec.app_id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                crate::log_format::emit_prefixed_line(app_id.as_str(), line.as_str());
            }
        });
    }

    let fallback_endpoint = format!("http://{MANAGED_APP_RUNTIME_HOST}:{reserved_port}");
    // Wait up to STARTUP_TIMEOUT for LISTEN — hot warmup used to run *before* bind,
    // so a 3s cap caused premature fallback + health storm on a closed port.
    let listen_endpoint = if let Some(stdout) = child.stdout.take() {
        let app_id = spec.app_id.clone();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut sender = Some(tx);
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(endpoint) = parse_listen_line(line.as_str()) {
                    if let Some(tx) = sender.take() {
                        let _ = tx.send(endpoint);
                    }
                } else if !line.trim().is_empty() {
                    crate::log_format::emit_prefixed_line(app_id.as_str(), line.as_str());
                }
            }
        });
        match timeout(STARTUP_TIMEOUT, rx).await {
            Ok(Ok(endpoint)) => endpoint,
            Ok(Err(_)) => fallback_endpoint,
            Err(_) => {
                tracing::warn!(
                    app = %spec.app_id,
                    timeout_secs = STARTUP_TIMEOUT.as_secs(),
                    "app-runtime LISTEN line timed out; falling back to reserved port"
                );
                fallback_endpoint
            }
        }
    } else {
        fallback_endpoint
    };

    if let Err(error) = wait_for_ready(listen_endpoint.as_str(), &mut child).await {
        crate::stale_runtime_sweep::clear_runtime_pid_file(
            workspace_root,
            spec.app_id.as_str(),
            spec.instance_id.as_str(),
        );
        let _ = child.start_kill();
        let _ = child.wait().await;
        if let Some(pid) = child_pid {
            crate::process_group::kill_process_group_if_alive(pid);
        }
        return Err(error);
    }

    let ready_lines = [
        format!("app={}", spec.app_id),
        format!("generation={}", spec.bundle.generation),
        format!("instance={}", spec.instance_id),
        format!("listen={listen_endpoint}"),
    ];
    let ready_refs: Vec<&str> = ready_lines.iter().map(String::as_str).collect();
    crate::startup_banner::emit_app_ready_banner(ready_refs.as_slice());

    Ok(ManagedRuntime {
        child,
        endpoint: listen_endpoint,
        token: token.to_string(),
        spec: spec.clone(),
        started_at_ms: crate::state::current_time_ms(),
        child_pid,
        workspace_root: workspace_root.to_path_buf(),
    })
}

pub fn reserve_loopback_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| anyhow::anyhow!("reserve app-runtime port: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| anyhow::anyhow!("read app-runtime port: {error}"))?
        .port();
    drop(listener);
    Ok(port)
}

async fn wait_for_ready(endpoint: &str, child: &mut Child) -> anyhow::Result<()> {
    let ready_url = format!("{endpoint}/api/app-runtime/ready");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| anyhow::anyhow!("build app-runtime ready client: {error}"))?;
    let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
    let mut last_phase = String::from("unknown");
    loop {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("app-runtime exited during startup (status={status})");
        }
        match client.get(ready_url.as_str()).send().await {
            Ok(response) if response.status().is_success() => {
                if let Ok(body) = response.json::<serde_json::Value>().await {
                    if body
                        .get("phase")
                        .and_then(|v| v.as_str())
                        .is_some_and(|p| p == "failed")
                    {
                        let detail = body
                            .get("lastError")
                            .and_then(|v| v.as_str())
                            .unwrap_or("bootstrap failed");
                        anyhow::bail!("app-runtime failed during startup: {detail}");
                    }
                    if let Some(phase) = body.get("phase").and_then(|v| v.as_str()) {
                        last_phase = phase.to_string();
                    }
                    if body.get("ready").and_then(|v| v.as_bool()) == Some(true)
                        || body.get("ok").and_then(|v| v.as_bool()) == Some(true)
                    {
                        return Ok(());
                    }
                }
            }
            Ok(_) | Err(_) => {}
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!(
                "app-runtime ready check timed out at {ready_url} (last_phase={last_phase})"
            );
        }
        sleep(HEALTH_POLL_INTERVAL).await;
    }
}

/// Parse `MEI_APP_RUNTIME_LISTEN=` lines (unit-testable).
pub fn parse_listen_line(line: &str) -> Option<String> {
    let addr = line.trim().strip_prefix(LISTEN_LINE_PREFIX)?.trim();
    if addr.is_empty() {
        return None;
    }
    if addr.starts_with("http://") || addr.starts_with("https://") {
        Some(addr.to_string())
    } else {
        Some(format!("http://{addr}"))
    }
}

/// How serve bootstrap treats persisted LaunchManifest Running rows.
#[derive(Debug, Clone)]
pub enum BootstrapRunningPolicy {
    /// Product path (`serve` / `--app` / `--launch`): never revive disk Running set.
    /// CLI autostart owns which apps spawn; bare serve stays control-plane only.
    CliOwned,
    /// Legacy in-process serve: optionally ensure one app Running, then reconcile disk.
    RevivePersisted { auto_launch_app: Option<String> },
}

/// Load LaunchManifest from host-control, reconcile Running instances, sync ShellState.
pub async fn bootstrap_supervisor_for_shell(
    workspace: &Path,
    shell: &crate::state::SharedState,
    policy: BootstrapRunningPolicy,
) -> SharedAppRuntime {
    use mei_host_core::{DesiredInstance, DesiredState, HostControlState, RouteBinding};

    let mut manifest = mei_host_core::read_host_control_state(workspace)
        .map(|state| state.launch_manifest)
        .unwrap_or_else(LaunchManifest::empty);

    match &policy {
        BootstrapRunningPolicy::CliOwned => {
            let mut stopped = 0usize;
            for desired in manifest.instances.values_mut() {
                if desired.desired_state == DesiredState::Running {
                    desired.desired_state = DesiredState::Stopped;
                    stopped += 1;
                }
            }
            // Also demote stale route.active → previous so topbar / overview
            // do not treat leftover slots as running or "starting".
            let mut cleared_routes = 0usize;
            for route in manifest.routes.values_mut() {
                if route.active.is_some() {
                    route.previous = route.active.take();
                    route.candidate = None;
                    cleared_routes += 1;
                }
            }
            if stopped > 0 || cleared_routes > 0 {
                manifest = manifest.with_recomputed_revision();
                let mut control = mei_host_core::read_host_control_state(workspace)
                    .unwrap_or_else(HostControlState::empty);
                control.launch_manifest = manifest.clone();
                if let Err(error) = mei_host_core::write_host_control_state(workspace, &control) {
                    tracing::warn!(
                        %error,
                        stopped,
                        cleared_routes,
                        "failed to persist cleared Running set on serve bootstrap"
                    );
                } else {
                    tracing::info!(
                        stopped,
                        cleared_routes,
                        "serve bootstrap cleared persisted Running apps (CLI autostart owns spawn)"
                    );
                }
            }
        }
        BootstrapRunningPolicy::RevivePersisted { auto_launch_app } => {
            if let Some(app_id) = auto_launch_app
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                let has_active = manifest
                    .routes
                    .get(app_id)
                    .and_then(|route| route.active.as_ref())
                    .is_some_and(|id| {
                        manifest
                            .instances
                            .get(id.as_str())
                            .is_some_and(|desired| desired.desired_state == DesiredState::Running)
                    });
                if !has_active {
                    let instance_id = format!("auto-{app_id}");
                    manifest.instances.insert(
                        instance_id.clone(),
                        DesiredInstance {
                            spec_ref: String::new(),
                            desired_state: DesiredState::Running,
                        },
                    );
                    manifest.routes.insert(
                        app_id.to_string(),
                        RouteBinding {
                            active: Some(instance_id),
                            candidate: None,
                            previous: None,
                        },
                    );
                    manifest = manifest.with_recomputed_revision();
                }
            }
        }
    }

    {
        let mut guard = shell.write().expect("state lock");
        guard.install_launch_manifest(manifest.clone());
    }

    // Align OS process table with Cleared/Stopped disk state before any spawn.
    let swept = crate::stale_runtime_sweep::sweep_stale_app_runtimes(
        workspace,
        &std::collections::BTreeSet::new(),
    );
    if swept > 0 {
        tracing::info!(
            swept,
            workspace = %workspace.display(),
            "bootstrap swept stale mei-app-runtime processes"
        );
    }

    let desired_running = manifest
        .instances
        .values()
        .any(|desired| desired.desired_state == DesiredState::Running);
    if !desired_running {
        return empty_shared_app_runtime(workspace);
    }

    // Skip spawn when binary is missing — keep in-process Host fallback.
    if crate::tool_exec::resolve_mei_app_runtime(Some(workspace)).is_err() {
        tracing::warn!(
            "mei-app-runtime binary not found; LaunchManifest Running instances will not be spawned"
        );
        return empty_shared_app_runtime(workspace);
    }

    let mut supervisor = AppRuntimeSupervisor::new(workspace);
    match supervisor.reconcile_launch_manifest(&manifest).await {
        Ok(observed) => {
            let ready = observed.iter().filter(|o| o.reachable).count();
            tracing::info!(
                ready,
                total = observed.len(),
                "app-runtime supervisor reconciled LaunchManifest"
            );
        }
        Err(error) => {
            tracing::warn!(error = %error, "app-runtime supervisor reconcile failed");
        }
    }
    let endpoints = supervisor.endpoint_map();
    let started_at = supervisor.started_at_map();
    {
        let mut guard = shell.write().expect("state lock");
        guard.sync_app_runtime_endpoints_with_started(endpoints, started_at);
    }
    Arc::new(tokio::sync::Mutex::new(supervisor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn reserve_loopback_port_returns_nonzero() {
        let port = reserve_loopback_port().expect("reserve");
        assert!(port > 0);
    }

    #[test]
    fn parse_listen_line_accepts_host_port_and_url() {
        assert_eq!(
            parse_listen_line("MEI_APP_RUNTIME_LISTEN=127.0.0.1:9123").as_deref(),
            Some("http://127.0.0.1:9123")
        );
        assert_eq!(
            parse_listen_line("MEI_APP_RUNTIME_LISTEN=http://127.0.0.1:9").as_deref(),
            Some("http://127.0.0.1:9")
        );
        assert_eq!(parse_listen_line("other"), None);
    }

    #[test]
    fn generate_instance_token_is_hex_and_stable_length() {
        let a = generate_instance_token("inst-a");
        let b = generate_instance_token("inst-a");
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn synthesize_instance_spec_uses_app_and_instance() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let spec = synthesize_instance_spec(tmp.path(), "mini-data", "inst-1");
        assert_eq!(spec.app_id, "mini-data");
        assert_eq!(spec.instance_id, "inst-1");
        assert!(!spec.spec_digest().is_empty());
    }

    #[test]
    fn runtime_env_pins_generation_reads_and_instance_var_writes() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let app_root = tmp.path().join("apps/demo");
        let env_root = app_root.join("env");
        std::fs::create_dir_all(env_root.join("WS-20260714.0")).expect("old");
        std::fs::create_dir_all(env_root.join("WS-20260715.0")).expect("candidate");
        #[cfg(unix)]
        std::os::unix::fs::symlink("WS-20260714.0", env_root.join("current")).expect("current");
        let instance_var = tmp
            .path()
            .join("deploy/runtime/apps/demo/instances/inst-new/var");
        std::env::set_var("MEI_APP_RUNTIME_APP_ID", "demo");
        std::env::set_var("MEI_APP_RUNTIME_GENERATION", "WS-20260715.0");
        std::env::set_var("MEI_APP_RUNTIME_VAR_ROOT", &instance_var);

        assert_eq!(
            mei_lang_kernel::resolve_app_build_generation_from_current(app_root.as_path())
                .expect("pinned generation"),
            "WS-20260715.0"
        );
        assert_eq!(
            mei_lang_kernel::resolve_app_var_root(app_root.as_path()),
            instance_var
        );

        std::env::remove_var("MEI_APP_RUNTIME_APP_ID");
        std::env::remove_var("MEI_APP_RUNTIME_GENERATION");
        std::env::remove_var("MEI_APP_RUNTIME_VAR_ROOT");
    }

    #[test]
    fn instance_spec_from_launch_carries_runtime_plan_ceiling_and_warmup() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app_root = tmp.path().join("apps/zhifa");
        std::fs::create_dir_all(app_root.join("env/current")).expect("mkdir");
        let launch = mei_host_core::AppLaunchDocument {
            id: "data-scoped".to_string(),
            path: "apps/zhifa/launch/data-scoped.json".to_string(),
            revision: "abcdefghijklmnop".to_string(),
            config: mei_host_core::AppLaunchConfig {
                schema_version: mei_host_core::SCHEMA_APP_LAUNCH_V1.to_string(),
                app_id: "zhifa".to_string(),
                display_name: Some("scoped".to_string()),
                generation: "current".to_string(),
                data_mode_ceiling: Some("scoped".to_string()),
                runtime_plan: Some(serde_json::json!({
                    "defaultMode": "frozen",
                    "apps": {
                        "zhifa": {
                            "targets": [
                                { "scope": "home/t1/r-right-rail/s-warning", "mode": "hot" }
                            ],
                            "metricOverrides": {}
                        }
                    }
                })),
                theme: None,
                warmup: Some(serde_json::json!({
                    "enabled": true,
                    "apps": { "zhifa": { "hotScenes": ["home"] } }
                })),
                menu: None,
            },
        };
        let spec = instance_spec_from_launch(tmp.path(), "zhifa", &launch).expect("spec");
        assert_eq!(spec.data_mode_ceiling.as_deref(), Some("scoped"));
        assert_eq!(
            spec.config_snapshot.runtime_plan.default_mode,
            RuntimeMode::Frozen
        );
        let app_plan = spec
            .config_snapshot
            .runtime_plan
            .apps
            .get("zhifa")
            .expect("app plan");
        // Without ephemeral overlay, effective plan is uniform defaultMode (no per-scope targets).
        assert!(app_plan.targets.is_empty());
        assert!(spec.config_snapshot.warmup.is_some());
        assert!(!mei_lang_kernel::runtime_plan_requires_warm(
            &spec.config_snapshot.runtime_plan,
            "zhifa"
        ));
    }
}
