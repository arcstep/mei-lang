use std::collections::BTreeMap;

use leptos::prelude::*;
use mei_lang_kernel::{
    BlockDecl, CompiledApp, LoadedResource, SceneContract, Severity, ThemeDecl, UiNodeDecl,
};
use serde_json::Value;

#[derive(Debug, Clone)]
struct FrameViewportConfig {
    design_width: f64,
    design_height: f64,
    scale_mode: String,
    align_x: String,
    align_y: String,
    safe_top: f64,
    safe_right: f64,
    safe_bottom: f64,
    safe_left: f64,
}

#[derive(Debug, Clone)]
struct PanelHeadingConfig {
    variant: String,
    subtitle: Option<String>,
    show_accent: bool,
    show_flair: bool,
    show_dots: bool,
}

#[derive(Debug, Clone)]
struct ThemeResolved {
    id: String,
    frame: Value,
    panel: Value,
    panel_bare: Value,
    heading: Value,
    css_vars: Vec<(String, String)>,
}

pub(super) fn compiled_uses_frame_viewport(compiled: &CompiledApp) -> bool {
    compiled
        .scene_contract
        .as_ref()
        .and_then(|scene_contract| scene_contract.frame.as_ref())
        .and_then(|frame| frame_viewport_config(&frame.props))
        .is_some()
}

pub(super) fn preview_view(compiled: &CompiledApp, app_path: &str) -> AnyView {
    let resource_map = compiled
        .resources
        .iter()
        .map(|resource| (resource.id.clone(), resource.clone()))
        .collect::<BTreeMap<_, _>>();

    if let Some(scene_contract) = &compiled.scene_contract {
        let theme = resolve_theme(scene_contract);
        if let Some(frame) = &scene_contract.frame {
            let frame_props = deep_merge_value(&theme.frame, &frame.props);
            let panels = scene_contract
                .panels
                .iter()
                .map(|panel| {
                    panel_view(
                        panel,
                        frame.layout.as_ref(),
                        compiled,
                        app_path,
                        scene_contract,
                        &resource_map,
                        &theme,
                    )
                })
                .collect_view();
            if let Some(viewport) = frame_viewport_config(&frame_props) {
                return view! {
                    <section
                        class="preview-viewport"
                        style=frame_viewport_style(&viewport)
                        data-mei-frame-viewport="true"
                        data-design-width=viewport.design_width.to_string()
                        data-design-height=viewport.design_height.to_string()
                        data-scale-mode=viewport.scale_mode.clone()
                        data-safe-top=viewport.safe_top.to_string()
                        data-safe-right=viewport.safe_right.to_string()
                        data-safe-bottom=viewport.safe_bottom.to_string()
                        data-safe-left=viewport.safe_left.to_string()
                    >
                        <div class="preview-stage-shell">
                            <section class="preview-surface preview-stage" style=frame_stage_style(frame.layout.as_ref(), &frame_props, &viewport, &theme)>
                                {panels}
                            </section>
                        </div>
                    </section>
                }
                .into_any();
            }
            return view! {
                <section class="preview-surface" style=frame_style(frame.layout.as_ref(), &frame_props, &theme)>
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

fn panel_view(
    panel: &mei_lang_kernel::PanelDecl,
    frame_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
    theme: &ThemeResolved,
) -> AnyView {
    let panel_props = resolve_panel_props(theme, &panel.props);
    let blocks = panel
        .blocks
        .iter()
        .map(|node| {
            node_view(
                node,
                panel.layout.as_ref(),
                compiled,
                app_path,
                scene_contract,
                resources,
                theme,
            )
        })
        .collect_view();
    let title = panel.title.clone().unwrap_or_else(|| panel.id.clone());
    let show_heading = panel_show_heading(&panel_props);
    let heading = panel_heading_config(&theme.heading, &panel_props);
    let heading_class = format!("panel-heading panel-heading-{}", heading.variant);
    view! {
        <section class="preview-card" style=panel_style(panel.area.as_deref(), frame_layout, &panel_props)>
            {if show_heading {
                view! {
                    <div
                        class=heading_class
                        data-heading-variant=heading.variant.clone()
                    >
                        {if heading.show_accent {
                            view! { <div class="panel-heading-accent" aria-hidden="true"></div> }.into_any()
                        } else {
                            view! { <></> }.into_any()
                        }}
                        {if heading.show_flair {
                            view! { <div class="panel-heading-flair panel-heading-flair-left" aria-hidden="true"></div> }.into_any()
                        } else {
                            view! { <></> }.into_any()
                        }}
                        <div class="panel-heading-copy">
                            <h3>{title}</h3>
                            {if let Some(subtitle) = heading.subtitle.clone() {
                                view! { <p>{subtitle}</p> }.into_any()
                            } else {
                                view! { <></> }.into_any()
                            }}
                        </div>
                        {if heading.show_flair {
                            view! { <div class="panel-heading-flair panel-heading-flair-right" aria-hidden="true"></div> }.into_any()
                        } else {
                            view! { <></> }.into_any()
                        }}
                        {if heading.show_dots {
                            view! {
                                <div class="panel-heading-dots" aria-hidden="true">
                                    <span></span><span></span><span></span>
                                </div>
                            }.into_any()
                        } else {
                            view! { <></> }.into_any()
                        }}
                    </div>
                }.into_any()
            } else {
                view! { <></> }.into_any()
            }}
            <div class="grid min-w-0 gap-3" style=panel_body_style(panel.layout.as_ref())>
                {blocks}
            </div>
        </section>
    }
    .into_any()
}

fn node_view(
    node: &UiNodeDecl,
    parent_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
    theme: &ThemeResolved,
) -> AnyView {
    match node {
        UiNodeDecl::Panel(panel) => panel_view(
            panel,
            parent_layout,
            compiled,
            app_path,
            scene_contract,
            resources,
            theme,
        ),
        UiNodeDecl::Block(block) => block_view(
            block,
            parent_layout,
            compiled,
            app_path,
            scene_contract,
            resources,
        ),
    }
}

fn block_view(
    block: &BlockDecl,
    panel_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
) -> AnyView {
    let props = attach_host_meta(
        resolve_value(&block.props, scene_contract, resources),
        compiled,
        app_path,
    );
    let tag = compiled
        .component_assets
        .iter()
        .find(|asset| asset.key == block.use_key)
        .map(|asset| asset.tag.clone())
        .unwrap_or_else(|| "mei-missing-component".to_string());
    let html = component_html(tag.as_str(), &props);
    view! {
        <section class="component-card" style=block_style(block.area.as_deref(), panel_layout)>
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}

fn attach_host_meta(mut props: Value, compiled: &CompiledApp, app_path: &str) -> Value {
    if let Some(map) = props.as_object_mut() {
        map.insert(
            "_mei".to_string(),
            serde_json::json!({
                "app_id": compiled.app_id,
                "app_path": app_path,
                "entry_target": compiled.entry_target,
                "step_api": format!("/api/sim/step/{}", app_path),
            }),
        );
    }
    props
}

fn resolve_value(
    value: &Value,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
) -> Value {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("world") {
                if let Some(id) = map.get("id").and_then(Value::as_str) {
                    if let Some(resource) = resources.get(id) {
                        return serde_json::to_value(resource).unwrap_or(Value::Null);
                    }
                }
            }
            if map.get("__ref").and_then(Value::as_str) == Some("scene") {
                return serde_json::to_value(scene_contract).unwrap_or(Value::Null);
            }
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some(dataset) = resolve_data_ref(map, resources) {
                    return serde_json::to_value(dataset).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                if let Some(metric) = resolve_metric_ref(map, resources) {
                    return serde_json::to_value(metric).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            if map.get("metric").and_then(Value::as_str).is_some() {
                let mut compat = serde_json::Map::new();
                compat.insert("__ref".to_string(), Value::String("metric".to_string()));
                if let Some(id) = map.get("metric").cloned() {
                    compat.insert("id".to_string(), id);
                }
                if let Some(from) = map
                    .get("from_dataset")
                    .cloned()
                    .or_else(|| map.get("from").cloned())
                {
                    compat.insert("from_dataset".to_string(), from);
                }
                if let Some(metric) = resolve_metric_ref(&compat, resources) {
                    return serde_json::to_value(metric).unwrap_or(Value::Null);
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("rows")
            {
                if let Some(dataset) = resolve_rows_expr(map, resources) {
                    return serde_json::to_value(dataset).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            let mut out = serde_json::Map::new();
            for (key, entry) in map {
                out.insert(key.clone(), resolve_value(entry, scene_contract, resources));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_value(item, scene_contract, resources))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn resolve_data_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::DatasetView> {
    let id = map.get("id").and_then(Value::as_str)?;
    let from_dataset = map.get("from_dataset").and_then(Value::as_str);
    let dataset_id = from_dataset.unwrap_or(id);
    resources.get(dataset_id)?.dataset.clone()
}

fn resolve_metric_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::MetricContract> {
    let metric_id = map.get("id").and_then(Value::as_str)?;
    if let Some(dataset_id) = map.get("from_dataset").and_then(Value::as_str) {
        return resources
            .get(dataset_id)?
            .dataset
            .as_ref()?
            .metrics
            .get(metric_id)
            .cloned();
    }
    resources
        .values()
        .filter_map(|resource| resource.dataset.as_ref())
        .find_map(|dataset| dataset.metrics.get(metric_id).cloned())
}

fn resolve_rows_expr(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::DatasetView> {
    let dataset = map
        .get("dataset")
        .and_then(Value::as_str)
        .map(|value| value.strip_prefix("dataset.").unwrap_or(value).to_string())?;
    resources.get(&dataset)?.dataset.clone()
}

fn component_html(tag: &str, props: &Value) -> String {
    let props =
        escape_html_attr(&serde_json::to_string(props).unwrap_or_else(|_| "{}".to_string()));
    format!("<{tag} data-props=\"{props}\"></{tag}>")
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn resolve_theme(scene_contract: &SceneContract) -> ThemeResolved {
    let mut theme_id = scene_contract
        .scene
        .theme
        .clone()
        .or_else(|| scene_contract.scene.profile.clone())
        .unwrap_or_else(|| "page".to_string());
    let mut theme = builtin_theme(theme_id.as_str());
    if theme.is_none() {
        theme_id = "page".to_string();
        theme = builtin_theme("page");
    }
    let mut theme = theme.unwrap_or_else(|| serde_json::json!({}));
    if let Some(custom) = scene_contract
        .themes
        .iter()
        .find(|item| item.id == theme_id)
        .or_else(|| scene_contract.themes.first())
    {
        theme = deep_merge_value(&theme, &theme_decl_value(custom));
        if theme_id != custom.id {
            theme_id = custom.id.clone();
        }
    }
    let frame = theme
        .as_object()
        .and_then(|map| map.get("frame"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let panel = theme
        .as_object()
        .and_then(|map| map.get("panel"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let panel_bare = theme
        .as_object()
        .and_then(|map| map.get("panel_bare"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let heading = theme
        .as_object()
        .and_then(|map| map.get("heading"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let css_vars = collect_theme_css_vars(&theme);
    ThemeResolved {
        id: theme_id,
        frame,
        panel,
        panel_bare,
        heading,
        css_vars,
    }
}

fn builtin_theme(theme_id: &str) -> Option<Value> {
    let value = match theme_id {
        "cockpit" => serde_json::json!({
            "frame": {
                "background": {
                    "image": "radial-gradient(120% 80% at 50% -10%, rgba(14,165,233,.22), transparent 55%), radial-gradient(80% 50% at 100% 50%, rgba(59,130,246,.12), transparent 45%), linear-gradient(180deg, #050b14 0%, #0a1628 40%, #071018 100%)",
                    "position": "center",
                    "repeat": "no-repeat"
                },
                "border": "1px solid rgba(56,189,248,.18)",
                "radius": "8px",
                "overflow": "hidden",
                "padding": "0",
            },
            "panel": {
                "background": {
                    "color": "rgba(3,10,20,.76)",
                    "image": "radial-gradient(120% 100% at 0% 0%, rgba(34,211,238,.10), transparent 36%), radial-gradient(120% 100% at 100% 0%, rgba(59,130,246,.08), transparent 34%), linear-gradient(180deg, rgba(8,28,48,.92) 0%, rgba(4,16,30,.9) 58%, rgba(2,10,20,.94) 100%)",
                    "position": "center",
                    "size": "cover",
                    "repeat": "no-repeat"
                },
                "border": "1px solid rgba(56,189,248,.14)",
                "radius": "6px",
                "box_shadow": "inset 0 1px 0 rgba(125,211,252,.08), inset 0 0 0 1px rgba(15,23,42,.22), 0 10px 24px rgba(2,8,23,.24)",
                "padding": "0",
                "overflow": "hidden",
            },
            "panel_bare": {
                "show_heading": false,
                "background": "transparent",
                "border": "none",
                "radius": "0",
                "box_shadow": "none",
                "padding": "0",
                "overflow": "visible"
            },
            "heading": {
                "variant": "screen",
                "accent": true,
                "flair": true,
                "dots": true
            },
            "font": {
                "1": "12px",
                "2": "14px",
                "3": "18px",
                "4": "24px"
            },
            "tokens": {
                "color": {
                    "text_primary": "#e0f2fe",
                    "text_muted": "#94a3b8",
                    "text_accent": "#fde68a"
                },
                "panel": {
                    "radius": "6px",
                    "padding": "12px"
                }
            }
        }),
        "game" => serde_json::json!({
            "frame": {
                "background": {
                    "image": "linear-gradient(180deg, #111827 0%, #1f2937 100%)"
                },
                "padding": "0"
            },
            "panel": {
                "background": "rgba(17, 24, 39, 0.78)",
                "border": "1px solid rgba(148,163,184,.18)",
                "radius": "8px",
                "padding": "0",
                "overflow": "hidden"
            },
            "panel_bare": {
                "show_heading": false,
                "background": "transparent",
                "border": "none",
                "padding": "0",
                "overflow": "visible"
            },
            "heading": {
                "variant": "compact",
                "accent": true
            },
            "font": {
                "1": "12px",
                "2": "14px",
                "3": "17px",
                "4": "22px"
            },
            "tokens": {
                "color": {
                    "text_primary": "#f3f4f6",
                    "text_muted": "#9ca3af",
                    "text_accent": "#fbbf24"
                }
            }
        }),
        _ => serde_json::json!({
            "frame": {
                "padding": "0"
            },
            "panel": {
                "background": "rgba(2,6,23,.32)",
                "border": "1px solid rgba(59,130,246,.18)",
                "radius": "14px",
                "padding": "12px"
            },
            "panel_bare": {
                "show_heading": false,
                "background": "transparent",
                "border": "none",
                "padding": "0",
                "overflow": "visible"
            },
            "heading": {
                "variant": "default",
                "accent": true
            },
            "font": {
                "1": "12px",
                "2": "14px",
                "3": "16px",
                "4": "20px"
            },
            "tokens": {
                "color": {
                    "text_primary": "#e2e8f0",
                    "text_muted": "#94a3b8",
                    "text_accent": "#f8fafc"
                }
            }
        }),
    };
    Some(value)
}

fn theme_decl_value(theme: &ThemeDecl) -> Value {
    serde_json::json!({
        "frame": theme.frame,
        "panel": theme.panel,
        "panel_bare": theme.panel_bare,
        "heading": theme.heading,
        "font": theme.font,
        "tokens": theme.tokens,
    })
}

fn resolve_panel_props(theme: &ThemeResolved, props: &Value) -> Value {
    let use_bare = props
        .as_object()
        .and_then(|map| map.get("chrome"))
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("bare"));
    if use_bare {
        deep_merge_value(&theme.panel_bare, props)
    } else {
        deep_merge_value(&theme.panel, props)
    }
}

fn deep_merge_value(base: &Value, overlay: &Value) -> Value {
    let (Some(base_obj), Some(overlay_obj)) = (base.as_object(), overlay.as_object()) else {
        return overlay.clone();
    };
    let mut merged = base_obj.clone();
    for (key, value) in overlay_obj {
        let next = if let Some(existing) = merged.get(key) {
            deep_merge_value(existing, value)
        } else {
            value.clone()
        };
        merged.insert(key.clone(), next);
    }
    Value::Object(merged)
}

fn collect_theme_css_vars(theme: &Value) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    if let Some(font) = theme
        .as_object()
        .and_then(|map| map.get("font"))
        .and_then(Value::as_object)
    {
        for (key, value) in font {
            if let Some(raw) = value.as_str() {
                vars.push((format!("--mei-font-{key}"), raw.to_string()));
            }
        }
    }
    if let Some(tokens) = theme.as_object().and_then(|map| map.get("tokens")) {
        flatten_tokens(tokens, "mei", &mut vars);
    }
    vars
}

fn flatten_tokens(value: &Value, prefix: &str, vars: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map {
                let path = format!("{prefix}-{}", key.replace('_', "-"));
                flatten_tokens(entry, path.as_str(), vars);
            }
        }
        Value::String(raw) if !raw.trim().is_empty() => {
            vars.push((format!("--{prefix}"), raw.to_string()));
        }
        Value::Number(raw) => {
            vars.push((format!("--{prefix}"), raw.to_string()));
        }
        Value::Bool(raw) => {
            vars.push((format!("--{prefix}"), raw.to_string()));
        }
        _ => {}
    }
}

fn theme_css_vars_style(theme: &ThemeResolved) -> String {
    let mut style = String::new();
    style.push_str(&format!("--mei-theme-id:'{}';", theme.id));
    for (key, value) in &theme.css_vars {
        style.push_str(&format!("{key}:{value};"));
    }
    style
}

fn frame_style(
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
    theme: &ThemeResolved,
) -> String {
    let mut style = surface_layout_style(layout);
    style.push_str(&container_visual_style(props));
    style.push_str(&theme_css_vars_style(theme));
    style
}

fn frame_stage_style(
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
    viewport: &FrameViewportConfig,
    theme: &ThemeResolved,
) -> String {
    let mut style = frame_style(layout, props, theme);
    style.push_str(&format!(
        "width:{}px;height:{}px;transform-origin:top left;",
        viewport.design_width, viewport.design_height
    ));
    style
}

fn frame_viewport_style(viewport: &FrameViewportConfig) -> String {
    format!(
        "width:100%;height:100%;min-width:0;min-height:0;overflow:hidden;display:grid;justify-items:{};align-items:{};padding:{}px {}px {}px {}px;",
        viewport.align_x,
        viewport.align_y,
        viewport.safe_top,
        viewport.safe_right,
        viewport.safe_bottom,
        viewport.safe_left,
    )
}

fn frame_viewport_config(props: &Value) -> Option<FrameViewportConfig> {
    let map = props.as_object()?;
    let viewport = map.get("viewport")?.as_object()?;
    if viewport
        .get("enabled")
        .and_then(Value::as_bool)
        .is_some_and(|value| !value)
    {
        return None;
    }
    let design_width = viewport
        .get("design_width")
        .and_then(Value::as_f64)
        .filter(|value| *value > 0.0)?;
    let design_height = viewport
        .get("design_height")
        .and_then(Value::as_f64)
        .filter(|value| *value > 0.0)?;
    let scale_mode = viewport
        .get("scale_mode")
        .and_then(Value::as_str)
        .unwrap_or("contain")
        .to_string();
    let (align_x, align_y) = viewport_align(viewport);
    let (safe_top, safe_right, safe_bottom, safe_left) = viewport_safe_inset(viewport);
    Some(FrameViewportConfig {
        design_width,
        design_height,
        scale_mode,
        align_x,
        align_y,
        safe_top,
        safe_right,
        safe_bottom,
        safe_left,
    })
}

fn viewport_align(viewport: &serde_json::Map<String, Value>) -> (String, String) {
    let align_x = viewport
        .get("align_x")
        .and_then(Value::as_str)
        .map(normalize_align_x);
    let align_y = viewport
        .get("align_y")
        .and_then(Value::as_str)
        .map(normalize_align_y);
    if align_x.is_some() || align_y.is_some() {
        return (
            align_x.unwrap_or_else(|| "center".to_string()),
            align_y.unwrap_or_else(|| "center".to_string()),
        );
    }
    let align = viewport
        .get("align")
        .and_then(Value::as_str)
        .unwrap_or("center");
    match align.trim().to_ascii_lowercase().as_str() {
        "top" | "top-center" => ("center".to_string(), "start".to_string()),
        "top-left" => ("start".to_string(), "start".to_string()),
        "top-right" => ("end".to_string(), "start".to_string()),
        "bottom" | "bottom-center" => ("center".to_string(), "end".to_string()),
        "bottom-left" => ("start".to_string(), "end".to_string()),
        "bottom-right" => ("end".to_string(), "end".to_string()),
        "left" | "center-left" => ("start".to_string(), "center".to_string()),
        "right" | "center-right" => ("end".to_string(), "center".to_string()),
        _ => ("center".to_string(), "center".to_string()),
    }
}

fn normalize_align_x(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "left" | "start" => "start".to_string(),
        "right" | "end" => "end".to_string(),
        _ => "center".to_string(),
    }
}

fn normalize_align_y(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "top" | "start" => "start".to_string(),
        "bottom" | "end" => "end".to_string(),
        _ => "center".to_string(),
    }
}

fn viewport_safe_inset(viewport: &serde_json::Map<String, Value>) -> (f64, f64, f64, f64) {
    let all = viewport
        .get("safe_padding")
        .or_else(|| viewport.get("safe_inset"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let Some(inset) = viewport.get("safe_inset").and_then(Value::as_object) else {
        return (all, all, all, all);
    };
    let top = inset
        .get("top")
        .and_then(Value::as_f64)
        .unwrap_or(all)
        .max(0.0);
    let right = inset
        .get("right")
        .and_then(Value::as_f64)
        .unwrap_or(all)
        .max(0.0);
    let bottom = inset
        .get("bottom")
        .and_then(Value::as_f64)
        .unwrap_or(all)
        .max(0.0);
    let left = inset
        .get("left")
        .and_then(Value::as_f64)
        .unwrap_or(all)
        .max(0.0);
    (top, right, bottom, left)
}

fn surface_layout_style(layout: Option<&mei_lang_kernel::LayoutDecl>) -> String {
    let Some(layout) = layout else {
        return "display:grid;gap:16px;".to_string();
    };
    match layout.layout_type.as_str() {
        "flex" => format!(
            "display:flex;flex-direction:{};gap:{};padding:{};",
            layout
                .direction
                .clone()
                .unwrap_or_else(|| "column".to_string()),
            layout.gap.clone().unwrap_or_else(|| "16px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
        _ => format!(
            "display:grid;grid-template-columns:{};grid-template-rows:{};{}gap:{};padding:{};",
            layout
                .columns
                .clone()
                .unwrap_or_else(|| vec!["1fr".to_string()])
                .join(" "),
            layout
                .rows
                .clone()
                .unwrap_or_else(|| vec!["auto".to_string()])
                .join(" "),
            grid_template_areas_style(layout),
            layout.gap.clone().unwrap_or_else(|| "16px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
    }
}

fn panel_style(
    area: Option<&str>,
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
) -> String {
    let mut style = String::new();
    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && area == Some("full")
    {
        style.push_str("grid-column:1 / -1;");
        style.push_str(&container_visual_style(props));
        return style;
    }

    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && layout
            .and_then(|value| value.areas.as_ref())
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    {
        if let Some(area) = area {
            style.push_str(&format!("grid-area:{};", area));
            style.push_str(&container_visual_style(props));
            return style;
        }
    }
    style.push_str(&container_visual_style(props));
    style
}

fn panel_show_heading(props: &Value) -> bool {
    let Some(map) = props.as_object() else {
        return true;
    };
    if let Some(value) = map.get("show_heading").and_then(Value::as_bool) {
        return value;
    }
    !matches!(map.get("chrome").and_then(Value::as_str), Some("bare"))
}

fn panel_heading_config(theme_heading: &Value, props: &Value) -> PanelHeadingConfig {
    let mut variant = "default".to_string();
    let mut subtitle = None;
    let mut show_accent = None;
    let mut show_flair = None;
    let mut show_dots = None;

    let heading_props = deep_merge_value(
        theme_heading,
        &props
            .as_object()
            .and_then(|map| map.get("heading"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    );

    if let Some(map) = props.as_object() {
        subtitle = map
            .get("subtitle")
            .and_then(Value::as_str)
            .map(|value| value.to_string());
    }
    if let Some(heading) = heading_props.as_object() {
        if let Some(value) = heading.get("variant").and_then(Value::as_str) {
            variant = value.to_string();
        }
        if let Some(value) = heading.get("subtitle").and_then(Value::as_str) {
            subtitle = Some(value.to_string());
        }
        show_accent = heading.get("accent").and_then(Value::as_bool);
        show_flair = heading.get("flair").and_then(Value::as_bool);
        show_dots = heading.get("dots").and_then(Value::as_bool);
    }

    let (default_accent, default_flair, default_dots) = match variant.as_str() {
        "screen" => (true, true, true),
        "compact" => (true, false, false),
        "plain" => (false, false, false),
        _ => (true, false, false),
    };

    PanelHeadingConfig {
        variant,
        subtitle,
        show_accent: show_accent.unwrap_or(default_accent),
        show_flair: show_flair.unwrap_or(default_flair),
        show_dots: show_dots.unwrap_or(default_dots),
    }
}

fn container_visual_style(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let mut style = String::new();

    if let Some(background) = map.get("background") {
        match background {
            Value::String(value) if !value.trim().is_empty() => {
                style.push_str(&format!("background:{};", value.trim()));
            }
            Value::Object(bg) => {
                if let Some(value) = bg.get("color").and_then(Value::as_str) {
                    style.push_str(&format!("background-color:{};", value));
                }
                if let Some(value) = bg.get("image").and_then(Value::as_str) {
                    style.push_str(&format!(
                        "background-image:{};",
                        normalize_background_image(value)
                    ));
                }
                if let Some(value) = bg.get("size").and_then(Value::as_str) {
                    style.push_str(&format!("background-size:{};", value));
                }
                if let Some(value) = bg.get("position").and_then(Value::as_str) {
                    style.push_str(&format!("background-position:{};", value));
                }
                if let Some(value) = bg.get("repeat").and_then(Value::as_str) {
                    style.push_str(&format!("background-repeat:{};", value));
                }
                if let Some(value) = bg.get("attachment").and_then(Value::as_str) {
                    style.push_str(&format!("background-attachment:{};", value));
                }
                if let Some(value) = bg.get("blend_mode").and_then(Value::as_str) {
                    style.push_str(&format!("background-blend-mode:{};", value));
                }
            }
            _ => {}
        }
    }

    append_string_style(&mut style, map.get("padding"), "padding");
    append_string_style(&mut style, map.get("margin"), "margin");
    append_string_style(&mut style, map.get("border"), "border");
    append_string_style(&mut style, map.get("radius"), "border-radius");
    append_string_style(&mut style, map.get("box_shadow"), "box-shadow");
    append_string_style(&mut style, map.get("overflow"), "overflow");
    append_string_style(&mut style, map.get("min_height"), "min-height");
    append_string_style(&mut style, map.get("min_width"), "min-width");

    style
}

fn append_string_style(style: &mut String, value: Option<&Value>, css_name: &str) {
    if let Some(value) = value.and_then(Value::as_str) {
        style.push_str(&format!("{css_name}:{value};"));
    }
}

fn normalize_background_image(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "none".to_string();
    }
    if value.contains('(') || value.starts_with("var(") || value.starts_with("url(") {
        value.to_string()
    } else {
        format!("url(\"{}\")", value.replace('"', "%22"))
    }
}

fn panel_body_style(layout: Option<&mei_lang_kernel::LayoutDecl>) -> String {
    let Some(layout) = layout else {
        return String::new();
    };
    match layout.layout_type.as_str() {
        "flex" => format!(
            "display:flex;flex-direction:{};gap:{};padding:{};",
            layout
                .direction
                .clone()
                .unwrap_or_else(|| "column".to_string()),
            layout.gap.clone().unwrap_or_else(|| "12px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
        _ => format!(
            "display:grid;grid-template-columns:{};grid-template-rows:{};{}gap:{};padding:{};",
            layout
                .columns
                .clone()
                .unwrap_or_else(|| vec!["1fr".to_string()])
                .join(" "),
            layout
                .rows
                .clone()
                .unwrap_or_else(|| vec!["auto".to_string()])
                .join(" "),
            grid_template_areas_style(layout),
            layout.gap.clone().unwrap_or_else(|| "12px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
    }
}

fn block_style(area: Option<&str>, layout: Option<&mei_lang_kernel::LayoutDecl>) -> String {
    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && area == Some("full")
    {
        return "grid-column:1 / -1;".to_string();
    }

    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && layout
            .and_then(|value| value.areas.as_ref())
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    {
        if let Some(area) = area {
            if !area.trim().is_empty() && area != "auto" {
                return format!("grid-area:{};", area);
            }
        }
    }
    String::new()
}

fn grid_template_areas_style(layout: &mei_lang_kernel::LayoutDecl) -> String {
    let Some(rows) = layout.areas.as_ref() else {
        return String::new();
    };
    let rows = rows
        .iter()
        .filter(|row| !row.is_empty())
        .map(|row| {
            let template = row
                .iter()
                .map(|area| {
                    let area = area.trim();
                    if area.is_empty() {
                        "."
                    } else {
                        area
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("'{template}'")
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        String::new()
    } else {
        format!("grid-template-areas:{};", rows.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        block_style, container_visual_style, frame_viewport_config, frame_viewport_style,
        panel_body_style, panel_show_heading, panel_style, resolve_panel_props, resolve_value,
        surface_layout_style, ThemeResolved,
    };
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
        let viewport = frame_viewport_config(&json!({
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
        assert_eq!(viewport.align_x, "center");
        assert_eq!(viewport.align_y, "start");
        assert_eq!(viewport.safe_top, 12.0);
        assert_eq!(viewport.safe_right, 24.0);
        assert_eq!(viewport.safe_bottom, 16.0);
        assert_eq!(viewport.safe_left, 20.0);
    }

    #[test]
    fn frame_viewport_style_applies_alignment_and_padding() {
        let viewport = frame_viewport_config(&json!({
            "viewport": {
                "design_width": 1920,
                "design_height": 1080,
                "align_x": "left",
                "align_y": "top",
                "safe_padding": 18,
            }
        }))
        .expect("viewport config");
        let style = frame_viewport_style(&viewport);
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
                        content: None,
                    },
                    sources: Vec::new(),
                    metrics: BTreeMap::from([(
                        "sales_total".to_string(),
                        MetricContract {
                            id: "sales_total".to_string(),
                            label: Some("销售总额".to_string()),
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
                }),
            },
        );

        let data_ref = json!({"__ref":"data","id":"sales_metrics"});
        let resolved_data = resolve_value(&data_ref, &scene_contract, &resources);
        assert_eq!(
            resolved_data.get("id").and_then(|value| value.as_str()),
            Some("sales_metrics")
        );

        let metric_ref =
            json!({"__ref":"metric","id":"sales_total","from_dataset":"sales_metrics"});
        let resolved_metric = resolve_value(&metric_ref, &scene_contract, &resources);
        assert_eq!(
            resolved_metric.get("id").and_then(|value| value.as_str()),
            Some("sales_total")
        );
    }
}
