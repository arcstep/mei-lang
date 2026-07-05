use std::fs;
use std::path::Path;

use axum::{
    extract::{Extension, State},
    response::{Html, IntoResponse, Response},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, html_escape, AuthEnforcement,
    AuthPrincipal, AuthServeState,
};
use mei_lang_app::{load_topbar_menu_context, WorkspaceShellNav};
use mei_lang_kernel::{
    resolve_app_root, resolve_workspace_app_build_generations, WorkspaceAppMeta,
};
use crate::build_info::workspace_descriptor;
use crate::landing::{app_has_prebuilt_access_entry, discover_workspace_apps, enrich_discovered_apps};
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;

fn list_app_env_versions(app_root: &Path) -> Vec<String> {
    let env_root = app_root.join("env");
    let Ok(entries) = fs::read_dir(env_root) else {
        return Vec::new();
    };
    let mut versions = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("WS-"))
        .collect::<Vec<_>>();
    versions.sort();
    versions.dedup();
    versions
}

fn render_runtime_hub_body_html(
    workspace_root: &Path,
    apps: &[WorkspaceAppMeta],
    current_by_app: &std::collections::BTreeMap<String, String>,
    workspace_meta: &serde_json::Value,
) -> String {
    let toolchain = workspace_meta
        .get("toolchain")
        .and_then(|value| value.get("active"))
        .and_then(|value| value.as_str())
        .unwrap_or("—");
    let build_generation = workspace_meta
        .get("buildGeneration")
        .and_then(|value| value.as_str())
        .unwrap_or("—");

    let app_cards = if apps.is_empty() {
        r#"<p class="mei-host-shell__message">当前没有可管理的应用。</p>"#.to_string()
    } else {
        apps.iter()
            .map(|app| {
                let app_root = resolve_app_root(workspace_root, app.id.as_str());
                let access_ready = app_has_prebuilt_access_entry(workspace_root, app.id.as_str());
                let current_env = current_by_app
                    .get(app.id.as_str())
                    .map(String::as_str)
                    .unwrap_or("—");
                let env_versions = list_app_env_versions(app_root.as_path());
                let env_options = env_versions
                    .iter()
                    .map(|version| {
                        let selected = version == current_env;
                        format!(
                            r#"<option value="{version}"{selected}>{version}</option>"#,
                            version = html_escape(version),
                            selected = if selected { " selected" } else { "" },
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("");
                let access_href = format!("/apps/app/{}/access", app.id);
                let observe_href = format!("/runtime?app={}", app.id);
                let prebuild_btn = if access_ready {
                    String::new()
                } else {
                    format!(
                        r#"<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-mei-prebuild-app="{}">预构建</button>"#,
                        html_escape(app.id.as_str()),
                    )
                };
                let access_btn = if access_ready {
                    format!(
                        r#"<a class="mei-host-shell__btn" href="{}">进入访问</a>"#,
                        html_escape(access_href.as_str()),
                    )
                } else {
                    r#"<span class="mei-host-shell__meta">未就绪</span>"#.to_string()
                };
                format!(
                    r#"<article class="mei-host-shell__runtime-card" data-app-id="{app_id}">
  <header class="mei-host-shell__runtime-card-head">
    <h2 class="mei-host-shell__card-title">{title}</h2>
    <code class="mei-host-shell__card-id">{app_id}</code>
  </header>
  <p class="mei-host-shell__card-desc">当前代次：<strong>{current_env}</strong> · 状态：<strong>{status}</strong></p>
  <label class="mei-host-shell__runtime-env-label">切换 bundle 代次
    <select class="mei-host-shell__runtime-env-select" data-mei-env-app="{app_id}">{env_options}</select>
  </label>
  <div class="mei-host-shell__card-actions">
    {access_btn}
    <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="{observe_href}">运行观测</a>
    {prebuild_btn}
  </div>
</article>"#,
                    app_id = html_escape(app.id.as_str()),
                    title = html_escape(app.title.as_str()),
                    current_env = html_escape(current_env),
                    status = if access_ready { "ready" } else { "missing" },
                    env_options = env_options,
                    access_btn = access_btn,
                    observe_href = html_escape(observe_href.as_str()),
                    prebuild_btn = prebuild_btn,
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        r#"<section class="mei-host-shell__runtime-meta">
  <p class="mei-host-shell__meta">工具链：<code>{toolchain}</code> · 工作区 buildGeneration：<code>{build_generation}</code></p>
  <div class="mei-host-shell__actions">
    <button class="mei-host-shell__btn" type="button" data-mei-ops-reload>全工作区 reload</button>
    <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-mei-ops-prebuild>全工作区 prebuild</button>
  </div>
</section>
<section class="mei-host-shell__runtime-grid">{app_cards}</section>
<script>
(function() {{
  async function postJson(url, body) {{
    const res = await fetch(url, {{ method: "POST", headers: {{ "Content-Type": "application/json" }}, body: JSON.stringify(body || {{}}) }});
    return res.json().catch(() => ({{}}));
  }}
  document.querySelector("[data-mei-ops-reload]")?.addEventListener("click", async () => {{
    await postJson("/api/host/ops/reload", {{}});
    location.reload();
  }});
  document.querySelector("[data-mei-ops-prebuild]")?.addEventListener("click", async () => {{
    await postJson("/api/host/ops/prebuild", {{}});
    location.reload();
  }});
  document.querySelectorAll("[data-mei-prebuild-app]").forEach((btn) => {{
    btn.addEventListener("click", async () => {{
      const appId = btn.getAttribute("data-mei-prebuild-app");
      if (!appId) return;
      await postJson("/api/host/ops/prebuild", {{ appId }});
      location.reload();
    }});
  }});
  document.querySelectorAll("[data-mei-env-app]").forEach((select) => {{
    select.addEventListener("change", async () => {{
      const appId = select.getAttribute("data-mei-env-app");
      const envVersion = select.value;
      if (!appId || !envVersion) return;
      const url = `/api/host/runtime/activate-env?appId=${{encodeURIComponent(appId)}}&envVersion=${{encodeURIComponent(envVersion)}}`;
      await fetch(url, {{ method: "POST" }});
      location.reload();
    }});
  }});
}})();
</script>"#,
        toolchain = html_escape(toolchain),
        build_generation = html_escape(build_generation),
        app_cards = app_cards,
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
    let app_ids: Vec<String> = apps.iter().map(|app| app.id.clone()).collect();
    let current_by_app =
        resolve_workspace_app_build_generations(workspace_root, &app_ids).unwrap_or_default();
    let body_html = render_runtime_hub_body_html(
        workspace_root,
        apps.as_slice(),
        &current_by_app,
        &workspace_meta,
    );
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
