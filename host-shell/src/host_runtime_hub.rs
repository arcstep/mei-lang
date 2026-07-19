use crate::host_page_pack::{
    render_native_recovery_html, render_runtime_page_body, runtime_page_pack, HostPagePack,
};
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;
use axum::{
    extract::{Extension, State},
    response::{Html, IntoResponse, Response},
};
use mei_host_auth::{account_view_for_principal, AuthEnforcement, AuthPrincipal, AuthServeState};
use mei_lang_app::{load_topbar_menu_context, HostAccountView, TopbarMenuContext, WorkspaceShellNav};
use mei_lang_kernel::WorkspaceAppMeta;
use std::path::Path;

fn runtime_hub_host_tools_html() -> String {
    let config_href = mei_lang_app::host_config_href(None);
    let upload_href = mei_lang_app::host_upload_href(None, None);
    let mcg_href = mei_lang_app::mcg_href(None);
    format!(
        r#"<nav class="mei-runtime-control__host-tools" aria-label="系统工具">
    <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="{config_href}">配置</a>
    <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="{upload_href}">上传</a>
    <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="{mcg_href}">MCG</a>
  </nav>"#
    )
}

fn runtime_hub_control_html() -> String {
    // Host tools: scope pickers still on /config|/upload (dual-render Admin Shell once app chosen).
    // Per-app cards link to /admin/apps/{id}/ops_config|upload_files (see host-runtime-control-center.js).
    r#"<div class="mei-runtime-control" data-host-runtime-control-center>
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
        .to_string()
}

fn render_runtime_hub_body_html_with_pack(pack: Option<&HostPagePack>) -> Result<String, crate::host_page_pack::HostPagePackError> {
    render_runtime_page_body(
        pack,
        runtime_hub_host_tools_html().as_str(),
        runtime_hub_control_html().as_str(),
    )
}

#[cfg(test)]
fn render_runtime_hub_body_html() -> String {
    render_runtime_hub_body_html_with_pack(Some(runtime_page_pack()))
        .unwrap_or_else(render_native_recovery_html)
}

fn render_runtime_hub_document_with_pack(
    pack: Option<&HostPagePack>,
    workspace_root: &Path,
    topbar_apps: &[WorkspaceAppMeta],
    topbar_menu: &TopbarMenuContext,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
) -> String {
    let body_html = match render_runtime_hub_body_html_with_pack(pack) {
        Ok(html) => html,
        Err(error) => return render_native_recovery_html(error),
    };
    render_workspace_shell_page(
        workspace_root,
        topbar_apps,
        topbar_menu,
        WorkspaceShellNav::Runtime,
        "运行中心",
        body_html.as_str(),
        auth_enabled,
        account_view,
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
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal_ref);
    let html = render_runtime_hub_document_with_pack(
        Some(runtime_page_pack()),
        workspace_root,
        topbar_apps.as_slice(),
        &topbar_menu,
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
        assert!(html.contains(r#"data-mei-pagepack="host.runtime""#));
        assert!(html.contains(r#"data-mei-page-surface="document""#));
        assert!(html.contains(r#"data-mei-pagepack-digest="sha256:"#));
        assert!(html.contains("href=\"/config\""));
        assert!(html.contains("href=\"/upload\""));
        assert!(html.contains("href=\"/mcg\""));
        assert!(!html.contains("data-runtime-page-status"));
        assert!(!html.contains("运行控制中心"));
        assert!(!html.contains("工具链"));
        assert!(!html.contains("data-runtime-profile-mount"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn runtime_hub_missing_or_invalid_pack_returns_native_recovery() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-runtime-recovery-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let topbar_menu = mei_lang_app::load_topbar_menu_context(root.as_path());
        let missing = render_runtime_hub_document_with_pack(
            None,
            root.as_path(),
            &[],
            &topbar_menu,
            false,
            None,
        );
        let mut invalid = runtime_page_pack().clone();
        invalid.digest = "sha256:invalid".to_string();
        let broken = render_runtime_hub_document_with_pack(
            Some(&invalid),
            root.as_path(),
            &[],
            &topbar_menu,
            false,
            None,
        );
        for html in [missing, broken] {
            assert!(html.contains("data-mei-native-recovery=\"host-page-pack\""));
            assert!(html.contains("href=\"/home\""));
            assert!(html.contains("href=\"/runtime\""));
            assert!(html.contains("href=\"/login\""));
            assert!(!html.contains("topbar-shell"));
            assert!(!html.contains("/app-assets/"));
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
