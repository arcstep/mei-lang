use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;
use axum::{
    extract::{Extension, State},
    response::{Html, IntoResponse, Response},
};
use mei_host_auth::{account_view_for_principal, AuthEnforcement, AuthPrincipal, AuthServeState};
use mei_lang_app::{load_topbar_menu_context, WorkspaceShellNav};

fn render_runtime_hub_body_html() -> String {
    // Host tools: scope pickers still on /config|/upload (dual-render Admin Shell once app chosen).
    // Per-app cards link to /admin/apps/{id}/ops_config|upload_files (see host-runtime-control-center.js).
    let config_href = mei_lang_app::host_config_href(None);
    let upload_href = mei_lang_app::host_upload_href(None, None);
    let mcg_href = mei_lang_app::mcg_href(None);
    format!(
        r#"<div class="mei-runtime-control" data-host-runtime-control-center>
  <nav class="mei-runtime-control__host-tools" aria-label="系统工具">
    <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="{config_href}">配置</a>
    <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="{upload_href}">上传</a>
    <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="{mcg_href}">MCG</a>
  </nav>
  <div class="mei-runtime-control__toolbar">
    <p class="mei-runtime-control__live" data-runtime-live role="status" aria-live="polite">正在载入应用…</p>
    <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-refresh-instances>刷新</button>
  </div>
  <div class="mei-runtime-control__global-ops" data-runtime-global-ops hidden></div>
  <section class="mei-runtime-control__zone mei-runtime-control__zone--apps" aria-label="应用">
    <div data-runtime-app-grid class="mei-runtime-control__app-grid">
      <p class="mei-host-shell__message">正在读取应用与 launch 配置…</p>
    </div>
  </section>
  <div class="mei-runtime-cleanup-modal" data-runtime-cleanup-modal hidden></div>
</div>"#
    )
}

pub async fn host_runtime_hub_page(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> Response {
    let principal_ref = principal.as_ref().map(|Extension(p)| p);
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let topbar_apps = crate::shell_chrome::apps_for_topbar(&guard);
    let body_html = render_runtime_hub_body_html();
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal_ref);
    let html = render_workspace_shell_page(
        workspace_root,
        topbar_apps.as_slice(),
        &topbar_menu,
        WorkspaceShellNav::Runtime,
        "运行中心",
        body_html.as_str(),
        auth_enabled,
        account_view.as_ref(),
    );
    // Page title stays operational; topbar labels this surface 「应用中心」(0544).
    Html(html).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_hub_renders_app_card_mounts_without_inline_script() {
        let html = render_runtime_hub_body_html();

        for mount in [
            "data-host-runtime-control-center",
            "data-runtime-app-grid",
            "data-runtime-global-ops",
            "data-runtime-live",
            "data-runtime-cleanup-modal",
            "data-runtime-refresh-instances",
            "mei-runtime-control__host-tools",
        ] {
            assert!(html.contains(mount), "missing runtime hub mount: {mount}");
        }
        assert!(html.contains("href=\"/config\""));
        assert!(html.contains("href=\"/upload\""));
        assert!(html.contains("href=\"/mcg\""));
        assert!(!html.contains("data-runtime-page-status"));
        assert!(!html.contains("运行控制中心"));
        assert!(!html.contains("工具链"));
        assert!(!html.contains("data-runtime-profile-mount"));
        assert!(!html.contains("<script"));
    }
}
