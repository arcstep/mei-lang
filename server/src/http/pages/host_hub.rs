use std::path::Path;

use axum::{extract::State, response::Html, Extension};
use mei_lang_kernel::{discover_apps, load_workspace_config, WorkspaceAppMeta};

use crate::{auth::AuthPrincipal, http::host_error_page, AppError, AppState};

use super::app_render::{app_has_prebuilt_access_entry, choose_default_app};

#[derive(Debug, Clone)]
struct HostHubAppRow {
    app_id: String,
    title: String,
    phase: String,
    access_ready: bool,
    warning_count: usize,
    build_href: String,
}

fn app_row_from_discover(
    source_root: &Path,
    app: &WorkspaceAppMeta,
    registry_app: Option<&crate::http::host_api::HostAppReadinessResponse>,
) -> HostHubAppRow {
    let access_ready = registry_app
        .map(|state| state.access_ready)
        .unwrap_or_else(|| app_has_prebuilt_access_entry(source_root, app.id.as_str()));
    let phase = registry_app
        .map(|state| state.phase.clone())
        .unwrap_or_else(|| {
            if access_ready {
                "ready".to_string()
            } else {
                "degraded".to_string()
            }
        });
    HostHubAppRow {
        app_id: app.id.clone(),
        title: app.title.clone(),
        phase,
        access_ready,
        warning_count: registry_app.map(|state| state.warnings.len()).unwrap_or(0),
        build_href: format!("/apps/{}/layout", app.id),
    }
}

pub(crate) fn render_host_hub_html(source_root: &Path) -> String {
    let workspace = load_workspace_config(source_root);
    let workspace_label = workspace
        .workspace
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let default_app = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let apps = discover_apps(source_root).unwrap_or_default();
    let snapshot = crate::http::host_api::registry_snapshot();
    let registry_by_id = snapshot
        .apps
        .iter()
        .map(|app| (app.app_id.clone(), app.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let rows = apps
        .iter()
        .map(|app| {
            app_row_from_discover(
                source_root,
                app,
                registry_by_id.get(app.id.as_str()),
            )
        })
        .collect::<Vec<_>>();
    let ready_landing = choose_default_app(source_root, &apps).is_some();
    let any_ready = rows.iter().any(|row| row.access_ready);
    let footer_html =
        host_error_page::render_host_shell_footer_for_source_root(source_root);
    let shell_theme = host_error_page::host_shell_body_theme_style(source_root);
    let prebuild_cmd = format!("./deploy/prebuild.sh");
    let table_rows = if rows.is_empty() {
        format!(
            r#"<tr><td colspan="5">未发现可加载的应用（需要在 workspace 根下存在含 <code>main.mei</code> 的一级 app 目录）。</td></tr>"#
        )
    } else {
        rows.iter()
            .map(|row| {
                let gate = registry_by_id
                    .get(row.app_id.as_str())
                    .and_then(|app| app.gate_summary.as_ref());
                let gate_text = gate
                    .map(|summary| {
                        format!(
                            "L2={} L3={} L4={}",
                            summary.l2_miss, summary.l3_fail, summary.l4_stale
                        )
                    })
                    .unwrap_or_else(|| "—".to_string());
                let ready_label = if row.access_ready {
                    "是"
                } else {
                    "否"
                };
                let phase_label = if row.warning_count > 0 {
                    format!("{} ({} warnings)", row.phase, row.warning_count)
                } else {
                    row.phase.clone()
                };
                format!(
                    r#"<tr>
  <td><code>{app_id}</code></td>
  <td>{title}</td>
  <td>{phase}</td>
  <td>{ready_label}</td>
  <td><code>{gate_text}</code></td>
  <td><a href="{build_href}">build</a></td>
</tr>"#,
                    app_id = host_error_page::html_escape(row.app_id.as_str()),
                    title = host_error_page::html_escape(row.title.as_str()),
                    phase = host_error_page::html_escape(phase_label.as_str()),
                    ready_label = ready_label,
                    gate_text = host_error_page::html_escape(gate_text.as_str()),
                    build_href = host_error_page::html_escape(row.build_href.as_str()),
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let headline = if ready_landing {
        "工作区已就绪"
    } else if any_ready {
        "部分应用尚未就绪"
    } else {
        "工作区壳层可用"
    };
    let intro = if ready_landing {
        "至少一个应用的 default-scope gate 已通过。你可以从下方进入 build，或等待 prebuild 完成后刷新首页。".to_string()
    } else {
        format!(
            "宿主 HTTP 已启动，但当前没有应用通过 default-scope gate（L2+L3）。请在工作区根目录执行 <code>{prebuild_cmd}</code> 后刷新；壳层功能（登录、配置、上传）仍可使用。"
        )
    };
    let default_app_line = default_app
        .as_ref()
        .map(|app| {
            format!(
                r#"<p class="mei-host-shell__meta">默认应用：<code>{}</code></p>"#,
                host_error_page::html_escape(app.as_str())
            )
        })
        .unwrap_or_default();
    let workspace_line = workspace_label
        .as_ref()
        .map(|label| {
            format!(
                r#"<p class="mei-host-shell__meta">工作区：{}</p>"#,
                host_error_page::html_escape(label.as_str())
            )
        })
        .unwrap_or_default();
    let body_html = format!(
        r#"{workspace_line}{default_app_line}
<p class="mei-host-shell__message">{intro}</p>
<table class="mei-host-shell__table">
  <thead>
    <tr>
      <th>App</th>
      <th>标题</th>
      <th>Phase</th>
      <th>accessReady</th>
      <th>Gate</th>
      <th>入口</th>
    </tr>
  </thead>
  <tbody>{table_rows}</tbody>
</table>
<div class="mei-host-shell__actions">
  <a class="mei-host-shell__btn mei-host-shell__btn--primary" href="/login">登录</a>
  <a class="mei-host-shell__btn" href="/api/host/readiness">Readiness JSON</a>
</div>
<p class="mei-host-shell__setup">Prebuild：<code>{prebuild_cmd}</code> 或 <code>{prebuild_cmd} --toolchain-mode cargo --json</code></p>"#,
        workspace_line = workspace_line,
        default_app_line = default_app_line,
        intro = intro,
        table_rows = table_rows,
        prebuild_cmd = host_error_page::html_escape(prebuild_cmd.as_str()),
    );
    host_error_page::render_auth_card_page(
        "工作区",
        headline,
        body_html.as_str(),
        footer_html.as_str(),
        shell_theme.as_str(),
    )
}

pub async fn host_hub_page(
    State(state): State<AppState>,
    _principal: Option<Extension<AuthPrincipal>>,
) -> Result<Html<String>, AppError> {
    Ok(Html(render_host_hub_html(state.source_root.as_path())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn host_hub_renders_without_apps() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-hub-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        let html = render_host_hub_html(root.as_path());
        assert!(html.contains("工作区壳层可用"));
        assert!(html.contains("prebuild.sh"));
        let _ = fs::remove_dir_all(&root);
    }
}
