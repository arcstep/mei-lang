//! Running-app topbar filtering and shell-chrome helpers (0537 closeout).

use std::collections::BTreeSet;
use std::path::Path;

use mei_host_auth::AuthEnforcement;
use mei_host_core::{
    read_instance_spec, read_instance_spec_for_app, read_launch_config, LaunchManifest,
};
use mei_lang_app::{load_topbar_menu_context, UiRouteMode};
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::landing::{discover_workspace_apps, enrich_discovered_apps, menu_label_for_app};
use crate::state::{HostHttpState, ShellState};

pub fn app_access_href(app_id: &str) -> String {
    format!("/apps/{}/home", app_id.trim().trim_matches('/'))
}

pub fn active_running_app_ids(manifest: &LaunchManifest) -> BTreeSet<String> {
    manifest
        .routes
        .iter()
        .filter_map(|(app_id, route)| route.active.as_ref().map(|_| app_id.clone()))
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
            let cfg = mei_host_core::load_app_config(&mei_lang_kernel::resolve_app_root(
                workspace.as_path(),
                app.id.as_str(),
            ))
            .unwrap_or_default();
            let generations = crate::generation_lifecycle::app_generation_summaries(
                workspace.as_path(),
                app.id.as_str(),
            );
            json!({
                "appId": app.id,
                "displayName": app.title,
                "href": app_access_href(app.id.as_str()),
                "defaultLaunch": cfg.default_launch,
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
                "href": app_access_href(app_id),
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

pub fn running_event_payload(
    workspace: &Path,
    app_id: &str,
    launch_id: &str,
    instance_id: &str,
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
    json!({
        "appId": app_id,
        "launchId": launch_id,
        "instanceId": instance_id,
        "displayName": display_name,
        "href": app_access_href(app_id),
        "phase": "ready",
    })
}

/// Apps list for topbar SSR: only currently running apps (0537).
pub fn apps_for_topbar(shell: &ShellState) -> Vec<WorkspaceAppMeta> {
    running_enriched_apps(shell.ctx.workspace_root.as_path(), &shell.launch_manifest)
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
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match render_shell_chrome_payload(&http, &query) {
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

    let (mut topbar_html, mut statusbar_html) =
        if let Some(shell_nav) = parse_workspace_shell_nav(query.shell_nav.as_deref()) {
            mei_lang_app::render_workspace_shell_chrome_html(
                apps.as_slice(),
                Some(&topbar_menu),
                shell_nav,
                auth_enabled,
                None,
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
                None,
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
