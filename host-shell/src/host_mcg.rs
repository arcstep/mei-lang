use axum::{
    extract::{Extension, Query, State},
    response::{Html, IntoResponse, Response},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, html_escape, AuthEnforcement,
    AuthPrincipal, AuthServeState,
};
use mei_lang_app::{load_topbar_menu_context, WorkspaceShellNav};
use mei_lang_kernel::WorkspaceAppMeta;
use serde::Deserialize;

use crate::landing::{discover_workspace_apps, enrich_discovered_apps};
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;

#[derive(Debug, Deserialize, Default)]
pub struct McgPageQuery {
    pub app: Option<String>,
    pub bundle: Option<String>,
}

fn render_mcg_viewer_body_html(
    apps: &[WorkspaceAppMeta],
    selected_app: Option<&str>,
    bundle: Option<&str>,
) -> String {
    let app_options = apps
        .iter()
        .map(|app| {
            let selected = selected_app == Some(app.id.as_str());
            format!(
                r#"<option value="{id}"{selected}>{title}</option>"#,
                id = html_escape(app.id.as_str()),
                title = html_escape(app.title.as_str()),
                selected = if selected { " selected" } else { "" },
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let bundle_value = bundle.unwrap_or("");
    format!(
        r#"<div class="mei-host-shell__mcg-toolbar">
  <label>应用
    <select id="mei-mcg-app-select">{app_options}</select>
  </label>
  <label>Bundle 路径（可选）
    <input id="mei-mcg-bundle-input" type="text" placeholder="相对工作区路径" value="{bundle_value}" />
  </label>
  <button class="mei-host-shell__btn" type="button" id="mei-mcg-load-btn">加载</button>
</div>
<div class="mei-host-shell__mcg-layout">
  <aside class="mei-host-shell__mcg-tree" id="mei-mcg-tree" aria-label="MCG 节点树"></aside>
  <section class="mei-host-shell__mcg-detail" id="mei-mcg-detail" aria-label="节点详情">
    <p class="mei-host-shell__meta">选择左侧节点查看 payload / revision / 依赖边。</p>
  </section>
</div>
<script>
(function () {{
  const appSelect = document.getElementById("mei-mcg-app-select");
  const bundleInput = document.getElementById("mei-mcg-bundle-input");
  const loadBtn = document.getElementById("mei-mcg-load-btn");
  const treeEl = document.getElementById("mei-mcg-tree");
  const detailEl = document.getElementById("mei-mcg-detail");

  function currentApp() {{
    return String(appSelect?.value || "").trim();
  }}

  function syncUrl() {{
    const app = currentApp();
    const bundle = String(bundleInput?.value || "").trim();
    const params = new URLSearchParams();
    if (app) params.set("app", app);
    if (bundle) params.set("bundle", bundle);
    const qs = params.toString();
    const next = qs ? `/mcg?${{qs}}` : "/mcg";
    history.replaceState(null, "", next);
  }}

  async function loadRegistry() {{
    const app = currentApp();
    if (!app) {{
      treeEl.innerHTML = "<p class='mei-host-shell__meta'>请选择应用。</p>";
      return;
    }}
    treeEl.innerHTML = "<p class='mei-host-shell__meta'>加载中…</p>";
    const res = await fetch(`/api/build/graph/mcg?appId=${{encodeURIComponent(app)}}`);
    if (!res.ok) {{
      treeEl.innerHTML = `<p class='mei-host-shell__message'>加载失败：${{res.status}}</p>`;
      return;
    }}
    const registry = await res.json();
    const nodes = Array.isArray(registry.nodes) ? registry.nodes : [];
    if (!nodes.length) {{
      treeEl.innerHTML = "<p class='mei-host-shell__meta'>MCG 为空。</p>";
      return;
    }}
    const items = nodes.map((node) => {{
      const stable = node?.id ? `${{node.id.kind || ""}}:${{node.id.key || ""}}` : "";
      const label = stable || "node";
      return `<button type="button" class="mei-host-shell__mcg-node" data-node-id="${{encodeURIComponent(stable)}}">${{label}}</button>`;
    }}).join("");
    treeEl.innerHTML = `<div class="mei-host-shell__mcg-node-list">${{items}}</div>`;
    treeEl.querySelectorAll("[data-node-id]").forEach((btn) => {{
      btn.addEventListener("click", () => loadNode(btn.getAttribute("data-node-id")));
    }});
  }}

  async function loadNode(encodedId) {{
    const app = currentApp();
    const nodeId = decodeURIComponent(String(encodedId || ""));
    if (!app || !nodeId) return;
    detailEl.innerHTML = "<p class='mei-host-shell__meta'>加载节点…</p>";
    const url = `/api/build/graph/mcg/node?appId=${{encodeURIComponent(app)}}&nodeId=${{encodeURIComponent(nodeId)}}&includeArtifact=true`;
    const res = await fetch(url);
    if (!res.ok) {{
      detailEl.innerHTML = `<p class='mei-host-shell__message'>节点加载失败：${{res.status}}</p>`;
      return;
    }}
    const payload = await res.json();
    detailEl.innerHTML = `<pre class="mei-host-shell__mcg-json">${{JSON.stringify(payload, null, 2)}}</pre>`;
  }}

  loadBtn?.addEventListener("click", () => {{
    syncUrl();
    loadRegistry();
  }});
  appSelect?.addEventListener("change", () => {{
    syncUrl();
    loadRegistry();
  }});
  if (currentApp()) loadRegistry();
}})();
</script>"#,
        app_options = app_options,
        bundle_value = html_escape(bundle_value),
    )
}

pub async fn host_mcg_page(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<McgPageQuery>,
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
    let selected_app = query
        .app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| apps.first().map(|app| app.id.as_str()));
    let bundle = query
        .bundle
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let topbar_apps = crate::shell_chrome::apps_for_topbar(&guard);
    let body_html = render_mcg_viewer_body_html(apps.as_slice(), selected_app, bundle);
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal_ref);
    let html = render_workspace_shell_page(
        workspace_root,
        // Topbar follows LaunchManifest running set; Mcg body picker still uses full discover.
        topbar_apps.as_slice(),
        &topbar_menu,
        WorkspaceShellNav::Mcg,
        "MCG 检视",
        body_html.as_str(),
        auth_enabled,
        account_view.as_ref(),
    );
    Html(html).into_response()
}
