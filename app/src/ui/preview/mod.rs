use std::collections::BTreeMap;

use leptos::prelude::*;
use mei_lang_kernel::{
    build_runtime_resource_index, build_runtime_resource_map, CompiledApp, LoadedResource,
    RuntimeResourceIndex,
};

use super::compile_status::{blocking_errors_for_preview, normalize_diagnostic_source};
use super::route::UiRouteMode;

mod nodes;
mod resolve;
mod style;
mod theme;
mod viewport;

pub(super) fn compiled_uses_frame_viewport(compiled: &CompiledApp) -> bool {
    compiled
        .scene_contract
        .as_ref()
        .and_then(|scene_contract| scene_contract.frame.as_ref())
        .and_then(|frame| viewport::frame_viewport_config(&frame.props))
        .is_some()
}

pub(super) struct PreviewRuntimeContext {
    pub resources: BTreeMap<String, LoadedResource>,
    pub index: RuntimeResourceIndex,
}

pub(super) fn build_preview_runtime_context(compiled: &CompiledApp) -> PreviewRuntimeContext {
    PreviewRuntimeContext {
        index: build_runtime_resource_index(compiled),
        resources: build_runtime_resource_map(compiled),
    }
}

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
            let frame_props = theme::deep_merge_value(&resolved_theme.frame, &frame.props);
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
                let content_max_width = content_bounds
                    .max_width
                    .unwrap_or(0.0)
                    .to_string();
                let content_height = if fluid_width {
                    "0".to_string()
                } else {
                    content_bounds.height.to_string()
                };
                let content_fluid_width = if fluid_width {
                    "true"
                } else {
                    "false"
                };
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
                    canvas_width, chrome_height.round() as i64, chrome_height_suffix, chrome_aspect
                );
                if is_manage && effective_canvas_width + 0.5 < vp.design_width {
                    viewport_style = viewport_style.replace("justify-items:center", "justify-items:start");
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
                <section class="preview-surface" style=viewport::frame_style(frame.layout.as_ref(), &frame_props, &resolved_theme)>
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
                let source = normalize_diagnostic_source(
                    &compiled.app_root,
                    diag.source_path.as_deref(),
                )
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::resolve::resolve_value;
    use super::style::{
        block_style, container_visual_style, container_visual_style_without_background,
        frame_backdrop_css_vars, frame_stage_content_bounds, frame_viewport_letterbox_style,
        has_frame_backdrop, normalize_background_image, panel_card_layout_style,
        panel_heading_config, panel_show_heading, panel_slot_typography_style, panel_style,
        surface_layout_style,
    };
    use super::theme::{
        resolve_panel_card_props, resolve_panel_head_props, resolve_panel_props, ThemeResolved,
    };
    use mei_lang_kernel::PanelDecl;
    use crate::ui::route::UiRouteMode;
    use super::viewport::{
        effective_viewport_overflow, effective_viewport_safe_inset,
        effective_canvas_width, frame_stage_content_bounds_for_viewport, frame_stage_style,
        frame_viewport_config,
        frame_viewport_style_for_route,
        viewport_overflow_is_debug,
    };
    use mei_lang_kernel::{
        build_runtime_resource_index, build_runtime_resource_map, ColumnSchema, CompiledApp,
        DatasetView, LayoutDecl, LoadedResource, MetricContract, MetricShape,
        SceneContract, SceneDecl, SourceDecl,
    };
    use serde_json::{json, Value};

    fn grid_layout() -> LayoutDecl {
        LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["1fr".to_string(), "2fr".to_string()]),
            rows: None,
            areas: Some(vec![vec!["doc".to_string(), "table".to_string()]]),
            gap: Some("16px".to_string()),
            padding: Some("20px".to_string()),
            align: None,
            justify: None,
        }
    }

    #[test]
    fn surface_layout_style_emits_grid_template_areas() {
        let layout = grid_layout();
        let style = surface_layout_style(Some(&layout));
        assert!(style.contains("grid-template-areas:'doc table';"));
    }

    #[test]
    fn panel_style_requires_named_grid_areas() {
        let mut layout = grid_layout();
        layout.areas = None;
        assert_eq!(panel_style(Some("doc"), Some(&layout), &json!({})), "");
    }

    #[test]
    fn panel_card_layout_style_applies_grid_columns() {
        let layout = grid_layout();
        let style = panel_card_layout_style(Some(&layout), &json!({}));
        assert!(style.contains("display:grid;"));
        assert!(style.contains("grid-template-columns:1fr 2fr;"));
    }

    #[test]
    fn panel_card_layout_style_emits_grid_align_items() {
        let mut layout = grid_layout();
        layout.align = Some("stretch".to_string());
        let style = panel_card_layout_style(Some(&layout), &json!({}));
        assert!(style.contains("align-items:stretch;"));
    }

    #[test]
    fn panel_card_layout_style_normalizes_bare_numeric_gap_to_px() {
        let mut layout = grid_layout();
        layout.gap = Some("5".to_string());
        let style = panel_card_layout_style(Some(&layout), &json!({}));
        assert!(style.contains("gap:5px;"));
    }

    #[test]
    fn block_style_uses_full_span_in_grid() {
        let layout = grid_layout();
        assert_eq!(
            block_style(Some("full"), Some(&layout)),
            "grid-column:1 / -1;"
        );
    }

    #[test]
    fn block_style_spans_head_row_across_columns() {
        let mut layout = grid_layout();
        layout.columns = Some(vec!["1fr".to_string(); 3]);
        layout.rows = Some(vec!["59px".to_string(), "102px".to_string()]);
        layout.areas = Some(vec![
            vec!["head".to_string(); 3],
            vec!["m0".to_string(), "m1".to_string(), "m2".to_string()],
        ]);
        let style = block_style(Some("head"), Some(&layout));
        assert!(style.contains("grid-area:head;"));
        assert!(style.contains("grid-column:1 / -1;"));
        assert!(style.contains("height:100%;"));
    }

    #[test]
    fn panel_style_merges_grid_area_and_visual_props() {
        let layout = grid_layout();
        let style = panel_style(
            Some("doc"),
            Some(&layout),
            &json!({
                "padding": "0",
                "border": "none",
                "background": {
                    "color": "#001122",
                }
            }),
        );
        assert!(style.contains("grid-area:doc;"));
        assert!(style.contains("padding:0;"));
        assert!(style.contains("border:none;"));
        assert!(style.contains("background-color:#001122;"));
    }

    #[test]
    fn container_visual_style_supports_background_image_shorthand() {
        let style = container_visual_style(&json!({
            "background": {
                "image": "/workspace-components/demo.png",
                "size": "cover",
                "repeat": "no-repeat",
            }
        }));
        assert!(style.contains("background-image:url(\"/workspace-components/demo.png\")"));
        assert!(style.contains("background-size:cover;"));
        assert!(style.contains("background-repeat:no-repeat;"));
    }

    #[test]
    fn normalize_background_image_none_is_not_wrapped_as_url() {
        assert_eq!(
            normalize_background_image("none"),
            "none".to_string()
        );
    }

    #[test]
    fn frame_viewport_letterbox_style_uses_background_color() {
        let style = frame_viewport_letterbox_style(&json!({
            "background": { "color": "rgb(29, 47, 65)" }
        }));
        assert!(style.contains("background:rgb(29, 47, 65);"));
    }

    #[test]
    fn frame_backdrop_css_vars_exports_layer_tokens_without_inline_background() {
        let props = json!({
            "background": {
                "color": "#182f42",
                "image": "linear-gradient(180deg, #1a3348, #0a1824)",
                "size": "100% 100%",
            }
        });
        let vars = frame_backdrop_css_vars(&props);
        assert!(vars.contains("--mei-frame-bg-color:#182f42;"));
        assert!(vars.contains("--mei-frame-bg-image:linear-gradient(180deg, #1a3348, #0a1824);"));
        assert!(vars.contains("--mei-frame-bg-size:100% 100%;"));
        assert!(has_frame_backdrop(&props));
        let stage = container_visual_style_without_background(&props);
        assert!(!stage.contains("background-color"));
        assert!(!stage.contains("background-image"));
    }

    #[test]
    fn panel_show_heading_uses_normalized_head_flag() {
        assert!(!panel_show_heading(&json!({"show_heading": false})));
        assert!(!panel_show_heading(&json!({"chrome": "bare"})));
        assert!(!panel_show_heading(&json!({})));
        assert!(panel_show_heading(&json!({"__mei_has_head": true})));
    }

    #[test]
    fn panel_heading_uses_theme_panel_head_and_head_props() {
        let theme_head = json!({"variant": "plain", "accent": false});
        let cfg = panel_heading_config(&theme_head, &json!({}), &json!({}));
        assert_eq!(cfg.variant, "plain");
        assert!(!cfg.show_flair);
        let cfg_screen = panel_heading_config(
            &theme_head,
            &json!({"variant": "screen", "flair": true}),
            &json!({}),
        );
        assert_eq!(cfg_screen.variant, "screen");
        assert!(cfg_screen.show_flair);
    }

    #[test]
    fn resolve_panel_card_props_strips_heading_from_card() {
        let theme = ThemeResolved {
            id: "page".to_string(),
            frame: json!({}),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            components: json!({}),
            css_vars: Vec::new(),
        };
        let panel = PanelDecl {
            kind: "panel".to_string(),
            id: "p".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![],
            props: json!({"heading": {"variant": "screen"}, "border": "1px solid red"}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
        };
        let card = resolve_panel_card_props(&theme, &panel);
        assert!(card.get("heading").is_none());
        assert_eq!(
            card.get("border").and_then(Value::as_str),
            Some("1px solid red")
        );
    }

    #[test]
    fn panel_slot_typography_style_maps_theme_font_keys() {
        assert_eq!(
            panel_slot_typography_style(&json!({"font": "4"})),
            "font-size:var(--mei-font-4,14px);"
        );
        assert_eq!(
            panel_slot_typography_style(&json!({"font": 3})),
            "font-size:var(--mei-font-3,14px);"
        );
        assert_eq!(
            panel_slot_typography_style(&json!({"font": "18px"})),
            "font-size:18px;"
        );
        assert!(panel_slot_typography_style(&json!({})).is_empty());
    }

    #[test]
    fn resolve_panel_head_props_merges_theme_and_panel() {
        let theme = ThemeResolved {
            id: "cockpit".to_string(),
            frame: json!({}),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({"variant": "plain"}),
            panel_body: json!({}),
            heading: json!({}),
            components: json!({}),
            css_vars: Vec::new(),
        };
        let panel = PanelDecl {
            kind: "panel".to_string(),
            id: "p".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![],
            props: json!({}),
            head_props: json!({"height": "54px"}),
            body_props: json!({}),
            base: None,
        };
        let head = resolve_panel_head_props(&theme, &panel);
        assert_eq!(head.get("variant").and_then(Value::as_str), Some("plain"));
        assert_eq!(head.get("height").and_then(Value::as_str), Some("54px"));
    }

    #[test]
    fn resolve_panel_props_merges_theme_panel_defaults() {
        let theme = ThemeResolved {
            id: "page".to_string(),
            frame: json!({}),
            panel: json!({
                "padding": "12px",
                "border": "1px solid #334155",
            }),
            panel_bare: json!({
                "padding": "0",
                "border": "none",
            }),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            components: json!({}),
            css_vars: Vec::new(),
        };
        let resolved = resolve_panel_props(
            &theme,
            &json!({
                "padding": "4px",
            }),
        );
        assert_eq!(resolved.get("padding").and_then(Value::as_str), Some("4px"));
        assert_eq!(
            resolved.get("border").and_then(Value::as_str),
            Some("1px solid #334155")
        );
    }

    #[test]
    fn resolve_panel_props_prefers_bare_theme_when_chrome_is_bare() {
        let theme = ThemeResolved {
            id: "cockpit".to_string(),
            frame: json!({}),
            panel: json!({
                "padding": "12px",
                "border": "1px solid #334155",
            }),
            panel_bare: json!({
                "padding": "0",
                "border": "none",
                "background": "transparent",
            }),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            components: json!({}),
            css_vars: Vec::new(),
        };
        let resolved = resolve_panel_props(
            &theme,
            &json!({
                "chrome": "bare",
                "padding": "2px",
            }),
        );
        assert_eq!(resolved.get("padding").and_then(Value::as_str), Some("2px"));
        assert_eq!(resolved.get("border").and_then(Value::as_str), Some("none"));
        assert_eq!(
            resolved.get("background").and_then(Value::as_str),
            Some("transparent")
        );
    }

    #[test]
    fn frame_viewport_config_supports_fluid_height() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1000,
                "design_height": 480,
                "fluid_height": true,
            }
        }))
        .expect("viewport config");
        assert!(vp.fluid_height);
        let locked = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1000,
                "design_height": 480,
                "lock_height": false,
            }
        }))
        .expect("viewport config");
        assert!(locked.fluid_height);
    }

    #[test]
    fn frame_viewport_config_supports_align_and_safe_inset() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1920,
                "design_height": 1080,
                "scale_mode": "contain",
                "align": "top-center",
                "safe_inset": {
                    "top": 12,
                    "right": 24,
                    "bottom": 16,
                    "left": 20,
                }
            }
        }))
        .expect("viewport config");
        assert_eq!(vp.align_x, "center");
        assert_eq!(vp.align_y, "start");
        assert_eq!(vp.safe_top, 12.0);
        assert_eq!(vp.safe_right, 24.0);
        assert_eq!(vp.safe_bottom, 16.0);
        assert_eq!(vp.safe_left, 20.0);
    }

    #[test]
    fn effective_viewport_overflow_is_fixed_by_route_not_frame_props() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1920,
                "design_height": 1080,
                "overflow": "scroll",
                "edit_overflow": "clip",
            }
        }))
        .expect("viewport config");
        assert_eq!(
            effective_viewport_overflow(&vp, UiRouteMode::Manage),
            "debug"
        );
        assert_eq!(
            effective_viewport_overflow(&vp, UiRouteMode::Access),
            "clip"
        );
    }

    #[test]
    fn frame_stage_style_debug_caps_canvas_width_to_frame_max_width() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1000,
                "design_height": 480,
                "fluid_height": true,
            }
        }))
        .expect("viewport config");
        let props = json!({
            "max_width": "972px",
            "width": "100%",
        });
        let theme = ThemeResolved {
            id: "cockpit".to_string(),
            frame: json!({}),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            components: json!({}),
            css_vars: Vec::new(),
        };
        let style = frame_stage_style(None, &props, &vp, &theme, "debug");
        assert!(style.contains("width:972px;"));
        assert!(!style.contains("width:1000px;"));
        assert_eq!(
            effective_canvas_width(&props, &vp),
            972.0
        );
    }

    #[test]
    fn frame_stage_style_debug_uses_full_canvas_without_css_scale() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1920,
                "design_height": 1080,
                "aspect_ratio": "16:9",
            }
        }))
        .expect("viewport config");
        let theme = ThemeResolved {
            id: "cockpit".to_string(),
            frame: json!({}),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            components: json!({}),
            css_vars: Vec::new(),
        };
        let style = frame_stage_style(None, &json!({}), &vp, &theme, "debug");
        assert!(style.contains("width:1920px;"));
        assert!(style.contains("min-height:1080px;"));
        assert!(style.contains("height:auto;"));
        assert!(style.contains("transform:none;"));
        let debug_style =
            frame_viewport_style_for_route(&vp, "debug", UiRouteMode::Manage);
        assert!(debug_style.contains("overflow-x:auto;"));
        assert!(viewport_overflow_is_debug("debug"));
        assert!(viewport_overflow_is_debug("scroll"));
    }

    #[test]
    fn frame_viewport_style_applies_alignment_and_padding() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1920,
                "design_height": 1080,
                "align_x": "left",
                "align_y": "top",
                "safe_padding": 18,
            }
        }))
        .expect("viewport config");
        let debug_style =
            frame_viewport_style_for_route(&vp, "debug", UiRouteMode::Manage);
        assert!(debug_style.contains("justify-items:start;"));
        assert!(debug_style.contains("align-items:start;"));
        assert!(debug_style.contains("padding:18px 18px 18px 18px;"));

        let access_style = frame_viewport_style_for_route(&vp, "clip", UiRouteMode::Access);
        assert!(access_style.contains("display:flex;"));
        assert!(access_style.contains("align-items:center;"));
        assert!(access_style.contains("justify-content:center;"));
        assert!(access_style.contains("padding:18px 18px 18px 18px;"));
    }

    #[test]
    fn effective_viewport_safe_inset_splits_access_and_edit() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1920,
                "design_height": 1080,
                "safe_inset": { "top": 0, "right": 0, "bottom": 0, "left": 0 },
                "edit_safe_inset": { "top": 32, "right": 16, "bottom": 12, "left": 8 },
            }
        }))
        .expect("viewport config");
        assert_eq!(
            effective_viewport_safe_inset(&vp, UiRouteMode::Access),
            (0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            effective_viewport_safe_inset(&vp, UiRouteMode::Manage),
            (32.0, 16.0, 12.0, 8.0)
        );
    }

    #[test]
    fn frame_stage_content_bounds_treats_max_width_as_cap() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1920,
                "design_height": 720,
            }
        }))
        .expect("viewport config");
        let props = json!({
            "width": "100%",
            "max_width": "520px",
        });
        let bounds = frame_stage_content_bounds(&props, vp.design_width, vp.design_height);
        assert_eq!(bounds.max_width, Some(520.0));
        assert_eq!(bounds.height, 720.0);
        assert_eq!(bounds.fallback_width, 1920.0);
        let viewport_bounds = frame_stage_content_bounds_for_viewport(&props, &vp);
        assert_eq!(viewport_bounds.max_width, Some(520.0));
    }

    #[test]
    fn frame_stage_style_uses_max_width_cap_not_fixed_canvas_width() {
        let vp = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1920,
                "design_height": 720,
            }
        }))
        .expect("viewport config");
        let props = json!({
            "max_width": "520px",
            "width": "100%",
        });
        let theme = ThemeResolved {
            id: "cockpit".to_string(),
            frame: json!({}),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            components: json!({}),
            css_vars: Vec::new(),
        };
        let style = frame_stage_style(None, &props, &vp, &theme, "clip");
        assert!(style.contains("--mei-frame-content-max-width:520px;"));
        assert!(style.contains("width:100%;"));
        assert!(style.contains("height:auto;"));
        assert!(style.contains("transform:none;"));
        assert!(!style.contains("width:1920px;"));
    }

    #[test]
    fn resolve_value_supports_data_and_metric_refs() {
        let scene_contract = SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                world: None,
                flow: None,
                frame: None,
                profile: None,
                theme: None,
                summary: None,
                goal: None,
                state: json!({}),
                access_export: true,
            },
            themes: vec![],
            world: None,
            flow: None,
            frame: None,
            panels: vec![],
        };
        let mut resources = BTreeMap::new();
        resources.insert(
            "sales_metrics".to_string(),
            LoadedResource {
                id: "sales_metrics".to_string(),
                kind: "dataset".to_string(),
                title: Some("Sales".to_string()),
                document: None,
                dataset: Some(DatasetView {
                    id: "sales_metrics".to_string(),
                    title: Some("Sales".to_string()),
                    purpose: None,
                    schema: vec![
                        ColumnSchema {
                            name: "label".to_string(),
                            type_name: "string".to_string(),
                            source: None,
                            optional: false,
                            unit: None,
                        },
                        ColumnSchema {
                            name: "value".to_string(),
                            type_name: "number".to_string(),
                            source: None,
                            optional: false,
                            unit: Some("元".to_string()),
                        },
                    ],
                    stage_schema: Vec::new(),
                    columns: vec!["label".to_string(), "value".to_string()],
                    rows: vec![json!({"label":"A","value":"100"})],
                    source: SourceDecl {
                        kind: "derived".to_string(),
                        path: "dataset_view:sales_metrics".to_string(),
                        sheet: None,
                        header_row: None,
                        preview_rows: None,
                        page_size: None,
                        max_page_size: None,
                        table: None,
                        query: None,
                        connection: None,
                        content: None,
                    },
                    sources: Vec::new(),
                    metrics: BTreeMap::from([(
                        "sales_total".to_string(),
                        MetricContract {
                            id: "sales_total".to_string(),
                            label: Some("销售总额".to_string()),
                            unit: Some("元".to_string()),
                            purpose: None,
                            shape: MetricShape::Scalar,
                            schema: vec![ColumnSchema {
                                name: "total_value".to_string(),
                                type_name: "number".to_string(),
                                source: None,
                                optional: false,
                                unit: Some("元".to_string()),
                            }],
                            dataset: None,
                            transforms: Vec::new(),
                            value: json!({"total_value": 100}),
                        },
                    )]),
                    runtime_metric_defs: BTreeMap::new(),
                }),
            },
        );

        let compiled = CompiledApp {
            app_id: "preview-test".to_string(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            resources: resources.values().cloned().collect(),
            scene_routes: Vec::new(),
            app_root: ".".to_string(),
            title: "preview-test".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
        };
        let resource_index = build_runtime_resource_index(&compiled);
        let scene_anchor = super::resolve::RuntimeSceneAnchor {
            scene_id: "home".to_string(),
            scene_path: Some("scenes/home.mei".to_string()),
        };

        let data_ref = json!({"__ref":"data","id":"sales_metrics"});
        let resolved_data = resolve_value(
            &data_ref,
            &scene_contract,
            &resources,
            &scene_anchor,
            &resource_index,
            &compiled,
        );
        assert_eq!(
            resolved_data.get("id").and_then(|value| value.as_str()),
            Some("sales_metrics")
        );
        assert_eq!(
            resolved_data
                .get("__mei_runtime_ref")
                .and_then(|value| value.get("dataset_id"))
                .and_then(|value| value.as_str()),
            Some("sales_metrics")
        );

        let metric_ref =
            json!({"__ref":"metric","id":"sales_total","from_dataset":"sales_metrics"});
        let resolved_metric = resolve_value(
            &metric_ref,
            &scene_contract,
            &resources,
            &scene_anchor,
            &resource_index,
            &compiled,
        );
        assert_eq!(
            resolved_metric.get("id").and_then(|value| value.as_str()),
            Some("sales_total")
        );
        assert_eq!(
            resolved_metric
                .get("__mei_runtime_ref")
                .and_then(|value| value.get("metric_id"))
                .and_then(|value| value.as_str()),
            Some("sales_total")
        );

        let dataset_ref = json!({"__ref": "dataset", "id": "sales_metrics"});
        let resolved_dataset = resolve_value(
            &dataset_ref,
            &scene_contract,
            &resources,
            &scene_anchor,
            &resource_index,
            &compiled,
        );
        assert_eq!(
            resolved_dataset.get("id").and_then(|value| value.as_str()),
            Some("sales_metrics")
        );
        assert!(resolved_dataset.get("rows").is_some());
        assert_eq!(
            resolved_dataset
                .get("__mei_runtime_ref")
                .and_then(|value| value.get("kind"))
                .and_then(|value| value.as_str()),
            Some("data")
        );
        assert_eq!(
            resolved_dataset
                .get("__mei_runtime_ref")
                .and_then(|value| value.get("dataset_id"))
                .and_then(|value| value.as_str()),
            Some("sales_metrics")
        );
    }

    #[test]
    fn resolve_value_route_target_alias_matches_canonical_dataset_id() {
        use mei_lang_kernel::{CompiledSceneRoute, MetricContract, MetricShape, SceneDecl};

        let scene_contract = SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                world: None,
                flow: None,
                frame: None,
                profile: None,
                theme: None,
                summary: None,
                goal: None,
                state: json!({}),
                access_export: true,
            },
            themes: vec![],
            world: None,
            flow: None,
            frame: None,
            panels: vec![],
        };
        let mut resources = BTreeMap::new();
        resources.insert(
            "home".to_string(),
            LoadedResource {
                id: "home".to_string(),
                kind: "dataset".to_string(),
                title: None,
                document: None,
                dataset: Some(DatasetView {
                    id: "home".to_string(),
                    title: None,
                    purpose: None,
                    schema: Vec::new(),
                    stage_schema: Vec::new(),
                    columns: vec!["value".to_string()],
                    rows: vec![json!({"value": 1})],
                    source: SourceDecl {
                        kind: "derived".to_string(),
                        path: "dataset_view:home".to_string(),
                        sheet: None,
                        header_row: None,
                        preview_rows: None,
                        page_size: None,
                        max_page_size: None,
                        table: None,
                        query: None,
                        connection: None,
                        content: None,
                    },
                    sources: Vec::new(),
                    metrics: BTreeMap::from([(
                        "sales_total".to_string(),
                        MetricContract {
                            id: "sales_total".to_string(),
                            label: None,
                            unit: None,
                            purpose: None,
                            shape: MetricShape::Scalar,
                            schema: Vec::new(),
                            dataset: None,
                            transforms: Vec::new(),
                            value: json!({"value": 1}),
                        },
                    )]),
                    runtime_metric_defs: Default::default(),
                }),
            },
        );
        let compiled = CompiledApp {
            app_id: "preview-alias".to_string(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            resources: resources.values().cloned().collect(),
            scene_routes: vec![CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "scenes/home.mei".to_string(),
                kind: "file_ref".to_string(),
                title: None,
                is_default: true,
                access_export: true,
            }],
            app_root: ".".to_string(),
            title: "preview-alias".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
        };
        let resource_index = build_runtime_resource_index(&compiled);
        let scene_anchor = super::resolve::RuntimeSceneAnchor {
            scene_id: "home".to_string(),
            scene_path: Some("scenes/home.mei".to_string()),
        };
        let metric_ref = json!({
            "__ref": "metric",
            "id": "sales_total",
            "from_dataset": "scenes/home.mei"
        });
        let resolved = resolve_value(
            &metric_ref,
            &scene_contract,
            &build_runtime_resource_map(&compiled),
            &scene_anchor,
            &resource_index,
            &compiled,
        );
        assert_eq!(
            resolved
                .get("__mei_runtime_ref")
                .and_then(|value| value.get("dataset_id"))
                .and_then(|value| value.as_str()),
            Some("home")
        );
    }
}
