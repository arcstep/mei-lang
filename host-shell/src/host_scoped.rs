use std::path::Path;

use axum::{
    extract::{Extension, OriginalUri, Path as AxumPath, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Response},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, html_escape, render_auth_card_page,
    render_host_shell_footer_for_source_root, host_shell_body_theme_style, AuthEnforcement,
    AuthPrincipal, AuthServeState,
};
use mei_lang_app::{load_topbar_menu_context, UiRouteMode};
use mei_lang_kernel::WorkspaceAppMeta;
use serde::Deserialize;

use crate::landing::{discover_workspace_apps, enrich_discovered_apps};
use crate::pages::{app_page, AppQuery};
use crate::shell_nav::{render_shell_nav_html, ShellNavItem};
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct HostScopeQuery {
    pub app: Option<String>,
    #[serde(flatten)]
    pub page: AppQuery,
}

fn resolve_scope_app_id<'a>(
    apps: &'a [WorkspaceAppMeta],
    query_app: Option<&str>,
) -> Option<&'a WorkspaceAppMeta> {
    if let Some(requested) = query_app.map(str::trim).filter(|value| !value.is_empty()) {
        return apps.iter().find(|app| app.id == requested);
    }
    None
}

fn render_scope_picker_html(
    workspace_root: &Path,
    apps: &[WorkspaceAppMeta],
    route_label: &str,
    route_path: &str,
    shell_nav_item: ShellNavItem,
) -> String {
    let footer_html = render_host_shell_footer_for_source_root(workspace_root);
    let shell_theme = host_shell_body_theme_style(workspace_root);
    let shell_nav = render_shell_nav_html(shell_nav_item);
    let rows = if apps.is_empty() {
        r#"<p class="mei-host-shell__message">当前没有可选择的应用。</p>"#.to_string()
    } else {
        let links = apps
            .iter()
            .map(|app| {
                let href = format!("{route_path}?app={}", urlencoding_path(app.id.as_str()));
                format!(
                    r#"<li><a class="mei-host-shell__link" href="{href}"><code>{app_id}</code> — {title}</a></li>"#,
                    href = html_escape(href.as_str()),
                    app_id = html_escape(app.id.as_str()),
                    title = html_escape(app.title.as_str()),
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!(
            r#"<p class="mei-host-shell__message">请选择要管理的应用（{route_label} 可按应用分别配置）：</p>
<ul class="mei-host-shell__setup">{links}</ul>"#,
            route_label = html_escape(route_label),
            links = links,
        )
    };
    let body_html = format!(
        r#"{shell_nav}
{rows}
<div class="mei-host-shell__actions">
  <a class="mei-host-shell__btn" href="/home">返回首页</a>
</div>"#,
        shell_nav = shell_nav,
        rows = rows,
    );
    render_auth_card_page(
        route_label,
        format!("选择应用 · {route_label}").as_str(),
        body_html.as_str(),
        footer_html.as_str(),
        shell_theme.as_str(),
    )
}

fn urlencoding_path(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => ch.to_string(),
            _ => format!("%{:02X}", ch as u32),
        })
        .collect()
}

async fn host_scoped_context(
    state: &SharedState,
    auth: &AuthServeState,
    principal: Option<&AuthPrincipal>,
) -> (
    std::path::PathBuf,
    Vec<WorkspaceAppMeta>,
    bool,
    Option<mei_lang_app::HostAccountView>,
) {
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    let discovered = discover_workspace_apps(workspace_root.as_path()).unwrap_or_default();
    let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
    let apps = enrich_discovered_apps(
        filter_apps_for_principal(discovered.as_slice(), principal).as_slice(),
        &topbar_menu,
    );
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal);
    (workspace_root, apps, auth_enabled, account_view)
}

async fn host_scoped_light_page(
    state: State<SharedState>,
    auth: State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    route_mode: UiRouteMode,
    route_label: &'static str,
    route_path: &'static str,
    shell_nav_item: ShellNavItem,
    Query(query): Query<HostScopeQuery>,
) -> Response {
    let principal_ref = principal.as_ref().map(|Extension(p)| p);
    let (workspace_root, apps, auth_enabled, account_view) =
        host_scoped_context(&state, &auth, principal_ref).await;
    let Some(app) = resolve_scope_app_id(apps.as_slice(), query.app.as_deref()) else {
        let html = render_scope_picker_html(
            workspace_root.as_path(),
            apps.as_slice(),
            route_label,
            route_path,
            shell_nav_item,
        );
        return Html(html).into_response();
    };
    let guard = state.read().expect("state lock");
    let package_root = guard.package_root.clone();
    let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
    let app_title = app.title.as_str();
    let scene_for_links = query
        .page
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(response) = crate::light_pages::try_render_light_page(
        crate::light_pages::LightPageContext {
            workspace_root: workspace_root.as_path(),
            _package_root: package_root.as_path(),
            route_mode,
            app_id: app.id.as_str(),
            apps: apps.as_slice(),
            app_title,
            topbar_menu: &topbar_menu,
            lightweight_scene: scene_for_links,
            request_file: query.page.file.as_deref(),
            auth_enabled,
            account_view: account_view.as_ref(),
        },
    ) {
        return response;
    }
    (
        axum::http::StatusCode::NOT_FOUND,
        format!("{route_label} is not available for app `{}`", app.id),
    )
        .into_response()
}

pub async fn host_config_page(
    state: State<SharedState>,
    auth: State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    query: Query<HostScopeQuery>,
) -> Response {
    host_scoped_light_page(
        state,
        auth,
        principal,
        UiRouteMode::Config,
        "配置",
        "/config",
        ShellNavItem::Config,
        query,
    )
    .await
}

pub async fn host_upload_page(
    state: State<SharedState>,
    auth: State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    query: Query<HostScopeQuery>,
) -> Response {
    host_scoped_light_page(
        state,
        auth,
        principal,
        UiRouteMode::Upload,
        "上传",
        "/upload",
        ShellNavItem::Upload,
        query,
    )
    .await
}

pub async fn host_runtime_observation_page(
    state: State<SharedState>,
    auth: State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    uri: OriginalUri,
    headers: HeaderMap,
    query: Query<HostScopeQuery>,
) -> Response {
    let principal_ref = principal.as_ref().map(|Extension(p)| p);
    let (workspace_root, apps, _auth_enabled, _account_view) =
        host_scoped_context(&state, &auth, principal_ref).await;
    let Some(app) = resolve_scope_app_id(apps.as_slice(), query.app.as_deref()) else {
        let html = render_scope_picker_html(
            workspace_root.as_path(),
            apps.as_slice(),
            "运行",
            "/runtime",
            ShellNavItem::Runtime,
        );
        return Html(html).into_response();
    };
    app_page(
        state,
        auth,
        principal,
        uri,
        headers,
        AxumPath((
            UiRouteMode::Runtime.slug().to_string(),
            app.id.clone(),
        )),
        Query(query.page.clone()),
    )
    .await
}

pub async fn host_runtime_page(
    state: State<SharedState>,
    auth: State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    uri: OriginalUri,
    headers: HeaderMap,
    query: Query<HostScopeQuery>,
) -> Response {
    if query
        .app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return host_runtime_observation_page(state, auth, principal, uri, headers, query).await;
    }
    crate::host_runtime_hub::host_runtime_hub_page(state, auth, principal).await
}
