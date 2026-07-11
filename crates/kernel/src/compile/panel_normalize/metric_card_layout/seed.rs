use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::{BlockDecl, UiNodeDecl, UiTreeNode};

use super::super::constants::PROP_METRIC_CARD;
use super::{
    block_has_metric_v_align, block_metric_role, block_metric_v_align,
    slot_vertical_align_prop_key, METRIC_SLOT_ROLES, PROP_MEI_METRIC_DESC_MODE,
    PROP_METRIC_DESC_MODE, PROP_METRIC_DESC_SHELL, PROP_METRIC_V_ALIGN, USE_MEI_TEXT,
    USE_METRIC_PROGRESS, USE_QUNFU_METRIC_TILE,
};

fn overlay_props_has_slot_v_align(overlay_value: &Value, role: &str) -> bool {
    overlay_value
        .get("props")
        .and_then(Value::as_object)
        .and_then(|map| map.get(&slot_vertical_align_prop_key(role)))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn metric_v_align_from_base_block(base: &UiNodeDecl, role: &str) -> Option<String> {
    for node in &base.blocks {
        let UiTreeNode::Block(block) = node else {
            continue;
        };
        if block_metric_role(block) != Some(role) {
            continue;
        }
        return block_metric_v_align(block);
    }
    None
}

fn metric_v_align_defaults_from_base_blocks(base: &UiNodeDecl) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for node in &base.blocks {
        let UiTreeNode::Block(block) = node else {
            continue;
        };
        let Some(role) = block_metric_role(block) else {
            continue;
        };
        if let Some(raw) = block_metric_v_align(block) {
            out.insert(role.to_string(), raw);
        }
    }
    out
}

fn metric_v_align_defaults_from_shell_props(card: &UiNodeDecl) -> BTreeMap<String, String> {
    let Some(shell) = card.props.as_object() else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    for role in METRIC_SLOT_ROLES {
        let Some(raw) = shell
            .get(&slot_vertical_align_prop_key(role))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        out.insert(role.to_string(), raw.to_string());
    }
    out
}

/// `metric_card(base=..., source=...)` 覆写 blocks 时写入各槽 `metric_v_align`。
/// 优先级：槽位显式 `vertical_align` > shell `__mei_metric_*_v_align`（来自 label_vertical_align 等）> 模板 base blocks。
pub(crate) fn seed_metric_block_vertical_align_from_base(
    base: &UiNodeDecl,
    merged: &mut UiNodeDecl,
) {
    if !merged
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_CARD))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return;
    }
    let shell_defaults = metric_v_align_defaults_from_shell_props(merged);
    let base_defaults = metric_v_align_defaults_from_base_blocks(base);
    if shell_defaults.is_empty() && base_defaults.is_empty() {
        return;
    }
    for node in &mut merged.blocks {
        let UiTreeNode::Block(block) = node else {
            continue;
        };
        let Some(role) = block_metric_role(block) else {
            continue;
        };
        if block_has_metric_v_align(block) {
            continue;
        }
        let Some(raw) = shell_defaults.get(role).or_else(|| base_defaults.get(role)) else {
            continue;
        };
        if !block.props.is_object() {
            block.props = Value::Object(Default::default());
        }
        if let Some(block_props) = block.props.as_object_mut() {
            block_props.insert(PROP_METRIC_V_ALIGN.to_string(), Value::String(raw.clone()));
        }
    }
}

fn metric_desc_mode_from_props(props: &Value) -> Option<String> {
    let raw = props
        .as_object()
        .and_then(|map| {
            map.get(PROP_METRIC_DESC_MODE)
                .or_else(|| map.get(PROP_MEI_METRIC_DESC_MODE))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(raw.to_string())
}

fn block_desc_mode(block: &BlockDecl) -> Option<&str> {
    block
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_DESC_MODE))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn block_is_desc_slot(block: &BlockDecl) -> bool {
    block
        .area
        .as_deref()
        .map(str::trim)
        .is_some_and(|area| area == "desc")
        || block
            .props
            .as_object()
            .and_then(|map| map.get("metric_role"))
            .and_then(Value::as_str)
            .map(str::trim)
            == Some("desc")
}

fn ensure_block_props(block: &mut BlockDecl) -> &mut serde_json::Map<String, Value> {
    if !block.props.is_object() {
        block.props = Value::Object(Default::default());
    }
    block.props.as_object_mut().expect("block props object")
}

fn merge_component_props(block: &mut BlockDecl, key: &str, value: Value) {
    let Some(component) = block.component.as_mut() else {
        return;
    };
    let Some(component_obj) = component.as_object_mut() else {
        return;
    };
    let props = component_obj
        .entry("props")
        .or_insert_with(|| Value::Object(Default::default()));
    if !props.is_object() {
        *props = Value::Object(Default::default());
    }
    let Some(props_obj) = props.as_object_mut() else {
        return;
    };
    if !props_obj.contains_key(key) {
        props_obj.insert(key.to_string(), value);
    }
}

fn seed_tile_metric_desc_props(block: &mut BlockDecl, mode: &str, shell: &Value) {
    if block.use_key != USE_QUNFU_METRIC_TILE {
        return;
    }
    if block_desc_mode(block).is_some() {
        return;
    }
    let mode_value = Value::String(mode.to_string());
    let block_props = ensure_block_props(block);
    block_props.insert(PROP_METRIC_DESC_MODE.to_string(), mode_value.clone());
    let shell_for_component = if shell.is_object()
        && shell.as_object().is_some_and(|map| !map.is_empty())
        && !block_props.contains_key(PROP_METRIC_DESC_SHELL)
    {
        block_props.insert(PROP_METRIC_DESC_SHELL.to_string(), shell.clone());
        Some(shell.clone())
    } else {
        None
    };
    merge_component_props(block, PROP_METRIC_DESC_MODE, mode_value);
    if let Some(shell_value) = shell_for_component {
        merge_component_props(block, PROP_METRIC_DESC_SHELL, shell_value);
    }
}

fn promote_desc_text_to_progress(block: &mut BlockDecl, shell: &Value) {
    if block.use_key != USE_MEI_TEXT || !block_is_desc_slot(block) {
        return;
    }
    let Some(content) = block
        .props
        .as_object()
        .and_then(|map| map.get("content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let mut progress_props = serde_json::json!({
        "value": content,
        "metric_role": "desc",
        "align": "center",
    });
    if let Some(v_align) = block
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_V_ALIGN))
        .filter(|value| !value.is_null())
    {
        progress_props[PROP_METRIC_V_ALIGN] = v_align.clone();
    }
    if shell.is_object() && shell.as_object().is_some_and(|map| !map.is_empty()) {
        progress_props["progress_shell"] = shell.clone();
    }
    block.use_key = USE_METRIC_PROGRESS.to_string();
    block.props = progress_props.clone();
    block.component = Some(serde_json::json!({
        "use": USE_METRIC_PROGRESS,
        "pack": "cockpit-default",
        "props": progress_props,
    }));
}

/// 克隆进度模板并覆写 blocks（静态 source / metric_ref tile）时，从 shell props 继承 desc 进度语义。
pub(crate) fn seed_metric_desc_runtime_from_shell(merged: &mut UiNodeDecl) {
    let Some(mode) = metric_desc_mode_from_props(&merged.props) else {
        return;
    };
    if !mode.eq_ignore_ascii_case("progress") {
        return;
    }
    let shell = merged
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_DESC_SHELL))
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    for node in &mut merged.blocks {
        let UiTreeNode::Block(block) = node else {
            continue;
        };
        seed_tile_metric_desc_props(block, &mode, &shell);
        promote_desc_text_to_progress(block, &shell);
    }
}

pub(crate) fn seed_metric_slot_vertical_align_defaults_from_base(
    base: &UiNodeDecl,
    merged: &mut UiNodeDecl,
    overlay_value: &Value,
) {
    if !merged
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_CARD))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return;
    }
    let Some(merged_props) = merged.props.as_object_mut() else {
        return;
    };
    for role in METRIC_SLOT_ROLES {
        if overlay_props_has_slot_v_align(overlay_value, role) {
            continue;
        }
        let key = slot_vertical_align_prop_key(role);
        // 模板 blocks 上的 vertical_align 优先于 props.__mei_metric_*（作者按槽位微调）。
        if let Some(raw) = metric_v_align_from_base_block(base, role) {
            merged_props.insert(key, Value::String(raw));
        }
    }
}
