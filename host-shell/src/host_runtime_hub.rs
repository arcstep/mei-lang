use crate::build_info::workspace_descriptor;
use crate::landing::{discover_workspace_apps, enrich_discovered_apps};
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;
use axum::{
    extract::{Extension, State},
    response::{Html, IntoResponse, Response},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, html_escape, AuthEnforcement,
    AuthPrincipal, AuthServeState,
};
use mei_lang_app::{load_topbar_menu_context, WorkspaceShellNav};
fn render_runtime_hub_body_html(workspace_meta: &serde_json::Value) -> String {
    let toolchain = workspace_meta
        .get("toolchain")
        .and_then(|value| value.get("active"))
        .and_then(|value| value.as_str())
        .unwrap_or("—");
    let build_generation = workspace_meta
        .get("buildGeneration")
        .and_then(|value| value.as_str())
        .unwrap_or("—");

    format!(
        r#"<div class="mei-runtime-control" data-host-runtime-control-center>
  <header class="mei-runtime-control__hero">
    <div>
      <p class="mei-runtime-control__eyebrow">Host Control Center</p>
      <h1 class="mei-runtime-control__title">运行控制中心</h1>
      <p class="mei-host-shell__meta">配置档与 LaunchManifest、实例路由切流、Builder 任务与 Bundle 容量。</p>
    </div>
    <div class="mei-runtime-control__version" aria-label="当前版本摘要">
      <span>工具链 <code>{toolchain}</code></span>
      <span>buildGeneration <code>{build_generation}</code></span>
      <span data-runtime-control-status>状态 <code>载入中</code></span>
      <span data-runtime-access-status>Access <code>载入中</code></span>
    </div>
  </header>

  <p class="mei-runtime-control__live" data-runtime-live role="status" aria-live="polite">正在载入运行配置…</p>

  <section class="mei-runtime-control__zone" aria-labelledby="runtime-profile-heading">
    <header class="mei-runtime-control__zone-head">
      <div>
        <p class="mei-runtime-control__zone-index">01</p>
        <h2 id="runtime-profile-heading">配置档 / 启动清单</h2>
      </div>
      <div data-runtime-profile-actions></div>
    </header>
    <div data-runtime-profile-mount class="mei-runtime-control__profile"></div>
    <div data-runtime-manifest-mount class="mei-runtime-control__manifest"></div>
    <div data-runtime-json-mount></div>
    <details class="mei-runtime-control__advanced">
      <summary>RuntimePlan（hot / lazy / frozen）</summary>
      <div data-runtime-plan-summary></div>
      <div data-runtime-plan-mount></div>
    </details>
    <div data-runtime-dry-run-mount class="mei-runtime-control__dry-run"></div>
  </section>

  <section class="mei-runtime-control__zone" aria-labelledby="runtime-instances-heading">
    <header class="mei-runtime-control__zone-head">
      <div>
        <p class="mei-runtime-control__zone-index">02</p>
        <h2 id="runtime-instances-heading">实例与路由</h2>
      </div>
      <div class="mei-host-shell__actions">
        <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-refresh-instances>刷新</button>
      </div>
    </header>
    <div data-runtime-instances-mount>
      <p class="mei-host-shell__message">正在读取实例与路由…</p>
    </div>
  </section>

  <section class="mei-runtime-control__zone" aria-labelledby="runtime-task-heading">
    <header class="mei-runtime-control__zone-head">
      <div>
        <p class="mei-runtime-control__zone-index">03</p>
        <h2 id="runtime-task-heading">Builder 任务</h2>
      </div>
      <div class="mei-host-shell__actions">
        <button class="mei-host-shell__btn" type="button" data-runtime-builds-request>请求 Build Worker</button>
        <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-mei-ops-reload>全工作区 reload</button>
        <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-mei-ops-prebuild>全工作区 prebuild</button>
      </div>
    </header>
    <div data-runtime-task-mount>
      <p class="mei-host-shell__message">正在读取当前 ops job…</p>
    </div>
  </section>

  <section class="mei-runtime-control__zone" aria-labelledby="runtime-artifact-heading">
    <header class="mei-runtime-control__zone-head">
      <div>
        <p class="mei-runtime-control__zone-index">04</p>
        <h2 id="runtime-artifact-heading">Bundle 与容量</h2>
      </div>
      <div class="mei-host-shell__actions">
        <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-refresh-builds>刷新</button>
        <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-cleanup-preview>清理预览</button>
      </div>
    </header>
    <p class="mei-runtime-control__notice">默认以全部活动 profile / discover app 为一个一致代次；仅 coherent 代次可激活。activate 会启动 candidate 实例并 cutover 路由。</p>
    <div data-runtime-builds-mount><p class="mei-host-shell__message">正在读取工作区 generation…</p></div>
    <div data-runtime-cleanup-mount></div>
    <details class="mei-runtime-control__advanced" data-runtime-single-app-diagnostic>
      <summary>高级诊断：单 app env 切换</summary>
      <p class="mei-runtime-control__validation is-invalid" role="alert">警告：单 app 激活会造成工作区代次不一致，仅用于诊断，不是默认发布操作。</p>
      <div data-runtime-single-app-mount></div>
    </details>
  </section>
</div>"#,
        toolchain = html_escape(toolchain),
        build_generation = html_escape(build_generation),
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
    let discovered = discover_workspace_apps(workspace_root).unwrap_or_default();
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let apps = enrich_discovered_apps(
        filter_apps_for_principal(discovered.as_slice(), principal_ref).as_slice(),
        &topbar_menu,
    );
    let workspace_meta = workspace_descriptor(workspace_root);
    let body_html = render_runtime_hub_body_html(&workspace_meta);
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal_ref);
    let html = render_workspace_shell_page(
        workspace_root,
        apps.as_slice(),
        &topbar_menu,
        WorkspaceShellNav::Runtime,
        "运行中心",
        body_html.as_str(),
        auth_enabled,
        account_view.as_ref(),
    );
    Html(html).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_hub_renders_stable_control_center_mounts_without_inline_script() {
        let html = render_runtime_hub_body_html(&serde_json::json!({
            "toolchain": {"active": "test"},
            "buildGeneration": "WS-test"
        }));

        for mount in [
            "data-host-runtime-control-center",
            "data-runtime-profile-mount",
            "data-runtime-json-mount",
            "data-runtime-plan-mount",
            "data-runtime-dry-run-mount",
            "data-runtime-task-mount",
            "data-runtime-builds-mount",
            "data-runtime-cleanup-mount",
            "data-runtime-instances-mount",
            "data-runtime-manifest-mount",
        ] {
            assert!(html.contains(mount), "missing runtime hub mount: {mount}");
        }
        assert!(!html.contains("<script"));
    }
}
