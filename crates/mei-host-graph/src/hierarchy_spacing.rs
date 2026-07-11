//! 0332 层级边距 / 边框 / 圆角：省略即默认（显式值含 `"0"` 覆盖）。

use mei_lang_kernel::hierarchy_spacing_defaults;
use serde_json::{Map, Value};

const REGION_BORDER_DEFAULT: &str = "none";
const SECTION_BORDER_DEFAULT: &str = "1px solid rgba(52, 82, 108, 0.5)";
const CONTENT_BORDER_DEFAULT: &str = "none";
const RADIUS_DEFAULT: &str = "0";

pub fn apply_hierarchy_spacing_defaults(
    role: &str,
    payload: &mut Map<String, Value>,
    props: &mut Map<String, Value>,
) {
    if let Some(defaults) = hierarchy_spacing_defaults(role) {
        if let Some(gap) = defaults.gap {
            if let Some(layout) = payload.get_mut("layout") {
                ensure_layout_gap(layout, gap);
            }
        }
        if let Some(padding) = defaults.padding {
            ensure_props_padding(props, padding);
            if role == "section" {
                ensure_section_shell_padding_default(payload);
            }
        }
    }
    apply_hierarchy_chrome_defaults(role, payload, props);
}

/// 无 `__mei_ui_role` 的叶子 / 嵌套 panel：按 content（section 内网格）默认注入。
pub fn apply_leaf_content_spacing_defaults(mut layout: Option<&mut Value>, props: &mut Value) {
    let Some(defaults) = hierarchy_spacing_defaults("content") else {
        return;
    };
    if let Some(layout_value) = layout.as_mut() {
        if let Some(gap) = defaults.gap {
            ensure_layout_gap(layout_value, gap);
        }
    }
    let Some(map) = props.as_object_mut() else {
        return;
    };
    if let Some(padding) = defaults.padding {
        ensure_props_padding(map, padding);
    }
    apply_content_fill_defaults(layout, map);
}

fn apply_hierarchy_chrome_defaults(
    role: &str,
    payload: &mut Map<String, Value>,
    props: &mut Map<String, Value>,
) {
    match role {
        "region" => {
            ensure_props_string(props, "radius", RADIUS_DEFAULT);
            ensure_props_string(props, "border", REGION_BORDER_DEFAULT);
        }
        "section" => {
            ensure_props_string(props, "radius", RADIUS_DEFAULT);
            ensure_props_string(props, "border", SECTION_BORDER_DEFAULT);
        }
        "content" => {
            apply_content_fill_defaults(payload.get_mut("layout"), props);
        }
        _ => {}
    }
}

fn apply_content_fill_defaults(layout: Option<&mut Value>, props: &mut Map<String, Value>) {
    ensure_props_string(props, "radius", RADIUS_DEFAULT);
    ensure_props_string(props, "border", CONTENT_BORDER_DEFAULT);
    if !props.contains_key("__mei_layout_fill") {
        props.insert("__mei_layout_fill".to_string(), Value::Bool(true));
    }
    ensure_props_string(props, "width", "100%");
    ensure_props_string(props, "height", "100%");
    ensure_props_string(props, "min_height", "0");
    if let Some(layout) = layout {
        ensure_layout_align_justify(layout, "stretch", "stretch");
    }
}

fn layout_args_mut(layout: &mut Value) -> Option<&mut Map<String, Value>> {
    if layout.get("__args").and_then(Value::as_object).is_some() {
        return layout.get_mut("__args").and_then(Value::as_object_mut);
    }
    layout.as_object_mut()
}

fn spacing_value_missing(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        _ => false,
    }
}

pub fn ensure_layout_gap(layout: &mut Value, default_gap: &str) {
    let Some(args) = layout_args_mut(layout) else {
        return;
    };
    if spacing_value_missing(args.get("gap")) {
        args.insert("gap".to_string(), Value::String(default_gap.to_string()));
    }
}

pub fn ensure_layout_align_justify(layout: &mut Value, align: &str, justify: &str) {
    let Some(args) = layout_args_mut(layout) else {
        return;
    };
    if spacing_value_missing(args.get("align")) {
        args.insert("align".to_string(), Value::String(align.to_string()));
    }
    if spacing_value_missing(args.get("justify")) {
        args.insert("justify".to_string(), Value::String(justify.to_string()));
    }
}

pub fn ensure_props_padding(props: &mut Map<String, Value>, default_padding: &str) {
    let has_profile = props
        .get("__mei_padding_profile")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    if has_profile {
        return;
    }
    if spacing_value_missing(props.get("padding")) {
        props.insert(
            "padding".to_string(),
            Value::String(default_padding.to_string()),
        );
    }
}

fn ensure_props_string(props: &mut Map<String, Value>, key: &str, default: &str) {
    if spacing_value_missing(props.get(key)) {
        props.insert(key.to_string(), Value::String(default.to_string()));
    }
}

fn ensure_section_shell_padding_default(payload: &mut Map<String, Value>) {
    let Some(shell) = payload.get_mut("shell") else {
        return;
    };
    let Some(args) = layout_args_mut(shell) else {
        return;
    };
    let has_profile = args
        .get("padding_profile")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    let has_padding = !spacing_value_missing(args.get("padding"));
    if !has_profile && !has_padding {
        args.insert(
            "padding_profile".to_string(),
            Value::String("space_1".to_string()),
        );
    }
}
