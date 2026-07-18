//! Running-app topbar filtering and shell-chrome helpers (0537 closeout).

use std::collections::BTreeSet;
use std::path::Path;

use mei_host_auth::{account_view_for_principal, AuthEnforcement, AuthPrincipal};
use mei_host_core::{
    read_instance_spec, read_instance_spec_for_app, read_launch_config, DesiredState,
    LaunchManifest,
};
use mei_lang_app::{load_topbar_menu_context, UiRouteMode};
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::landing::{discover_workspace_apps, enrich_discovered_apps, menu_label_for_app};
use crate::state::{HostHttpState, ShellState};

pub fn app_access_href(workspace: &Path, app_id: &str) -> String {
    let app_id = app_id.trim().trim_matches('/');
    let app_root = mei_lang_kernel::resolve_app_root(workspace, app_id);
    let scene = mei_lang_kernel::resolve_default_scene_from_root(app_root.as_path())
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "home".to_string());
    format!("/apps/{app_id}/{scene}")
}

pub fn default_access_scene(workspace: &Path, app_id: &str) -> String {
    let app_root = mei_lang_kernel::resolve_app_root(workspace, app_id.trim());
    mei_lang_kernel::resolve_default_scene_from_root(app_root.as_path())
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "home".to_string())
}

/// Unknown stage → Phase 9: no silent redirect to default_stage (callers must 404/diagnose).
pub fn redirect_unknown_access_stage(
    _workspace: &Path,
    _app_id: &str,
    _stage: &str,
    _query: Option<&str>,
) -> Option<String> {
    None
}

/// Apps whose active route targets a `DesiredState::Running` instance.
/// Stale `route.active` pointing at Stopped instances are excluded (topbar / running list).
pub fn active_running_app_ids(manifest: &LaunchManifest) -> BTreeSet<String> {
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

/// Discover + menu-enrich apps, then keep only those with an active LaunchManifest route.
/// Titles prefer the active launch config `displayName`.
pub fn running_enriched_apps(workspace: &Path, manifest: &LaunchManifest) -> Vec<WorkspaceAppMeta> {
    let topbar_menu = load_topbar_menu_context(workspace);
    let discovered = discover_workspace_apps(workspace).unwrap_or_default();
    let enriched = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
    let running = active_running_app_ids(manifest);
    if running.is_empty() {
        return Vec::new();
    }
    enriched
        .into_iter()
        .filter(|app| running.contains(app.id.as_str()))
        .map(|mut app| {
            let launch_id = manifest
                .routes
                .get(app.id.as_str())
                .and_then(|route| route.active.as_ref())
                .and_then(|instance_id| {
                    read_instance_spec_for_app(workspace, app.id.as_str())
                        .or_else(|| read_instance_spec(workspace, instance_id.as_str()))
                        .and_then(|spec| spec.config_snapshot.launch_config_id)
                });
            app.title = display_name_for_running_app(
                workspace,
                app.id.as_str(),
                launch_id.as_deref(),
                Some(app.title.as_str()),
            );
            app
        })
        .collect()
}

pub fn display_name_for_running_app(
    workspace: &Path,
    app_id: &str,
    launch_id: Option<&str>,
    enriched_title: Option<&str>,
) -> String {
    if let Some(id) = launch_id {
        if let Ok(doc) = read_launch_config(workspace, app_id, id) {
            if let Some(name) = doc
                .config
                .display_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return name.to_string();
            }
        }
    }
    if let Some(title) = enriched_title.map(str::trim).filter(|s| !s.is_empty()) {
        return title.to_string();
    }
    app_id.to_string()
}

pub fn build_apps_overview_payload(http: &HostHttpState) -> Value {
    let (workspace, manifest) = {
        let guard = http.shell.read().expect("state lock");
        (
            guard.ctx.workspace_root.clone(),
            guard.launch_manifest.clone(),
        )
    };
    let topbar_menu = load_topbar_menu_context(workspace.as_path());
    let discovered = discover_workspace_apps(workspace.as_path()).unwrap_or_default();
    let enriched = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);

    let apps: Vec<Value> = enriched
        .iter()
        .map(|app| {
            let launches = mei_host_core::list_launch_configs(workspace.as_path(), app.id.as_str())
                .unwrap_or_default();
            let launch_doc =
                mei_host_core::read_launch_config(workspace.as_path(), app.id.as_str(), "launch")
                    .ok();
            let overlay =
                mei_host_core::read_runtime_overlay(workspace.as_path(), app.id.as_str());
            let (git_default_mode, effective_default_mode) = match launch_doc.as_ref() {
                Some(doc) => {
                    let base = crate::app_launch_api::base_launch_runtime_plan(
                        workspace.as_path(),
                        &doc.config,
                    );
                    let git_mode = base.default_mode.slug().to_string();
                    let effective = mei_host_core::effective_runtime_plan(
                        &base,
                        app.id.as_str(),
                        overlay.as_ref(),
                    );
                    (Some(git_mode), Some(effective.default_mode.slug().to_string()))
                }
                None => (None, None),
            };
            let generations = crate::generation_lifecycle::app_generation_summaries(
                workspace.as_path(),
                app.id.as_str(),
            );
            json!({
                "appId": app.id,
                "displayName": app.title,
                "href": app_access_href(workspace.as_path(), app.id.as_str()),
                "launchPath": format!("apps/{}/launch.json", app.id),
                "hasLaunch": launch_doc.is_some(),
                "launchDisplayName": launch_doc.as_ref().and_then(|d| d.config.display_name.clone()),
                "gitDefaultMode": git_default_mode,
                "overlayDefaultMode": overlay.as_ref().and_then(|o| o.default_mode.clone()),
                "effectiveDefaultMode": effective_default_mode,
                "overlayRevision": overlay.as_ref().map(|o| o.revision.clone()),
                "launches": launches,
                "generations": generations,
            })
        })
        .collect();

    let running: Vec<Value> = manifest
        .routes
        .iter()
        .filter_map(|(app_id, route)| {
            let instance_id = route.active.as_ref()?;
            let desired = manifest.instances.get(instance_id.as_str())?;
            // Stopped / stale active slots must not appear as "starting" in /runtime.
            if desired.desired_state != DesiredState::Running {
                return None;
            }
            let spec = read_instance_spec_for_app(workspace.as_path(), app_id)
                .or_else(|| read_instance_spec(workspace.as_path(), instance_id.as_str()));
            let launch_id = spec.and_then(|s| s.config_snapshot.launch_config_id);
            let enriched_title = enriched
                .iter()
                .find(|app| app.id == *app_id)
                .map(|app| app.title.as_str());
            let menu_title = menu_label_for_app(&topbar_menu, app_id);
            let display_name = display_name_for_running_app(
                workspace.as_path(),
                app_id,
                launch_id.as_deref(),
                enriched_title.or(menu_title.as_deref()),
            );
            let (phase, started_at_ms) = {
                let guard = http.shell.read().expect("state lock");
                let phase = if guard
                    .app_runtime_by_instance
                    .contains_key(instance_id.as_str())
                {
                    "ready"
                } else {
                    "starting"
                };
                let started_at_ms = guard
                    .app_runtime_started_at_ms
                    .get(instance_id.as_str())
                    .copied();
                (phase, started_at_ms)
            };
            Some(json!({
                "appId": app_id,
                "instanceId": instance_id,
                "launchId": launch_id,
                "displayName": display_name,
                "href": app_access_href(workspace.as_path(), app_id),
                "phase": phase,
                "startedAtMs": started_at_ms,
            }))
        })
        .collect();

    json!({
        "apps": apps,
        "running": running,
        "menuRevision": menu_revision_digest(workspace.as_path()),
    })
}

pub fn menu_revision_digest(workspace: &Path) -> String {
    let mut hasher = Sha256::new();
    for candidate in [
        workspace.join("workspace.json"),
        workspace.join(".mei-workspace.json"),
        workspace.join("_menu.json"),
    ] {
        if let Ok(bytes) = std::fs::read(&candidate) {
            hasher.update(candidate.display().to_string().as_bytes());
            hasher.update(&bytes);
        }
    }
    format!("{:x}", hasher.finalize())
}

pub fn running_event_payload_with_plan(
    workspace: &Path,
    app_id: &str,
    launch_id: &str,
    instance_id: &str,
    runtime_plan: Option<&mei_lang_kernel::RuntimePlan>,
) -> Value {
    let topbar_menu = load_topbar_menu_context(workspace);
    let discovered = discover_workspace_apps(workspace).unwrap_or_default();
    let enriched = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
    let enriched_title = enriched
        .iter()
        .find(|app| app.id == app_id)
        .map(|app| app.title.as_str());
    let display_name =
        display_name_for_running_app(workspace, app_id, Some(launch_id), enriched_title);
    let mut payload = json!({
        "appId": app_id,
        "launchId": launch_id,
        "instanceId": instance_id,
        "displayName": display_name,
        "href": app_access_href(workspace, app_id),
        "phase": "ready",
    });
    if let Some(plan) = runtime_plan {
        payload
            .as_object_mut()
            .expect("running payload object")
            .insert(
                "runtimePlan".to_string(),
                serde_json::to_value(plan).unwrap_or(Value::Null),
            );
    }
    payload
}

/// Apps list for topbar SSR: only apps with a live runtime endpoint (accessible).
/// Desired Running without endpoint (still starting) stays off the topbar.
pub fn apps_for_topbar(shell: &ShellState) -> Vec<WorkspaceAppMeta> {
    running_enriched_apps(shell.ctx.workspace_root.as_path(), &shell.launch_manifest)
        .into_iter()
        .filter(|app| shell.endpoint_for_app(app.id.as_str()).is_some())
        .collect()
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShellChromeQuery {
    pub app_id: Option<String>,
    pub app: Option<String>,
    pub scene: Option<String>,
    pub surface: Option<String>,
    pub chrome: Option<String>,
    /// Workspace shell nav highlight: `home` | `config` | `upload` | `runtime` | `mcg`.
    pub shell_nav: Option<String>,
}

pub async fn api_host_shell_chrome(
    axum::extract::State(http): axum::extract::State<HostHttpState>,
    axum::extract::Query(query): axum::extract::Query<ShellChromeQuery>,
    principal: Option<axum::extract::Extension<AuthPrincipal>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let principal_ref = principal.as_ref().map(|axum::extract::Extension(p)| p);
    match render_shell_chrome_payload(&http, &query, principal_ref) {
        Ok(payload) => axum::Json(payload).into_response(),
        Err(error) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(json!({ "error": error })),
        )
            .into_response(),
    }
}

fn parse_workspace_shell_nav(raw: Option<&str>) -> Option<mei_lang_app::WorkspaceShellNav> {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("home") => Some(mei_lang_app::WorkspaceShellNav::Home),
        Some("config") => Some(mei_lang_app::WorkspaceShellNav::Config),
        Some("upload") => Some(mei_lang_app::WorkspaceShellNav::Upload),
        Some("runtime") => Some(mei_lang_app::WorkspaceShellNav::Runtime),
        Some("mcg") => Some(mei_lang_app::WorkspaceShellNav::Mcg),
        _ => None,
    }
}

pub fn render_shell_chrome_payload(
    http: &HostHttpState,
    query: &ShellChromeQuery,
    principal: Option<&AuthPrincipal>,
) -> Result<Value, String> {
    let workspace = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let apps = {
        let guard = http.shell.read().expect("state lock");
        apps_for_topbar(&guard)
    };
    let topbar_menu = load_topbar_menu_context(workspace.as_path());
    let auth_enabled = http.auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal);
    let auth_account = account_view.as_ref();

    let (mut topbar_html, mut statusbar_html) =
        if let Some(shell_nav) = parse_workspace_shell_nav(query.shell_nav.as_deref()) {
            mei_lang_app::render_workspace_shell_chrome_html(
                apps.as_slice(),
                Some(&topbar_menu),
                shell_nav,
                auth_enabled,
                auth_account,
            )
        } else {
            let app_id = query
                .app_id
                .as_deref()
                .or(query.app.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| apps.first().map(|app| app.id.clone()))
                .unwrap_or_default();
            let scene = query
                .scene
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("home");
            let surface = query
                .surface
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("app");
            let chrome_hidden = matches!(
                query
                    .chrome
                    .as_deref()
                    .map(str::trim)
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("none") | Some("0") | Some("false") | Some("off")
            );
            // Shell refreshes must preserve the compiled stage routes used by the
            // initial scene manifest. An empty synthetic app disables the stage
            // switcher and standalone-launch button after runtime event refresh.
            let compiled = mei_host_graph::assemble_scope_from_registry(
                workspace.as_path(),
                app_id.as_str(),
                scene,
            )
            .ok()
            .flatten()
            .map(|outcome| outcome.compiled)
            .unwrap_or_else(|| CompiledApp {
                app_id: app_id.clone(),
                title: app_id.clone(),
                app_root: format!("apps/{app_id}"),
                scene_routes: Vec::new(),
                active_scene: Some(scene.to_string()),
                stage_registry: Default::default(),
                stage_programs: Default::default(),
                scene_slot_modules: Default::default(),
                content_capabilities: Default::default(),
                narration_catalogs: Default::default(),
                active_target_file: format!("src/scene/{scene}.mei"),
                file_tree: Vec::new(),
                scene_contract: None,
                scene_local_nav_by_target: Default::default(),
                scene_bindings_by_id: Default::default(),
                scene_examples_by_id: Default::default(),
                scene_projection_assembly_by_id: Default::default(),
                resources: Vec::new(),
                world_metrics: Default::default(),
                world_semantic_by_file: Default::default(),
                component_assets: Vec::new(),
                diagnostics: Vec::new(),
                build_experience_index: Default::default(),
                build_t2_page_index: Default::default(),
                build_template_index: Default::default(),
                ui_layout_index: Default::default(),
            });
            mei_lang_app::render_access_shell_chrome_html(
                apps.as_slice(),
                &compiled,
                app_id.as_str(),
                Some(&topbar_menu),
                UiRouteMode::App,
                Some(scene),
                None,
                Some(surface),
                auth_enabled,
                auth_account,
                None,
                None,
                chrome_hidden,
            )
        };
    topbar_html = crate::build_info::fill_page_shell_placeholders(topbar_html, workspace.as_path());
    statusbar_html =
        crate::build_info::fill_page_shell_placeholders(statusbar_html, workspace.as_path());

    let mut hasher = Sha256::new();
    hasher.update(topbar_html.as_bytes());
    hasher.update(statusbar_html.as_bytes());
    let digest = format!("{:x}", hasher.finalize());

    Ok(json!({
        "topbarHtml": topbar_html,
        "statusbarHtml": statusbar_html,
        "digest": digest,
        "menuRevision": menu_revision_digest(workspace.as_path()),
        "runningAppIds": apps.iter().map(|app| app.id.clone()).collect::<Vec<_>>(),
    }))
}


#[cfg(test)]
mod tests {
    use super::*;
    use mei_host_core::{DesiredInstance, RouteBinding};

    #[test]
    fn active_running_app_ids_skips_stopped_active_slots() {
        let mut manifest = LaunchManifest::empty();
        manifest.instances.insert(
            "inst-ready".into(),
            DesiredInstance {
                spec_ref: "sha256:a".into(),
                desired_state: DesiredState::Running,
            },
        );
        manifest.instances.insert(
            "inst-stale".into(),
            DesiredInstance {
                spec_ref: "sha256:b".into(),
                desired_state: DesiredState::Stopped,
            },
        );
        manifest.routes.insert(
            "qunfu".into(),
            RouteBinding {
                active: Some("inst-ready".into()),
                candidate: None,
                previous: None,
            },
        );
        manifest.routes.insert(
            "charts-grid".into(),
            RouteBinding {
                active: Some("inst-stale".into()),
                candidate: None,
                previous: None,
            },
        );
        let running = active_running_app_ids(&manifest);
        assert!(running.contains("qunfu"));
        assert!(!running.contains("charts-grid"));
    }

    #[test]
    fn mei_tutorial_access_href_uses_intro() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        let app_root = workspace.join("apps/mei-tutorial");
        std::fs::create_dir_all(app_root.join("src/presentation/intro")).expect("mkdir");
        std::fs::write(
            app_root.join("app.toml"),
            r#"
schema_version = "mei-app-v1"
default_stage = "intro"
app_id = "mei-tutorial"
"#,
        )
        .expect("app.toml");
        std::fs::write(
            app_root.join("src/presentation/intro/intro.deck.mdx"),
            r#"---
id: intro
title: Intro
---

# Intro
"#,
        )
        .expect("deck");

        assert_eq!(
            app_access_href(workspace, "mei-tutorial"),
            "/apps/mei-tutorial/intro"
        );
        assert_eq!(
            redirect_unknown_access_stage(workspace, "mei-tutorial", "home", None),
            None
        );
        assert_eq!(
            redirect_unknown_access_stage(workspace, "mei-tutorial", "intro", None),
            None
        );
    }
}
