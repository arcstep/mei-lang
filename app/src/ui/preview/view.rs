use leptos::prelude::*;

use super::{build_preview_runtime_context, nodes, style, theme, viewport};
use crate::ui::compile_status::{blocking_errors_for_preview, normalize_diagnostic_source};
use crate::ui::route::UiRouteMode;
use mei_lang_kernel::CompiledApp;

pub(super) fn preview_view(
    compiled: &CompiledApp,
    app_path: &str,
    selected_target: &str,
    route_mode: UiRouteMode,
) -> AnyView {
    let runtime_ctx = build_preview_runtime_context(compiled);

    let preview_scene_path = {
        let selected = selected_target.trim();
        if !selected.is_empty() {
            selected.to_string()
        } else {
            compiled.active_target_file.clone()
        }
    };
    if let Some(scene_contract) = &compiled.scene_contract {
        let resolved_theme = theme::resolve_theme(scene_contract);
        if let Some(frame) = &scene_contract.frame {
            let frame_props = theme::resolve_shared_refs(
                &theme::deep_merge_value(&resolved_theme.frame, &frame.props),
                &resolved_theme.shared,
            );
            let panels = scene_contract
                .panels
                .iter()
                .map(|panel| {
                    nodes::panel_view(
                        panel,
                        frame.layout.as_ref(),
                        compiled,
                        app_path,
                        scene_contract,
                        &runtime_ctx,
                        &resolved_theme,
                        0,
                        preview_scene_path.as_str(),
                    )
                })
                .collect_view();
            if let Some(vp) = viewport::frame_viewport_config(&frame_props) {
                let overflow_mode = viewport::effective_viewport_overflow(&vp, route_mode);
                let is_manage = route_mode == UiRouteMode::Manage;
                let content_bounds =
                    viewport::frame_stage_content_bounds_for_viewport(&frame_props, &vp);
                let fluid_height = vp.fluid_height;
                let fluid_width = content_bounds.max_width.is_some() && !fluid_height;
                let mut viewport_style = if fluid_width {
                    viewport::frame_viewport_style_fluid_width_for_route(
                        &vp,
                        overflow_mode.as_str(),
                        route_mode,
                    )
                } else {
                    viewport::frame_viewport_style_for_route(
                        &vp,
                        overflow_mode.as_str(),
                        route_mode,
                    )
                };
                viewport_style.push_str(&style::frame_viewport_letterbox_style(&frame_props));
                let content_max_width = content_bounds.max_width.unwrap_or(0.0).to_string();
                let content_height = if fluid_width {
                    "0".to_string()
                } else {
                    content_bounds.height.to_string()
                };
                let content_fluid_width = if fluid_width { "true" } else { "false" };
                let content_fluid_height = if fluid_height { "true" } else { "false" };
                let viewport_class = if is_manage {
                    if fluid_width {
                        "preview-viewport preview-viewport-edit-debug preview-viewport-fluid-width"
                    } else if fluid_height {
                        "preview-viewport preview-viewport-edit-debug preview-viewport-fluid-height"
                    } else {
                        "preview-viewport preview-viewport-edit-debug"
                    }
                } else if fluid_width {
                    "preview-viewport preview-viewport-access-clip preview-viewport-fluid-width"
                } else {
                    "preview-viewport preview-viewport-access-clip"
                };
                let stage_class = if style::has_frame_backdrop(&frame_props) {
                    "preview-surface preview-stage preview-stage-has-backdrop"
                } else {
                    "preview-surface preview-stage"
                };
                let show_viewport_chrome = is_manage;
                let chrome_height = if fluid_height {
                    content_bounds.height
                } else {
                    vp.design_height
                };
                let chrome_height_suffix = if fluid_height { " (内容高)" } else { "" };
                let chrome_aspect = vp
                    .aspect_ratio
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
                    .map(|value| format!(" · {value}"))
                    .unwrap_or_default();
                let effective_canvas_width = viewport::effective_canvas_width(&frame_props, &vp);
                let canvas_width = effective_canvas_width.round() as i64;
                let canvas_width_attr = effective_canvas_width.to_string();
                let chrome_label = format!(
                    "{} × {}{}{}",
                    canvas_width,
                    chrome_height.round() as i64,
                    chrome_height_suffix,
                    chrome_aspect
                );
                if is_manage && effective_canvas_width + 0.5 < vp.design_width {
                    viewport_style =
                        viewport_style.replace("justify-items:center", "justify-items:start");
                }
                return view! {
                    <section
                        class=viewport_class
                        style=viewport_style
                        data-mei-frame-viewport="true"
                        data-content-fluid-width=content_fluid_width
                        data-content-fluid-height=content_fluid_height
                        data-design-width=vp.design_width.to_string()
                        data-canvas-width=canvas_width_attr
                        data-design-height=vp.design_height.to_string()
                        data-content-max-width=content_max_width
                        data-content-height=content_height
                        data-scale-mode=vp.scale_mode.clone()
                        data-safe-top=vp.safe_top.to_string()
                        data-safe-right=vp.safe_right.to_string()
                        data-safe-bottom=vp.safe_bottom.to_string()
                        data-safe-left=vp.safe_left.to_string()
                        data-edit-safe-top=vp.edit_safe_top.to_string()
                        data-edit-safe-right=vp.edit_safe_right.to_string()
                        data-edit-safe-bottom=vp.edit_safe_bottom.to_string()
                        data-edit-safe-left=vp.edit_safe_left.to_string()
                        data-source-path=selected_target.to_string()
                        data-target-file=selected_target.to_string()
                        data-scene-id=scene_contract.scene.id.clone()
                        data-route-mode=route_mode.slug()
                        data-overflow-mode=overflow_mode.clone()
                        data-show-design-bounds="true"
                        data-aspect-ratio=vp.aspect_ratio.clone().unwrap_or_else(|| "16:9".to_string())
                    >
                        {show_viewport_chrome.then(|| view! {
                            <div class="preview-viewport-toolbar">
                                <div class="preview-viewport-zoom-bar" data-preview-zoom-bar="true">
                                    <span class="preview-viewport-zoom-title">"视窗"</span>
                                    <button type="button" class="preview-viewport-zoom-btn is-active" data-preview-zoom="fit">"自适应"</button>
                                    <button type="button" class="preview-viewport-zoom-btn" data-preview-zoom="1">"100%"</button>
                                    <button type="button" class="preview-viewport-zoom-btn" data-preview-zoom="0.75">"75%"</button>
                                    <button type="button" class="preview-viewport-zoom-btn" data-preview-zoom="0.5">"50%"</button>
                                    <button type="button" class="preview-viewport-zoom-btn" data-preview-zoom="minus" title="缩小">"−"</button>
                                    <button type="button" class="preview-viewport-zoom-btn" data-preview-zoom="plus" title="放大">"+"</button>
                                    <span class="preview-viewport-zoom-readout" data-preview-zoom-readout="true">"—"</span>
                                </div>
                                <div class="preview-viewport-chrome" aria-hidden="true">
                                    {chrome_label.clone()}
                                </div>
                            </div>
                        })}
                        <div class="preview-stage-shell">
                            <section class=stage_class style=viewport::frame_stage_style(frame.layout.as_ref(), &frame_props, &vp, &resolved_theme, overflow_mode.as_str())>
                                {panels}
                            </section>
                        </div>
                    </section>
                }
                .into_any();
            }
            return view! {
                <section
                    class="preview-surface"
                    style=viewport::frame_style(frame.layout.as_ref(), &frame_props, &resolved_theme)
                    data-mei-layout-audit-root="true"
                    data-source-path=selected_target.to_string()
                    data-target-file=selected_target.to_string()
                    data-scene-id=scene_contract.scene.id.clone()
                    data-route-mode=route_mode.slug()
                >
                    {panels}
                </section>
            }
            .into_any();
        }

        return view! {
            <section class="scene-placeholder rounded-[14px] border border-blue-500/20 bg-slate-950/35 p-4">
                <h3 class="mb-2 text-base font-semibold text-slate-100">{scene_contract.scene.id.clone()}</h3>
                <p class="text-slate-300">{scene_contract.scene.summary.clone().unwrap_or_else(|| "已生成 scene contract，运行态将在后续阶段接入。".to_string())}</p>
                <ul class="mt-3 list-disc pl-[18px] text-slate-400">
                    <li>{format!("观察面区块：{}", scene_contract.panels.len())}</li>
                    <li>{format!("目标：{}", scene_contract.scene.goal.clone().unwrap_or_else(|| "未声明".to_string()))}</li>
                </ul>
            </section>
        }
        .into_any();
    }

    let blocking_errors = blocking_errors_for_preview(compiled, selected_target, 3);
    if !blocking_errors.is_empty() {
        let error_items = blocking_errors
            .into_iter()
            .map(|diag| {
                let source =
                    normalize_diagnostic_source(&compiled.app_root, diag.source_path.as_deref())
                        .map(|path| format!(" · {path}"))
                        .unwrap_or_default();
                view! {
                    <li class="rounded-xl border border-red-400/25 bg-red-950/30 px-3 py-2">
                        <div class="text-xs font-semibold uppercase tracking-[0.02em] text-red-200">
                            {diag.code.clone()}
                        </div>
                        <div class="mt-1 text-sm leading-6 text-slate-200">
                            {diag.message.clone()}
                        </div>
                        <div class="mt-1 text-[11px] text-slate-400">
                            {source}
                        </div>
                    </li>
                }
            })
            .collect_view();
        return view! {
            <section class="scene-placeholder rounded-[14px] border border-red-500/30 bg-slate-950/35 p-4">
                <h3 class="mb-2 text-base font-semibold text-slate-100">"编译失败，预览已降级"</h3>
                <p class="text-slate-300">
                    "当前入口未能生成可渲染的 scene/frame。你仍可继续查看源码、错误诊断，并切换到其他文件或应用。"
                </p>
                <ul class="mt-3 grid gap-2 pl-0">{error_items}</ul>
            </section>
        }
        .into_any();
    }

    view! { <div class="empty-preview rounded-[14px] border border-blue-500/20 bg-slate-950/35 p-4 text-slate-300">"当前入口还没有可渲染的 frame 或 scene。"</div> }.into_any()
}
