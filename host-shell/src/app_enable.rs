//! App enable (admission) vs load (runtime process).
//!
//! - **enable**: allow Access; hot → load immediately; lazy/frozen → admit only
//! - **disable**: revoke Access and unload runtime if present
//! - **demand load**: Access hit on enabled+unloaded → spawn

use mei_host_core::{
    effective_runtime_plan, read_host_control_state, read_runtime_overlay, write_host_control_state,
    HostControlState,
};
use mei_lang_kernel::RuntimeMode;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use crate::app_launch_api::{
    base_launch_runtime_plan, start_app_with_launch, stop_app_runtime, StartStopError,
};
use crate::state::HostHttpState;

fn last_active() -> &'static Mutex<std::collections::BTreeMap<String, u64>> {
    static MAP: OnceLock<Mutex<std::collections::BTreeMap<String, u64>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(std::collections::BTreeMap::new()))
}

pub fn record_app_activity(app_id: &str) {
    let id = app_id.trim();
    if id.is_empty() {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if let Ok(mut guard) = last_active().lock() {
        guard.insert(id.to_string(), now);
    }
}

pub fn idle_stop_secs() -> u64 {
    std::env::var("MEI_HOST_IDLE_STOP_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
}

pub fn on_demand_start_enabled() -> bool {
    match std::env::var("MEI_HOST_ON_DEMAND_START") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !(t == "0" || t == "false" || t == "off" || t == "no")
        }
        Err(_) => true,
    }
}

pub fn load_enabled_apps(workspace: &Path) -> BTreeSet<String> {
    read_host_control_state(workspace)
        .map(|s| s.enabled_apps)
        .unwrap_or_default()
}

pub fn persist_app_enabled(workspace: &Path, app_id: &str, enabled: bool) -> anyhow::Result<()> {
    let mut state = read_host_control_state(workspace).unwrap_or_else(HostControlState::empty);
    state.set_app_enabled(app_id, enabled);
    write_host_control_state(workspace, &state)?;
    Ok(())
}

pub fn is_app_enabled(http: &HostHttpState, app_id: &str) -> bool {
    let guard = http.shell.read().expect("state lock");
    guard.enabled_apps.contains(app_id.trim())
}

pub fn sync_enabled_apps_into_shell(http: &HostHttpState) {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let mut enabled = load_enabled_apps(workspace.as_path());
    // Migration: previously Running apps had no enabled set — treat them as admitted.
    let running = {
        let guard = http.shell.read().expect("state lock");
        crate::shell_chrome::active_running_app_ids(&guard.launch_manifest)
    };
    let before = enabled.len();
    for app in &running {
        enabled.insert(app.clone());
    }
    if enabled.len() > before {
        let mut state =
            read_host_control_state(workspace.as_path()).unwrap_or_else(HostControlState::empty);
        state.enabled_apps = enabled.clone();
        if let Err(error) = write_host_control_state(workspace.as_path(), &state) {
            tracing::warn!(
                detail = %error,
                "failed to persist migrated enabled_apps"
            );
        }
    }
    let mut guard = http.shell.write().expect("state lock");
    guard.enabled_apps = enabled;
    if !guard.enabled_apps.is_empty() {
        guard.data_plane_enabled = true;
    }
}

pub fn resolve_effective_mode(workspace: &Path, app_id: &str) -> RuntimeMode {
    let launch = mei_host_core::read_launch_config(workspace, app_id, "launch").ok();
    let Some(doc) = launch else {
        return RuntimeMode::Lazy;
    };
    let base = base_launch_runtime_plan(workspace, &doc.config);
    let overlay = read_runtime_overlay(workspace, app_id);
    let plan = effective_runtime_plan(&base, app_id, overlay.as_ref());
    plan.default_mode
}

fn mode_should_preload(mode: RuntimeMode) -> bool {
    matches!(mode, RuntimeMode::Hot)
}

/// Enable admission. Hot → also load runtime; lazy/frozen → admit only.
pub async fn enable_app(
    http: &HostHttpState,
    app_id: &str,
    config: Option<&str>,
    mode: Option<&str>,
    follow_git: bool,
) -> Result<Value, StartStopError> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Err(StartStopError::BadRequest("app id required".into()));
    }
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    persist_app_enabled(workspace.as_path(), app_id, true)
        .map_err(|e| StartStopError::Unavailable(e.to_string()))?;
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.enabled_apps.insert(app_id.to_string());
        // Admission opens Access demand-load; do not leave data_plane_enabled=false
        // (control-plane first boot) or starting pages mis-report「工作区尚未配置」.
        guard.data_plane_enabled = true;
    }
    tracing::info!(
        target: "mei.app_lifecycle",
        app_id = %app_id,
        action = "enable",
        "app enabled (admission)"
    );

    let effective = if let Some(m) = mode.map(str::trim).filter(|s| !s.is_empty()) {
        match m {
            "hot" => RuntimeMode::Hot,
            "frozen" => RuntimeMode::Frozen,
            _ => RuntimeMode::Lazy,
        }
    } else {
        resolve_effective_mode(workspace.as_path(), app_id)
    };

    if mode_should_preload(effective) {
        tracing::info!(
            target: "mei.app_lifecycle",
            app_id = %app_id,
            action = "load",
            mode = %effective.slug(),
            "hot enable → load runtime"
        );
        let payload = start_app_with_launch(http, app_id, config, mode, follow_git).await?;
        record_app_activity(app_id);
        return Ok(json!({
            "appId": app_id,
            "enabled": true,
            "loaded": true,
            "loadState": "loaded",
            "effectiveDefaultMode": effective.slug(),
            "runtime": payload,
        }));
    }

    Ok(json!({
        "appId": app_id,
        "enabled": true,
        "loaded": false,
        "loadState": "enabled_unloaded",
        "effectiveDefaultMode": effective.slug(),
    }))
}

/// Disable admission and unload runtime if present.
pub async fn disable_app(http: &HostHttpState, app_id: &str) -> Result<Value, StartStopError> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Err(StartStopError::BadRequest("app id required".into()));
    }
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    persist_app_enabled(workspace.as_path(), app_id, false)
        .map_err(|e| StartStopError::Unavailable(e.to_string()))?;
    {
        let mut guard = http.shell.write().expect("state lock");
        guard.enabled_apps.remove(app_id);
    }
    tracing::info!(
        target: "mei.app_lifecycle",
        app_id = %app_id,
        action = "disable",
        "app disabled (admission revoked)"
    );

    let unload = match stop_app_runtime(http, app_id).await {
        Ok(payload) => {
            tracing::info!(
                target: "mei.app_lifecycle",
                app_id = %app_id,
                action = "unload",
                "runtime unloaded on disable"
            );
            Some(payload)
        }
        Err(StartStopError::BadRequest(_)) => None,
        Err(other) => return Err(other),
    };

    Ok(json!({
        "appId": app_id,
        "enabled": false,
        "loaded": false,
        "loadState": "disabled",
        "runtime": unload,
    }))
}

/// Unload runtime process but keep admission (idle / reload).
pub async fn unload_app(http: &HostHttpState, app_id: &str) -> Result<Value, StartStopError> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Err(StartStopError::BadRequest("app id required".into()));
    }
    tracing::info!(
        target: "mei.app_lifecycle",
        app_id = %app_id,
        action = "unload",
        "unload runtime (admission unchanged)"
    );
    let payload = match stop_app_runtime(http, app_id).await {
        Ok(payload) => Some(payload),
        Err(StartStopError::BadRequest(_)) => None,
        Err(other) => return Err(other),
    };
    let enabled = is_app_enabled(http, app_id);
    Ok(json!({
        "appId": app_id,
        "enabled": enabled,
        "loaded": false,
        "loadState": if enabled { "enabled_unloaded" } else { "disabled" },
        "runtime": payload,
    }))
}

/// Unload then load (explicit operator reload; always loads, even for lazy).
pub async fn reload_app(
    http: &HostHttpState,
    app_id: &str,
    config: Option<&str>,
    mode: Option<&str>,
    follow_git: bool,
) -> Result<Value, StartStopError> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Err(StartStopError::BadRequest("app id required".into()));
    }
    if !is_app_enabled(http, app_id) {
        let workspace = {
            let guard = http.shell.read().expect("state lock");
            guard.ctx.workspace_root.clone()
        };
        persist_app_enabled(workspace.as_path(), app_id, true)
            .map_err(|e| StartStopError::Unavailable(e.to_string()))?;
        let mut guard = http.shell.write().expect("state lock");
        guard.enabled_apps.insert(app_id.to_string());
    }
    match stop_app_runtime(http, app_id).await {
        Ok(_) | Err(StartStopError::BadRequest(_)) => {}
        Err(other) => return Err(other),
    }
    tracing::info!(
        target: "mei.app_lifecycle",
        app_id = %app_id,
        action = "load",
        reason = "reload",
        "reload → load runtime"
    );
    let payload = start_app_with_launch(http, app_id, config, mode, follow_git).await?;
    record_app_activity(app_id);
    Ok(json!({
        "appId": app_id,
        "enabled": true,
        "loaded": true,
        "loadState": "loaded",
        "runtime": payload,
    }))
}

/// Demand-load an already-enabled app (Access path).
pub async fn demand_load_app(http: &HostHttpState, app_id: &str) -> Result<Value, StartStopError> {
    if !is_app_enabled(http, app_id) {
        return Err(StartStopError::BadRequest(format!(
            "app `{app_id}` is not enabled"
        )));
    }
    tracing::info!(
        target: "mei.app_lifecycle",
        app_id = %app_id,
        action = "load",
        reason = "demand",
        "demand-load runtime for enabled app"
    );
    let payload = start_app_with_launch(http, app_id, Some("launch"), None, true).await?;
    record_app_activity(app_id);
    Ok(payload)
}

/// Fire-and-forget demand load via runtime actor when possible.
pub fn kick_demand_load(http: &HostHttpState, app_id: &str) {
    if !on_demand_start_enabled() {
        return;
    }
    if !is_app_enabled(http, app_id) {
        return;
    }
    let app = app_id.to_string();
    let http = http.clone();
    tokio::spawn(async move {
        let result = if let Some(actor) = http.runtime_actor.as_ref() {
            // Actor start = load; ensure admission already recorded.
            actor.start(app.as_str(), Some("launch"), None, true).await
        } else {
            demand_load_app(&http, app.as_str()).await
        };
        match result {
            Ok(_) => tracing::info!(
                target: "mei.app_lifecycle",
                app_id = %app,
                action = "load",
                "demand-load completed"
            ),
            Err(StartStopError::Conflict(msg)) => tracing::info!(
                target: "mei.app_lifecycle",
                app_id = %app,
                detail = %msg,
                "demand-load already in flight"
            ),
            Err(error) => tracing::warn!(
                target: "mei.app_lifecycle",
                app_id = %app,
                detail = %match error {
                    StartStopError::BadRequest(m)
                    | StartStopError::Conflict(m)
                    | StartStopError::Unavailable(m) => m,
                },
                "demand-load failed"
            ),
        }
    });
}

/// Unload idle lazy/frozen apps; keep enabled.
pub async fn tick_idle_unload(http: &HostHttpState) {
    let ttl = idle_stop_secs();
    if ttl == 0 {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let ttl_ms = ttl.saturating_mul(1000);
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let enabled: Vec<String> = {
        let guard = http.shell.read().expect("state lock");
        guard.enabled_apps.iter().cloned().collect()
    };
    let candidates: Vec<String> = {
        let Ok(guard) = last_active().lock() else {
            return;
        };
        enabled
            .into_iter()
            .filter(|app| {
                let mode = resolve_effective_mode(workspace.as_path(), app.as_str());
                if mode_should_preload(mode) {
                    return false;
                }
                match guard.get(app.as_str()) {
                    Some(ts) => now.saturating_sub(*ts) >= ttl_ms,
                    None => false,
                }
            })
            .collect()
    };
    for app in candidates {
        let loaded = {
            let guard = http.shell.read().expect("state lock");
            crate::shell_chrome::active_running_app_ids(&guard.launch_manifest).contains(&app)
        };
        if !loaded {
            continue;
        }
        match stop_app_runtime(http, app.as_str()).await {
            Ok(_) => {
                tracing::info!(
                    target: "mei.app_lifecycle",
                    app_id = %app,
                    action = "unload",
                    reason = "idle",
                    "idle-unload runtime (admission kept)"
                );
                if let Ok(mut guard) = last_active().lock() {
                    guard.remove(app.as_str());
                }
            }
            Err(error) => tracing::debug!(
                app_id = %app,
                detail = %match error {
                    StartStopError::BadRequest(m)
                    | StartStopError::Conflict(m)
                    | StartStopError::Unavailable(m) => m,
                },
                "idle-unload skipped"
            ),
        }
    }
}

/// Spawn a background idle ticker once.
pub fn spawn_idle_ticker(http: HostHttpState) {
    if idle_stop_secs() == 0 {
        return;
    }
    static STARTED: OnceLock<Arc<()>> = OnceLock::new();
    if STARTED.set(Arc::new(())).is_err() {
        return;
    }
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            tick_idle_unload(&http).await;
        }
    });
}

pub fn compute_load_state(
    enabled: bool,
    loaded: bool,
    loading: bool,
    failed: bool,
) -> &'static str {
    if !enabled {
        return "disabled";
    }
    if failed {
        return "load_failed";
    }
    if loading {
        return "loading";
    }
    if loaded {
        return "loaded";
    }
    "enabled_unloaded"
}
