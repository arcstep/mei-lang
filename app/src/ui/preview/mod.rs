use std::collections::BTreeMap;

use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, LoadedResource, Severity};

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

pub(super) fn build_resource_map(compiled: &CompiledApp) -> BTreeMap<String, LoadedResource> {
    let mut resource_map = compiled
        .resources
        .iter()
        .map(|resource| (resource.id.clone(), resource.clone()))
        .collect::<BTreeMap<_, _>>();
    // 允许 metric_ref(from_dataset="data/dataset/.../*.mei")：按 scene 路由 target_file 别名到 world 资源 id。
    for route in &compiled.scene_routes {
        let Some(resource) = compiled
            .resources
            .iter()
            .find(|resource| resource.id == route.scene_id)
            .cloned()
        else {
            continue;
        };
        let target = route.target_file.trim();
        if target.is_empty() {
            continue;
        }
        resource_map.insert(target.to_string(), resource.clone());
        let normalized = target.trim_start_matches("./");
        if normalized != target {
            resource_map.insert(normalized.to_string(), resource);
        }
    }
    resource_map
}

pub(super) fn preview_view(compiled: &CompiledApp, app_path: &str) -> AnyView {
    let resource_map = build_resource_map(compiled);

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
                        &resource_map,
                        &resolved_theme,
                    )
                })
                .collect_view();
            if let Some(vp) = viewport::frame_viewport_config(&frame_props) {
                return view! {
                    <section
                        class="preview-viewport"
                        style=viewport::frame_viewport_style(&vp)
                        data-mei-frame-viewport="true"
                        data-design-width=vp.design_width.to_string()
                        data-design-height=vp.design_height.to_string()
                        data-scale-mode=vp.scale_mode.clone()
                        data-safe-top=vp.safe_top.to_string()
                        data-safe-right=vp.safe_right.to_string()
                        data-safe-bottom=vp.safe_bottom.to_string()
                        data-safe-left=vp.safe_left.to_string()
                    >
                        <div class="preview-stage-shell">
                            <section class="preview-surface preview-stage" style=viewport::frame_stage_style(frame.layout.as_ref(), &frame_props, &vp, &resolved_theme)>
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

    let blocking_errors = compiled
        .diagnostics
        .iter()
        .filter(|diag| matches!(diag.severity, Severity::Error))
        .cloned()
        .collect::<Vec<_>>();
    if !blocking_errors.is_empty() {
        let error_items = blocking_errors
            .into_iter()
            .take(3)
            .map(|diag| {
                let source = diag
                    .source_path
                    .map(|path| format!(" · {}", path))
                    .unwrap_or_default();
                view! {
                    <li class="rounded-xl border border-red-400/25 bg-red-950/30 px-3 py-2">
                        <div class="text-xs font-semibold uppercase tracking-[0.02em] text-red-200">
                            {diag.code}
                        </div>
                        <div class="mt-1 text-sm leading-6 text-slate-200">
                            {diag.message}
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
        block_style, container_visual_style, panel_body_style, panel_show_heading, panel_style,
        surface_layout_style,
    };
    use super::theme::{resolve_panel_props, ThemeResolved};
    use super::viewport::{frame_viewport_config, frame_viewport_style};
    use mei_lang_kernel::{
        ColumnSchema, DatasetView, LayoutDecl, LoadedResource, MetricContract, MetricShape,
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
    fn panel_body_style_applies_grid_columns() {
        let layout = grid_layout();
        let style = panel_body_style(Some(&layout));
        assert!(style.contains("display:grid;"));
        assert!(style.contains("grid-template-columns:1fr 2fr;"));
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
    fn panel_show_heading_supports_bare_chrome() {
        assert!(!panel_show_heading(&json!({"show_heading": false})));
        assert!(!panel_show_heading(&json!({"chrome": "bare"})));
        assert!(panel_show_heading(&json!({})));
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
        let style = frame_viewport_style(&vp);
        assert!(style.contains("justify-items:start;"));
        assert!(style.contains("align-items:start;"));
        assert!(style.contains("padding:18px 18px 18px 18px;"));
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

        let scene_anchor = super::resolve::RuntimeSceneAnchor {
            scene_id: "home".to_string(),
            scene_path: Some("scenes/home.mei".to_string()),
        };

        let data_ref = json!({"__ref":"data","id":"sales_metrics"});
        let resolved_data = resolve_value(&data_ref, &scene_contract, &resources, &scene_anchor);
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
        let resolved_metric =
            resolve_value(&metric_ref, &scene_contract, &resources, &scene_anchor);
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

        let world_ref = json!({"__ref": "world", "id": "sales_metrics"});
        let resolved_world = resolve_value(&world_ref, &scene_contract, &resources, &scene_anchor);
        assert_eq!(
            resolved_world.get("id").and_then(|value| value.as_str()),
            Some("sales_metrics")
        );
        assert!(resolved_world.get("rows").is_some());
        assert_eq!(
            resolved_world
                .get("__mei_runtime_ref")
                .and_then(|value| value.get("kind"))
                .and_then(|value| value.as_str()),
            Some("data")
        );
        assert_eq!(
            resolved_world
                .get("__mei_runtime_ref")
                .and_then(|value| value.get("dataset_id"))
                .and_then(|value| value.as_str()),
            Some("sales_metrics")
        );
    }
}
