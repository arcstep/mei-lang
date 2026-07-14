//! Host → App Runtime cutover gates (Phase 9: fail-closed by default).
//!
//! Public URLs (`/apps/{app}/{stage}`, `/api/datasets/*`, …) stay stable.
//! Prefer reachable `mei-app-runtime` reverse proxy. Legacy Host in-process /
//! managed plug-ds require explicit `MEI_APP_RUNTIME_ALLOW_LEGACY=1` (tests only).

use std::collections::BTreeSet;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use mei_host_core::{DesiredState, LaunchManifest};
use serde_json::json;

/// Legacy override: allow Host in-process / plug-ds when no runtime (non-product).
pub fn app_runtime_allow_legacy() -> bool {
    env_flag_truthy("MEI_APP_RUNTIME_ALLOW_LEGACY")
}

pub fn env_flag_truthy(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        let trimmed = value.trim();
        trimmed == "1"
            || trimmed.eq_ignore_ascii_case("true")
            || trimmed.eq_ignore_ascii_case("yes")
            || trimmed.eq_ignore_ascii_case("on")
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataPlaneGate {
    /// Active LaunchManifest route + reachable runtime → must proxy.
    PreferRuntime,
    /// Explicit `MEI_APP_RUNTIME_ALLOW_LEGACY` and no runtime → Host fallback.
    AllowLegacyFallback,
    /// Default Phase 9: no runtime → fail closed.
    RuntimeRequired,
}

pub fn decide_data_plane_gate(has_reachable_runtime: bool) -> DataPlaneGate {
    if has_reachable_runtime {
        DataPlaneGate::PreferRuntime
    } else if app_runtime_allow_legacy() {
        DataPlaneGate::AllowLegacyFallback
    } else {
        DataPlaneGate::RuntimeRequired
    }
}

pub fn runtime_required_unavailable_response(app_id: &str, surface: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "app-runtime required but unavailable",
            "appId": app_id,
            "surface": surface,
            "hint": "Start the app via Host LaunchManifest / --app, or set MEI_APP_RUNTIME_ALLOW_LEGACY=1 for non-product Host fallback",
        })),
    )
        .into_response()
}

pub fn warn_legacy_data_plane_fallback(app_id: &str, surface: &str) {
    tracing::warn!(
        app_id = %app_id,
        surface = %surface,
        "legacy Host data-plane fallback; prefer mei-app-runtime via LaunchManifest active route"
    );
}

pub fn warn_migration_plug_ds_url() {
    tracing::warn!(
        "MEI_PLUG_DS_URL is a migration-period override; prefer mei-app-runtime embedded DS in production"
    );
}

pub fn warn_migration_activate_env(app_id: &str) {
    tracing::warn!(
        app_id = %app_id,
        "activate-env is a migration-period path; prefer LaunchManifest candidate cutover / route lifecycle"
    );
}

/// Apps whose LaunchManifest active route targets a DesiredState::Running instance.
/// These are intended to be served by app-runtime (managed plug-ds may be skipped).
pub fn apps_covered_by_desired_runtime(manifest: &LaunchManifest) -> BTreeSet<String> {
    manifest
        .routes
        .iter()
        .filter_map(|(app_id, route)| {
            let instance_id = route.active.as_ref()?;
            let desired = manifest.instances.get(instance_id.as_str())?;
            (desired.desired_state == DesiredState::Running).then(|| app_id.clone())
        })
        .collect()
}

/// Apps that still need a managed plug-ds sidecar (not covered by app-runtime intent).
pub fn apps_needing_managed_plug_ds(
    app_ids: &[String],
    covered_by_runtime: &BTreeSet<String>,
) -> Vec<String> {
    app_ids
        .iter()
        .filter(|app_id| !covered_by_runtime.contains(app_id.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_host_core::{DesiredInstance, RouteBinding};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn decide_gate_prefers_runtime_then_required_then_legacy_escape() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("MEI_APP_RUNTIME_ALLOW_LEGACY");
        std::env::remove_var("MEI_APP_RUNTIME_REQUIRED");
        assert_eq!(decide_data_plane_gate(true), DataPlaneGate::PreferRuntime);
        assert_eq!(
            decide_data_plane_gate(false),
            DataPlaneGate::RuntimeRequired
        );
        std::env::set_var("MEI_APP_RUNTIME_ALLOW_LEGACY", "1");
        assert_eq!(
            decide_data_plane_gate(false),
            DataPlaneGate::AllowLegacyFallback
        );
        assert_eq!(decide_data_plane_gate(true), DataPlaneGate::PreferRuntime);
        std::env::remove_var("MEI_APP_RUNTIME_ALLOW_LEGACY");
    }

    #[test]
    fn apps_needing_managed_plug_skips_runtime_covered() {
        let covered = BTreeSet::from(["mini-data".to_string()]);
        let needing =
            apps_needing_managed_plug_ds(&["mini-data".into(), "data-demo".into()], &covered);
        assert_eq!(needing, vec!["data-demo".to_string()]);
        assert!(apps_needing_managed_plug_ds(&["mini-data".into()], &covered).is_empty());
    }

    #[test]
    fn apps_covered_by_desired_runtime_reads_active_running() {
        let mut manifest = LaunchManifest::empty();
        manifest.instances.insert(
            "inst-1".into(),
            DesiredInstance {
                spec_ref: "sha256:a".into(),
                desired_state: DesiredState::Running,
            },
        );
        manifest.instances.insert(
            "inst-stopped".into(),
            DesiredInstance {
                spec_ref: "sha256:b".into(),
                desired_state: DesiredState::Stopped,
            },
        );
        manifest.routes.insert(
            "mini-data".into(),
            RouteBinding {
                active: Some("inst-1".into()),
                candidate: None,
                previous: None,
            },
        );
        manifest.routes.insert(
            "other".into(),
            RouteBinding {
                active: Some("inst-stopped".into()),
                candidate: None,
                previous: None,
            },
        );
        let covered = apps_covered_by_desired_runtime(&manifest);
        assert!(covered.contains("mini-data"));
        assert!(!covered.contains("other"));
    }
}
