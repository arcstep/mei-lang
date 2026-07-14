//! Runtime rewrites for `biz.*` template macros left unexpanded in content_panel artifacts.
//!
//! The mei-compiler expand pass should inline these at compile time; when they remain in
//! stored JSON, `v2_lower` must expand them before lowering to `UiNodeDecl` / `BlockDecl`.

use serde_json::{json, Map, Value};

fn call_args(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()?.get("__args")?.as_object()
}

fn arg_value<'a>(args: &'a Map<String, Value>, key: &str, fallback: Value) -> Value {
    args.get(key).cloned().unwrap_or(fallback)
}

fn transparent_panel_props(height: Value) -> Value {
    json!({
        "padding": "0",
        "background": "transparent",
        "box_shadow": "var(--mei-layout-debug-micro-shadow, inset 0 0 0 0 transparent)",
        "height": height,
        "min_height": "0",
        "box_sizing": "border-box",
        "overflow": "hidden"
    })
}

const TPL_METRICS: &str = "/workspace-app-assets/templates/cockpit/assets/metrics";

fn slot_stretch_background(image: &str) -> Value {
    json!({
        "color": "rgba(98,190,235,0.10)",
        "image": format!("url({TPL_METRICS}/{image})"),
        "size": "100% 100%",
        "position": "center",
        "repeat": "no-repeat",
        "origin": "border-box",
        "clip": "border-box",
    })
}

fn grid_layout(rows: Value, columns: Value, areas: Value, gap: &str, align: &str) -> Value {
    let justify = if align == "stretch" {
        "stretch"
    } else {
        "center"
    };
    json!({
        "__call": "grid",
        "__args": {
            "rows": rows,
            "columns": columns,
            "areas": areas,
            "gap": gap,
            "align": align,
            "justify": justify
        }
    })
}

fn compound_metric_slot_panel(id: Value, area: &str, child: Value) -> Value {
    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": area,
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": {
                "padding": "0",
                "background": "transparent",
                "width": "100%",
                "height": "100%",
                "min_height": "0",
                "box_sizing": "border-box",
                "overflow": "hidden"
            },
            "layout": grid_layout(
                json!(["1fr"]),
                json!(["1fr"]),
                json!([["content"]]),
                "0",
                "stretch",
            ),
            "blocks": [child]
        }
    })
}

fn rewrite_story_opinion_block(args: &Map<String, Value>) -> Value {
    json!({
        "__call": "component",
        "__args": {
            "arg0": "cockpit.opinion-panel",
            "area": arg_value(args, "area", json!("auto")),
            "props": {
                "point_id": arg_value(args, "point_id", json!("")),
                "title": arg_value(args, "title", json!("说明板")),
                "body_format": "html",
                "body": arg_value(args, "body", json!("<p>这里是一段说明。</p>")),
                "action_text": arg_value(args, "action_text", json!("")),
                "action_link": arg_value(args, "action_link", Value::Null),
                "emphasis": arg_value(args, "emphasis", json!(false)),
            }
        }
    })
}

fn rewrite_metric_triptych_compound_body(args: &Map<String, Value>) -> Value {
    let fill_strip = args
        .get("fill_strip")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let row_height = if fill_strip {
        json!("1fr")
    } else {
        arg_value(args, "row_height", json!("auto"))
    };
    let row_align = if fill_strip {
        json!("stretch")
    } else {
        arg_value(args, "row_align", json!("start"))
    };
    let mut props = transparent_panel_props(if fill_strip {
        json!("100%")
    } else {
        json!("auto")
    });
    if fill_strip {
        if let Some(obj) = props.as_object_mut() {
            obj.insert("width".to_string(), json!("100%"));
        }
    }
    json!({
        "__call": "panel",
        "__args": {
            "id": arg_value(args, "id", json!("metric_triptych_compound")),
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": props,
            "layout": grid_layout(
                json!([row_height]),
                json!([
                    arg_value(args, "first_width", json!("88px")),
                    arg_value(args, "second_width", json!("88px")),
                    arg_value(args, "third_width", json!("88px")),
                    arg_value(args, "compound_width", json!("220px")),
                ]),
                json!([["first", "second", "third", "compound"]]),
                arg_value(args, "gap", json!("2px")).as_str().unwrap_or("2px"),
                row_align.as_str().unwrap_or("start"),
            ),
            "blocks": [
                arg_value(args, "first", Value::Null),
                arg_value(args, "second", Value::Null),
                arg_value(args, "third", Value::Null),
                arg_value(args, "compound", Value::Null),
            ]
        }
    })
}

fn id_suffix(id: Value, suffix: &str) -> Value {
    json!({
        "__binop": "Add",
        "left": id,
        "right": suffix
    })
}

fn long_compound_template(width: Value, gap: &str) -> Value {
    // Visual gap between main/rtop/rbottom must stay 0 so the shared
    // metric-bg-long frame reads as one card (seams look like per-slot skins).
    let _ = gap;
    let layout_gap = "0";
    json!({
        "__call": "content_panel",
        "__args": {
            "show_heading": false,
            "chrome": "bare",
            "variant": "container",
            "props": {
                "background": {
                    "color": "rgba(98,190,235,0.10)",
                    "image": "url(/workspace-app-assets/templates/cockpit/assets/metrics/metric-bg-long@3x.svg)",
                    "size": "100% 100%",
                    "position": "center",
                    "repeat": "no-repeat",
                    "origin": "border-box",
                    "clip": "border-box"
                },
                "width": width,
                "height": "100%",
                "min_height": "0",
                "box_sizing": "border-box",
                "overflow": "hidden",
                "box_shadow": "var(--mei-layout-debug-card-shadow, inset 0 0 0 0 transparent)",
                "padding": "2px 6px",
                "gap": layout_gap,
                "__mei_slot_frame_bg": true
            },
            "layout": grid_layout(
                json!(["minmax(0, 1fr)", "minmax(0, 1fr)"]),
                json!(["1.05fr", "1.95fr"]),
                json!([["main", "rtop"], ["main", "rbottom"]]),
                layout_gap,
                "center",
            ),
            "blocks": []
        }
    })
}

fn rewrite_primary_progress_triptych_body(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("primary_progress_triptych"));
    let gap = arg_value(args, "gap", json!("6px"));
    let gap = gap.as_str().unwrap_or("6px");
    json!({
        "__call": "panel",
        "__args": {
            "id": id.clone(),
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": transparent_panel_props(json!("auto")),
            "layout": grid_layout(
                json!(["auto"]),
                json!([
                    arg_value(args, "primary_width", json!("168px")),
                    "1fr",
                ]),
                json!([["primary", "triptych"]]),
                gap,
                "start",
            ),
            "blocks": [
                arg_value(args, "primary", Value::Null),
                {
                    "__call": "panel",
                    "__args": {
                        "id": id_suffix(id, "_triptych"),
                        "area": "triptych",
                        "variant": "container",
                        "show_heading": false,
                        "chrome": "bare",
                        "props": transparent_panel_props(json!("auto")),
                        "layout": grid_layout(
                            json!(["auto"]),
                            json!(["1fr", "1fr", "1fr"]),
                            json!([["first", "second", "third"]]),
                            "4px",
                            "start",
                        ),
                        "blocks": [
                            arg_value(args, "first", Value::Null),
                            arg_value(args, "second", Value::Null),
                            arg_value(args, "third", Value::Null),
                        ]
                    }
                }
            ]
        }
    })
}

/// Fill-down progress triptych（zhifa 行政检查「无违规」行）。
/// 与 `long_metric_compound_fill_body` 同理：content_panel 产物里嵌套 biz 宏若未
/// 在此重写，`v2_lower` 会 `Ok(Vec::new())` 静默丢掉整块。
fn rewrite_primary_progress_triptych_fill_body(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("primary_progress_triptych"));
    let gap = arg_value(args, "gap", json!("6px"));
    let gap = gap.as_str().unwrap_or("6px");
    json!({
        "__call": "panel",
        "__args": {
            "id": id.clone(),
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": transparent_panel_props(json!("100%")),
            "layout": grid_layout(
                json!(["1fr"]),
                json!([
                    arg_value(args, "primary_width", json!("168px")),
                    "1fr",
                ]),
                json!([["primary", "triptych"]]),
                gap,
                "stretch",
            ),
            "blocks": [
                rewrite_nested_biz_arg(args.get("primary")),
                {
                    "__call": "panel",
                    "__args": {
                        "id": id_suffix(id, "_triptych"),
                        "area": "triptych",
                        "variant": "container",
                        "show_heading": false,
                        "chrome": "bare",
                        "props": transparent_panel_props(json!("100%")),
                        "layout": grid_layout(
                            json!(["1fr"]),
                            json!(["1fr", "1fr", "1fr"]),
                            json!([["first", "second", "third"]]),
                            "4px",
                            "stretch",
                        ),
                        "blocks": [
                            rewrite_nested_biz_arg(args.get("first")),
                            rewrite_nested_biz_arg(args.get("second")),
                            rewrite_nested_biz_arg(args.get("third")),
                        ]
                    }
                }
            ]
        }
    })
}

fn rewrite_nested_biz_arg(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    try_rewrite_biz_macro(value).unwrap_or_else(|| value.clone())
}

/// `progress_metric_fill_slot` → slot panel + `stack_progress` metric（fill-down）。
fn rewrite_progress_metric_fill_slot(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("metric_progress_slot"));
    let variant = args.get("variant").and_then(Value::as_str);
    let mut metric = metric_atom(
        id_suffix(id.clone(), "_content"),
        "stack_progress",
        arg_value(
            args,
            "source",
            json!({"label": "指标", "value": "--", "unit": "", "desc": ""}),
        ),
        variant,
    );
    if let Some(desc) = args.get("desc") {
        if let Some(metric_args) = metric.get_mut("__args").and_then(Value::as_object_mut) {
            metric_args.insert("desc".to_string(), desc.clone());
        }
    }
    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": {
                "padding": "0",
                "background": slot_stretch_background("metric-bg-clean@3x.svg"),
                "border": "none",
                "radius": "4px",
                "width": "100%",
                "height": "100%",
                "min_height": "0",
                "box_sizing": "border-box",
                "overflow": "hidden",
                "__mei_slot_frame_bg": true
            },
            "layout": grid_layout(
                json!(["1fr"]),
                json!(["1fr"]),
                json!([["content"]]),
                "0",
                "stretch",
            ),
            "blocks": [metric]
        }
    })
}

fn rewrite_long_metric_compound_body(args: &Map<String, Value>) -> Value {
    let width = arg_value(args, "width", json!("100%"));
    let gap = "2px";
    let template = long_compound_template(width.clone(), gap);
    json!({
        "__call": "panel",
        "__args": {
            "id": arg_value(args, "id", json!("long_metric_compound")),
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "template": template.clone(),
            "props": template
                .pointer("/__args/props")
                .cloned()
                .unwrap_or_else(|| json!({})),
            "layout": template.pointer("/__args/layout").cloned().unwrap_or(Value::Null),
            "blocks": [
                arg_value(args, "main", Value::Null),
                arg_value(args, "top", Value::Null),
                arg_value(args, "bottom", Value::Null),
            ]
        }
    })
}

/// Fill-down long compound used by zhifa 行政检查 bottom row.
/// Must be rewritten in-host: template expand of nested `metric(...)` is not
/// reliable for content_panel artifacts, and a failed expand silently drops the block.
fn rewrite_long_metric_compound_fill_body(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("long_metric_compound"));
    let width = arg_value(args, "width", json!("100%"));
    let gap = "2px";
    let template = long_compound_template(width, gap);
    let main_id = id_suffix(id.clone(), "_main");
    let top_id = id_suffix(id.clone(), "_top");
    let bottom_id = id_suffix(id.clone(), "_bottom");
    let mut top_metric = metric_atom(
        id_suffix(top_id.clone(), "_content"),
        "compound_top_row",
        arg_value(
            args,
            "top_source",
            json!({"label": "次指标A", "value": "0", "unit": "台"}),
        ),
        Some("sub"),
    );
    let mut bottom_metric = metric_atom(
        id_suffix(bottom_id.clone(), "_content"),
        "compound_top_row",
        arg_value(
            args,
            "bottom_source",
            json!({"label": "次指标B", "value": "0", "unit": "小时"}),
        ),
        Some("sub"),
    );
    for metric in [&mut top_metric, &mut bottom_metric] {
        if let Some(metric_args) = metric.get_mut("__args").and_then(Value::as_object_mut) {
            metric_args.insert("label_vertical_align".to_string(), json!("center"));
            metric_args.insert("value_vertical_align".to_string(), json!("center"));
            metric_args.insert("unit_vertical_align".to_string(), json!("center"));
        }
    }
    json!({
        "__call": "panel",
        "__args": {
            "id": id.clone(),
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "template": template.clone(),
            "props": template
                .pointer("/__args/props")
                .cloned()
                .unwrap_or_else(|| transparent_panel_props(json!("100%"))),
            "layout": template.pointer("/__args/layout").cloned().unwrap_or(Value::Null),
            "blocks": [
                compound_metric_slot_panel(
                    main_id.clone(),
                    "main",
                    metric_atom(
                        id_suffix(main_id, "_content"),
                        "plain",
                        arg_value(
                            args,
                            "main_source",
                            json!({"label": "主指标", "value": "0", "unit": "次"}),
                        ),
                        None,
                    ),
                ),
                compound_metric_slot_panel(top_id, "rtop", top_metric),
                compound_metric_slot_panel(bottom_id, "rbottom", bottom_metric),
            ]
        }
    })
}

fn rewrite_wide_metric_compound_body(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("wide_metric_compound"));
    let id_body = id_suffix(id.clone(), "_body");
    let top_band_fr = args
        .get("top_band_fr")
        .and_then(Value::as_f64)
        .or_else(|| {
            args.get("top_band_fr")
                .and_then(Value::as_i64)
                .map(|n| n as f64)
        })
        .unwrap_or(48.0);
    let top_band_ratio = json!(format!("{top_band_fr}%"));
    let gap = json!("2px");
    let main = arg_value(args, "main", Value::Null);
    let sub_a = arg_value(args, "sub_a", Value::Null);
    let sub_b = arg_value(args, "sub_b", Value::Null);
    let sub_c = arg_value(args, "sub_c", Value::Null);
    let top_id = id_suffix(id.clone(), "_top");
    let b0_id = id_suffix(id.clone(), "_b0");
    let b1_id = id_suffix(id.clone(), "_b1");
    let b2_id = id_suffix(id.clone(), "_b2");

    let inner_panel = json!({
        "__call": "panel",
        "__args": {
            "id": id_body,
            "area": "content",
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": {
                "__mei_compound_top_band_ratio": top_band_ratio,
                "width": "100%",
                "height": "100%",
                "min_height": "0",
                "max_height": "100%",
                "box_sizing": "border-box",
                "overflow": "hidden",
                "background": "transparent",
                "padding": "0 4px",
                "gap": gap
            },
            "template": {
                "__call": "content_panel",
                "__args": {
                    "show_heading": false,
                    "chrome": "bare",
                    "variant": "container",
                    "props": {
                        "__mei_compound_top_band_ratio": top_band_ratio,
                        "width": "100%",
                        "height": "100%",
                        "min_height": "0",
                        "max_height": "100%",
                        "box_sizing": "border-box",
                        "overflow": "hidden",
                        "background": "transparent",
                        "padding": "0 4px",
                        "gap": gap
                    },
                    "layout": grid_layout(
                        json!([top_band_ratio, "1fr"]),
                        json!(["1fr", "1fr", "1fr"]),
                        json!([["top", "top", "top"], ["b0", "b1", "b2"]]),
                        "2px",
                        "stretch",
                    ),
                    "blocks": []
                }
            },
            "layout": grid_layout(
                json!([top_band_ratio, "1fr"]),
                json!(["1fr", "1fr", "1fr"]),
                json!([["top", "top", "top"], ["b0", "b1", "b2"]]),
                "2px",
                "stretch",
            ),
            "blocks": [
                compound_metric_slot_panel(top_id, "top", main),
                compound_metric_slot_panel(b0_id, "b0", sub_a),
                compound_metric_slot_panel(b1_id, "b1", sub_b),
                compound_metric_slot_panel(b2_id, "b2", sub_c),
            ]
        }
    });

    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": {
                "padding": arg_value(args, "shell_padding", json!("0")),
                "background": slot_stretch_background("metric-bg-target@3x.svg"),
                "border": "none",
                "radius": "4px",
                "width": arg_value(args, "width", json!("220px")),
                "height": arg_value(args, "height", json!("100%")),
                "min_height": "0",
                "box_sizing": "border-box",
                "overflow": "hidden",
                "box_shadow": "var(--mei-layout-debug-micro-shadow, inset 0 0 0 0 transparent)",
                "__mei_slot_frame_bg": true
            },
            "layout": grid_layout(
                json!(["1fr"]),
                json!(["1fr"]),
                json!([["content"]]),
                "0",
                "stretch",
            ),
            "blocks": [inner_panel]
        }
    })
}

fn rewrite_content_fill_props(_args: &Map<String, Value>) -> Value {
    let mut props = transparent_panel_props(json!("100%"));
    if let Some(obj) = props.as_object_mut() {
        obj.remove("background");
        obj.insert("__mei_layout_fill".to_string(), json!(true));
    }
    props
}

fn rewrite_content_strip_props(_args: &Map<String, Value>) -> Value {
    // content_strip / row_budgets deleted; emit fill props only.
    rewrite_content_fill_props(_args)
}

fn metric_atom(id: Value, surface: &str, source: Value, variant: Option<&str>) -> Value {
    let mut args = json!({
        "id": id,
        "area": "content",
        "surface": surface,
        "source": source,
        "props": {
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "background": "transparent",
            "border": "none",
            "box_shadow": "none",
        }
    });
    if let Some(variant) = variant {
        args.as_object_mut()
            .expect("metric args")
            .insert("variant".to_string(), json!(variant));
    }
    json!({
        "__call": "metric",
        "__args": args
    })
}

fn compound_inner_body_panel(id: Value, args: &Map<String, Value>) -> Value {
    let top_id = id_suffix(id.clone(), "_top");
    let b0_id = id_suffix(id.clone(), "_b0");
    let b1_id = id_suffix(id.clone(), "_b1");
    let b2_id = id_suffix(id.clone(), "_b2");

    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": "content",
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": transparent_panel_props(json!("100%")),
            "layout": grid_layout(
                json!(["44%", "1fr"]),
                json!(["1fr", "1fr", "1fr"]),
                json!([["top", "top", "top"], ["b0", "b1", "b2"]]),
                "2px",
                "stretch",
            ),
            "blocks": [
                compound_metric_slot_panel(
                    top_id.clone(),
                    "top",
                    metric_atom(
                        id_suffix(top_id, "_content"),
                        "compound_top_row",
                        arg_value(
                            args,
                            "top_source",
                            json!({"label": "执法对象", "value": "--", "unit": "万"}),
                        ),
                        None,
                    ),
                ),
                compound_metric_slot_panel(
                    b0_id.clone(),
                    "b0",
                    metric_atom(
                        id_suffix(b0_id, "_content"),
                        "compound_sub_stack",
                        arg_value(
                            args,
                            "sub_a_source",
                            json!({"label": "重点企业", "value": "--", "unit": "家"}),
                        ),
                        Some("sub"),
                    ),
                ),
                compound_metric_slot_panel(
                    b1_id.clone(),
                    "b1",
                    metric_atom(
                        id_suffix(b1_id, "_content"),
                        "compound_sub_stack",
                        arg_value(
                            args,
                            "sub_b_source",
                            json!({"label": "园区", "value": "--", "unit": "个"}),
                        ),
                        Some("sub"),
                    ),
                ),
                compound_metric_slot_panel(
                    b2_id.clone(),
                    "b2",
                    metric_atom(
                        id_suffix(b2_id, "_content"),
                        "compound_sub_stack",
                        arg_value(
                            args,
                            "sub_c_source",
                            json!({"label": "白名单", "value": "--", "unit": "家"}),
                        ),
                        Some("sub"),
                    ),
                ),
            ]
        }
    })
}

fn narrow_metric_slot_panel(id: Value, area: &str, source: Value) -> Value {
    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": area,
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": {
                "padding": "0",
                "background": slot_stretch_background("metric-bg-normal@3x.svg"),
                "border": "none",
                "radius": "4px",
                "width": "100%",
                "height": "100%",
                "min_height": "0",
                "box_sizing": "border-box",
                "overflow": "hidden",
                "box_shadow": "var(--mei-layout-debug-micro-shadow, inset 0 0 0 0 transparent)",
                "__mei_slot_frame_bg": true
            },
            "layout": grid_layout(
                json!(["1fr"]),
                json!(["1fr"]),
                json!([["content"]]),
                "0",
                "stretch",
            ),
            "blocks": [
                metric_atom(id_suffix(id, "_content"), "plain", source, None)
            ]
        }
    })
}

fn optional_icon_presentation(args: &Map<String, Value>, key: &str) -> Option<Value> {
    let icon = args.get(key)?;
    if icon.is_null() {
        return None;
    }
    if icon.as_str().is_some_and(str::is_empty) {
        return None;
    }
    Some(json!({"icon": icon}))
}

fn status_card_shell_background(icon: Option<&Value>, strip: bool) -> Value {
    // Structured multi-layer background so unresolved icon refs (ops_param_ref)
    // survive rewrite, then resolve to: icon + metric slot fill + cyan L-corners.
    let slot_fill = if strip {
        "url(/workspace-app-assets/templates/cockpit/assets/metrics/metric-bg-long@3x.svg)"
    } else {
        "url(/workspace-app-assets/templates/cockpit/assets/metrics/metric-bg-normal@3x.svg)"
    };
    let mut images = Vec::new();
    let mut sizes = Vec::new();
    let mut positions = Vec::new();
    let mut repeats = Vec::new();
    if let Some(icon) = icon {
        let empty = icon.as_str().is_some_and(str::is_empty);
        if !empty {
            images.push(icon.clone());
            sizes.push(json!("48px 48px"));
            positions.push(json!(if strip { "24px center" } else { "11px center" }));
            repeats.push(json!("no-repeat"));
        }
    }
    images.push(json!(slot_fill));
    sizes.push(json!("100% 100%"));
    positions.push(json!("center"));
    repeats.push(json!("no-repeat"));
    for (pos, _) in [
        ("left top", ()),
        ("right top", ()),
        ("left bottom", ()),
        ("right bottom", ()),
    ] {
        images.push(json!("linear-gradient(#71F1EA,#71F1EA)"));
        sizes.push(json!("4px 2px"));
        positions.push(json!(pos));
        repeats.push(json!("no-repeat"));
    }
    json!({
        "color": "rgba(98,190,235,0.10)",
        "image": images,
        "size": sizes,
        "position": positions,
        "repeat": repeats,
        "origin": "border-box",
        "clip": "border-box",
    })
}

fn status_metric_card_panel(
    id: Value,
    area: &str,
    template: &str,
    source: Value,
    args: &Map<String, Value>,
    icon_key: &str,
    card_props: Value,
) -> Value {
    let icon = optional_icon_presentation(args, icon_key)
        .and_then(|presentation| presentation.get("icon").cloned());
    let strip = template == "strip_icon_left";
    // Shell owns icon + left padding (zhifa icon_left / strip_icon_left geometry).
    // Inner metric_card must not re-apply the same left pad or text collapses to ~12px.
    let mut metric_card_args = json!({
        "id": id_suffix(id.clone(), "_content"),
        "area": "content",
        "template": {
            "__call": template,
            "__args": {}
        },
        "source": source,
        "props": {
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "padding": "0",
            "background": "transparent",
            "border": "none",
            "box_shadow": "none",
        }
    });
    if let Some(extra) = card_props.as_object() {
        if !extra.is_empty() {
            let props = metric_card_args
                .as_object_mut()
                .expect("metric card args")
                .get_mut("props")
                .and_then(Value::as_object_mut)
                .expect("metric card props");
            for (key, value) in extra {
                // Never let callers reintroduce left icon padding on the inner card.
                if key == "padding" {
                    continue;
                }
                props.insert(key.clone(), value.clone());
            }
        }
    }
    let shell_padding = if strip {
        json!("0 16px 0 92px")
    } else {
        json!("10px 8px 10px 70px")
    };
    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": area,
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": {
                "padding": shell_padding,
                "background": status_card_shell_background(icon.as_ref(), strip),
                "border": "none",
                "radius": "4px",
                "width": "100%",
                "height": "100%",
                "min_height": "0",
                "box_sizing": "border-box",
                "overflow": "hidden",
                "box_shadow": "var(--mei-layout-debug-micro-shadow, inset 0 0 0 0 transparent)",
                "__mei_slot_frame_bg": true
            },
            "layout": grid_layout(
                json!(["1fr"]),
                json!(["1fr"]),
                json!([["content"]]),
                "0",
                "stretch",
            ),
            "blocks": [
                json!({
                    "__call": "metric_card",
                    "__args": metric_card_args
                })
            ]
        }
    })
}

fn rewrite_metric_triptych_compound_fill_body(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("metric_triptych_compound"));
    let gap = arg_value(args, "gap", json!("2px"));
    let gap = gap.as_str().unwrap_or("2px");
    let compound_id = id_suffix(id.clone(), "_compound");
    let compound_body_id = id_suffix(id.clone(), "_compound_body");
    let compound_inner = compound_inner_body_panel(compound_body_id, args);

    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": transparent_panel_props(json!("100%")),
            "layout": grid_layout(
                json!(["1fr"]),
                json!(["1fr", "1fr", "1fr", "2.2fr"]),
                json!([["first", "second", "third", "compound"]]),
                gap,
                "stretch",
            ),
            "blocks": [
                narrow_metric_slot_panel(
                    id_suffix(id.clone(), "_first"),
                    "first",
                    arg_value(
                        args,
                        "first",
                        json!({"label": "执法单位", "value": "--", "unit": "个"}),
                    ),
                ),
                narrow_metric_slot_panel(
                    id_suffix(id.clone(), "_second"),
                    "second",
                    arg_value(
                        args,
                        "second",
                        json!({"label": "执法人员", "value": "--", "unit": "人"}),
                    ),
                ),
                narrow_metric_slot_panel(
                    id_suffix(id.clone(), "_third"),
                    "third",
                    arg_value(
                        args,
                        "third",
                        json!({"label": "执法事项", "value": "--", "unit": "项"}),
                    ),
                ),
                json!({
                    "__call": "panel",
                    "__args": {
                        "id": compound_id,
                        "area": "compound",
                        "variant": "container",
                        "show_heading": false,
                        "chrome": "bare",
                        "props": {
                            "padding": "0 4px",
                            "background": slot_stretch_background("metric-bg-target@3x.svg"),
                            "border": "none",
                            "radius": "4px",
                            "width": "100%",
                            "height": "100%",
                            "min_height": "0",
                            "box_sizing": "border-box",
                            "overflow": "hidden",
                            "box_shadow": "var(--mei-layout-debug-micro-shadow, inset 0 0 0 0 transparent)",
                            "__mei_slot_frame_bg": true
                        },
                        "layout": grid_layout(
                            json!(["1fr"]),
                            json!(["1fr"]),
                            json!([["content"]]),
                            "0",
                            "stretch",
                        ),
                        "blocks": [compound_inner]
                    }
                })
            ]
        }
    })
}

fn rewrite_status_triptych_summary_fill_body(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("status_triptych_summary"));
    let gap = arg_value(args, "gap", json!("2px"));
    let gap = gap.as_str().unwrap_or("2px");
    let summary_padding = arg_value(args, "summary_padding", json!("0 16px 0 72px"));

    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": transparent_panel_props(json!("100%")),
            "layout": grid_layout(
                json!(["1fr", "1fr"]),
                json!(["1fr", "1fr", "1fr"]),
                json!([["pending", "doing", "done"], ["summary", "summary", "summary"]]),
                gap,
                "stretch",
            ),
            "blocks": [
                status_metric_card_panel(
                    id_suffix(id.clone(), "_pending"),
                    "pending",
                    "icon_left",
                    arg_value(
                        args,
                        "pending",
                        json!({"label": "待办", "value": "--", "unit": "件"}),
                    ),
                    args,
                    "pending_icon",
                    json!({}),
                ),
                status_metric_card_panel(
                    id_suffix(id.clone(), "_doing"),
                    "doing",
                    "icon_left",
                    arg_value(
                        args,
                        "doing",
                        json!({"label": "在办", "value": "--", "unit": "件"}),
                    ),
                    args,
                    "doing_icon",
                    json!({}),
                ),
                status_metric_card_panel(
                    id_suffix(id.clone(), "_done"),
                    "done",
                    "icon_left",
                    arg_value(
                        args,
                        "done",
                        json!({"label": "已办", "value": "--", "unit": "件"}),
                    ),
                    args,
                    "done_icon",
                    json!({}),
                ),
                status_metric_card_panel(
                    id_suffix(id.clone(), "_summary"),
                    "summary",
                    "strip_icon_left",
                    arg_value(
                        args,
                        "summary",
                        json!({"label": "查实率", "value": "--", "unit": "%"}),
                    ),
                    args,
                    "summary_icon",
                    json!({"padding": summary_padding}),
                ),
            ]
        }
    })
}

fn rewrite_compound_only_fill_body(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("compound_only"));
    let inner_panel = compound_inner_body_panel(id_suffix(id.clone(), "_body"), args);

    json!({
        "__call": "panel",
        "__args": {
            "id": id,
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": {
                "padding": "0 4px",
                "background": slot_stretch_background("metric-bg-target@3x.svg"),
                "border": "none",
                "radius": "4px",
                "width": "100%",
                "height": "100%",
                "min_height": "0",
                "box_sizing": "border-box",
                "overflow": "hidden",
                "__mei_slot_frame_bg": true
            },
            "layout": grid_layout(
                json!(["1fr"]),
                json!(["1fr"]),
                json!([["content"]]),
                "0",
                "stretch",
            ),
            "blocks": [inner_panel]
        }
    })
}

pub fn try_rewrite_biz_macro(value: &Value) -> Option<Value> {
    let call = value.as_object()?.get("__call")?.as_str()?;
    let method = call.rsplit('.').next()?;
    let args = call_args(value)?;
    let rewritten = match method {
        "content_strip_props" => rewrite_content_strip_props(args),
        "content_fill_props" => rewrite_content_fill_props(args),
        "story_opinion_block" => rewrite_story_opinion_block(args),
        "compound_only_fill_body" => rewrite_compound_only_fill_body(args),
        "metric_triptych_compound_fill_body" => rewrite_metric_triptych_compound_fill_body(args),
        "status_triptych_summary_fill_body" => rewrite_status_triptych_summary_fill_body(args),
        "metric_triptych_compound_body" => rewrite_metric_triptych_compound_body(args),
        "wide_metric_compound_body" => rewrite_wide_metric_compound_body(args),
        "primary_progress_triptych_body" => rewrite_primary_progress_triptych_body(args),
        "primary_progress_triptych_fill_body" => rewrite_primary_progress_triptych_fill_body(args),
        "progress_metric_fill_slot" => rewrite_progress_metric_fill_slot(args),
        "long_metric_compound_body" => rewrite_long_metric_compound_body(args),
        "long_metric_compound_fill_body" => rewrite_long_metric_compound_fill_body(args),
        _ => return None,
    };
    // If Merge was constant-folded onto a macro call first, sibling keys such as
    // `__mei_viewpoint` must survive the rewrite (they are overlays, not call args).
    let Some(map) = value.as_object() else {
        return Some(rewritten);
    };
    let mut out = rewritten;
    if let Some(out_map) = out.as_object_mut() {
        for (key, child) in map {
            if key == "__call" || key == "__args" {
                continue;
            }
            out_map.insert(key.clone(), child.clone());
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_content_strip_props_as_fill() {
        let value = json!({
            "__call": "biz.content_strip_props",
            "__args": {
                "row_budgets": [70, 70],
                "gap": "6px"
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        assert_eq!(
            rewritten.get("__mei_layout_fill").and_then(Value::as_bool),
            Some(true)
        );
        assert!(rewritten.get("__mei_content_budget").is_none());
    }

    #[test]
    fn rewrites_content_fill_props_preserves_sibling_overlays() {
        let value = json!({
            "__call": "shell_macros.content_fill_props",
            "__args": {},
            "__mei_viewpoint": "vp_warnings_detail_table",
            "overflow": "auto",
            "padding": "4px"
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        assert_eq!(
            rewritten.get("__mei_viewpoint").and_then(Value::as_str),
            Some("vp_warnings_detail_table")
        );
        assert_eq!(
            rewritten.get("overflow").and_then(Value::as_str),
            Some("auto")
        );
        assert_eq!(
            rewritten.get("padding").and_then(Value::as_str),
            Some("4px")
        );
        assert_eq!(
            rewritten.get("__mei_layout_fill").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn rewrites_content_fill_props() {
        let value = json!({
            "__call": "shell.content_fill_props",
            "__args": {}
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        assert_eq!(
            rewritten.get("__mei_layout_fill").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            rewritten.get("height").and_then(Value::as_str),
            Some("100%")
        );
        assert!(
            rewritten.get("background").is_none(),
            "content_fill_props should not pin background; author merge owns it"
        );
    }

    #[test]
    fn rewrites_story_opinion_block() {
        let value = json!({
            "__call": "biz.story_opinion_block",
            "__args": {
                "title": "湖心亭",
                "body": "<p>test</p>"
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        assert_eq!(
            rewritten.get("__call").and_then(|v| v.as_str()),
            Some("component")
        );
    }

    #[test]
    fn rewrites_metric_triptych_to_panel_with_four_blocks() {
        let value = json!({
            "__call": "biz.metric_triptych_compound_body",
            "__args": {
                "id": "strip",
                "first": { "__call": "panel", "__args": { "id": "a" } },
                "compound": {
                    "__call": "biz.wide_metric_compound_body",
                    "__args": { "id": "compound" }
                }
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        let blocks = rewritten
            .pointer("/__args/blocks")
            .and_then(|v| v.as_array())
            .expect("blocks");
        assert_eq!(blocks.len(), 4);
        assert_eq!(
            blocks[3].get("__call").and_then(|v| v.as_str()),
            Some("biz.wide_metric_compound_body")
        );
    }

    #[test]
    fn rewrites_wide_metric_compound_shell_background() {
        let value = json!({
            "__call": "biz.wide_metric_compound_body",
            "__args": {
                "id": "enforcement_objects_card",
                "shell_padding": "0",
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        let background = rewritten
            .pointer("/__args/props/background")
            .expect("background");
        assert_eq!(
            background.get("color").and_then(|v| v.as_str()),
            Some("rgba(98,190,235,0.10)")
        );
        assert!(
            background
                .get("image")
                .and_then(|v| v.as_str())
                .is_some_and(|value| value.contains("metric-bg-target@3x.svg")),
            "compound shell should include metric-bg-target frame, got {background}"
        );
        assert_eq!(
            background.get("size").and_then(|v| v.as_str()),
            Some("100% 100%")
        );
    }

    #[test]
    fn rewrites_long_metric_compound_shell_background() {
        let value = json!({
            "__call": "biz.long_metric_compound_body",
            "__args": {
                "id": "ai_compound_card",
                "main": { "__call": "metric_card", "__args": { "id": "main" } },
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        let background = rewritten
            .pointer("/__args/props/background")
            .expect("background");
        assert!(
            background
                .get("image")
                .and_then(|v| v.as_str())
                .is_some_and(|value| value.contains("metric-bg-long@3x.svg")),
            "long compound should include metric-bg-long frame, got {background}"
        );
        assert_eq!(
            background.get("origin").and_then(|v| v.as_str()),
            Some("border-box")
        );
        assert_eq!(
            rewritten
                .pointer("/__args/props/height")
                .and_then(|v| v.as_str()),
            Some("100%")
        );
        assert_eq!(
            rewritten
                .pointer("/__args/props/__mei_slot_frame_bg")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn rewrites_long_metric_compound_fill_body_to_three_slots() {
        let value = json!({
            "__call": "biz.long_metric_compound_fill_body",
            "__args": {
                "id": "ai_compound_card",
                "area": "block_ai",
                "main_source": {"__ref": "metric_ref", "__args": {"arg0": "ai_enforcement_recognition_count"}},
                "top_source": {"__ref": "metric_ref", "__args": {"arg0": "law_enforcement_recorder_count"}},
                "bottom_source": {"__ref": "metric_ref", "__args": {"arg0": "playback_duration_hours"}},
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        assert_eq!(
            rewritten.get("__call").and_then(|v| v.as_str()),
            Some("panel")
        );
        assert_eq!(
            rewritten.pointer("/__args/area").and_then(|v| v.as_str()),
            Some("block_ai")
        );
        let blocks = rewritten
            .pointer("/__args/blocks")
            .and_then(|v| v.as_array())
            .expect("blocks");
        assert_eq!(blocks.len(), 3);
        assert_eq!(
            blocks[0].pointer("/__args/area").and_then(|v| v.as_str()),
            Some("main")
        );
        assert_eq!(
            blocks[1].pointer("/__args/area").and_then(|v| v.as_str()),
            Some("rtop")
        );
        assert_eq!(
            blocks[2].pointer("/__args/area").and_then(|v| v.as_str()),
            Some("rbottom")
        );
        let background = rewritten
            .pointer("/__args/props/background")
            .expect("background");
        assert!(
            background
                .get("image")
                .and_then(|v| v.as_str())
                .is_some_and(|value| value.contains("metric-bg-long@3x.svg")),
            "fill long compound should include metric-bg-long frame, got {background}"
        );
    }

    #[test]
    fn rewrites_metric_triptych_compound_fill_body_to_four_slots() {
        let value = json!({
            "__call": "biz.metric_triptych_compound_fill_body",
            "__args": {
                "id": "enforcement_body",
                "area": "content_zone",
                "first": {"__ref": "metric_ref", "__args": {"arg0": "enforcement_units_count"}},
                "top_source": {"__ref": "metric_ref", "__args": {"arg0": "enforcement_objects_count"}},
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        assert_eq!(
            rewritten.get("__call").and_then(|v| v.as_str()),
            Some("panel")
        );
        let blocks = rewritten
            .pointer("/__args/blocks")
            .and_then(|v| v.as_array())
            .expect("blocks");
        assert_eq!(blocks.len(), 4);
        assert!(
            rewritten
                .pointer("/__args/blocks/3/__args/props/__mei_slot_frame_bg")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            "compound slot should carry frame bg flag"
        );
    }

    #[test]
    fn rewrites_status_triptych_summary_fill_body_to_four_cards() {
        let value = json!({
            "__call": "biz.status_triptych_summary_fill_body",
            "__args": {
                "id": "issue_body",
                "area": "content_zone",
                "pending": {"__ref": "metric_ref", "__args": {"arg0": "warnings_pending_count"}},
                "pending_icon": {"__ref": "ops_param_ref", "__args": {"arg0": "issue_icon_pending_css"}},
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        let blocks = rewritten
            .pointer("/__args/blocks")
            .and_then(|v| v.as_array())
            .expect("blocks");
        assert_eq!(blocks.len(), 4);
        assert!(
            rewritten
                .pointer("/__args/blocks/0/__args/blocks/0/__call")
                .and_then(|v| v.as_str())
                == Some("metric_card"),
            "pending card should lower via metric_card template"
        );
        assert!(
            rewritten
                .pointer("/__args/blocks/0/__args/blocks/0/__args/template/__call")
                .and_then(|v| v.as_str())
                == Some("icon_left"),
            "pending card should use icon_left template"
        );
        let pending_bg = rewritten
            .pointer("/__args/blocks/0/__args/props/background")
            .cloned()
            .unwrap_or(json!(null));
        let pending_bg_s = serde_json::to_string(&pending_bg).unwrap_or_default();
        assert!(
            pending_bg_s.contains("issue_icon_pending_css") || pending_bg_s.contains("image"),
            "pending shell should carry icon background image, got {pending_bg_s}"
        );
        assert!(
            pending_bg_s.contains("metric-bg-normal@3x.svg"),
            "pending shell should layer metric slot fill under icon, got {pending_bg_s}"
        );
        assert!(
            pending_bg_s.contains("#71F1EA"),
            "pending shell should keep cyan corner decor, got {pending_bg_s}"
        );
        let summary_bg = rewritten
            .pointer("/__args/blocks/3/__args/props/background")
            .cloned()
            .unwrap_or(json!(null));
        let summary_bg_s = serde_json::to_string(&summary_bg).unwrap_or_default();
        assert!(
            summary_bg_s.contains("metric-bg-long@3x.svg"),
            "summary shell should use long metric slot fill, got {summary_bg_s}"
        );
        assert_eq!(
            rewritten
                .pointer("/__args/layout/__args/areas")
                .and_then(|v| v.as_array())
                .map(|rows| rows.len()),
            Some(2),
            "status triptych should keep 2-row grid areas"
        );
        assert_eq!(
            rewritten
                .pointer("/__args/blocks/0/__args/blocks/0/__args/props/padding")
                .and_then(|v| v.as_str()),
            Some("0"),
            "inner metric_card must not re-apply icon-left padding (shell owns it)"
        );
        assert_eq!(
            rewritten
                .pointer("/__args/blocks/0/__args/props/padding")
                .and_then(|v| v.as_str()),
            Some("10px 8px 10px 70px"),
            "pending shell should keep icon-left padding"
        );
        assert_eq!(
            rewritten
                .pointer("/__args/blocks/3/__args/props/padding")
                .and_then(|v| v.as_str()),
            Some("0 16px 0 92px"),
            "summary shell should keep strip-icon-left padding"
        );
        assert_eq!(
            rewritten
                .pointer("/__args/blocks/3/__args/blocks/0/__args/props/padding")
                .and_then(|v| v.as_str()),
            Some("0"),
            "summary inner card padding must stay zero"
        );
    }

    #[test]
    fn rewrites_inspection_macro_blocks() {
        let no_violation = json!({
            "__call": "biz.primary_progress_triptych_body",
            "__args": {
                "id": "inspection_no_violation_layout",
                "primary": { "__call": "panel", "__args": { "id": "primary" } },
                "first": { "__call": "panel", "__args": { "id": "first" } },
            }
        });
        let rewritten = try_rewrite_biz_macro(&no_violation).expect("rewrite triptych");
        assert_eq!(
            rewritten.get("__call").and_then(|v| v.as_str()),
            Some("panel")
        );
        assert_eq!(
            rewritten
                .pointer("/__args/blocks")
                .and_then(|v| v.as_array())
                .map(|b| b.len()),
            Some(2)
        );

        let no_violation_fill = json!({
            "__call": "biz.primary_progress_triptych_fill_body",
            "__args": {
                "id": "inspection_no_violation_layout",
                "area": "block_no_violation",
                "primary": {
                    "__call": "biz.progress_metric_fill_slot",
                    "__args": {
                        "id": "inspection_no_violation_card",
                        "area": "primary",
                        "desc": "81%",
                        "source": {"label": "无违规", "value": "33994", "unit": "次"}
                    }
                },
                "first": {
                    "__call": "biz.progress_metric_fill_slot",
                    "__args": {
                        "id": "park_rate_a",
                        "area": "first",
                        "source": {"label": "三峡商圈", "value": "36", "unit": "%"}
                    }
                }
            }
        });
        let rewritten_fill =
            try_rewrite_biz_macro(&no_violation_fill).expect("rewrite fill triptych");
        assert_eq!(
            rewritten_fill.get("__call").and_then(|v| v.as_str()),
            Some("panel")
        );
        assert_eq!(
            rewritten_fill
                .pointer("/__args/layout/__args/rows/0")
                .and_then(|v| v.as_str()),
            Some("1fr"),
            "fill triptych should use 1fr row"
        );
        assert_eq!(
            rewritten_fill
                .pointer("/__args/blocks/0/__args/blocks/0/__args/surface")
                .and_then(|v| v.as_str()),
            Some("stack_progress"),
            "nested progress_metric_fill_slot must expand to stack_progress"
        );
        let primary_bg = rewritten_fill
            .pointer("/__args/blocks/0/__args/props/background/image")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            primary_bg.contains("metric-bg-clean@3x.svg"),
            "progress slot should use clean bg, got {primary_bg}"
        );

        let ai_block = json!({
            "__call": "biz.long_metric_compound_body",
            "__args": {
                "id": "ai_compound_card",
                "main": { "__call": "metric_card", "__args": { "id": "main" } },
            }
        });
        let rewritten = try_rewrite_biz_macro(&ai_block).expect("rewrite long compound");
        assert_eq!(
            rewritten.get("__call").and_then(|v| v.as_str()),
            Some("panel")
        );
        assert_eq!(
            rewritten
                .pointer("/__args/blocks")
                .and_then(|v| v.as_array())
                .map(|b| b.len()),
            Some(3)
        );
    }
}
