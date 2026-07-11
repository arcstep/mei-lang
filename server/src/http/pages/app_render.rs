use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Local};
use mei_lang_app::SourcePanelMeta;
use mei_lang_kernel::{discover_apps, load_workspace_config, resolve_app_id, WorkspaceAppMeta};
use std::fs;

/// 宿主 landing 只认统一 scope gate（L2 MRG + L3 assemble）；不在 HTTP 路径触发 compile。
pub(crate) fn app_has_prebuilt_access_entry(source_root: &Path, app_id: &str) -> bool {
    crate::readiness::scope_gate::default_app_access_ready(source_root, app_id)
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
        }
    }
    for app in apps {
        if app_has_prebuilt_access_entry(source_root, &app.id) {
            return Some(app);
        }
    }
    None
}

#[derive(Debug, Clone)]
pub(crate) struct LandingProbeReport {
    pub ready_app_id: Option<String>,
    pub app_count: usize,
    pub configured_default_app: Option<String>,
    pub message: Option<String>,
}

fn landing_gate_failure_message(source_root: &Path, apps: &[WorkspaceAppMeta]) -> String {
    let workspace = load_workspace_config(source_root);
    let preferred = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut lines = vec![
        "host landing gate failed: no app has default-scope scope gate ready (L2+L3).".to_string(),
        format!("source-root: {}", source_root.display()),
    ];
    if let Some(preferred) = preferred {
        let canonical = resolve_app_id(source_root, preferred);
        lines.push(format!(
            "workspace.defaultApp `{preferred}` (resolved `{canonical}`) is missing default-scope artifacts."
        ));
    } else if apps.is_empty() {
        lines.push(format!(
            "no discoverable apps under `{}` (need a first-level app directory with `main.mei`)",
            source_root.display()
        ));
    } else {
        lines.push(
            "no landing app passed manifest probe; configure workspace.defaultApp or prebuild at least one app."
                .to_string(),
        );
    }
    lines.push(String::new());
    lines.push("Run prebuild before serve (host HTTP paths do not compile):".to_string());
    lines.push(format!("  {}/deploy/prebuild.sh", source_root.display()));
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
    lines.join("\n")
}

fn host_landing_gate_strict() -> bool {
    std::env::var("MEI_HOST_LANDING_GATE")
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("strict"))
        .unwrap_or(false)
}

/// 探测 landing 就绪情况；不阻塞 serve（除非 `MEI_HOST_LANDING_GATE=strict`）。
/// 只认 `workspace.defaultApp`（canonical resolve 后）的 landing gate，不因其它 app ready 误报 passed。
pub(crate) fn probe_landing_readiness(source_root: &Path) -> LandingProbeReport {
    let apps = match discover_apps(source_root) {
        Ok(apps) => apps,
        Err(error) => {
            tracing::warn!(%error, "host landing probe: failed to discover apps");
            return LandingProbeReport {
                ready_app_id: None,
                app_count: 0,
                configured_default_app: load_workspace_config(source_root)
                    .workspace
                    .default_app
                    .clone(),
                message: Some(error.to_string()),
            };
        }
    };
    let workspace = load_workspace_config(source_root);
    let preferred = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(preferred) = preferred.as_ref() {
        let canonical = resolve_app_id(source_root, preferred.as_str());
        if apps
            .iter()
            .any(|app| app.id == canonical || app.id == preferred.as_str())
        {
            if app_has_prebuilt_access_entry(source_root, canonical.as_str()) {
                tracing::info!(
                    app_id = %canonical,
                    default_app = preferred.as_str(),
                    "host landing probe passed: defaultApp scope gate ready (L2 navigation + L3 MCG assemble)"
                );
                return LandingProbeReport {
                    ready_app_id: Some(canonical),
                    app_count: apps.len(),
                    configured_default_app: Some(preferred.clone()),
                    message: None,
                };
            }
        }
    } else if let Some(app) = choose_default_app(source_root, &apps) {
        tracing::info!(
            app_id = %app.id,
            default_app = "(auto)",
            "host landing probe passed: auto-selected app scope gate ready (L2 navigation + L3 MCG assemble)"
        );
        return LandingProbeReport {
            ready_app_id: Some(app.id.clone()),
            app_count: apps.len(),
            configured_default_app: None,
            message: None,
        };
    }
    let message = landing_gate_failure_message(source_root, &apps);
    let app_summaries = apps
        .iter()
        .map(|app| {
            format!(
                "{}: {}",
                app.id,
                crate::readiness::scope_gate::format_landing_gate_summary(
                    &crate::readiness::scope_gate::resolve_default_app_access_gate(
                        source_root,
                        app.id.as_str(),
                    ),
                )
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    tracing::warn!(
        app_count = apps.len(),
        default_app = preferred.as_deref().unwrap_or("(none)"),
        app_gate_summary = %app_summaries,
        "host landing probe: no app landing-ready; host shell remains at /host"
    );
    LandingProbeReport {
        ready_app_id: None,
        app_count: apps.len(),
        configured_default_app: preferred,
        message: Some(message),
    }
}

/// `serve` 启动门禁：默认 warn-only；`MEI_HOST_LANDING_GATE=strict` 时 hard-fail。
pub(crate) fn prepare_landing_artifacts_for_serve(source_root: &Path) -> Result<()> {
    let probe = probe_landing_readiness(source_root);
    if probe.ready_app_id.is_some() {
        return Ok(());
    }
    if host_landing_gate_strict() {
        tracing::warn!(
            app_count = probe.app_count,
            default_app = probe.configured_default_app.as_deref().unwrap_or("(none)"),
            "host landing gate strict mode failed"
        );
        anyhow::bail!(probe.message.unwrap_or_else(|| {
            "host landing gate failed under MEI_HOST_LANDING_GATE=strict".to_string()
        }));
    }
    Ok(())
}
