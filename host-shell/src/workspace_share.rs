use axum::{
    extract::{Extension, State},
    response::{Html, IntoResponse},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, html_escape, AuthEnforcement,
    AuthPrincipal, AuthServeState,
};
use mei_lang_app::{load_topbar_menu_context, HostCapabilities, WorkspaceShellNav};

use crate::host_page_pack::{
    render_native_recovery_html, render_workspace_share_page_body, workspace_share_page_pack,
};
use crate::shell_chrome::apps_for_topbar;
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;

pub async fn workspace_share_page(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    let principal_ref = principal.as_ref().map(|value| &value.0);
    let capabilities = principal_ref
        .map(AuthPrincipal::capabilities)
        .unwrap_or_else(HostCapabilities::auth_disabled);
    let account_view = account_view_for_principal(principal_ref);
    let (workspace_root, running_apps) = {
        let guard = state.read().expect("state lock");
        let apps = apps_for_topbar(&guard);
        (
            guard.ctx.workspace_root.clone(),
            filter_apps_for_principal(apps.as_slice(), principal_ref),
        )
    };
    let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
    let props = serde_json::to_string(&serde_json::json!({
        "title": "资料交换",
        "capabilities": {
            "view": capabilities.workspace_share_view,
            "upload": capabilities.workspace_share_upload,
            "organize": capabilities.workspace_share_organize,
            "delete": capabilities.workspace_share_delete,
        }
    }))
    .unwrap_or_else(|_| "{}".to_string());
    let share_explorer = format!(
        r#"<section class="mei-workspace-share-page">
  <header>
    <h1>资料交换</h1>
    <p>工作区成员共享的文件交换区。文件是资源，文件夹是分类；与业务应用的 <code>upload/admin</code> 数据源文件隔离。</p>
  </header>
  <mei-workspace-share data-props="{}"></mei-workspace-share>
</section>
<script type="module" src="/workspace-components/admin/runtime.js" data-mei-persistent-script="/workspace-components/admin/runtime.js"></script>"#,
        html_escape(props.as_str())
    );
    let body = match render_workspace_share_page_body(
        Some(workspace_share_page_pack()),
        share_explorer.as_str(),
    ) {
        Ok(body) => body,
        Err(error) => return Html(render_native_recovery_html(error)),
    };
    let html = render_workspace_shell_page(
        workspace_root.as_path(),
        running_apps.as_slice(),
        &topbar_menu,
        WorkspaceShellNav::Home,
        "资料交换",
        body.as_str(),
        auth.auth_enforcement == AuthEnforcement::Required,
        account_view.as_ref(),
    );
    Html(html)
}
