use std::path::Path;

use axum::{
    extract::{Extension, OriginalUri, Path as AxumPath, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Response},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, html_escape, AuthEnforcement,
    AuthPrincipal, AuthServeState,
};
use mei_lang_app::{load_topbar_menu_context, UiRouteMode, WorkspaceShellNav};
use mei_lang_kernel::WorkspaceAppMeta;
use serde::Deserialize;

use crate::landing::{discover_workspace_apps, enrich_discovered_apps};
use crate::pages::{app_page, AppQuery};
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;

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

fn workspace_shell_nav_for_route(route_path: &str) -> WorkspaceShellNav {
    match route_path {
        "/runtime" | "/mcg" => WorkspaceShellNav::Runtime,
        "/share" => WorkspaceShellNav::Share,
        _ => WorkspaceShellNav::Home,
    }
}

fn render_scope_picker_body_html(apps: &[WorkspaceAppMeta], route_path: &str) -> String {
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
            r#"<h2>请选择要管理的应用</h2><ul class="mei-host-shell__setup">{links}</ul>"#,
            links = links,
        )
    };
    format!(
        r#"{rows}
<div class="mei-host-shell__actions">
  <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="/home">返回首页</a>
</div>"#,
        rows = rows,
    )
}

fn render_scope_picker_html(
    workspace_root: &Path,
    picker_apps: &[WorkspaceAppMeta],
    topbar_apps: &[WorkspaceAppMeta],
    topbar_menu: &mei_lang_app::TopbarMenuContext,
    route_label: &str,
    route_path: &str,
    auth_enabled: bool,
    account_view: Option<&mei_lang_app::HostAccountView>,
) -> String {
    let body_html = render_scope_picker_body_html(picker_apps, route_path);
    render_workspace_shell_page(
        workspace_root,
        topbar_apps,
        topbar_menu,
        workspace_shell_nav_for_route(route_path),
        route_label,
        body_html.as_str(),
        auth_enabled,
        account_view,
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
    let topbar_apps = filter_apps_for_principal(
        crate::shell_chrome::apps_for_topbar(&guard).as_slice(),
        principal,
    );
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal);
    (
        workspace_root,
        apps,
        topbar_apps,
        auth_enabled,
        account_view,
    )
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
    let (workspace_root, apps, topbar_apps, auth_enabled, account_view) =
        host_scoped_context(&state, &auth, principal_ref).await;
    let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
    let Some(app) = resolve_scope_app_id(apps.as_slice(), query.app.as_deref()) else {
        let html = render_scope_picker_html(
            workspace_root.as_path(),
            apps.as_slice(),
            topbar_apps.as_slice(),
            &topbar_menu,
            "运行",
            "/runtime",
            auth_enabled,
            account_view.as_ref(),
        );
        return Html(html).into_response();
    };
    app_page(
        state,
        auth,
        principal,
        uri,
        headers,
        AxumPath((UiRouteMode::Runtime.slug().to_string(), app.id.clone())),
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
