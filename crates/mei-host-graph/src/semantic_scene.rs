use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use mei_lang_kernel::{
    hierarchy_spacing_defaults, padding_profile_css, FrameDecl, UiNodeDecl, UiTreeNode,
};
use serde_json::{json, Map, Value};

use crate::assemble::assembly_key_to_target;
use crate::hierarchy_spacing::{
    apply_hierarchy_spacing_defaults, ensure_layout_gap, ensure_props_padding,
};
use crate::import::load_block_artifact;
use crate::mcg::registry::McgRegistry;
use crate::types::GraphNodeKind;
use crate::v2_lower::{
    load_content_panel_payload, lower_frame_from_assembly, lower_layout, lower_panel_payload,
    PanelLowerContext,
};

#[derive(Debug, Clone)]
pub struct SemanticSceneAssembly {
    pub scene_id: String,
    pub summary: Option<String>,
    pub profile: Option<String>,
    pub theme: Option<Value>,
    pub default_script: Option<Value>,
    pub frame: FrameDecl,
    pub panels: Vec<UiNodeDecl>,
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
        let plane_grid = plane_args.and_then(|map| map.get("layout"));
        let slides = child_nodes(&plane, &["slides"], "slide_ref", ctx)?;
        if !slides.is_empty() {
            if plane_args
                .and_then(|map| map.get("regions"))
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty())
            {
                anyhow::bail!(
                    "presentation plane `{plane_id}` must use slides = [slide_ref(...)]; regions are not allowed"
                );
            }
            let mut plane_children = Vec::new();
            for slide in slides {
                let slide_payload =
                    build_panel_payload(&slide, "slide", &tier, Some(&plane_id), ctx)?;
                let counter = tier_counters.entry(tier.clone()).or_insert(0);
                let panel_ctx = ctx.with_assembly_stack_order(*counter);
                let lowered = lower_panel_payload(
                    &slide_payload,
                    slide_payload
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("slide"),
                    &panel_ctx,
                )?;
                *counter = counter.saturating_add(1);
                collect_payload_index(&slide_payload, &mut panel_payloads, ctx);
                let mut lowered = lowered;
                apply_padding_profile_body_props(&mut lowered);
                plane_children.push(lowered);
            }
            if let Some(grid) = plane_grid {
                let mut plane_panel = build_plane_grid_panel(
                    plane_id.as_str(),
                    tier.as_str(),
                    Some(grid),
                    plane_args,
                    plane_children,
                )?;
                apply_padding_profile_body_props(&mut plane_panel);
                panels.push(plane_panel);
            } else {
                panels.extend(plane_children);
            }
            continue;
        }
        let regions = child_nodes(&plane, &["regions", "nodes"], "region_ref", ctx)?;
        if plane_grid.is_some() {
            let mut grid_regions = Vec::new();
            let mut overlay_regions = Vec::new();
            for region in regions {
                let region_args = call_args(&region);
                if is_plane_grid_overlay_region(region_args) {
                    overlay_regions.push(region);
                } else {
                    grid_regions.push(region);
                }
            }
            let mut plane_children = Vec::new();
            for region in grid_regions {
                let region_payload =
                    build_plane_grid_region_payload(&region, &tier, plane_id.as_str(), ctx)?;
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
                plane_children.push(lowered);
            }
            if !plane_children.is_empty() {
                let mut plane_panel = build_plane_grid_panel(
                    plane_id.as_str(),
                    tier.as_str(),
                    plane_grid,
                    plane_args,
                    plane_children,
                )?;
                apply_padding_profile_body_props(&mut plane_panel);
                panels.push(plane_panel);
            }
            for region in overlay_regions {
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
            continue;
        }
        for region in regions {
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

    enrich_panel_payloads_from_tree(&panels, &mut panel_payloads, ctx);

    Ok(SemanticSceneAssembly {
        scene_id,
        summary: string_field(payload, &["summary"]).map(str::to_string),
        profile: string_field(payload, &["profile"]).map(str::to_string),
        theme: payload.get("theme").cloned(),
        default_script: payload.get("default_script").cloned(),
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
    if role == "slide" {
        copy_if_present(args, &mut payload, "pattern");
        copy_if_present(args, &mut payload, "chapter");
        copy_if_present(args, &mut payload, "viewpoints");
    }
    if let Some(chrome_role) = string_field_map(args, &["chrome_role"]) {
        payload.insert(
            "chrome_role".to_string(),
            Value::String(chrome_role.to_string()),
        );
    }
    let mut props = args
        .and_then(|map| map.get("props"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    props.insert("__mei_ui_role".to_string(), Value::String(role.to_string()));
    props.insert("__mei_tier".to_string(), Value::String(tier.to_string()));
    if role == "slide" {
        if let Some(pattern) = string_field_map(args, &["pattern"]) {
            props.insert(
                "__mei_slide_pattern".to_string(),
                Value::String(pattern.to_string()),
            );
        }
        if let Some(chapter) = string_field_map(args, &["chapter"]) {
            props.insert(
                "__mei_slide_chapter".to_string(),
                Value::String(chapter.to_string()),
            );
        }
        if let Some(title) = string_field_map(args, &["title"]) {
            props.insert(
                "__mei_slide_title".to_string(),
                Value::String(title.to_string()),
            );
        }
    }
    if let Some(plane_id) = plane_id {
        props.insert(
            "__mei_plane_id".to_string(),
            Value::String(plane_id.to_string()),
        );
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
    apply_hierarchy_spacing_defaults(role, &mut payload, &mut props);
    payload.insert("props".to_string(), Value::Object(props));

    let has_shell = args.and_then(|map| map.get("shell")).is_some();
    let mut blocks = Vec::new();
    if !has_shell {
        match role {
            "slide" => {
                if !child_nodes(value, &["contents", "content"], "", ctx)?.is_empty() {
                    anyhow::bail!(
                        "slide `{id}` must use region_ref children; direct content is not allowed"
                    );
                }
                if args
                    .and_then(|map| map.get("blocks"))
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty())
                {
                    anyhow::bail!(
                        "slide `{id}` must use region_ref children; direct blocks are not allowed"
                    );
                }
                for region in child_nodes(value, &["regions"], "region_ref", ctx)? {
                    blocks.push(panel_call(build_panel_payload(
                        &region, "region", tier, plane_id, ctx,
                    )?));
                }
                if blocks.is_empty() {
                    anyhow::bail!("slide `{id}` must declare at least one region_ref child");
                }
            }
            "region" => {
                if !child_nodes(value, &["contents", "content"], "", ctx)?.is_empty() {
                    anyhow::bail!(
                        "region `{id}` must use section_ref children; direct content is not allowed"
                    );
                }
                if args
                    .and_then(|map| map.get("blocks"))
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty())
                {
                    anyhow::bail!(
                        "region `{id}` must use section_ref children; direct blocks are not allowed"
                    );
                }
                for section in child_nodes(value, &["sections"], "section_ref", ctx)? {
                    blocks.push(panel_call(build_panel_payload(
                        &section, "section", tier, plane_id, ctx,
                    )?));
                }
                if blocks.is_empty() && !region_allows_empty_sections(args) {
                    anyhow::bail!("region `{id}` must declare at least one section_ref child");
                }
            }
            "section" => {
                for content in child_nodes(value, &["contents", "content"], "", ctx)? {
                    blocks.push(panel_call(build_panel_payload(
                        &content, "content", tier, plane_id, ctx,
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
            let ref_key =
                v2_ref_arg0(&value).with_context(|| format!("{expected_ref} missing arg0"))?;
            out.push(load_semantic_fragment_payload(
                ctx,
                ref_key.as_str(),
                expected_ref,
            )?);
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
        "slide_ref" => "slide_layout",
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
        anyhow::bail!("semantic ref `{normalized}` expected `{expected_kind}`, got `{kind}`");
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

fn region_allows_empty_sections(args: Option<&Map<String, Value>>) -> bool {
    let Some(args) = args else {
        return false;
    };
    string_field_map(Some(args), &["id"]) == Some("stage_aperture_frame")
        || string_field_map(Some(args), &["chrome_role"]) == Some("stage_aperture")
}

fn config_value(config: &Map<String, Value>, key: &str) -> Value {
    config.get(key).cloned().unwrap_or_else(|| json!({}))
}

fn insert_budget_props(props: &mut Map<String, Value>, budget: &Map<String, Value>) {
    // content-budget / card_height height path deleted; keep non-height presentation knobs only.
    if let Some(value) = budget.get("padding_profile") {
        props.insert("__mei_padding_profile".to_string(), value.clone());
    }
    for key in ["padding", "width", "min_width", "max_width"] {
        if let Some(value) = budget.get(key) {
            props.insert(key.to_string(), value.clone());
        }
    }
}

fn enrich_panel_payloads_from_tree(
    panels: &[UiNodeDecl],
    out: &mut BTreeMap<String, Value>,
    ctx: &PanelLowerContext<'_>,
) {
    for panel in crate::layer_plan::flatten_panel_tree(panels) {
        for ref_key in [
            format!("home:{}", panel.id),
            format!("content/{}", panel.id),
            panel.id.clone(),
        ] {
            if let Ok(payload) = load_content_panel_payload(ctx, ref_key.as_str()) {
                out.insert(panel.id.clone(), payload);
                break;
            }
        }
    }
}

fn collect_payload_index(
    payload: &Value,
    out: &mut BTreeMap<String, Value>,
    ctx: &PanelLowerContext<'_>,
) {
    let payload = if payload.get("__call").and_then(Value::as_str) == Some("content_panel") {
        payload.get("__args").unwrap_or(payload)
    } else {
        payload
    };
    let Some(obj) = payload.as_object() else {
        return;
    };
    if let Some(id) = obj.get("id").and_then(Value::as_str) {
        out.insert(id.to_string(), payload.clone());
    }
    if let Some(shell) = obj.get("shell") {
        collect_payload_index(shell, out, ctx);
    }
    if let Some(sections) = obj.get("sections").and_then(Value::as_array) {
        for section in sections {
            if v2_ref_name(section) == Some("section_ref") {
                if let Some(ref_key) = v2_ref_arg0(section) {
                    if let Ok(payload) =
                        load_semantic_fragment_payload(ctx, ref_key.as_str(), "section_ref")
                    {
                        collect_payload_index(&payload, out, ctx);
                    }
                }
            }
        }
    }
    if let Some(blocks) = obj.get("blocks").and_then(Value::as_array) {
        for block in blocks {
            if block.get("__call").and_then(Value::as_str) == Some("panel") {
                if let Some(args) = block.get("__args") {
                    collect_payload_index(args, out, ctx);
                }
                continue;
            }
            if v2_ref_name(block) == Some("panel_ref") {
                if let Some(ref_key) = v2_ref_arg0(block) {
                    if let Ok(payload) = load_content_panel_payload(ctx, ref_key.as_str()) {
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

fn apply_padding_profile_body_props(panel: &mut UiNodeDecl) {
    let explicit = panel
        .props
        .get("padding")
        .cloned()
        .filter(|value| value.as_str().is_some_and(|s| !s.trim().is_empty()));
    let from_profile = panel
        .props
        .get("__mei_padding_profile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(padding_profile_css)
        .map(|padding| Value::String(padding.to_string()));
    if let Some(padding) = explicit.or(from_profile) {
        let mut body_props = panel.body_props.as_object().cloned().unwrap_or_default();
        body_props.entry("padding".to_string()).or_insert(padding);
        body_props
            .entry("box_sizing".to_string())
            .or_insert_with(|| Value::String("border-box".to_string()));
        body_props
            .entry("min_height".to_string())
            .or_insert_with(|| Value::String("0".to_string()));
        panel.body_props = Value::Object(body_props);
    }
    for block in &mut panel.blocks {
        if let UiTreeNode::Panel(nested) = block {
            apply_padding_profile_body_props(nested);
        }
    }
}

fn is_plane_grid_overlay_region(args: Option<&serde_json::Map<String, Value>>) -> bool {
    let Some(args) = args else {
        return false;
    };
    if matches!(
        string_field_map(Some(args), &["chrome_role"]),
        Some("float_dock") | Some("viewport_frame") | Some("stage_aperture")
    ) {
        return true;
    }
    false
}

fn grid_area_name_for_region(args: Option<&serde_json::Map<String, Value>>) -> Option<String> {
    let id = string_field_map(args, &["id"])?;
    if let Some(area) = string_field_map(args, &["area"]) {
        if area != "content_zone" {
            return Some(area.to_string());
        }
    }
    Some(match id {
        "home_header" => "header".to_string(),
        other => other.to_string(),
    })
}

fn build_plane_grid_region_payload(
    region: &Value,
    tier: &str,
    plane_id: &str,
    ctx: &PanelLowerContext<'_>,
) -> Result<Value> {
    let area = grid_area_name_for_region(call_args(region));
    let mut region_value = region.clone();
    if let Some(obj) = region_value.as_object_mut() {
        obj.remove("placement");
        if let Some(area) = area {
            obj.insert("area".to_string(), Value::String(area));
        }
    }
    build_panel_payload(&region_value, "region", tier, Some(plane_id), ctx)
}

fn build_plane_grid_panel(
    plane_id: &str,
    tier: &str,
    plane_grid: Option<&Value>,
    plane_args: Option<&Map<String, Value>>,
    children: Vec<UiNodeDecl>,
) -> Result<UiNodeDecl> {
    let mut layout_value = plane_grid.cloned();
    if let Some(layout) = layout_value.as_mut() {
        if let Some(defaults) = hierarchy_spacing_defaults("plane") {
            if let Some(gap) = defaults.gap {
                ensure_layout_gap(layout, gap);
            }
        }
    }
    let mut props = json!({
        "__mei_ui_role": "plane",
        "__mei_tier": tier,
        "__mei_plane_id": plane_id,
        "width": "100%",
        "height": "100%",
        "min_height": "0",
        "box_sizing": "border-box",
        "overflow": "hidden",
    });
    if let Some(props_map) = props.as_object_mut() {
        if let Some(author_props) = plane_args
            .and_then(|map| map.get("props"))
            .and_then(Value::as_object)
        {
            for (key, value) in author_props {
                props_map
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
        if let Some(budget_args) = plane_args
            .and_then(|map| map.get("budget"))
            .and_then(call_args)
        {
            insert_budget_props(props_map, budget_args);
        }
        if let Some(defaults) = hierarchy_spacing_defaults("plane") {
            if let Some(padding) = defaults.padding {
                ensure_props_padding(props_map, padding);
            }
        }
    }
    Ok(UiNodeDecl {
        kind: "panel".to_string(),
        id: plane_id.to_string(),
        title: None,
        head: None,
        area: None,
        layout: layout_value.as_ref().and_then(lower_layout),
        blocks: children.into_iter().map(UiTreeNode::Panel).collect(),
        slot: None,
        props,
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    })
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

pub fn default_target_for_scene(app_root: &Path, scene_id: &str) -> String {
    assembly_key_to_target(&mei_lang_kernel::default_scene_assembly_key(
        app_root, scene_id,
    ))
}

#[cfg(test)]
mod plane_grid_overlay_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn is_plane_grid_overlay_region_includes_mini_park_chrome_roles() {
        let stage = json!({"chrome_role": "stage_aperture"});
        let viewport = json!({"chrome_role": "viewport_frame"});
        let float_dock = json!({"chrome_role": "float_dock"});
        let rail = json!({"chrome_role": "rail"});
        assert!(is_plane_grid_overlay_region(stage.as_object()));
        assert!(is_plane_grid_overlay_region(viewport.as_object()));
        assert!(is_plane_grid_overlay_region(float_dock.as_object()));
        assert!(!is_plane_grid_overlay_region(rail.as_object()));
    }

    #[test]
    fn hierarchy_spacing_injects_when_gap_and_padding_omitted() {
        let mut payload = Map::new();
        payload.insert(
            "layout".to_string(),
            json!({"__call": "grid", "__args": {"rows": ["1fr"]}}),
        );
        let mut props = Map::new();
        apply_hierarchy_spacing_defaults("region", &mut payload, &mut props);
        assert_eq!(
            payload["layout"]["__args"]["gap"].as_str(),
            Some(mei_lang_kernel::HIERARCHY_SECTION_OUTER)
        );
        assert_eq!(
            props["padding"].as_str(),
            Some(mei_lang_kernel::HIERARCHY_PX_1)
        );
        assert_eq!(props["radius"].as_str(), Some("0"));
        assert_eq!(props["border"].as_str(), Some("none"));
    }

    #[test]
    fn hierarchy_spacing_respects_explicit_zero_gap() {
        let mut payload = Map::new();
        payload.insert(
            "layout".to_string(),
            json!({"__call": "grid", "__args": {"gap": "0", "rows": ["1fr"]}}),
        );
        let mut props = Map::new();
        props.insert("padding".to_string(), json!("0"));
        apply_hierarchy_spacing_defaults("section", &mut payload, &mut props);
        assert_eq!(payload["layout"]["__args"]["gap"].as_str(), Some("0"));
        assert_eq!(props["padding"].as_str(), Some("0"));
    }

    #[test]
    fn is_plane_grid_overlay_region_rejects_unknown_chrome_role() {
        let region = json!({
            "chrome_role": "overlay",
            "placement": {"top": "0", "left": "0", "width": "0px", "height": "0px"}
        });
        assert!(!is_plane_grid_overlay_region(region.as_object()));
    }
}
