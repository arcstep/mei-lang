use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use mei_lang_app::SourcePanelMeta;
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{
    discover_apps, load_workspace_config, resolve_app_id, WorkspaceAppMeta,
};
use std::fs;

/// 宿主 landing 只认统一 scope gate（L2 MRG + L3 assemble）；不在 HTTP 路径触发 compile。
fn app_has_prebuilt_access_entry(source_root: &Path, app_id: &str) -> bool {
    use crate::graph::mrg::navigation::resolve_default_scope;
    use crate::http::pages::AppQuery;
    use crate::readiness::scope_gate::resolve_scope_gate;
    use crate::readiness::types::UiMode;

    let nav = resolve_default_scope(source_root, app_id, UiMode::App);
    let query = AppQuery {
        file: Some(nav.scope.target_file.clone()),
        scene: Some(nav.scope.scene_id.clone()),
        tab: None,
        diag_filter: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        node: None,
        scope: None,
        focus: None,
        chrome: None,
        catalog: None,
        pack: None,
    };
    resolve_scope_gate(
        source_root,
        app_id,
        UiRouteMode::App,
        Some(nav.scope.scene_id.as_str()),
        &query,
    )
    .access_ready
}

pub(crate) fn source_panel_meta(source_path: &Path, source: &str) -> SourcePanelMeta {
    let line_count = if source.is_empty() {
        0
    } else {
        source.split('\n').count()
    };
    let char_count = source.chars().count();
    let last_modified_label = fs::metadata(source_path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|modified| {
            let modified: DateTime<Local> = modified.into();
            modified.format("%Y-%m-%d %H:%M:%S").to_string()
        });
    SourcePanelMeta {
        line_count,
        char_count,
        last_modified_label,
    }
}

pub(crate) fn choose_default_app<'a>(
    source_root: &Path,
    apps: &'a [WorkspaceAppMeta],
) -> Option<&'a WorkspaceAppMeta> {
    let workspace = load_workspace_config(source_root);
    if let Some(preferred) = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let canonical = resolve_app_id(source_root, preferred);
        if let Some(app) = apps
            .iter()
            .find(|app| app.id == canonical || app.id == preferred)
        {
            if app_has_prebuilt_access_entry(source_root, &app.id) {
                return Some(app);
            }
            tracing::warn!(
                app_id = %app.id,
                "configured workspace.defaultApp default-scope scope gate not ready (L2+L3)"
            );
        } else {
            tracing::warn!(
                app_id = preferred,
                "configured workspace.defaultApp is not discoverable in this workspace"
            );
        }
    }
    for app in apps {
        if app_has_prebuilt_access_entry(source_root, &app.id) {
            return Some(app);
        }
        tracing::warn!(
            app_id = %app.id,
            "skip app without default-scope scope gate ready as landing target"
        );
    }
    None
}

/// `serve` 启动门禁：landing 目标 app 的 default-scope scope gate（L2+L3）必须 ready。
pub(crate) fn prepare_landing_artifacts_for_serve(source_root: &Path) -> Result<()> {
    let apps = discover_apps(source_root)
        .with_context(|| format!("discover apps under `{}`", source_root.display()))?;
    if apps.is_empty() {
        anyhow::bail!(
            "host landing gate failed: no discoverable apps under `{}` (need a first-level app directory with `main.mei`)",
            source_root.display()
        );
    }
    let workspace = load_workspace_config(source_root);
    let preferred = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(app) = choose_default_app(source_root, &apps) {
        tracing::info!(
            app_id = %app.id,
            default_app = preferred.unwrap_or("(auto)"),
            "host landing gate passed: default-scope scope gate ready (L2 navigation + L3 MCG assemble)"
        );
        return Ok(());
    }
    let mut lines = vec![
        "host landing gate failed: no app has default-scope scope gate ready (L2+L3)."
            .to_string(),
        format!("source-root: {}", source_root.display()),
    ];
    if let Some(preferred) = preferred {
        let canonical = resolve_app_id(source_root, preferred);
        lines.push(format!(
            "workspace.defaultApp `{preferred}` (resolved `{canonical}`) is missing default-scope artifacts."
        ));
    } else {
        lines.push(
            "no landing app passed manifest probe; configure workspace.defaultApp or prebuild at least one app."
                .to_string(),
        );
    }
    lines.push(String::new());
    lines.push("Run prebuild before serve (host HTTP paths do not compile):".to_string());
    lines.push(format!(
        "  {}/deploy/prebuild.sh",
        source_root.display()
    ));
    lines.push("Or with cargo toolchain (dev checkout):".to_string());
    lines.push(format!(
        "  {}/deploy/prebuild.sh --toolchain-mode cargo --json",
        source_root.display()
    ));
    lines.push("Or verify existing artifacts:".to_string());
    lines.push(format!(
        "  {}/deploy/prebuild.sh --verify --json",
        source_root.display()
    ));
    anyhow::bail!("{}", lines.join("\n"))
}
