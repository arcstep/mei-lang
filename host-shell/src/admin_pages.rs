//! Admin Platform SSR page handler (0547 + 0548 Host builtins / dual render).

use axum::{
    extract::{Extension, Path as AxumPath, State},
    response::IntoResponse,
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, AuthEnforcement, AuthPrincipal,
    AuthServeState,
};
use mei_lang_app::{
    load_topbar_menu_context, page_body_theme_style, render_admin_page, AdminMainSurface,
    AdminNavItem, AdminUploadEmbed, HostAccountView, TopbarMenuContext, UploadFileEntry,
};
use mei_lang_kernel::{AdminUiSurface, WorkspaceAppMeta};

use crate::admin_registry::SharedAdminRegistry;
use crate::build_info::fill_page_shell_placeholders;
use crate::landing::{discover_workspace_apps, enrich_discovered_apps};
use crate::light_pages::light_page_response;
use crate::state::SharedState;
use crate::upload_support::{list_upload_files, upload_rel_from_config};

fn principal_caps_fn<'a>(
    principal_ref: Option<&'a AuthPrincipal>,
) -> impl Fn(&str) -> bool + 'a {
    move |cap: &str| -> bool {
        let Some(p) = principal_ref else {
            return matches!(cap, "config_upload" | "access_view");
        };
        let caps = p.capabilities();
        match cap {
            "config_upload" => caps.config_upload,
            "build_view" => caps.build_view,
            "access_view" => caps.access_view,
            _ => false,
        }
    }
}

fn surface_from_resource(ui: AdminUiSurface) -> AdminMainSurface {
    match ui {
        AdminUiSurface::FormCard => AdminMainSurface::FormCard,
        AdminUiSurface::OpsEmbed => AdminMainSurface::OpsEmbed,
        AdminUiSurface::UploadEmbed => AdminMainSurface::UploadEmbed,
        AdminUiSurface::AssetSlotCollection => AdminMainSurface::AssetSlotCollection,
    }
}

pub(crate) struct AdminPageRenderArgs<'a> {
    pub workspace_root: &'a std::path::Path,
    pub registry: &'a SharedAdminRegistry,
    /// Full discovered set used to refresh Registry (includes stopped apps).
    pub apps: &'a [WorkspaceAppMeta],
    /// 0544 App Switcher list: running ∩ endpoint-ready only.
    pub topbar_apps: &'a [WorkspaceAppMeta],
    pub app_id: &'a str,
    pub app_title: &'a str,
    pub resource_id: &'a str,
    pub topbar_menu: &'a TopbarMenuContext,
    pub auth_enabled: bool,
    pub account_view: Option<&'a HostAccountView>,
    pub principal_ref: Option<&'a AuthPrincipal>,
    pub upload_selected_file: Option<&'a str>,
}

pub(crate) fn render_admin_resource_html(args: AdminPageRenderArgs<'_>) -> String {
    let AdminPageRenderArgs {
        workspace_root,
        registry,
        apps,
        topbar_apps,
        app_id,
        app_title,
        resource_id,
        topbar_menu,
        auth_enabled,
        account_view,
        principal_ref,
        upload_selected_file,
    } = args;

    registry.refresh_workspace(workspace_root, apps);
    let caps = principal_caps_fn(principal_ref);
    let nav_items = registry.nav_items_for_capabilities(app_id, &caps);
    let resource = registry.resource(app_id, resource_id);
    let workspace = mei_lang_kernel::load_workspace_config(workspace_root);
    let theme_style = page_body_theme_style(&workspace, None, None);

    let resource_json = resource
        .as_ref()
        .and_then(|r| serde_json::to_string(r).ok())
        .unwrap_or_else(|| "null".to_string());

    let admin_nav: Vec<AdminNavItem> = nav_items
        .iter()
        .map(|r| AdminNavItem {
            id: r.resource_id.clone(),
            label: r.title.clone(),
            href: r.href.clone(),
        })
        .collect();

    let surface = resource
        .as_ref()
        .map(|r| surface_from_resource(r.ui_surface))
        .unwrap_or(AdminMainSurface::FormCard);

    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, app_id);
    let default_scene = mei_lang_kernel::resolve_default_scene_from_root(app_root.as_path())
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "home".to_string());
    // Same Stage Registry source as Access topbar — not stock-catalog main.mei parsing.
    // assemble_scope_from_registry panics without env/current (build prepare); Admin fixtures
    // and apps that only ship admin MDX may not have an active generation yet.
    let (stage_routes, access_scene) = if app_root.join("env/current").exists() {
        match mei_host_graph::assemble_scope_from_registry(
            workspace_root,
            app_id,
            default_scene.as_str(),
        ) {
            Ok(Some(outcome)) => {
                let scene = outcome
                    .compiled
                    .active_scene
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| default_scene.clone());
                (outcome.compiled.scene_routes, scene)
            }
            _ => (Vec::new(), default_scene),
        }
    } else {
        (Vec::new(), default_scene)
    };
    let upload_rel = upload_rel_from_config(app_root.as_path(), workspace_root);
    let upload_root_label = upload_rel.as_deref().unwrap_or("upload").to_string();
    let upload_files: Vec<UploadFileEntry> = upload_rel
        .as_deref()
        .map(|rel| list_upload_files(&app_root.join(rel), rel))
        .unwrap_or_default();

    let upload_embed = if surface == AdminMainSurface::UploadEmbed {
        Some(AdminUploadEmbed {
            upload_root_label: upload_root_label.as_str(),
            files: upload_files.as_slice(),
            selected_file: upload_selected_file,
        })
    } else {
        None
    };

    let source_anchor = resource
        .as_ref()
        .map(|r| r.page_program.page.source_anchor.as_str())
        .unwrap_or("");
    let projection_digest = registry
        .projection_for_app(app_id)
        .map(|p| p.manifest_digest)
        .unwrap_or_default();

    let mut html = render_admin_page(
        topbar_apps,
        app_title,
        app_id,
        resource_id,
        resource.as_ref().map(|r| r.title.as_str()),
        Some(topbar_menu),
        &admin_nav,
        Some(resource_id),
        resource_json.as_str(),
        surface,
        upload_embed,
        auth_enabled,
        account_view,
        theme_style.as_str(),
        stage_routes.as_slice(),
        Some(access_scene.as_str()),
        source_anchor,
        projection_digest.as_str(),
    );
    html = fill_page_shell_placeholders(html, workspace_root);
    html
}

pub async fn host_admin_resource_page(
    State(state): State<SharedState>,
    auth_state: Option<Extension<AuthServeState>>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath((app_id, resource_id)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let (workspace_root, registry) = {
        let guard = state.read().expect("state lock");
        (
            guard.ctx.workspace_root.clone(),
            guard.admin_registry.clone(),
        )
    };
    let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
    let discovered = discover_workspace_apps(workspace_root.as_path()).unwrap_or_default();
    let apps_all = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);

    let auth_enabled = matches!(
        auth_state.as_ref().map(|item| item.0.auth_enforcement),
        Some(AuthEnforcement::Required)
    );
    let principal_ref = principal.as_ref().map(|item| &item.0);
    // 0544: App Switcher only lists running ∩ endpoint-ready apps (not all discovered).
    let topbar_apps = {
        let guard = state.read().expect("state lock");
        let running = crate::shell_chrome::apps_for_topbar(&guard);
        filter_apps_for_principal(running.as_slice(), principal_ref)
    };
    let account_view = account_view_for_principal(principal_ref);

    // Title may come from discovery even when the app is not yet in the topbar list
    // (admin is allowed while app is stopped; switcher still won't list stopped apps).
    let apps_for_title = filter_apps_for_principal(&apps_all, principal_ref);
    let app_meta = apps_for_title
        .iter()
        .find(|a| a.id == app_id)
        .or_else(|| topbar_apps.iter().find(|a| a.id == app_id));
    let app_title = app_meta
        .map(|a| a.title.as_str())
        .unwrap_or(app_id.as_str());

    // Registry refresh needs the full discovered set so stopped apps still project resources.
    let html = render_admin_resource_html(AdminPageRenderArgs {
        workspace_root: workspace_root.as_path(),
        registry: &registry,
        apps: &apps_all,
        topbar_apps: &topbar_apps,
        app_id: app_id.as_str(),
        app_title,
        resource_id: resource_id.as_str(),
        topbar_menu: &topbar_menu,
        auth_enabled,
        account_view: account_view.as_ref(),
        principal_ref,
        upload_selected_file: None,
    });
    light_page_response(html)
}
