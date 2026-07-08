//! Runtime rewrites for `biz.*` template macros left unexpanded in panel_contract artifacts.
//!
//! The mei-compiler expand pass should inline these at compile time; when they remain in
//! stored JSON, `v2_lower` must expand them before lowering to `PanelDecl` / `BlockDecl`.

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
    json!({
        "__call": "grid",
        "__args": {
            "rows": rows,
            "columns": columns,
            "areas": areas,
            "gap": gap,
            "align": align,
            "justify": "center"
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
    let row_height = arg_value(args, "row_height", json!("auto"));
    json!({
        "__call": "panel",
        "__args": {
            "id": arg_value(args, "id", json!("metric_triptych_compound")),
            "area": arg_value(args, "area", json!("auto")),
            "variant": "container",
            "show_heading": false,
            "chrome": "bare",
            "props": transparent_panel_props(json!("auto")),
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
                arg_value(args, "row_align", json!("start")).as_str().unwrap_or("start"),
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
    json!({
        "__call": "panel_contract",
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
                "gap": gap,
                "__mei_slot_frame_bg": true
            },
            "layout": grid_layout(
                json!(["minmax(0, 1fr)", "minmax(0, 1fr)"]),
                json!(["1.05fr", "1.95fr"]),
                json!([["main", "rtop"], ["main", "rbottom"]]),
                gap,
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

fn rewrite_wide_metric_compound_body(args: &Map<String, Value>) -> Value {
    let id = arg_value(args, "id", json!("wide_metric_compound"));
    let id_body = id_suffix(id.clone(), "_body");
    let top_band_fr = args
        .get("top_band_fr")
        .and_then(Value::as_f64)
        .or_else(|| args.get("top_band_fr").and_then(Value::as_i64).map(|n| n as f64))
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
                "__call": "panel_contract",
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

fn rewrite_content_strip_props(args: &Map<String, Value>) -> Value {
    let row_budgets = arg_value(args, "row_budgets", json!([]));
    let gap = arg_value(args, "gap", json!("0"));
    let mut props = transparent_panel_props(json!("100%"));
    if let Some(obj) = props.as_object_mut() {
        obj.insert(
            "__mei_content_budget".to_string(),
            json!({ "rows": row_budgets, "gap": gap }),
        );
    }
    props
}

pub fn try_rewrite_biz_macro(value: &Value) -> Option<Value> {
    let call = value.as_object()?.get("__call")?.as_str()?;
    let method = call.rsplit('.').next()?;
    let args = call_args(value)?;
    let rewritten = match method {
        "content_strip_props" => rewrite_content_strip_props(args),
        "content_fill_props" => rewrite_content_fill_props(args),
        "story_opinion_block" => rewrite_story_opinion_block(args),
        "metric_triptych_compound_body" => rewrite_metric_triptych_compound_body(args),
        "wide_metric_compound_body" => rewrite_wide_metric_compound_body(args),
        "primary_progress_triptych_body" => rewrite_primary_progress_triptych_body(args),
        "long_metric_compound_body" => rewrite_long_metric_compound_body(args),
        _ => return None,
    };
    Some(rewritten)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_content_strip_props_budget() {
        let value = json!({
            "__call": "biz.content_strip_props",
            "__args": {
                "row_budgets": [70, 70],
                "gap": "6px"
            }
        });
        let rewritten = try_rewrite_biz_macro(&value).expect("rewrite");
        assert_eq!(
            rewritten
                .get("__mei_content_budget")
                .and_then(|b| b.get("rows"))
                .and_then(|rows| rows.as_array())
                .map(|rows| rows.len()),
            Some(2)
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
