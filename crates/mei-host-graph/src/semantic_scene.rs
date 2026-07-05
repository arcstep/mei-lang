use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use mei_lang_kernel::{padding_profile_css, FrameDecl, PanelDecl, UiNodeDecl};
use serde_json::{json, Map, Value};

use crate::assemble::assembly_key_to_target;
use crate::import::load_block_artifact;
use crate::mcg::registry::McgRegistry;
use crate::types::GraphNodeKind;
use crate::v2_lower::{
    lower_frame_from_assembly, lower_panel_payload, PanelLowerContext,
};

#[derive(Debug, Clone)]
pub struct SemanticSceneAssembly {
    pub scene_id: String,
    pub summary: Option<String>,
    pub profile: Option<String>,
    pub theme: Option<Value>,
    pub frame: FrameDecl,
    pub panels: Vec<PanelDecl>,
    pub panel_payloads: BTreeMap<String, Value>,
    pub shared: Value,
    pub local_nav: Value,
    pub params: Value,
    pub capabilities: Value,
    pub bindings: Value,
    pub steps: Vec<Value>,
    pub actions: Vec<Value>,
    pub viewpoints: Vec<Value>,
    pub world_payloads: BTreeMap<String, Value>,
}

pub fn has_semantic_scene(registry: &McgRegistry) -> bool {
    registry
        .nodes
        .iter()
        .any(|node| node.id.kind == GraphNodeKind::SemanticGraph)
}

pub fn load_semantic_scene_payload(
    app_root: &Path,
    registry: &McgRegistry,
    assembly_key: &str,
) -> Result<Value> {
    let node = registry
        .nodes
        .iter()
        .find(|node| node.id.kind == GraphNodeKind::SemanticGraph && node.id.key == assembly_key)
        .with_context(|| format!("semantic scene not found: {assembly_key}"))?;
    let pref = node
        .payload_ref
        .as_ref()
        .context("semantic scene missing payload ref")?;
    let artifact = load_block_artifact(app_root, pref)?
        .with_context(|| format!("semantic scene artifact missing for {assembly_key}"))?;
    Ok(artifact.get("payload").cloned().unwrap_or(Value::Null))
}

pub fn collect_world_payloads_from_scene(payload: &Value) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    collect_world_payloads(payload, &mut out);
    out
}

pub fn assemble_semantic_scene(
    payload: &Value,
    ctx: &PanelLowerContext<'_>,
) -> Result<SemanticSceneAssembly> {
    let scene_id = string_field(payload, &["id"])
        .map(str::to_string)
        .or_else(|| {
            payload
                .get("key")
                .and_then(Value::as_str)
                .and_then(|key| key.split('@').next())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "home".to_string());
    let mut panel_payloads = BTreeMap::new();
    let mut panels = Vec::new();
    let mut tier_counters = BTreeMap::<String, u8>::new();

    for plane in scene_array(payload, "planes", "plane_ref", ctx)? {
        let plane_args = call_args(&plane);
        let plane_id = string_field_map(plane_args, &["id", "tier", "name"])
            .map(str::to_string)
            .unwrap_or_else(|| "t1".to_string());
        let tier = string_field_map(plane_args, &["tier", "id"])
            .map(str::to_string)
            .unwrap_or_else(|| plane_id.clone());
        for region in child_nodes(&plane, &["regions", "nodes"], "region_ref", ctx)? {
            let region_payload =
                build_panel_payload(&region, "region", &tier, Some(&plane_id), ctx)?;
            let counter = tier_counters.entry(tier.clone()).or_insert(0);
            let panel_ctx = ctx.with_assembly_stack_order(*counter);
            let lowered = lower_panel_payload(
                &region_payload,
                region_payload
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("region"),
                &panel_ctx,
            )?;
            *counter = counter.saturating_add(1);
            collect_payload_index(&region_payload, &mut panel_payloads, ctx);
            let mut lowered = lowered;
            apply_padding_profile_body_props(&mut lowered);
            panels.push(lowered);
        }
    }

    let mut frame_payload = payload.clone();
    if let Some(obj) = frame_payload.as_object_mut() {
        obj.insert("scene".to_string(), Value::String(scene_id.clone()));
        if obj.get("panels").is_none() {
            obj.insert("panels".to_string(), Value::Array(Vec::new()));
        }
    }
    let frame = lower_frame_from_assembly(&frame_payload);
    let config = payload
        .get("config")
        .and_then(call_args)
        .cloned()
        .unwrap_or_default();

    Ok(SemanticSceneAssembly {
        scene_id,
        summary: string_field(payload, &["summary"]).map(str::to_string),
        profile: string_field(payload, &["profile"]).map(str::to_string),
        theme: payload.get("theme").cloned(),
        frame,
        panels,
        panel_payloads,
        shared: config_value(&config, "shared"),
        local_nav: config_value(&config, "local_nav"),
        params: config_value(&config, "params"),
        capabilities: config_value(&config, "capabilities"),
        bindings: config_value(&config, "bindings"),
        steps: scene_array(payload, "steps", "", ctx)?,
        actions: scene_array(payload, "actions", "", ctx)?,
        viewpoints: scene_array(payload, "viewpoints", "", ctx)?,
        world_payloads: collect_world_payloads_from_scene(payload),
    })
}

fn build_panel_payload(
    value: &Value,
    role: &str,
    tier: &str,
    plane_id: Option<&str>,
    ctx: &PanelLowerContext<'_>,
) -> Result<Value> {
    let args = call_args(value);
    let id = string_field_map(args, &["id"])
        .map(str::to_string)
        .with_context(|| format!("semantic `{role}` is missing `id`"))?;
    let mut payload = Map::new();
    payload.insert("id".to_string(), Value::String(id.clone()));
    payload.insert("tier".to_string(), Value::String(tier.to_string()));
    copy_if_present(args, &mut payload, "title");
    copy_if_present(args, &mut payload, "area");
    copy_if_present(args, &mut payload, "layout");
    copy_if_present(args, &mut payload, "placement");
    copy_if_present(args, &mut payload, "shell");
    copy_if_present(args, &mut payload, "head_props");
    copy_if_present(args, &mut payload, "body_props");
    if let Some(chrome_role) = string_field_map(args, &["chrome_role"]) {
        payload.insert("chrome_role".to_string(), Value::String(chrome_role.to_string()));
    }
    let mut props = args
        .and_then(|map| map.get("props"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    props.insert("__mei_ui_role".to_string(), Value::String(role.to_string()));
    props.insert("__mei_tier".to_string(), Value::String(tier.to_string()));
    if let Some(plane_id) = plane_id {
        props.insert("__mei_plane_id".to_string(), Value::String(plane_id.to_string()));
    }
    if let Some(chrome_role) = string_field_map(args, &["chrome_role"]) {
        props.insert(
            "__mei_chrome_role".to_string(),
            Value::String(chrome_role.to_string()),
        );
    }
    if let Some(content_kind) = string_field_map(args, &["content_kind", "kind"]) {
        if role == "content" {
            props.insert(
                "__mei_content_kind".to_string(),
                Value::String(content_kind.to_string()),
            );
        }
    }
    if let Some(budget_args) = args
        .and_then(|map| map.get("budget"))
        .and_then(call_args)
        .cloned()
    {
        insert_budget_props(&mut props, &budget_args);
    }
    payload.insert("props".to_string(), Value::Object(props));

    let has_shell = args.and_then(|map| map.get("shell")).is_some();
    let mut blocks = Vec::new();
    if !has_shell {
        match role {
            "region" => {
                for section in child_nodes(value, &["sections"], "section_ref", ctx)? {
                    blocks.push(panel_call(build_panel_payload(
                        &section,
                        "section",
                        tier,
                        plane_id,
                        ctx,
                    )?));
                }
                for content in child_nodes(value, &["contents", "content"], "", ctx)? {
                    blocks.push(panel_call(build_panel_payload(
                        &content,
                        "content",
                        tier,
                        plane_id,
                        ctx,
                    )?));
                }
                if blocks.is_empty() {
                    blocks.extend(collect_leaf_blocks(args)?);
                }
            }
            "section" => {
                for content in child_nodes(value, &["contents", "content"], "", ctx)? {
                    blocks.push(panel_call(build_panel_payload(
                        &content,
                        "content",
                        tier,
                        plane_id,
                        ctx,
                    )?));
                }
                if blocks.is_empty() {
                    blocks.extend(collect_leaf_blocks(args)?);
                }
            }
            "content" => {
                blocks.extend(collect_leaf_blocks(args)?);
            }
            _ => {}
        }
    }
    payload.insert("blocks".to_string(), Value::Array(blocks));
    Ok(Value::Object(payload))
}

fn collect_leaf_blocks(args: Option<&Map<String, Value>>) -> Result<Vec<Value>> {
    let Some(args) = args else {
        return Ok(Vec::new());
    };
    if let Some(source) = args.get("source") {
        return Ok(vec![normalize_source_node(source.clone())]);
    }
    if let Some(blocks) = args.get("blocks").and_then(Value::as_array) {
        return Ok(blocks.iter().cloned().map(normalize_source_node).collect());
    }
    if let Some(block) = args.get("block") {
        return Ok(vec![normalize_source_node(block.clone())]);
    }
    if let Some(component) = args.get("component") {
        return Ok(vec![normalize_source_node(component.clone())]);
    }
    Ok(Vec::new())
}

fn normalize_source_node(value: Value) -> Value {
    value
}

fn panel_call(payload: Value) -> Value {
    json!({
        "__call": "panel",
        "__args": payload
    })
}

fn call_args(value: &Value) -> Option<&Map<String, Value>> {
    if let Some(args) = value.get("__args").and_then(Value::as_object) {
        return Some(args);
    }
    value.as_object()
}

fn scene_array(
    payload: &Value,
    key: &str,
    expected_ref: &str,
    ctx: &PanelLowerContext<'_>,
) -> Result<Vec<Value>> {
    let values = payload
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    resolve_semantic_nodes(values, expected_ref, ctx)
}

fn child_nodes(
    value: &Value,
    keys: &[&str],
    expected_ref: &str,
    ctx: &PanelLowerContext<'_>,
) -> Result<Vec<Value>> {
    let args = call_args(value);
    for key in keys {
        if let Some(items) = args.and_then(|map| map.get(*key)).and_then(Value::as_array) {
            return resolve_semantic_nodes(items.clone(), expected_ref, ctx);
        }
    }
    Ok(Vec::new())
}

fn resolve_semantic_nodes(
    values: Vec<Value>,
    expected_ref: &str,
    ctx: &PanelLowerContext<'_>,
) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for value in values {
        if !expected_ref.is_empty() && v2_ref_name(&value) == Some(expected_ref) {
            let ref_key = v2_ref_arg0(&value)
                .with_context(|| format!("{expected_ref} missing arg0"))?;
            out.push(load_semantic_fragment_payload(ctx, ref_key.as_str(), expected_ref)?);
        } else {
            out.push(value);
        }
    }
    Ok(out)
}

fn load_semantic_fragment_payload(
    ctx: &PanelLowerContext<'_>,
    ref_key: &str,
    expected_ref: &str,
) -> Result<Value> {
    let expected_kind = match expected_ref {
        "plane_ref" => "plane_layout",
        "region_ref" => "region_layout",
        "section_ref" => "section_layout",
        other => {
            anyhow::bail!("unsupported semantic ref `{other}`");
        }
    };
    let normalized = ref_key.trim();
    let node = ctx
        .registry
        .nodes
        .iter()
        .find(|node| {
            node.id.kind == GraphNodeKind::SemanticGraph
                && (node.id.key == normalized
                    || node.id.key == format!("{expected_kind}:{normalized}")
                    || node.id.stable_key() == normalized)
        })
        .with_context(|| format!("semantic fragment not found: {expected_kind}:{normalized}"))?;
    let pref = node
        .payload_ref
        .as_ref()
        .with_context(|| format!("semantic fragment missing payload ref: {}", node.id.key))?;
    let artifact = load_block_artifact(ctx.app_root, pref)?
        .with_context(|| format!("semantic fragment artifact missing: {}", node.id.key))?;
    let kind = artifact
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind != expected_kind {
        anyhow::bail!(
            "semantic ref `{normalized}` expected `{expected_kind}`, got `{kind}`"
        );
    }
    Ok(artifact.get("payload").cloned().unwrap_or(Value::Null))
}

fn v2_ref_name(value: &Value) -> Option<&str> {
    value
        .get("__ref")
        .and_then(Value::as_str)
        .or_else(|| value.get("__call").and_then(Value::as_str))
}

fn v2_ref_arg0(value: &Value) -> Option<String> {
    value
        .get("__args")
        .and_then(|args| args.get("arg0"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    value
        .as_object()
        .and_then(|obj| string_field_map(Some(obj), keys))
}

fn string_field_map<'a>(map: Option<&'a Map<String, Value>>, keys: &[&str]) -> Option<&'a str> {
    let map = map?;
    keys.iter()
        .filter_map(|key| map.get(*key))
        .find_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn copy_if_present(args: Option<&Map<String, Value>>, out: &mut Map<String, Value>, key: &str) {
    if let Some(value) = args.and_then(|map| map.get(key)).cloned() {
        out.insert(key.to_string(), value);
    }
}

fn config_value(config: &Map<String, Value>, key: &str) -> Value {
    config.get(key).cloned().unwrap_or_else(|| json!({}))
}

fn insert_budget_props(props: &mut Map<String, Value>, budget: &Map<String, Value>) {
    let mut content_budget = Map::new();
    if let Some(rows) = budget.get("rows") {
        content_budget.insert("rows".to_string(), rows.clone());
    }
    if let Some(gap) = budget.get("gap") {
        content_budget.insert("gap".to_string(), gap.clone());
    }
    if !content_budget.is_empty() {
        props.insert(
            "__mei_content_budget".to_string(),
            Value::Object(content_budget),
        );
    }
    if let Some(value) = budget.get("padding_profile") {
        props.insert("__mei_padding_profile".to_string(), value.clone());
    }
    if let Some(value) = budget.get("section_derived_height_px") {
        props.insert("__mei_section_derived_height_px".to_string(), value.clone());
    }
    for key in ["padding", "width", "min_width", "max_width", "card_height"] {
        if let Some(value) = budget.get(key) {
            props.insert(key.to_string(), value.clone());
        }
    }
}

fn collect_payload_index(
    payload: &Value,
    out: &mut BTreeMap<String, Value>,
    ctx: &PanelLowerContext<'_>,
) {
    let Some(obj) = payload.as_object() else {
        return;
    };
    if let Some(id) = obj.get("id").and_then(Value::as_str) {
        out.insert(id.to_string(), payload.clone());
    }
    if let Some(blocks) = obj.get("blocks").and_then(Value::as_array) {
        for block in blocks {
            if block.get("__call").and_then(Value::as_str) == Some("panel") {
                if let Some(args) = block.get("__args") {
                    collect_payload_index(args, out, ctx);
                }
                continue;
            }
            if block.get("__ref").and_then(Value::as_str) == Some("panel_ref") {
                if let Some(ref_key) = block
                    .get("__args")
                    .and_then(|args| args.get("arg0"))
                    .and_then(Value::as_str)
                {
                    if let Ok(payload) = crate::v2_lower::load_panel_contract_payload(ctx, ref_key) {
                        collect_payload_index(&payload, out, ctx);
                    }
                }
            }
        }
    }
}

fn collect_world_payloads(value: &Value, out: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(map) => {
            if map.get("__call").and_then(Value::as_str) == Some("world") {
                let args = map.get("__args").cloned().unwrap_or(Value::Null);
                let world_id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| "world".to_string());
                out.insert(world_id, args);
                return;
            }
            for value in map.values() {
                collect_world_payloads(value, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_world_payloads(item, out);
            }
        }
        _ => {}
    }
}

fn apply_padding_profile_body_props(panel: &mut PanelDecl) {
    if let Some(profile) = panel
        .props
        .get("__mei_padding_profile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(padding) = padding_profile_css(profile) {
            let mut body_props = panel.body_props.as_object().cloned().unwrap_or_default();
            body_props
                .entry("padding".to_string())
                .or_insert_with(|| Value::String(padding.to_string()));
            body_props
                .entry("box_sizing".to_string())
                .or_insert_with(|| Value::String("border-box".to_string()));
            body_props
                .entry("min_height".to_string())
                .or_insert_with(|| Value::String("0".to_string()));
            panel.body_props = Value::Object(body_props);
        }
    }
    for block in &mut panel.blocks {
        if let UiNodeDecl::Panel(nested) = block {
            apply_padding_profile_body_props(nested);
        }
    }
}

pub fn target_key_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("key")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            let scene_id = payload.get("id").and_then(Value::as_str)?;
            let source_file = payload.get("source_file").and_then(Value::as_str)?;
            Some(format!("{scene_id}@{source_file}"))
        })
}

pub fn default_target_for_scene(scene_id: &str) -> String {
    assembly_key_to_target(&format!("{scene_id}@src/scene/{scene_id}/assembly.mei"))
}
