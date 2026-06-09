use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::compile_status::{
    compile_status_counts_for_display, compile_status_counts_for_target, compile_status_summary,
    compile_status_title, compile_status_tone,
};
use super::manage_routing::{is_ops_config_target, manage_tab_href, ManageViewTab};
use super::SourcePanelMeta;
pub(crate) fn statusbar_view(
    app_path: &str,
    app_title: &str,
    route_mode: &'static str,
    current_target: &str,
    source_meta: Option<&SourcePanelMeta>,
    compiled: Option<&CompiledApp>,
    runtime_enabled: bool,
    show_compile_center: bool,
) -> AnyView {
    let app_summary = format!("应用 {app_title}");
    let app_summary_title = format!("应用：{app_path}");
    let route_mode_label = match route_mode {
        "build" | "manage" => "构建",
        "app" | "access" => "访问",
        "presentation" | "slides" => "演示",
        "config" => "配置",
        "upload" => "上传",
        _ => route_mode,
    };
    let file_label = if is_ops_config_target(current_target) {
        ".mei-config.json"
    } else {
        current_target
            .rsplit('/')
            .next()
            .filter(|value| !value.is_empty())
            .unwrap_or(current_target)
    };
    let file_summary = if let Some(meta) = source_meta {
        format!("文件 {file_label} · {}行", meta.line_count)
    } else {
        format!("文件 {file_label}")
    };
    let file_summary_title = if let Some(meta) = source_meta {
        format!(
            "当前文件：{} · {}行 · {}字",
            current_target, meta.line_count, meta.char_count
        )
    } else {
        format!("当前文件：{current_target}")
    };
    let (errors, warnings, infos) = compiled
        .map(|compiled| compile_status_counts_for_display(compiled, current_target))
        .unwrap_or((0, 0, 0));
    let error_tone = if errors > 0 { "danger" } else { "neutral" };
    let warning_tone = if warnings > 0 { "warn" } else { "neutral" };
    let info_tone = if infos > 0 { "info" } else { "neutral" };
    let compile_summary = compiled
        .map(|compiled| compile_status_summary(compiled, current_target))
        .unwrap_or_else(|| "未触发编译".to_string());
    let compile_summary_title = compiled
        .map(|compiled| compile_status_title(compiled, current_target))
        .unwrap_or_else(|| "当前页面未依赖编译结果".to_string());
    let compile_tone = compiled
        .map(|compiled| compile_status_tone(compiled, current_target))
        .unwrap_or("neutral");
    let (cur_errors, _, _) = compiled
        .map(|compiled| compile_status_counts_for_target(compiled, current_target))
        .unwrap_or((0, 0, 0));
    let diagnostics_tab_href =
        if show_compile_center && compiled.is_some() && (errors > 0 || warnings > 0) {
            Some(manage_tab_href(
                app_path,
                Some(current_target),
                current_target,
                current_target.ends_with(".mei"),
                ManageViewTab::Diagnostics,
                None,
            ))
        } else {
            None
        };
    let model_service_summary = if runtime_enabled {
        "模型服务检测中"
    } else {
        "模型服务 --"
    };
    view! {
        <footer class="statusbar statusbar-shell chrome-inset chrome-safe-x sticky bottom-0 z-10 py-1.5 backdrop-blur-md">
            <div class="statusbar-layout min-w-0 text-[10px]">
                <div class="statusbar-track statusbar-track-left min-w-0">
                    <span class="status-chip status-chip-app max-w-[18vw]" title=app_summary_title>{app_summary}</span>
                    <span class="status-chip status-chip-file max-w-[26vw]" title=file_summary_title>{file_summary}</span>
                    <span class="status-chip status-chip-mode" data-tone="info">{route_mode_label}</span>
                </div>
                <div class="statusbar-track statusbar-track-center min-w-0" aria-hidden=(!show_compile_center).then_some("true")>
                    {if show_compile_center {
                        view! {
                            <>
                                <span class="status-chip status-chip-compile" data-tone=compile_tone title=compile_summary_title.clone()>{compile_summary}</span>
                                {diagnostics_tab_href
                                    .map(|href| {
                                        view! {
                                            <a
                                                class="status-chip status-chip-diagnostic status-chip-link"
                                                data-tone=error_tone
                                                href=href
                                                title=format!("当前文件 {current_target}：{cur_errors} 个错误；点击查看调试页")
                                            >
                                                {format!("Error {} (文件 {})", errors, cur_errors)}
                                            </a>
                                        }
                                        .into_any()
                                    })
                                    .unwrap_or_else(|| {
                                        view! {
                                            <span class="status-chip status-chip-diagnostic" data-tone=error_tone title=compile_summary_title>
                                                {format!("Error {}", errors)}
                                            </span>
                                        }
                                        .into_any()
                                    })}
                                <span class="status-chip status-chip-diagnostic" data-tone=warning_tone>{format!("Warning {}", warnings)}</span>
                                <span class="status-chip status-chip-diagnostic" data-tone=info_tone>{format!("Info {}", infos)}</span>
                            </>
                        }
                            .into_any()
                    } else {
                        view! { <></> }.into_any()
                    }}
                </div>
                <div class="statusbar-track statusbar-track-right min-w-0">
                    <span class="status-chip status-chip-compliance max-w-[280px]" id="mei-status-compliance" data-tone="neutral" hidden></span>
                    <span class="status-chip status-chip-host max-w-[220px]" id="mei-status-host-version" data-tone="neutral">"Mei --"</span>
                    <span class="status-chip status-chip-runtime max-w-[300px]" id="mei-status-model-service" data-tone="neutral">{model_service_summary}</span>
                </div>
            </div>
        </footer>
    }
    .into_any()
}
