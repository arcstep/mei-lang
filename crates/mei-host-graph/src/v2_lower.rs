use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use mei_graph::{collect_template_imports, try_expand_artifact_macro_call, MacroRegistry};
use mei_lang_kernel::{
    decode_config_ref_value, load_mei_config_for_app, BlockDecl, ConfigRefKind, FrameDecl,
    LayoutDecl, PanelDecl, UiNodeDecl,
};
use serde_json::{json, Map, Value};

use crate::assemble::{assembly_key_to_target, assembly_source_file_from_payload};
use crate::import::load_block_artifact;
use crate::mcg::registry::McgRegistry;
use crate::presentation_map::resolve_viewpoint_id;
use crate::tier::{
    canonical_tier, compute_panel_z_index, parse_stack_order_value, props_contain_forbidden_z_index,
    resolve_stack_order,
};
use crate::types::GraphNodeKind;

pub struct PanelLowerContext<'a> {
    pub app_root: &'a Path,
    pub app_id: &'a str,
    pub registry: &'a McgRegistry,
    pub scene_id: &'a str,
    /// Top-level `NAME = expr` constants from the panel `.mei` source file.
    pub panel_constants: BTreeMap<String, Value>,
    /// Assembly `panels` list order within the same tier (0-based).
    pub assembly_stack_order: Option<u8>,
}

impl<'a> PanelLowerContext<'a> {
    pub fn with_panel_constants(&self, panel_key: &str) -> Self {
        Self {
            app_root: self.app_root,
            app_id: self.app_id,
            registry: self.registry,
            scene_id: self.scene_id,
            panel_constants: load_panel_file_constants(self.app_root, panel_key),
            assembly_stack_order: self.assembly_stack_order,
        }
    }

    pub fn with_assembly_stack_order(&self, order: u8) -> Self {
        Self {
            app_root: self.app_root,
            app_id: self.app_id,
            registry: self.registry,
            scene_id: self.scene_id,
            panel_constants: self.panel_constants.clone(),
            assembly_stack_order: Some(order),
        }
    }
}

fn panel_key_segments(panel_key: &str) -> (Option<&str>, String) {
    let key = panel_key
        .strip_prefix("panel_contract:")
        .unwrap_or(panel_key);
    if let Some((scope, id)) = key.split_once(':') {
        return (Some(scope), id.to_string());
    }
    if let Some((scope, id)) = key.split_once('/') {
        return (Some(scope), id.to_string());
    }
    (None, key.to_string())
}

fn underscore_to_kebab(id: &str) -> String {
    id.replace('_', "-")
}

fn panel_constant_candidate_paths(panel_key: &str) -> Vec<String> {
    let (scope, local_id) = panel_key_segments(panel_key);
    let kebab = underscore_to_kebab(local_id.as_str());
    let mut paths = Vec::new();

    paths.push(format!("src/content/panels/{kebab}.panel.mei"));
    if kebab != local_id {
        paths.push(format!("src/content/panels/{local_id}.panel.mei"));
    }

    if let Some(scope) = scope {
        paths.push(format!("src/scene/{scope}/{kebab}.panel.mei"));
        if kebab != local_id {
            paths.push(format!("src/scene/{scope}/{local_id}.panel.mei"));
        }
    }

    paths
}

fn load_panel_file_constants(app_root: &Path, panel_key: &str) -> BTreeMap<String, Value> {
    for rel in panel_constant_candidate_paths(panel_key) {
        let path = app_root.join(rel);
        if let Ok(content) = std::fs::read_to_string(path.as_path()) {
            return crate::panel_constants::parse_panel_constants_from_source(&content);
        }
    }
    BTreeMap::new()
}

fn metric_card_macro_constants_path(app_root: &Path) -> PathBuf {
    let workspace = app_root
        .parent()
        .and_then(|apps| apps.parent())
        .unwrap_or(app_root);
    workspace.join("stock/templates/cockpit/metric-card/macros.mei")
}

fn load_metric_card_macro_constants(app_root: &Path) -> BTreeMap<String, Value> {
    let path = metric_card_macro_constants_path(app_root);
    std::fs::read_to_string(path.as_path())
        .map(|content| crate::v2_bundle_constants::parse_bundle_constants_from_source(&content))
        .unwrap_or_default()
}

fn combined_panel_constants(ctx: &PanelLowerContext<'_>) -> BTreeMap<String, Value> {
    let mut constants = load_metric_card_macro_constants(ctx.app_root);
    constants.extend(
        ctx.panel_constants
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    constants
}

fn resolve_panel_constant_exprs(value: &Value, ctx: &PanelLowerContext<'_>) -> Value {
    let constants = combined_panel_constants(ctx);
    crate::v2_bundle_constants::resolve_v2_constants(value, &constants)
}

fn resolve_panel_id_value(
    value: Option<&Value>,
    ctx: &PanelLowerContext<'_>,
    default: &str,
) -> String {
    let Some(raw) = value else {
        return default.to_string();
    };
    let resolved = resolve_panel_constant_exprs(raw, ctx);
    resolved
        .as_str()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default.to_string())
}

fn resolve_panel_props_value(value: &Value, ctx: &PanelLowerContext<'_>) -> Value {
    if let Some(rewritten) = crate::artifact_biz_macros::try_rewrite_biz_macro(value) {
        if rewritten != *value {
            return resolve_panel_props_value(&rewritten, ctx);
        }
    }
    if value.get("__binop").and_then(Value::as_str) == Some("Merge") {
        let mut merged = value
            .get("left")
            .map(|left| resolve_panel_props_value(left, ctx))
            .unwrap_or_else(|| json!({}));
        if let Some(right) = value.get("right") {
            let resolved_right = resolve_panel_props_value(right, ctx);
            if resolved_right.is_object() {
                deep_merge_value(&mut merged, &resolved_right);
            }
        }
        return merged;
    }
    if value.get("__call").is_some() {
        if let Some(expanded) = try_expand_unlowered_block(value, ctx) {
            return resolve_panel_props_value(&expanded, ctx);
        }
    }
    resolve_panel_constant_exprs(value, ctx)
}

pub(crate) fn resolve_panel_props_for_shell(
    value: &Value,
    ctx: &PanelLowerContext<'_>,
) -> Value {
    resolve_panel_props_value(value, ctx)
}

pub fn lower_frame_from_assembly(payload: &Value) -> FrameDecl {
    let layout = payload.get("layout").and_then(lower_layout);
    let mut props = json!({});
    if let Some(canvas) = payload.get("canvas") {
        let vp_args = v2_call_args(canvas).unwrap_or(canvas);
        if let Some(obj) = props.as_object_mut() {
            obj.insert("viewport".to_string(), lower_viewport_props(vp_args));
        }
    }
    FrameDecl {
        kind: "frame".to_string(),
        id: payload
            .get("scene")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        title: payload
            .get("summary")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        layout,
        props,
        base: None,
        panels: payload
            .get("panels")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
    }
}

pub fn lower_panel_payload(
    payload: &Value,
    panel_key: &str,
    ctx: &PanelLowerContext<'_>,
) -> Result<PanelDecl> {
    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(panel_key)
        .to_string();
    let area = payload
        .get("area")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    if payload.get("slots").and_then(Value::as_array).is_some() {
        return lower_panel_with_slots(payload, id, area, ctx);
    }

    if let Some(shell) = payload.get("shell") {
        return lower_panel_from_shell(payload, shell, id, area, ctx);
    }

    let mut props = json!({});
    merge_card_fields(&mut props, payload);
    if let Some(extra) = payload.get("props") {
        let resolved = resolve_panel_props_value(
            &resolve_config_refs_in_value(extra, ctx),
            ctx,
        );
        if resolved.is_object() {
            deep_merge_value(&mut props, &resolved);
        }
    }
    apply_tier_and_placement(payload, &mut props, ctx.assembly_stack_order)?;

    let blocks = lower_blocks(payload.get("blocks"), ctx)?;
    apply_view_family_hints(payload, &blocks, &mut props);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: payload
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        head: None,
        area,
        layout: payload.get("layout").and_then(lower_layout),
        blocks,
        slot: None,
        props,
        head_props: lower_head_props(payload),
        body_props: payload.get("body_props").cloned().unwrap_or(json!({})),
        base: None,
        import_scope: None,
    })
}

fn lower_panel_with_slots(
    payload: &Value,
    id: String,
    area: Option<String>,
    ctx: &PanelLowerContext<'_>,
) -> Result<PanelDecl> {
    let mut props = json!({});
    merge_card_fields(&mut props, payload);
    if let Some(extra) = payload.get("props").filter(|value| value.is_object()) {
        deep_merge_value(&mut props, extra);
    }
    apply_tier_and_placement(payload, &mut props, ctx.assembly_stack_order)?;

    let mut blocks = Vec::new();
    if let Some(slots) = payload.get("slots").and_then(Value::as_array) {
        for slot in slots {
            if v2_call_name(slot) != Some("panel_slot") {
                continue;
            }
            let slot_args = v2_call_args(slot).unwrap_or(slot);
            let slot_area = slot_args
                .get("area")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let slot_id = slot_area.clone().unwrap_or_else(|| "slot".to_string());
            if let Some(shell) = slot_args.get("shell") {
                let slot_panel = if shell_is_titled_panel_contract(shell)
                    || v2_call_name(shell) == Some("titled_shell")
                    || v2_call_name(shell) == Some("section_shell")
                {
                    if v2_call_name(shell) == Some("section_shell") {
                        lower_section_shell_panel(shell, slot_id, slot_area, ctx, None)?
                    } else {
                        lower_titled_shell_panel(shell, slot_id, slot_area, ctx, None)?
                    }
                } else {
                    lower_panel_from_generic_shell(payload, shell, slot_id, slot_area, ctx)?
                };
                blocks.push(UiNodeDecl::Panel(slot_panel));
            }
        }
    }

    apply_view_family_hints(payload, &blocks, &mut props);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: None,
        head: None,
        area,
        layout: payload.get("layout").and_then(lower_layout),
        blocks,
        slot: None,
        props,
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    })
}

fn lower_panel_from_shell(
    payload: &Value,
    shell: &Value,
    id: String,
    area: Option<String>,
    ctx: &PanelLowerContext<'_>,
) -> Result<PanelDecl> {
    if shell_is_titled_panel_contract(shell) {
        return lower_titled_shell_panel(shell, id, area, ctx, Some(payload));
    }
    match v2_call_name(shell) {
        Some("screen_header") => lower_screen_header_panel(payload, shell, id, area, ctx),
        Some("section_shell") => lower_section_shell_panel(shell, id, area, ctx, Some(payload)),
        Some("titled_shell") => lower_titled_shell_panel(shell, id, area, ctx, Some(payload)),
        _ => lower_panel_from_generic_shell(payload, shell, id, area, ctx),
    }
}

fn shell_is_titled_panel_contract(shell: &Value) -> bool {
    if v2_call_name(shell) != Some("panel_contract") {
        return false;
    }
    let Some(args) = v2_call_args(shell) else {
        return false;
    };
    args.get("title")
        .and_then(|v| v.as_str())
        .is_some_and(|title| !title.trim().is_empty())
}

fn lower_screen_header_panel(
    payload: &Value,
    shell: &Value,
    id: String,
    area: Option<String>,
    ctx: &PanelLowerContext<'_>,
) -> Result<PanelDecl> {
    let args = v2_call_args(shell).context("screen_header missing __args")?;
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cap_min_width = args
        .get("cap_min_width")
        .or_else(|| args.get("capMinWidth"))
        .and_then(Value::as_i64)
        .unwrap_or(633);
    let assets = resolve_assets_map(args.get("assets"), ctx.app_id);

    let mut props = json!({
        "chrome": "bare",
        "variant": "container",
        "show_heading": false,
        "padding": "0",
        "background": "transparent",
        "border": "none",
        "width": "100%",
        "box_sizing": "border-box",
        "overflow": "hidden"
    });
    apply_tier_and_placement(payload, &mut props, ctx.assembly_stack_order)?;

    let block = BlockDecl {
        kind: "block".to_string(),
        use_key: "cockpit.header-brand".to_string(),
        id: Some("screen_header_brand".to_string()),
        title: None,
        area: None,
        props: json!({
            "title": title,
            "assets": assets,
            "capMinWidth": cap_min_width,
            "titleColor": "#E8F0FF",
            "titleLineHeight": "68px",
            "titleLetterSpacing": "0"
        }),
        base: None,
        layout: None,
        blocks: Vec::new(),
        component: None,
        placement: None,
        interactions: Vec::new(),
        lifecycle: None,
        constraints: None,
        data: None,
    };

    apply_view_family_hints(payload, &[UiNodeDecl::Block(block.clone())], &mut props);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: None,
        head: None,
        area,
        layout: None,
        blocks: vec![UiNodeDecl::Block(block)],
        slot: None,
        props,
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    })
}

fn body_props_has_padding(body_props: &Value) -> bool {
    body_props
        .as_object()
        .and_then(|map| map.get("padding"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn hoist_titled_shell_body_padding(blocks: &mut [UiNodeDecl], body_props: &mut Value) {
    if body_props_has_padding(body_props) {
        return;
    }
    let Some(UiNodeDecl::Panel(wrapper)) = blocks.first_mut() else {
        return;
    };
    let Some(padding) = wrapper
        .props
        .as_object()
        .and_then(|map| map.get("padding"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return;
    };
    let mut map = body_props.as_object().cloned().unwrap_or_default();
    map.insert("padding".to_string(), json!(padding));
    map.entry("box_sizing".to_string())
        .or_insert_with(|| json!("border-box"));
    map.entry("min_height".to_string())
        .or_insert_with(|| json!("0"));
    *body_props = Value::Object(map);
    if let Some(props) = wrapper.props.as_object_mut() {
        props.remove("padding");
    }
}

fn titled_shell_body_props(args: &Value, ctx: &PanelLowerContext<'_>) -> Value {
    let body_props = args.get("body_props").cloned().unwrap_or(json!({}));
    let Some(padding) = args.get("body_padding") else {
        return body_props;
    };
    let resolved = resolve_panel_constant_exprs(padding, ctx);
    let Some(padding_str) = resolved.as_str().map(str::trim).filter(|value| !value.is_empty())
    else {
        return body_props;
    };
    let map = body_props
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut map = map;
    map.insert("padding".to_string(), json!(padding_str));
    map.entry("box_sizing".to_string())
        .or_insert_with(|| json!("border-box"));
    map.entry("min_height".to_string())
        .or_insert_with(|| json!("0"));
    Value::Object(map)
}

fn lower_section_shell_panel(
    shell: &Value,
    id: String,
    area: Option<String>,
    ctx: &PanelLowerContext<'_>,
    outer_payload: Option<&Value>,
) -> Result<PanelDecl> {
    let mut panel = lower_titled_shell_panel(shell, id, area, ctx, outer_payload)?;
    let args = v2_call_args(shell).context("section shell missing __args")?;
    if let Some(map) = panel.props.as_object_mut() {
        map.remove("height");
        map.remove("min_height");
        map.remove("max_height");
        if let Some(profile) = args.get("padding_profile").and_then(|v| v.as_str()) {
            map.insert("__mei_padding_profile".to_string(), json!(profile));
            if let Some(padding) = mei_lang_kernel::padding_profile_css(profile) {
                let mut body_map = panel
                    .body_props
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                body_map
                    .entry("padding".to_string())
                    .or_insert_with(|| json!(padding));
                body_map
                    .entry("box_sizing".to_string())
                    .or_insert_with(|| json!("border-box"));
                body_map
                    .entry("min_height".to_string())
                    .or_insert_with(|| json!("0"));
                panel.body_props = Value::Object(body_map);
            }
        }
    }
    Ok(panel)
}

fn lower_titled_shell_panel(
    shell: &Value,
    id: String,
    area: Option<String>,
    ctx: &PanelLowerContext<'_>,
    outer_payload: Option<&Value>,
) -> Result<PanelDecl> {
    let args = v2_call_args(shell).context("titled shell missing __args")?;
    if v2_call_name(shell) == Some("titled_shell") {
        if args
            .get("height")
            .and_then(|v| v.as_str())
            .is_some_and(|h| !h.trim().is_empty() && h != "auto" && h != "100%")
        {
            anyhow::bail!(
                "titled_shell(height=...) is forbidden for panel `{id}`; use section_shell + content_budget"
            );
        }
    }
    let mut props = titled_shell_template_props(args);
    merge_card_fields(&mut props, args);
    if let Some(extra) = args.get("props").filter(|value| value.is_object()) {
        deep_merge_value(&mut props, extra);
    }
    if let Some(outer) = outer_payload {
        apply_tier_and_placement(outer, &mut props, ctx.assembly_stack_order)?;
    }
    if let Some(map) = props.as_object_mut() {
        map.insert("__mei_ui_role".to_string(), json!("section"));
        if let Some(title) = args.get("title").and_then(|v| v.as_str()) {
            map.insert("__mei_section_title".to_string(), json!(title));
        }
    }

    let mut head_props = titled_shell_template_head_props();
    merge_head_props_from_source(&mut head_props, args);
    finalize_panel_head_props(&mut head_props);

    let mut blocks = Vec::new();
    if let Some(body) = args.get("body") {
        blocks.extend(lower_block_node(body, ctx)?);
    } else {
        blocks.extend(lower_blocks(args.get("blocks"), ctx)?);
    }

    let mut body_props = titled_shell_body_props(args, ctx);
    hoist_titled_shell_body_padding(&mut blocks, &mut body_props);

    let family_source = outer_payload.unwrap_or(args);
    apply_view_family_hints(family_source, &blocks, &mut props);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: args
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        head: None,
        area,
        layout: args.get("layout").and_then(lower_layout),
        blocks,
        slot: None,
        props,
        head_props,
        body_props,
        base: None,
        import_scope: None,
    })
}

fn lower_panel_from_generic_shell(
    payload: &Value,
    shell: &Value,
    id: String,
    area: Option<String>,
    ctx: &PanelLowerContext<'_>,
) -> Result<PanelDecl> {
    let args = v2_call_args(shell).context("panel shell missing __args")?;
    let mut props = args.get("props").cloned().unwrap_or(json!({}));
    merge_card_fields(&mut props, args);
    apply_tier_and_placement(payload, &mut props, ctx.assembly_stack_order)?;

    let mut head_props = lower_head_props(args);
    if let Some(heading) = args.get("heading") {
        if let Some(map) = head_props.as_object_mut() {
            map.insert("heading".to_string(), heading.clone());
        }
    }
    merge_head_props_from_source(&mut head_props, args);
    finalize_panel_head_props(&mut head_props);

    let blocks = lower_blocks(args.get("blocks"), ctx)?;
    apply_view_family_hints(payload, &blocks, &mut props);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: args
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        head: None,
        area,
        layout: args.get("layout").and_then(lower_layout),
        blocks,
        slot: None,
        props,
        head_props,
        body_props: args.get("body_props").cloned().unwrap_or(json!({})),
        base: None,
        import_scope: None,
    })
}

fn merge_card_fields(props: &mut Value, source: &Value) {
    let Some(map) = props.as_object_mut() else {
        return;
    };
    for key in [
        "chrome",
        "variant",
        "show_heading",
        "title",
        "title_align",
        "title_height",
        "heading_variant",
    ] {
        if let Some(value) = source.get(key) {
            map.insert(key.to_string(), value.clone());
        }
    }
}

fn lower_head_props(source: &Value) -> Value {
    let mut head = Map::new();
    if let Some(value) = source.get("heading_variant") {
        head.insert("heading_variant".to_string(), value.clone());
    }
    if let Some(value) = source.get("heading") {
        head.insert("heading".to_string(), value.clone());
    }
    Value::Object(head)
}

fn apply_tier_and_placement(
    payload: &Value,
    props: &mut Value,
    assembly_stack_order: Option<u8>,
) -> Result<()> {
    if props_contain_forbidden_z_index(props) {
        anyhow::bail!(
            "panel props must not set z_index; use stack_order (viewport tier panels) or layout_stack (local stacking within a parent panel)"
        );
    }
    let raw_tier = payload.get("tier").and_then(|v| v.as_str());
    let tier = match raw_tier {
        Some(t) => Some(
            canonical_tier(t)
                .map_err(|message| anyhow::anyhow!("invalid panel tier \"{t}\": {message}"))?,
        ),
        None => None,
    };
    if let Some(map) = props.as_object_mut() {
        if let Some(tier) = tier {
            map.insert("__mei_tier".to_string(), json!(tier));
        }
        if let Some(role) = payload.get("chrome_role").and_then(|v| v.as_str()) {
            map.insert("__mei_chrome_role".to_string(), json!(role));
            let ui_role = match role {
                "header" => "header",
                "viewport" | "viewport_frame" | "map_tools" => "viewport_chrome",
                "center_float" | "float_dock" => "float_dock",
                "map" | "stage" | "map_stage" | "stage_aperture" | "map_interaction_surface" => "stage",
                _ => "region",
            };
            map.insert("__mei_ui_role".to_string(), json!(ui_role));
        } else if let Some(tier) = tier {
            if tier == "t0" {
                map.insert("__mei_ui_role".to_string(), json!("stage"));
            }
        }
    }
    apply_placement(payload.get("placement"), props);
    apply_float_dock_overlay_defaults(payload, props);
    if let Some(tier) = tier {
        let chrome_role = payload.get("chrome_role").and_then(|v| v.as_str());
        let explicit_stack = payload
            .get("stack_order")
            .or_else(|| payload.get("stackOrder"))
            .map(parse_stack_order_value)
            .transpose()
            .map_err(anyhow::Error::msg)?;
        let stack_order = resolve_stack_order(explicit_stack, assembly_stack_order.unwrap_or(0))
            .map_err(anyhow::Error::msg)?;
        let z = compute_panel_z_index(tier, chrome_role, stack_order);
        if let Some(map) = props.as_object_mut() {
            map.insert("__mei_stack_order".to_string(), json!(stack_order));
            map.insert("z_index".to_string(), json!(z));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ViewFamilyHints {
    family: Option<String>,
    world_ref: Option<String>,
    stage_kind: Option<String>,
}

impl ViewFamilyHints {
    fn is_empty(&self) -> bool {
        self.family.is_none() && self.world_ref.is_none() && self.stage_kind.is_none()
    }

    fn fill_missing(&mut self, fallback: Self) {
        if self.family.is_none() {
            self.family = fallback.family;
        }
        if self.world_ref.is_none() {
            self.world_ref = fallback.world_ref;
        }
        if self.stage_kind.is_none() {
            self.stage_kind = fallback.stage_kind;
        }
    }
}

fn view_family_stage_kind(family: &str) -> Option<&'static str> {
    match family {
        "map" => Some("map-stage"),
        "world" => Some("world-stage"),
        "canvas" => Some("viewport-canvas"),
        _ => None,
    }
}

fn object_string(obj: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn value_string_hint(value: &Value, keys: &[&str]) -> Option<String> {
    let Some(obj) = value.as_object() else {
        return None;
    };
    object_string(obj, keys).or_else(|| {
        obj.get("__args")
            .and_then(Value::as_object)
            .and_then(|args| object_string(args, keys))
    })
}

fn view_family_hints_from_value(value: &Value) -> ViewFamilyHints {
    let Some(obj) = value.as_object() else {
        return ViewFamilyHints::default();
    };
    let mut hints = ViewFamilyHints {
        family: object_string(obj, &["__mei_view_family", "viewFamily", "view_family"]),
        world_ref: object_string(obj, &["__mei_world_ref", "worldRef", "world_ref"]),
        stage_kind: object_string(obj, &["__mei_stage_kind", "stageKind", "stage_kind"]),
    };
    match obj.get("__call").and_then(|v| v.as_str()) {
        Some("map_view") => {
            hints.family.get_or_insert_with(|| "map".to_string());
        }
        Some("world_view") => {
            hints.family.get_or_insert_with(|| "world".to_string());
        }
        Some("viewport_canvas") => {
            hints.family.get_or_insert_with(|| "canvas".to_string());
        }
        _ => {}
    }
    if let Some(args) = obj.get("__args").and_then(|v| v.as_object()) {
        if hints.world_ref.is_none() {
            hints.world_ref = object_string(args, &["worldRef", "world_ref", "arg0"]);
        }
        if hints.stage_kind.is_none() {
            hints.stage_kind = object_string(args, &["stageKind", "stage_kind"]);
        }
    }
    if hints.stage_kind.is_none() {
        hints.stage_kind = hints
            .family
            .as_deref()
            .and_then(view_family_stage_kind)
            .map(str::to_string);
    }
    hints
}

fn infer_view_family_from_panel_id(payload: &Value) -> ViewFamilyHints {
    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    let family = match id {
        "viewport_canvas" => Some("canvas"),
        "map_stage" | "basemap" => Some("map"),
        _ => {
            if id.contains("viewport_canvas") {
                Some("canvas")
            } else if id.contains("map_stage") || id.contains("basemap") {
                Some("map")
            } else {
                None
            }
        }
    };
    let mut hints = ViewFamilyHints::default();
    if let Some(family) = family {
        hints.family = Some(family.to_string());
        hints.stage_kind = view_family_stage_kind(family).map(str::to_string);
    }
    hints
}

fn infer_view_family_from_block(block: &BlockDecl) -> ViewFamilyHints {
    let mut hints = view_family_hints_from_value(&block.props);
    if hints.family.is_none() {
        hints.family = match block.use_key.as_str() {
            "map.maplibre" | "cockpit.basemap-stage" => Some("map".to_string()),
            "cockpit.world-stage" | "world.stage" | "world.world-stage" => {
                Some("world".to_string())
            }
            _ => None,
        };
    }
    if hints.stage_kind.is_none() {
        hints.stage_kind = hints
            .family
            .as_deref()
            .and_then(view_family_stage_kind)
            .map(str::to_string);
    }
    hints
}

fn infer_view_family_from_nodes(nodes: &[UiNodeDecl]) -> ViewFamilyHints {
    for node in nodes {
        match node {
            UiNodeDecl::Block(block) => {
                let hints = infer_view_family_from_block(block);
                if !hints.is_empty() {
                    return hints;
                }
            }
            UiNodeDecl::Panel(panel) => {
                let mut hints = view_family_hints_from_value(&panel.props);
                if hints.is_empty() {
                    hints = infer_view_family_from_nodes(&panel.blocks);
                }
                if !hints.is_empty() {
                    return hints;
                }
            }
            UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
    ViewFamilyHints::default()
}

fn apply_view_family_hints(payload: &Value, blocks: &[UiNodeDecl], props: &mut Value) {
    let tier = props
        .get("__mei_tier")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("tier").and_then(|v| v.as_str()));
    let mut hints = view_family_hints_from_value(props);
    hints.fill_missing(view_family_hints_from_value(
        payload.get("content").unwrap_or(&Value::Null),
    ));
    hints.fill_missing(view_family_hints_from_value(
        payload.get("view").unwrap_or(&Value::Null),
    ));
    hints.fill_missing(view_family_hints_from_value(payload));
    if tier == Some("t0") || !hints.is_empty() {
        hints.fill_missing(infer_view_family_from_nodes(blocks));
        hints.fill_missing(infer_view_family_from_panel_id(payload));
    }
    if hints.is_empty() {
        return;
    }
    let Some(map) = props.as_object_mut() else {
        return;
    };
    if let Some(family) = hints.family {
        map.insert("__mei_view_family".to_string(), json!(family));
    }
    if let Some(world_ref) = hints.world_ref {
        map.insert("__mei_world_ref".to_string(), json!(world_ref));
    }
    if let Some(stage_kind) = hints.stage_kind {
        map.insert("__mei_stage_kind".to_string(), json!(stage_kind));
    }
    for (target_key, source_keys) in [
        ("entityId", ["entityId", "entity_id"]),
        ("groupId", ["groupId", "group_id"]),
        ("cameraPreset", ["cameraPreset", "camera_preset"]),
    ] {
        if map.contains_key(target_key) {
            continue;
        }
        for source in [
            payload.get("content").unwrap_or(&Value::Null),
            payload.get("view").unwrap_or(&Value::Null),
            payload,
        ] {
            if let Some(value) = value_string_hint(source, &source_keys) {
                map.insert(target_key.to_string(), json!(value));
                break;
            }
        }
    }
}

const PLACEMENT_DIMENSION_KEYS: &[&str] = &[
    "width",
    "height",
    "min_width",
    "min_height",
    "max_width",
    "max_height",
];

fn apply_placement(placement: Option<&Value>, props: &mut Value) {
    let Some(placement) = placement else {
        return;
    };
    let call = v2_call_name(placement);
    let args = v2_call_args(placement).unwrap_or(placement);
    let Some(map) = props.as_object_mut() else {
        return;
    };
    if call.as_deref() == Some("absolute") {
        map.insert("position".to_string(), json!("absolute"));
    }
    if let Some(args_obj) = args.as_object() {
        for (key, value) in args_obj {
            if PLACEMENT_DIMENSION_KEYS.contains(&key.as_str()) {
                if let Some(existing) = map.get(key) {
                    if dimension_values_conflict(existing, value) {
                        let conflicts = map
                            .entry("__mei_placement_dimension_conflicts".to_string())
                            .or_insert_with(|| json!([]));
                        if let Some(arr) = conflicts.as_array_mut() {
                            if !arr.iter().any(|v| v.as_str() == Some(key.as_str())) {
                                arr.push(json!(key));
                            }
                        }
                    }
                }
            }
            map.insert(key.clone(), value.clone());
        }
    }
}

fn apply_float_dock_overlay_defaults(payload: &Value, props: &mut Value) {
    if payload.get("placement").is_some() {
        return;
    }
    if payload.get("chrome_role").and_then(Value::as_str) != Some("float_dock") {
        return;
    }
    let Some(map) = props.as_object_mut() else {
        return;
    };
    map.entry("position".to_string())
        .or_insert_with(|| json!("absolute"));
    map.entry("top".to_string()).or_insert_with(|| json!("0"));
    map.entry("left".to_string()).or_insert_with(|| json!("0"));
    map.entry("width".to_string())
        .or_insert_with(|| json!("0px"));
    map.entry("height".to_string())
        .or_insert_with(|| json!("0px"));
    map.entry("pointer_events".to_string())
        .or_insert_with(|| json!("none"));
    map.insert("__mei_platform_placement".to_string(), json!(true));
}

fn dimension_values_conflict(existing: &Value, incoming: &Value) -> bool {
    let Some(existing_text) = dimension_as_text(existing) else {
        return false;
    };
    let Some(incoming_text) = dimension_as_text(incoming) else {
        return false;
    };
    if existing_text == incoming_text {
        return false;
    }
    // placement sets the stage box; shell props often use 100% fill inside the box.
    if existing_text == "100%" || incoming_text == "100%" {
        return false;
    }
    if existing_text.eq_ignore_ascii_case("auto") || incoming_text.eq_ignore_ascii_case("auto") {
        return false;
    }
    true
}

fn dimension_as_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| value.as_i64().map(|n| format!("{n}px")))
}

pub(crate) fn lower_layout(value: &Value) -> Option<LayoutDecl> {
    let layout_type = v2_call_name(value)?.to_string();
    let args = v2_call_args(value).unwrap_or(value);
    let obj = args.as_object()?;
    Some(LayoutDecl {
        layout_type,
        direction: obj
            .get("direction")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        columns: obj.get("columns").and_then(|v| v.as_array()).map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        }),
        rows: obj.get("rows").and_then(|v| v.as_array()).map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        }),
        areas: obj.get("areas").and_then(|v| v.as_array()).map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    row.as_array().map(|cells| {
                        cells
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect::<Vec<_>>()
                    })
                })
                .collect()
        }),
        gap: obj.get("gap").and_then(|v| v.as_str()).map(str::to_string),
        padding: obj
            .get("padding")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        align: obj
            .get("align")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        justify: obj
            .get("justify")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

fn lower_viewport_props(args: &Value) -> Value {
    let mut viewport = Map::new();
    if let Some(obj) = args.as_object() {
        for (key, value) in obj {
            viewport.insert(key.clone(), value.clone());
        }
    }
    Value::Object(viewport)
}

fn lower_blocks(value: Option<&Value>, ctx: &PanelLowerContext<'_>) -> Result<Vec<UiNodeDecl>> {
    let Some(array) = value.and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };
    let mut blocks = Vec::new();
    for item in array {
        blocks.extend(lower_block_node(item, ctx)?);
    }
    Ok(blocks)
}

fn workspace_stock_templates(app_root: &Path) -> PathBuf {
    app_root
        .parent()
        .and_then(|apps| apps.parent())
        .map(|workspace| workspace.join("stock/templates"))
        .unwrap_or_else(|| app_root.join("stock/templates"))
}

struct TemplateMacroCache {
    registry: MacroRegistry,
    imports: BTreeMap<String, String>,
}

fn template_macro_cache(app_root: &Path) -> Option<TemplateMacroCache> {
    static CACHE: std::sync::OnceLock<Mutex<BTreeMap<PathBuf, TemplateMacroCache>>> =
        std::sync::OnceLock::new();
    let stock = workspace_stock_templates(app_root);
    if !stock.is_dir() {
        return None;
    }
    let canonical = stock.canonicalize().ok()?;
    let mutex = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = mutex.lock().ok()?;
    if let Some(cached) = guard.get(&canonical) {
        return Some(TemplateMacroCache {
            registry: cached.registry.clone(),
            imports: cached.imports.clone(),
        });
    }
    let registry = MacroRegistry::load_dir(canonical.as_path()).ok()?;
    let imports = collect_template_imports(canonical.as_path());
    guard.insert(
        canonical.clone(),
        TemplateMacroCache {
            registry: registry.clone(),
            imports: imports.clone(),
        },
    );
    Some(TemplateMacroCache { registry, imports })
}

fn try_expand_unlowered_block(value: &Value, ctx: &PanelLowerContext<'_>) -> Option<Value> {
    if let Some(rewritten) = crate::artifact_biz_macros::try_rewrite_biz_macro(value) {
        if rewritten != *value {
            return Some(rewritten);
        }
    }
    let cache = template_macro_cache(ctx.app_root)?;
    try_expand_artifact_macro_call(value, &cache.registry, &cache.imports)
}

fn lower_block_node(value: &Value, ctx: &PanelLowerContext<'_>) -> Result<Vec<UiNodeDecl>> {
    if v2_ref_name(value) == Some("panel_ref") {
        let ref_path = v2_ref_arg0(value).context("panel_ref missing arg0")?;
        let payload = load_panel_contract_payload(ctx, ref_path.as_str())?;
        let panel_ctx = ctx.with_panel_constants(ref_path.as_str());
        let panel = lower_panel_payload(&payload, ref_path.as_str(), &panel_ctx)?;
        return Ok(vec![UiNodeDecl::Panel(panel)]);
    }
    if v2_call_name(value).as_deref() == Some("component") {
        return Ok(vec![UiNodeDecl::Block(lower_component(value, ctx)?)]);
    }
    if v2_call_name(value).as_deref() == Some("metric") {
        return Ok(vec![lower_metric(value, ctx)?]);
    }
    if v2_call_name(value).as_deref() == Some("metric_card") {
        return Ok(vec![lower_metric_card(value, ctx)?]);
    }
    if v2_call_name(value).as_deref() == Some("panel") {
        return Ok(vec![UiNodeDecl::Panel(lower_inline_panel(value, ctx)?)]);
    }
    if let Some(expanded) = try_expand_unlowered_block(value, ctx) {
        return lower_block_node(&expanded, ctx);
    }
    if value.get("use_key").is_some() || value.get("kind").and_then(|v| v.as_str()) == Some("block")
    {
        return Ok(vec![UiNodeDecl::Block(
            serde_json::from_value(value.clone()).context("decode legacy block")?,
        )]);
    }
    Ok(Vec::new())
}

pub(crate) fn lower_v2_inline_panels_from_assembly(
    payload: &Value,
    ctx: &PanelLowerContext<'_>,
) -> Result<Vec<PanelDecl>> {
    let panels_value = payload
        .get("panels")
        .or_else(|| payload.get("frame").and_then(|frame| frame.get("panels")));
    let Some(array) = panels_value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut panels = Vec::new();
    for item in array {
        if v2_call_name(item) == Some("panel") {
            panels.push(lower_inline_panel(item, ctx)?);
        }
    }
    Ok(panels)
}

fn lower_inline_panel(value: &Value, ctx: &PanelLowerContext<'_>) -> Result<PanelDecl> {
    let args = v2_call_args(value).context("panel missing __args")?;
    let expanded_template = metric_expanded_template_args(args.get("template"));
    let id = resolve_panel_id_value(args.get("id"), ctx, "panel");
    let area = args
        .get("area")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    if args.get("slots").and_then(Value::as_array).is_some() {
        return lower_panel_with_slots(args, id, area, ctx);
    }

    if let Some(shell) = args
        .get("shell")
        .or_else(|| expanded_template.and_then(|template| template.get("shell")))
    {
        return lower_panel_from_shell(args, shell, id, area, ctx);
    }

    let mut props = json!({});
    if let Some(expanded) = expanded_template {
        merge_card_fields(&mut props, expanded);
        if let Some(template_props) = expanded.get("props").filter(|value| value.is_object()) {
            let resolved = resolve_config_refs_in_value(template_props, ctx);
            deep_merge_value(&mut props, &resolved);
        }
    }
    merge_card_fields(&mut props, args);
    if let Some(extra) = args.get("props").filter(|value| value.is_object()) {
        let resolved = resolve_config_refs_in_value(extra, ctx);
        deep_merge_value(&mut props, &resolved);
    }
    for key in ["variant", "chrome", "show_heading"] {
        if let Some(value) = args.get(key) {
            props
                .as_object_mut()
                .expect("panel props object")
                .insert(key.to_string(), value.clone());
        } else if let Some(expanded) = expanded_template.and_then(|t| t.get(key)) {
            props
                .as_object_mut()
                .expect("panel props object")
                .insert(key.to_string(), expanded.clone());
        }
    }
    apply_tier_and_placement(args, &mut props, ctx.assembly_stack_order)?;

    let layout = args
        .get("layout")
        .or_else(|| expanded_template.and_then(|t| t.get("layout")))
        .and_then(lower_layout);

    let blocks = lower_blocks(args.get("blocks"), ctx)?;
    apply_view_family_hints(args, &blocks, &mut props);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: args
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        head: None,
        area,
        layout,
        blocks,
        slot: None,
        props,
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    })
}

fn metric_expanded_template_args(template: Option<&Value>) -> Option<&Value> {
    let template = template?;
    if v2_call_name(template) == Some("panel_contract") {
        v2_call_args(template)
    } else {
        None
    }
}

fn metric_ratio_from_props(props: &Value, key: &str, fallback: u32) -> u32 {
    props
        .get(key)
        .and_then(|value| {
            value
                .as_str()
                .and_then(|raw| raw.parse().ok())
                .or_else(|| value.as_u64().map(|n| n as u32))
        })
        .unwrap_or(fallback)
}

fn metric_layout_role_to_preset(role: &str) -> &str {
    match role.trim() {
        "plain" => "plain",
        "solid_stack" => "solid_stack",
        "stack_desc" => "stack_desc",
        "stack_progress" => "stack_desc",
        "compound_top_row" => "compound_top_row",
        "compound_sub_stack" => "compound_sub_stack",
        "icon_left" => "icon_left",
        "strip_icon_left" => "strip_icon_left",
        "solid_row_accent" => "solid_row_accent",
        "solid_row_compact" => "solid_row_compact",
        _ => "plain",
    }
}

fn resolve_metric_atom_source(args: &Value, ctx: &PanelLowerContext<'_>) -> Value {
    if let Some(source) = args.get("source") {
        return resolve_config_refs_in_value(source, ctx);
    }
    if let (Some(label), value, unit) = (
        args.get("arg0").or_else(|| args.get("label")),
        args.get("arg1")
            .or_else(|| args.get("value"))
            .cloned()
            .unwrap_or(json!("--")),
        args.get("arg2")
            .or_else(|| args.get("unit"))
            .cloned()
            .unwrap_or(json!("")),
    ) {
        let mut out = Map::new();
        out.insert(
            "label".to_string(),
            label.clone(),
        );
        out.insert("value".to_string(), value);
        out.insert("unit".to_string(), unit);
        if let Some(desc) = args.get("desc") {
            out.insert("desc".to_string(), desc.clone());
        }
        return Value::Object(out);
    }
    json!({})
}

fn strip_metric_atom_shell_chrome(props: &mut Value) {
    if let Some(map) = props.as_object_mut() {
        map.insert("background".to_string(), json!("transparent"));
        map.insert("border".to_string(), json!("none"));
        map.insert("box_shadow".to_string(), json!("none"));
    }
}

fn lower_metric(value: &Value, ctx: &PanelLowerContext<'_>) -> Result<UiNodeDecl> {
    let args = v2_call_args(value).context("metric missing __args")?;
    let layout_role = args
        .get("layout_role")
        .and_then(Value::as_str)
        .unwrap_or("plain");
    let template_name = metric_layout_role_to_preset(layout_role).to_string();
    let mut args_with_source = args.clone();
    if let Some(obj) = args_with_source.as_object_mut() {
        obj.insert(
            "source".to_string(),
            resolve_metric_atom_source(args, ctx),
        );
    }
    lower_metric_inner(
        value,
        &args_with_source,
        ctx,
        &template_name,
        None,
        args.get("desc")
            .and_then(Value::as_str)
            .map(str::to_string),
        true,
    )
}

fn lower_metric_card(value: &Value, ctx: &PanelLowerContext<'_>) -> Result<UiNodeDecl> {
    let args = v2_call_args(value).context("metric_card missing __args")?;
    let expanded_template = metric_expanded_template_args(args.get("template"));
    let template_name = if let Some(expanded) = expanded_template {
        expanded
            .get("props")
            .and_then(|props| props.get("__mei_metric_template"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| metric_template_name(args.get("template")))
    } else {
        metric_template_name(args.get("template"))
    };
    lower_metric_inner(
        value,
        args,
        ctx,
        &template_name,
        expanded_template,
        args.get("template")
            .and_then(v2_call_args)
            .and_then(|template| template.get("desc"))
            .and_then(Value::as_str)
            .map(str::to_string),
        false,
    )
}

fn lower_metric_inner(
    value: &Value,
    args: &Value,
    ctx: &PanelLowerContext<'_>,
    template_name: &str,
    expanded_template: Option<&Value>,
    template_desc: Option<String>,
    transparent_shell: bool,
) -> Result<UiNodeDecl> {
    let preset = metric_template_preset(template_name);
    let layout_template = preset.layout_template;
    let height_px = args.get("height_px").and_then(Value::as_i64);
    let density = metric_density(height_px, layout_template);
    let mut props = metric_shell_props(height_px, layout_template, &density);
    deep_merge_value(&mut props, &preset.shell);
    if let Some(expanded) = expanded_template {
        if let Some(template_props) = expanded.get("props").filter(|value| value.is_object()) {
            let resolved = resolve_config_refs_in_value(template_props, ctx);
            deep_merge_value(&mut props, &resolved);
        }
    }
    if let Some(extra) = args.get("props").filter(|value| value.is_object()) {
        let resolved = resolve_config_refs_in_value(extra, ctx);
        deep_merge_value(&mut props, &resolved);
    }
    stamp_metric_vertical_align(&mut props, args);
    props = resolve_panel_constant_exprs(&props, ctx);

    let title_ratio =
        metric_ratio_from_props(&props, "__mei_metric_title_ratio", preset.title_ratio);
    let content_ratio =
        metric_ratio_from_props(&props, "__mei_metric_content_ratio", preset.content_ratio);

    let source =
        resolve_config_refs_in_value(&args.get("source").cloned().unwrap_or(json!({})), ctx);
    let map = args.get("map").cloned();
    let patch = args.get("patch").cloned();
    let presentation = merge_metric_presentation(args, &source, patch.as_ref())
        .map(|value| resolve_config_refs_in_value(&value, ctx));
    let popup = args
        .get("popup")
        .map(|popup| resolve_popup_config(popup, ctx, Some(&source)));
    let desc_text = template_desc.or_else(|| {
        args.get("desc")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let mut blocks = metric_runtime_blocks(
        &source,
        layout_template,
        map.as_ref(),
        patch.as_ref(),
        popup.as_ref(),
        args,
        ctx,
    );
    if layout_template == "stack_desc" {
        let template_desc_from_macro = metric_template_desc_text(args.get("template"));
        let desc = desc_text
            .as_deref()
            .or(template_desc_from_macro.as_deref());
        if let Some(desc) = desc {
            blocks.push(UiNodeDecl::Block(metric_desc_slot_block(desc)));
        }
    }
    let layout = if let Some(expanded) = expanded_template {
        if expanded
            .get("layout")
            .and_then(v2_call_name)
            .is_some_and(|name| name == "layout_metric_stack")
        {
            Some(metric_layout_from_template(
                layout_template,
                &density,
                title_ratio,
                content_ratio,
            ))
        } else {
            expanded.get("layout").and_then(lower_layout)
        }
    } else if layout_template == "stack_desc" {
        Some(metric_stack_desc_layout())
    } else {
        preset.layout.clone()
    }
    .or_else(|| {
        Some(metric_layout_from_template(
            layout_template,
            &density,
            title_ratio,
            content_ratio,
        ))
    });

    let mut panel_props = props;
    if let Some(viewpoint) = args.get("viewpoint") {
        if let Some(id) = resolve_viewpoint_id(viewpoint) {
            panel_props
                .as_object_mut()
                .expect("metric card props")
                .insert("__mei_viewpoint".to_string(), json!(id));
        }
    }
    if let Some(presentation) = presentation.as_ref() {
        apply_presentation_icon_to_shell(&mut panel_props, presentation);
        stamp_metric_presentation_on_value_slot(&mut blocks, presentation);
    }
    if transparent_shell {
        strip_metric_atom_shell_chrome(&mut panel_props);
    }

    let default_id = if v2_call_name(value) == Some("metric") {
        "metric"
    } else {
        "metric_card"
    };

    Ok(UiNodeDecl::Panel(PanelDecl {
        kind: "panel".to_string(),
        id: resolve_panel_id_value(args.get("id"), ctx, default_id),
        title: None,
        head: None,
        area: args
            .get("area")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        layout,
        blocks,
        slot: None,
        props: panel_props,
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }))
}

fn metric_template_desc_text(template: Option<&Value>) -> Option<String> {
    let expanded = metric_expanded_template_args(template)?;
    expanded
        .get("blocks")
        .and_then(Value::as_array)
        .and_then(|blocks| {
            for block in blocks {
                let props = if v2_call_name(block) == Some("component") {
                    v2_call_args(block).and_then(|args| args.get("props"))
                } else {
                    block.get("props")
                };
                let role = props
                    .and_then(|p| p.get("metric_role"))
                    .and_then(Value::as_str);
                if role == Some("desc") {
                    return props
                        .and_then(|p| p.get("content"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            None
        })
}

fn metric_stack_desc_layout() -> LayoutDecl {
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["auto".to_string(), "auto".to_string()]),
        rows: Some(vec![
            "14px".to_string(),
            "auto".to_string(),
            "54px".to_string(),
            "6px".to_string(),
            "20px".to_string(),
            "14px".to_string(),
        ]),
        areas: Some(vec![
            vec![".".to_string(), ".".to_string()],
            vec!["label".to_string(), "label".to_string()],
            vec!["value".to_string(), "unit".to_string()],
            vec![".".to_string(), ".".to_string()],
            vec!["desc".to_string(), "desc".to_string()],
            vec![".".to_string(), ".".to_string()],
        ]),
        gap: Some("0".to_string()),
        padding: None,
        align: Some("stretch".to_string()),
        justify: Some("center".to_string()),
    }
}

fn metric_desc_slot_block(desc: &str) -> BlockDecl {
    let mut props = Map::new();
    props.insert("content".to_string(), json!(desc));
    props.insert("metric_role".to_string(), json!("desc"));
    props.insert("metric_v_align".to_string(), json!("center"));
    props.insert("align".to_string(), json!("center"));
    props.insert(
        "desc_shell".to_string(),
        json!({
            "width": "80px",
            "height": "20px",
            "background": "rgba(201, 233, 248, 0.2)",
            "border_radius": "2px",
            "font_family": "Microsoft YaHei, MicrosoftYaHei, PingFang SC, sans-serif",
            "font_size": "12px",
            "color": "rgba(255, 255, 255, 0.8)",
            "letter_spacing": "0",
            "font_weight": "400",
        }),
    );
    BlockDecl {
        kind: "block".to_string(),
        use_key: "mei.text".to_string(),
        id: None,
        title: None,
        area: Some("desc".to_string()),
        props: Value::Object(props),
        base: None,
        layout: None,
        blocks: Vec::new(),
        component: None,
        placement: None,
        interactions: Vec::new(),
        lifecycle: None,
        constraints: None,
        data: None,
    }
}

fn metric_runtime_blocks(
    source: &Value,
    template: &str,
    map: Option<&Value>,
    patch: Option<&Value>,
    popup: Option<&Value>,
    args: &Value,
    ctx: &PanelLowerContext<'_>,
) -> Vec<UiNodeDecl> {
    let constants = combined_panel_constants(ctx);
    let roles = ["label", "value", "unit"];
    roles
        .into_iter()
        .map(|role| {
            UiNodeDecl::Block(metric_runtime_slot_block(
                source, role, role, template, map, patch, popup, args, &constants,
            ))
        })
        .collect()
}

fn metric_runtime_slot_block(
    source: &Value,
    role: &str,
    area: &str,
    template: &str,
    map: Option<&Value>,
    patch: Option<&Value>,
    popup: Option<&Value>,
    args: &Value,
    constants: &BTreeMap<String, Value>,
) -> BlockDecl {
    let mut props = Map::new();
    let content = if v2_ref_name(source) == Some("metric") {
        source.clone()
    } else {
        lower_v2_metric_ref(source, constants).unwrap_or_else(|| source.clone())
    };
    props.insert("content".to_string(), content);
    props.insert("metric_role".to_string(), json!(role));
    props.insert(
        "align".to_string(),
        json!(metric_slot_align(template, role)),
    );
    if let Some(map) = map.filter(|value| value.is_object()) {
        props.insert("metric_map".to_string(), map.clone());
    }
    if let Some(patch) = patch.filter(|value| value.is_object()) {
        props.insert("metric_patch".to_string(), patch.clone());
    }
    if role == "value" {
        if let Some(popup) = popup.filter(|value| !value.is_null()) {
            props.insert("popup".to_string(), popup.clone());
        }
    }
    if let Some(v_align) = args
        .get(format!("{role}_vertical_align"))
        .or_else(|| args.get(format!("{role}VerticalAlign")))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        props.insert("metric_v_align".to_string(), json!(v_align));
    }

    BlockDecl {
        kind: "block".to_string(),
        use_key: "mei.text".to_string(),
        id: None,
        title: None,
        area: Some(area.to_string()),
        props: Value::Object(props),
        base: None,
        layout: None,
        blocks: Vec::new(),
        component: None,
        placement: None,
        interactions: Vec::new(),
        lifecycle: None,
        constraints: None,
        data: None,
    }
}

fn metric_template_name(value: Option<&Value>) -> String {
    if let Some(call) = value.and_then(v2_call_name) {
        return call.to_string();
    }
    value
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("stack")
        .to_string()
}

fn metric_density(height_px: Option<i64>, template: &str) -> String {
    let Some(height_px) = height_px else {
        return "normal".to_string();
    };
    if height_px <= 84 {
        return "compact".to_string();
    }
    if template == "row" && height_px >= 120 {
        return "roomy".to_string();
    }
    if height_px >= 132 {
        return "roomy".to_string();
    }
    "normal".to_string()
}

fn metric_default_gap(template: &str, density: &str) -> &'static str {
    match (template, density) {
        ("row", "compact") => "3px",
        ("row", _) => "4px",
        ("column", "compact") => "3px",
        ("column", _) => "4px",
        (_, "compact") => "2px 2px",
        (_, "roomy") => "4px 3px",
        _ => "3px 2px",
    }
}

fn metric_layout_from_template(
    template: &str,
    density: &str,
    title_ratio: u32,
    content_ratio: u32,
) -> LayoutDecl {
    if template == "row" {
        return LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec![
                "auto".to_string(),
                "auto".to_string(),
                "auto".to_string(),
            ]),
            rows: Some(vec!["1fr".to_string()]),
            areas: Some(vec![vec![
                "label".to_string(),
                "value".to_string(),
                "unit".to_string(),
            ]]),
            gap: Some(metric_default_gap(template, density).to_string()),
            padding: None,
            align: Some("center".to_string()),
            justify: Some("start".to_string()),
        };
    }
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["auto".to_string(), "auto".to_string()]),
        rows: Some(vec![
            format!("{title_ratio}fr"),
            format!("{content_ratio}fr"),
        ]),
        areas: Some(vec![
            vec!["label".to_string(), "label".to_string()],
            vec!["value".to_string(), "unit".to_string()],
        ]),
        gap: Some(metric_default_gap(template, density).to_string()),
        padding: None,
        align: Some("stretch".to_string()),
        justify: Some("center".to_string()),
    }
}

struct MetricTemplatePreset {
    layout_template: &'static str,
    title_ratio: u32,
    content_ratio: u32,
    shell: Value,
    layout: Option<LayoutDecl>,
}

fn metric_template_preset(name: &str) -> MetricTemplatePreset {
    const CORNER_DECOR_BG: &str = concat!(
        "linear-gradient(#71F1EA,#71F1EA) left top / 4px 2px no-repeat,",
        "linear-gradient(#71F1EA,#71F1EA) right top / 4px 2px no-repeat,",
        "linear-gradient(#71F1EA,#71F1EA) left bottom / 4px 2px no-repeat,",
        "linear-gradient(#71F1EA,#71F1EA) right bottom / 4px 2px no-repeat,",
        "rgba(98,190,235,0.10)"
    );
    match name {
        "solid_stack" => MetricTemplatePreset {
            layout_template: "stack",
            title_ratio: 2,
            content_ratio: 3,
            shell: json!({
                "padding": "4px 8px",
                "border": "1px solid rgba(98,190,235,0.35)",
                "background": CORNER_DECOR_BG,
                "__mei_metric_density": "compact",
                "__mei_metric_template": "stack",
                "__mei_metric_inline_align": "compact",
                "__mei_metric_title_ratio": "2",
                "__mei_metric_content_ratio": "3",
            }),
            layout: None,
        },
        "icon_left" => MetricTemplatePreset {
            layout_template: "stack",
            title_ratio: 2,
            content_ratio: 3,
            shell: json!({
                "padding": "10px 8px 10px 70px",
                "__mei_metric_density": "compact",
                "__mei_metric_template": "stack",
                "__mei_metric_inline_align": "compact",
                "__mei_metric_title_ratio": "2",
                "__mei_metric_content_ratio": "3",
                "background": {
                    "color": "rgba(98,190,235,0.10)",
                    "size": "48px 48px",
                    "position": "11px center",
                    "repeat": "no-repeat",
                },
            }),
            layout: None,
        },
        "strip_icon_left" => MetricTemplatePreset {
            layout_template: "row",
            title_ratio: 1,
            content_ratio: 1,
            shell: json!({
                "padding": "0 16px 0 92px",
                "__mei_metric_density": "compact",
                "__mei_metric_template": "row",
                "__mei_metric_inline_align": "compact",
                "__mei_metric_label_v_align": "center",
                "__mei_metric_value_v_align": "center",
                "__mei_metric_unit_v_align": "center",
                "background": {
                    "color": "rgba(98,190,235,0.10)",
                    "size": "48px 48px",
                    "position": "24px center",
                    "repeat": "no-repeat",
                },
            }),
            layout: Some(LayoutDecl {
                layout_type: "grid".to_string(),
                direction: None,
                columns: Some(vec![
                    "auto".to_string(),
                    "auto".to_string(),
                    "auto".to_string(),
                ]),
                rows: Some(vec!["1fr".to_string()]),
                areas: Some(vec![vec![
                    "label".to_string(),
                    "value".to_string(),
                    "unit".to_string(),
                ]]),
                gap: Some("4px".to_string()),
                padding: None,
                align: Some("center".to_string()),
                justify: Some("start".to_string()),
            }),
        },
        "stack" => MetricTemplatePreset {
            layout_template: "stack",
            title_ratio: 1,
            content_ratio: 1,
            shell: json!({}),
            layout: None,
        },
        "row" => MetricTemplatePreset {
            layout_template: "row",
            title_ratio: 1,
            content_ratio: 1,
            shell: json!({}),
            layout: None,
        },
        "column" => MetricTemplatePreset {
            layout_template: "column",
            title_ratio: 1,
            content_ratio: 1,
            shell: json!({}),
            layout: None,
        },
        "stack_desc" => MetricTemplatePreset {
            layout_template: "stack_desc",
            title_ratio: 1,
            content_ratio: 1,
            shell: json!({}),
            layout: None,
        },
        "plain" | "compound_sub_stack" => MetricTemplatePreset {
            layout_template: "stack",
            title_ratio: 1,
            content_ratio: 1,
            shell: json!({
                "__mei_metric_density": "normal",
                "__mei_metric_template": "stack",
                "__mei_metric_inline_align": "compact",
            }),
            layout: None,
        },
        "compound_top_row" | "solid_row_accent" | "solid_row_compact" => MetricTemplatePreset {
            layout_template: "row",
            title_ratio: 1,
            content_ratio: 1,
            shell: json!({
                "__mei_metric_density": "normal",
                "__mei_metric_template": "row",
                "__mei_metric_inline_align": "compact",
            }),
            layout: None,
        },
        _ => MetricTemplatePreset {
            layout_template: "stack",
            title_ratio: 1,
            content_ratio: 1,
            shell: json!({}),
            layout: None,
        },
    }
}

fn deep_merge_value(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                if let Some(existing) = base_map.get_mut(key) {
                    deep_merge_value(existing, value);
                } else {
                    base_map.insert(key.clone(), value.clone());
                }
            }
        }
        (base_slot, overlay) => *base_slot = overlay.clone(),
    }
}

fn merge_metric_presentation(
    args: &Value,
    source: &Value,
    patch: Option<&Value>,
) -> Option<Value> {
    let mut merged = Map::new();
    if let Some(presentation) = source
        .as_object()
        .and_then(|source| source.get("presentation"))
        .filter(|value| value.is_object())
    {
        if let Some(map) = presentation.as_object() {
            for (key, value) in map {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(presentation) = args.get("presentation").filter(|value| value.is_object()) {
        if let Some(map) = presentation.as_object() {
            for (key, value) in map {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(presentation) = patch
        .and_then(|patch| patch.get("presentation"))
        .filter(|value| value.is_object())
    {
        if let Some(map) = presentation.as_object() {
            for (key, value) in map {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    if merged.is_empty() {
        None
    } else {
        Some(Value::Object(merged))
    }
}

fn apply_presentation_icon_to_shell(props: &mut Value, presentation: &Value) {
    let map = props.as_object_mut().expect("metric card props");
    if presentation.is_object() && !presentation.as_object().is_some_and(|m| m.is_empty()) {
        map.insert(
            "__mei_metric_presentation".to_string(),
            presentation.clone(),
        );
    }
    let Some(icon) = presentation
        .get("icon")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let bg = map
        .entry("background".to_string())
        .or_insert_with(|| json!({}));
    if !bg.is_object() {
        *bg = json!({ "color": bg });
    }
    if let Some(bg_map) = bg.as_object_mut() {
        bg_map.insert("image".to_string(), json!(icon));
    }
}

fn stamp_metric_presentation_on_value_slot(blocks: &mut [UiNodeDecl], presentation: &Value) {
    for node in blocks.iter_mut() {
        let UiNodeDecl::Block(block) = node else {
            continue;
        };
        if block.props.get("metric_role").and_then(Value::as_str) != Some("value") {
            continue;
        }
        if let Some(map) = block.props.as_object_mut() {
            map.insert(
                "__mei_metric_presentation".to_string(),
                presentation.clone(),
            );
        }
        return;
    }
}

fn resolve_metric_ref_bundle_arg(
    bundle: &Value,
    constants: &BTreeMap<String, Value>,
) -> Option<String> {
    let resolved = if constants.is_empty() {
        bundle.clone()
    } else {
        crate::v2_bundle_constants::resolve_v2_constants(bundle, constants)
    };
    resolved
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
}

fn lower_v2_metric_ref(value: &Value, constants: &BTreeMap<String, Value>) -> Option<Value> {
    if v2_ref_name(value) != Some("metric_ref") {
        return None;
    }
    let args = value.get("__args")?.as_object()?;
    let metric_id = args
        .get("arg0")
        .or_else(|| args.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    let bundle = args
        .get("bundle")
        .and_then(|bundle| resolve_metric_ref_bundle_arg(bundle, constants))?;
    let mut out = Map::new();
    out.insert("__ref".to_string(), Value::String("metric".to_string()));
    out.insert("id".to_string(), Value::String(metric_id.to_string()));
    out.insert(
        "from_dataset".to_string(),
        Value::String(format!("__world_metrics__::{bundle}")),
    );
    Some(Value::Object(out))
}

fn resolve_popup_config(
    popup: &Value,
    ctx: &PanelLowerContext<'_>,
    metric_source: Option<&Value>,
) -> Value {
    if v2_ref_name(popup) == Some("link_ref") {
        if let Some(key) = v2_ref_arg0(popup) {
            if let Some(mut resolved) = resolve_link_decl_popup(ctx, key.as_str()) {
                merge_popup_metric_source(&mut resolved, metric_source, ctx);
                return resolve_config_refs_in_value(&resolved, ctx);
            }
        }
    }
    let mut resolved = resolve_config_refs_in_value(popup, ctx);
    merge_popup_metric_source(&mut resolved, metric_source, ctx);
    resolved
}

fn merge_popup_metric_source(
    popup: &mut Value,
    metric_source: Option<&Value>,
    ctx: &PanelLowerContext<'_>,
) {
    let Some(source) = metric_source else {
        return;
    };
    let Some(metric) = lower_v2_metric_ref(source, &combined_panel_constants(ctx)) else {
        return;
    };
    let Some(params) = popup
        .as_object_mut()
        .and_then(|popup| popup.get_mut("params"))
        .and_then(|params| params.as_object_mut())
    else {
        return;
    };
    if !params.contains_key("metric") {
        params.insert("metric".to_string(), metric);
    }
}

struct BoardSceneTarget {
    scene_id: String,
    scene_file: String,
    accepts: Option<Value>,
    capabilities: Option<Value>,
}

fn resolve_link_decl_popup(ctx: &PanelLowerContext<'_>, link_key: &str) -> Option<Value> {
    let payload = load_link_decl_payload(ctx, link_key)?;
    let target_ref = payload.get("target").or_else(|| payload.get("board"))?;
    let params = payload
        .get("params")
        .cloned()
        .or_else(|| payload.get("default_params").cloned())
        .unwrap_or(json!({}));
    let overlay_projection = payload
        .get("projection")
        .cloned()
        .unwrap_or(json!("overlay"));
    let popup_type = payload.get("type").cloned().unwrap_or(json!("popup"));
    let overlay_size = payload
        .get("overlay_size")
        .and_then(|v| v.as_str())
        .unwrap_or("large");
    let overlay_workspace = payload.get("overlay_workspace").cloned();
    if v2_ref_name(target_ref) == Some("panel_ref") {
        let panel_ref = v2_ref_arg0(target_ref)?;
        let panel_payload = load_panel_contract_payload(ctx, panel_ref.as_str()).ok()?;
        let panel_id = panel_payload
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .or_else(|| panel_ref.split(':').next_back().map(str::to_string))?;
        let mut popup = json!({
            "kind": "t2_panel_open",
            "mode": "popup",
            "type": popup_type.clone(),
            "projection": "t2",
            "overlay_size": overlay_size,
            "plane": "t2",
            "link_key": normalized_link_key(link_key),
            "panel_id": panel_id.clone(),
            "page_panel_id": panel_id.clone(),
            "panel_ref": panel_ref.clone(),
            "page_panel_ref": panel_ref.clone(),
            "params": params.clone(),
            "context": {
                "params": params,
            },
            "target": {
                "kind": "panel",
                "panel_id": panel_id.clone(),
                "panel_ref": panel_ref,
                "plane": "t2",
            },
        });
        if let Some(title) = panel_payload.get("title").cloned().filter(|v| !v.is_null()) {
            if let Some(map) = popup.as_object_mut() {
                map.insert("title".to_string(), title);
            }
        }
        return Some(popup);
    }
    let board_key = v2_ref_arg0(target_ref)?;
    let target = resolve_board_assembly_target(ctx, board_key.as_str())?;
    let target_scene_id = target.scene_id.clone();
    let target_scene_file = target.scene_file.clone();
    let mut target_json = json!({
        "kind": "page_instance",
        "legacy_kind": "board",
        "scene_id": target_scene_id,
        "scene_file": target_scene_file,
    });
    if let Some(map) = target_json.as_object_mut() {
        if let Some(accepts) = target.accepts.filter(|value| !value.is_null()) {
            map.insert("accepts".to_string(), accepts);
        }
        if let Some(capabilities) = target.capabilities.filter(|value| !value.is_null()) {
            map.insert("capabilities".to_string(), capabilities);
        }
    }
    let mut presentation = json!({
        "kind": "overlay_page",
        "legacy_kind": "overlay_board",
        "projection": overlay_projection.clone(),
        "type": popup_type.clone(),
        "overlay_size": overlay_size,
    });
    if let Some(workspace) = overlay_workspace.clone().filter(|v| v.is_object()) {
        if let Some(map) = presentation.as_object_mut() {
            map.insert("overlay_workspace".to_string(), workspace);
        }
    }
    let mut popup = json!({
        "kind": "scene_open",
        "mode": "popup",
        "type": popup_type.clone(),
        "projection": overlay_projection.clone(),
        "overlay_size": overlay_size,
        "link_key": normalized_link_key(link_key),
        "scene_id": target_scene_id.clone(),
        "scene_file": target_scene_file.clone(),
        "page_scene_id": target_scene_id.clone(),
        "page_scene_file": target_scene_file.clone(),
        "scene": {
            "scene_id": target_scene_id,
            "scene_file": target_scene_file,
        },
        "params": params.clone(),
        "context": {
            "params": params,
        },
        "target": target_json,
        "presentation": presentation,
    });
    if let Some(map) = popup.as_object_mut() {
        if let Some(accepts) = map
            .get("target")
            .and_then(Value::as_object)
            .and_then(|target| target.get("accepts"))
            .cloned()
        {
            map.insert("accepts".to_string(), accepts);
        }
        if let Some(capabilities) = map
            .get("target")
            .and_then(Value::as_object)
            .and_then(|target| target.get("capabilities"))
            .cloned()
        {
            map.insert("capabilities".to_string(), capabilities);
        }
    }
    if let Some(workspace) = overlay_workspace.filter(|v| v.is_object()) {
        if let Some(map) = popup.as_object_mut() {
            if let Some(size) = workspace
                .get("size")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                map.insert("overlay_size".to_string(), json!(size));
            }
            map.insert("overlay_workspace".to_string(), workspace);
        }
    }
    Some(popup)
}

fn normalized_link_key(link_key: &str) -> String {
    let trimmed = link_key.trim();
    if trimmed.starts_with("overlay/links/") {
        trimmed.to_string()
    } else if trimmed.contains('/') {
        trimmed.to_string()
    } else {
        format!("overlay/links/{trimmed}")
    }
}

fn load_link_decl_payload(ctx: &PanelLowerContext<'_>, link_key: &str) -> Option<Value> {
    let normalized = link_key.trim();
    let node = ctx.registry.nodes.iter().find(|node| {
        node.id.kind == GraphNodeKind::Navigation
            && (node.id.key == normalized
                || node.id.key.ends_with(&format!(":{normalized}"))
                || node.id.key == format!("overlay/links/{normalized}"))
    })?;
    let pref = node.payload_ref.as_ref()?;
    let artifact = load_block_artifact(ctx.app_root, pref).ok()??;
    artifact.get("payload").cloned()
}

fn resolve_board_assembly_target(
    ctx: &PanelLowerContext<'_>,
    board_key: &str,
) -> Option<BoardSceneTarget> {
    let node = ctx
        .registry
        .nodes
        .iter()
        .find(|node| node.id.kind == GraphNodeKind::AssemblyView && node.id.key == board_key)?;
    let pref = node.payload_ref.as_ref()?;
    let artifact = load_block_artifact(ctx.app_root, pref).ok()??;
    let payload = artifact.get("payload")?;
    Some(BoardSceneTarget {
        scene_id: payload.get("scene").and_then(|v| v.as_str())?.to_string(),
        scene_file: assembly_source_file_from_payload(payload)
            .unwrap_or_else(|| assembly_key_to_target(board_key)),
        accepts: payload
            .get("accepts")
            .cloned()
            .or_else(|| payload.get("params").cloned()),
        capabilities: payload.get("capabilities").cloned(),
    })
}

fn resolve_basemap_value(ctx: &PanelLowerContext<'_>, id: &str) -> Option<Value> {
    let config = load_mei_config_for_app(ctx.app_root, None);
    let entry = config.ops.basemaps.get(id)?;
    let mut map = Map::new();
    if let Some(base_url) = entry
        .tiles_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        map.insert("tilesUrl".to_string(), Value::String(base_url.to_string()));
    }
    if let Some(path) = entry
        .tilejson_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        map.insert("tilesJsonPath".to_string(), Value::String(normalized));
    }
    if let Some(style) = entry.style.as_ref().and_then(Value::as_object) {
        for (key, value) in style {
            map.insert(key.clone(), value.clone());
        }
    }
    if let Some(layer_spec) = &entry.layer_spec {
        map.insert("layerSpec".to_string(), layer_spec.clone());
    }
    Some(Value::Object(map))
}

fn resolve_config_refs_in_value(value: &Value, ctx: &PanelLowerContext<'_>) -> Value {
    let value = resolve_panel_constant_exprs(value, ctx);
    if let Some(expr) = decode_config_ref_value(&value) {
        if let Some(resolved) = resolve_config_ref_expr(expr, ctx) {
            return resolve_config_refs_in_value(&resolved, ctx);
        }
    }
    match value {
        Value::Object(ref map) => {
            if v2_ref_name(&value) == Some("link_ref") {
                if let Some(key) = v2_ref_arg0(&value) {
                    if let Some(resolved) = resolve_link_decl_popup(ctx, key.as_str()) {
                        return resolve_config_refs_in_value(&resolved, ctx);
                    }
                }
            }
            if v2_ref_name(&value) == Some("metric_ref") {
                if let Some(lowered) = lower_v2_metric_ref(&value, &combined_panel_constants(ctx)) {
                    return lowered;
                }
            }
            if v2_ref_name(&value) == Some("basemap_ref")
                || v2_call_name(&value) == Some("basemap_ref")
            {
                if let Some(key) = v2_ref_arg0(&value) {
                    if let Some(resolved) = resolve_basemap_value(ctx, key.as_str()) {
                        return resolve_config_refs_in_value(&resolved, ctx);
                    }
                }
            }
            if v2_ref_name(&value) == Some("ops_param_ref")
                || v2_call_name(&value) == Some("ops_param_ref")
            {
                if let Some(key) = v2_ref_arg0(&value) {
                    if let Some(resolved) = resolve_ops_param(ctx, key.as_str()) {
                        return resolved;
                    }
                }
            }
            if v2_ref_name(&value) == Some("world_ref")
                || v2_call_name(&value) == Some("world_ref")
            {
                if let Some(key) = v2_ref_arg0(&value) {
                    return json!(resolve_world_ref_id(ctx, key.as_str()));
                }
            }
            if v2_ref_name(&value) == Some("map_ref")
                || v2_call_name(&value) == Some("map_ref")
            {
                if let Some(key) = v2_ref_arg0(&value) {
                    if let Some(resolved) = resolve_semantic_resource_value(ctx, key.as_str(), "map_spec") {
                        return resolve_config_refs_in_value(&resolved, ctx);
                    }
                }
            }
            if v2_ref_name(&value) == Some("view_ref")
                || v2_call_name(&value) == Some("view_ref")
            {
                if let Some(key) = v2_ref_arg0(&value) {
                    if let Some(resolved) = resolve_semantic_resource_value(ctx, key.as_str(), "view_spec") {
                        return resolve_config_refs_in_value(&resolved, ctx);
                    }
                }
            }
            if v2_ref_name(&value) == Some("asset_ref") {
                return resolve_asset_value(&value, ctx.app_id);
            }
            let mut out = Map::new();
            for (key, entry) in map {
                out.insert(key.clone(), resolve_config_refs_in_value(&entry, ctx));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_config_refs_in_value(item, ctx))
                .collect(),
        ),
        other => other,
    }
}

fn resolve_ops_param(ctx: &PanelLowerContext<'_>, key: &str) -> Option<Value> {
    let config = load_mei_config_for_app(ctx.app_root, None);
    config.ops.params.get(key).cloned()
}

fn resolve_world_ref_id(ctx: &PanelLowerContext<'_>, key: &str) -> String {
    let normalized = key.trim();
    if normalized.is_empty() {
        return String::new();
    }
    if let Some(payload) = load_semantic_resource_payload(ctx, normalized, "world") {
        if let Some(id) = payload.get("id").and_then(Value::as_str).map(str::trim).filter(|id| !id.is_empty()) {
            return id.to_string();
        }
    }
    normalized.to_string()
}

fn resolve_semantic_resource_value(
    ctx: &PanelLowerContext<'_>,
    key: &str,
    expected_kind: &str,
) -> Option<Value> {
    let payload = load_semantic_resource_payload(ctx, key, expected_kind)?;
    Some(semantic_resource_value(&payload))
}

fn semantic_resource_value(payload: &Value) -> Value {
    if let Some(value) = payload.get("value") {
        return value.clone();
    }
    let Some(obj) = payload.as_object() else {
        return payload.clone();
    };
    let mut out = Map::new();
    for (key, value) in obj {
        if matches!(key.as_str(), "id" | "key" | "source_file") {
            continue;
        }
        out.insert(key.clone(), value.clone());
    }
    Value::Object(out)
}

fn load_semantic_resource_payload(
    ctx: &PanelLowerContext<'_>,
    key: &str,
    expected_kind: &str,
) -> Option<Value> {
    let normalized = key.trim();
    let node = ctx.registry.nodes.iter().find(|node| {
        node.id.kind == GraphNodeKind::SemanticGraph
            && (node.id.key == normalized
                || node.id.key == format!("{expected_kind}:{normalized}")
                || node.id.stable_key() == normalized)
    })?;
    let pref = node.payload_ref.as_ref()?;
    let artifact = load_block_artifact(ctx.app_root, pref).ok()??;
    let kind = artifact.get("kind").and_then(Value::as_str).unwrap_or_default();
    if kind != expected_kind && !(expected_kind == "world" && kind == "world") {
        return None;
    }
    artifact.get("payload").cloned()
}

fn resolve_config_ref_expr(
    expr: mei_lang_kernel::ConfigRefExpr,
    ctx: &PanelLowerContext<'_>,
) -> Option<Value> {
    match expr.kind {
        ConfigRefKind::Basemap => resolve_basemap_value(ctx, expr.id.as_str()),
        ConfigRefKind::OpsParam => resolve_ops_param(ctx, expr.id.as_str()),
        _ => None,
    }
}

fn metric_shell_props(height_px: Option<i64>, template: &str, density: &str) -> Value {
    let mut props = Map::new();
    props.insert("chrome".to_string(), json!("bare"));
    props.insert("variant".to_string(), json!("container"));
    props.insert("show_heading".to_string(), json!(false));
    props.insert(
        "padding".to_string(),
        json!(match density {
            "compact" => "4px 3px",
            "roomy" => "8px 5px",
            _ => "6px 4px",
        }),
    );
    props.insert("width".to_string(), json!("100%"));
    props.insert("box_sizing".to_string(), json!("border-box"));
    props.insert("overflow".to_string(), json!("hidden"));
    props.insert("background".to_string(), json!("transparent"));
    props.insert("__mei_metric_card".to_string(), json!(true));
    props.insert("__mei_metric_density".to_string(), json!(density));
    props.insert("__mei_metric_template".to_string(), json!(template));
    props.insert("__mei_metric_inline_align".to_string(), json!("compact"));
    props.insert("__mei_metric_title_ratio".to_string(), json!("1"));
    props.insert("__mei_metric_content_ratio".to_string(), json!("1"));
    if let Some(height_px) = height_px {
        props.insert("height".to_string(), json!(format!("{height_px}px")));
    }
    Value::Object(props)
}

fn stamp_metric_vertical_align(props: &mut Value, args: &Value) {
    let Some(map) = props.as_object_mut() else {
        return;
    };
    for role in ["label", "value", "unit", "desc"] {
        if let Some(raw) = args
            .get(format!("{role}_vertical_align"))
            .or_else(|| args.get(format!("{role}VerticalAlign")))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            map.insert(format!("__mei_metric_{role}_v_align"), json!(raw));
        }
    }
}

fn metric_slot_align(template: &str, role: &str) -> &'static str {
    if template == "column" {
        return "center";
    }
    if template == "row" {
        return match role {
            "label" | "desc" => "left",
            _ => "right",
        };
    }
    "center"
}

fn titled_shell_template_props(args: &Value) -> Value {
    let mut props = json!({
        "chrome": "default",
        "variant": "container",
        "show_heading": true,
        "border": "1px solid rgba(52, 82, 108, 0.5)",
        "radius": "4px",
        "box_sizing": "border-box",
        "overflow": "hidden"
    });
    if let Some(map) = props.as_object_mut() {
        for key in ["width", "height", "min_height", "max_height"] {
            if let Some(value) = args.get(key) {
                map.insert(key.to_string(), value.clone());
            }
        }
    }
    props
}

fn titled_shell_template_head_props() -> Value {
    json!({
        "heading_variant": "plain",
        "heading": {
            "font_family": "Microsoft YaHei Bold, Microsoft YaHei, PingFang SC, sans-serif",
            "font": "4",
            "font_weight": "700",
            "letter_spacing": "20px",
            "color": "panel_title"
        },
        "background": {
            "image": "panel_title_bar",
            "position": "center",
            "size": "100% 100%",
            "repeat": "no-repeat"
        },
        "carets": {
            "url": "/workspace-app-assets/templates/cockpit/assets/panel/caret-left-filled@3x.svg",
            "left": "26.2%",
            "right": "71.2%",
            "left_rotate": "180deg",
            "size": "14px 24px"
        },
        "height": "54px",
        "align": "center"
    })
}

/// Align v2 DSL `title_*` head fields with SSR chrome keys (`ui.star` `_panel_node` mapping).
fn finalize_panel_head_props(head_props: &mut Value) {
    let Some(map) = head_props.as_object_mut() else {
        return;
    };
    if !map.contains_key("background") {
        if let Some(value) = map.remove("title_background") {
            map.insert("background".to_string(), value);
        }
    }
    if !map.contains_key("carets") {
        if let Some(value) = map.remove("title_decor") {
            map.insert("carets".to_string(), value);
        }
    }
    if !map.contains_key("height") {
        if let Some(value) = map.remove("title_height") {
            map.insert("height".to_string(), value);
        }
    }
    if !map.contains_key("align") {
        if let Some(value) = map.remove("title_align") {
            map.insert("align".to_string(), value);
        }
    }
    map.remove("title_background");
    map.remove("title_decor");
    map.remove("title_height");
    map.remove("title_align");
}

fn merge_head_props_from_source(head_props: &mut Value, source: &Value) {
    let Some(map) = head_props.as_object_mut() else {
        return;
    };
    for key in [
        "heading_variant",
        "heading",
        "title_background",
        "title_decor",
        "title_align",
        "title_height",
    ] {
        if let Some(value) = source.get(key) {
            map.insert(key.to_string(), value.clone());
        }
    }
}

fn resolve_assets_map(value: Option<&Value>, app_id: &str) -> Value {
    let Some(obj) = value.and_then(Value::as_object) else {
        return json!({});
    };
    let mut out = Map::new();
    for (key, asset) in obj {
        out.insert(key.clone(), resolve_asset_value(asset, app_id));
    }
    Value::Object(out)
}

fn resolve_asset_value(value: &Value, app_id: &str) -> Value {
    if v2_ref_name(value) == Some("asset_ref") {
        if let Some(path) = v2_ref_arg0(value) {
            return json!(format!("/workspace-app-assets/{app_id}/assets/{path}"));
        }
    }
    value.clone()
}

fn lower_component(value: &Value, ctx: &PanelLowerContext<'_>) -> Result<BlockDecl> {
    let args = v2_call_args(value).context("component missing __args")?;
    let use_key = args
        .get("arg0")
        .and_then(|v| v.as_str())
        .context("component missing arg0")?
        .to_string();
    let mut props = args
        .get("props")
        .map(|raw| resolve_config_refs_in_value(raw, ctx))
        .unwrap_or(json!({}));
    if let Some(map) = props.as_object_mut() {
        if let Some(viewpoint) = args.get("viewpoint") {
            if let Some(id) = resolve_viewpoint_id(viewpoint) {
                map.insert("__mei_viewpoint".to_string(), json!(id));
            }
        }
        for (source_key, target_key) in [
            ("worldRef", "__mei_world_ref"),
            ("world_ref", "__mei_world_ref"),
            ("viewFamily", "__mei_view_family"),
            ("view_family", "__mei_view_family"),
            ("entityId", "entityId"),
            ("entity_id", "entityId"),
            ("groupId", "groupId"),
            ("group_id", "groupId"),
            ("cameraPreset", "cameraPreset"),
            ("camera_preset", "cameraPreset"),
        ] {
            if map.contains_key(target_key) {
                continue;
            }
            if let Some(value) = args.get(source_key).cloned() {
                map.insert(target_key.to_string(), resolve_config_refs_in_value(&value, ctx));
            }
        }
    }
    Ok(BlockDecl {
        kind: "block".to_string(),
        use_key,
        id: args.get("id").and_then(|v| v.as_str()).map(str::to_string),
        title: args
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        area: args
            .get("area")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        props,
        base: None,
        layout: args.get("layout").and_then(lower_layout),
        blocks: Vec::new(),
        component: None,
        placement: args.get("placement").cloned(),
        interactions: Vec::new(),
        lifecycle: None,
        constraints: None,
        data: None,
    })
}

pub fn panel_contract_lookup_keys(panel_key: &str, scene_id: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut push = |key: &str| {
        if !keys.iter().any(|existing| existing == key) {
            keys.push(key.to_string());
        }
    };

    if panel_key.starts_with("panel_contract:") {
        push(panel_key);
        if let Some(stripped) = panel_key.strip_prefix("panel_contract:") {
            push(stripped);
        }
        return keys;
    }

    push(&format!("panel_contract:{panel_key}"));
    push(panel_key);
    if !panel_key.contains(':') {
        push(&format!("panel_contract:{scene_id}:{panel_key}"));
        push(&format!("{scene_id}:{panel_key}"));
    }
    if let Some(basename) = panel_key.rsplit('/').next() {
        if basename != panel_key {
            push(&format!("panel_contract:{basename}"));
            push(basename);
        }
    }
    keys
}

pub fn find_panel_contract_node<'a>(
    registry: &'a McgRegistry,
    panel_key: &str,
    scene_id: &str,
) -> Option<&'a crate::mcg::registry::McgNodeRecord> {
    for key in panel_contract_lookup_keys(panel_key, scene_id) {
        if let Some(node) = registry
            .nodes
            .iter()
            .find(|node| node.id.kind == GraphNodeKind::PanelContract && node.id.key == key)
        {
            return Some(node);
        }
    }
    None
}

pub(crate) fn load_panel_contract_payload(
    ctx: &PanelLowerContext<'_>,
    ref_path: &str,
) -> Result<Value> {
    let node = find_panel_contract_node(ctx.registry, ref_path, ctx.scene_id)
        .with_context(|| format!("panel contract not found for ref `{ref_path}`"))?;
    let pref = node
        .payload_ref
        .as_ref()
        .context("panel contract missing payload ref")?;
    let artifact = load_block_artifact(ctx.app_root, pref)?
        .with_context(|| format!("panel artifact missing for ref `{ref_path}`"))?;
    let mut payload = artifact.get("payload").cloned().unwrap_or(json!({}));
    if let Some(obj) = payload.as_object_mut() {
        if let Some(viewpoints) = obj.get("viewpoints").cloned() {
            obj.insert(
                "viewpoints".to_string(),
                resolve_config_refs_in_value(&viewpoints, ctx),
            );
        }
        for key in ["worldRef", "world_ref"] {
            if let Some(value) = obj.get(key).cloned() {
                obj.insert(key.to_string(), resolve_config_refs_in_value(&value, ctx));
            }
        }
    }
    Ok(payload)
}

fn v2_call_name(value: &Value) -> Option<&str> {
    value.get("__call").and_then(|v| v.as_str())
}

fn v2_ref_name(value: &Value) -> Option<&str> {
    value.get("__ref").and_then(|v| v.as_str())
}

fn v2_call_args(value: &Value) -> Option<&Value> {
    value.get("__args")
}

fn v2_ref_arg0(value: &Value) -> Option<String> {
    value
        .get("__args")
        .and_then(|args| args.get("arg0"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};

    #[test]
    fn lower_frame_builds_viewport_from_canvas() {
        let payload = json!({
            "scene": "home",
            "canvas": {
                "__call": "viewport",
                "__args": {
                    "design_width": 1920,
                    "design_height": 1080,
                    "scale_mode": "contain",
                    "aspect_ratio": "16:9"
                }
            },
            "layout": {
                "__call": "grid",
                "__args": {
                    "columns": ["1fr"],
                    "rows": ["1064px"],
                    "areas": [["body"]]
                }
            }
        });
        let frame = lower_frame_from_assembly(&payload);
        assert_eq!(frame.kind, "frame");
        assert_eq!(frame.layout.as_ref().unwrap().layout_type, "grid");
        assert_eq!(frame.props["viewport"]["design_width"].as_i64(), Some(1920));
    }

    #[test]
    fn lower_component_block_from_v2_ir() {
        let value = json!({
            "__call": "component",
            "__args": {
                "arg0": "cockpit.header-brand",
                "viewpoint": { "__ref": "viewpoint_ref", "__args": { "arg0": "demo_stage" } },
                "worldRef": "park_world",
                "entityId": "lake_pavilion",
                "groupId": "park_story_overview",
                "cameraPreset": "park_overview_orbit",
                "props": { "title": "Demo" }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "data-demo",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "data-demo".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let block = lower_component(&value, &ctx).expect("component");
        assert_eq!(block.use_key, "cockpit.header-brand");
        assert_eq!(block.props["title"], json!("Demo"));
        assert_eq!(block.props["__mei_viewpoint"], json!("demo_stage"));
        assert_eq!(block.props["__mei_world_ref"], json!("park_world"));
        assert_eq!(block.props["entityId"], json!("lake_pavilion"));
        assert_eq!(block.props["groupId"], json!("park_story_overview"));
        assert_eq!(block.props["cameraPreset"], json!("park_overview_orbit"));
    }

    #[test]
    fn panel_contract_lookup_resolves_content_ref_basename() {
        let keys = panel_contract_lookup_keys("content/realtime-table", "home");
        assert!(keys
            .iter()
            .any(|key| key == "panel_contract:realtime-table"));
    }

    #[test]
    fn lower_screen_header_shell_emits_header_brand_block() {
        let payload = json!({
            "id": "home_header",
            "placement": {
                "__call": "absolute",
                "__args": { "top": "0", "height": "72px" }
            },
            "shell": {
                "__call": "screen_header",
                "__args": {
                    "title": "预警与问题线索 Data Demo",
                    "cap_min_width": 663,
                    "assets": {
                        "title_bg": {
                            "__ref": "asset_ref",
                            "__args": { "arg0": "header/screen-title-bg@3x.svg" }
                        }
                    }
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: std::path::Path::new("/tmp"),
            app_id: "data-demo",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "data-demo".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_panel_payload(&payload, "home:home_header", &ctx).expect("panel");
        assert_eq!(panel.blocks.len(), 1);
        let block = match &panel.blocks[0] {
            UiNodeDecl::Block(block) => block,
            other => panic!("expected block, got {other:?}"),
        };
        assert_eq!(block.use_key, "cockpit.header-brand");
        assert_eq!(block.props["title"], json!("预警与问题线索 Data Demo"));
        assert_eq!(
            block.props["assets"]["title_bg"],
            json!("/workspace-app-assets/data-demo/assets/header/screen-title-bg@3x.svg")
        );
    }

    #[test]
    fn lower_component_resolves_asset_ref_in_props() {
        let value = json!({
            "__call": "component",
            "__args": {
                "arg0": "cockpit.header-brand",
                "props": {
                    "title": "Demo",
                    "assets": {
                        "title_bg": {
                            "__ref": "asset_ref",
                            "__args": { "arg0": "header/screen-title-bg@3x.svg" }
                        }
                    }
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "data-demo",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "data-demo".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let block = lower_block_node(&value, &ctx)
            .expect("component block")
            .into_iter()
            .next()
            .expect("one block");
        let UiNodeDecl::Block(block) = block else {
            panic!("expected block");
        };
        assert_eq!(
            block.props["assets"]["title_bg"],
            json!("/workspace-app-assets/data-demo/assets/header/screen-title-bg@3x.svg")
        );
    }

    #[test]
    fn lower_panel_payload_infers_map_view_family_from_stage_block() {
        let payload = json!({
            "id": "basemap",
            "tier": "t0",
            "blocks": [{
                "__call": "component",
                "__args": {
                    "arg0": "cockpit.basemap-stage",
                    "props": {
                        "kind": "svg"
                    }
                }
            }]
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "mini-park",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "mini-park".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_panel_payload(&payload, "home:basemap", &ctx).expect("panel");
        assert_eq!(
            panel
                .props
                .get("__mei_view_family")
                .and_then(|v| v.as_str()),
            Some("map")
        );
        assert_eq!(
            panel.props.get("__mei_stage_kind").and_then(|v| v.as_str()),
            Some("map-stage")
        );
    }

    #[test]
    fn lower_panel_payload_preserves_world_view_payload_hints() {
        let payload = json!({
            "id": "world_stage",
            "tier": "t0",
            "content": {
                "__call": "world_view",
                "__args": {
                    "worldRef": "park_world"
                }
            },
            "blocks": []
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "mini-park",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "mini-park".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_panel_payload(&payload, "home:world_stage", &ctx).expect("panel");
        assert_eq!(
            panel
                .props
                .get("__mei_view_family")
                .and_then(|v| v.as_str()),
            Some("world")
        );
        assert_eq!(
            panel.props.get("__mei_world_ref").and_then(|v| v.as_str()),
            Some("park_world")
        );
        assert_eq!(
            panel.props.get("__mei_stage_kind").and_then(|v| v.as_str()),
            Some("world-stage")
        );
    }

    #[test]
    fn lower_panel_payload_marks_viewport_canvas_family() {
        let payload = json!({
            "id": "viewport_canvas",
            "tier": "t0",
            "blocks": []
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "mini-park",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "mini-park".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_panel_payload(&payload, "home:viewport_canvas", &ctx).expect("panel");
        assert_eq!(
            panel
                .props
                .get("__mei_view_family")
                .and_then(|v| v.as_str()),
            Some("canvas")
        );
        assert_eq!(
            panel.props.get("__mei_stage_kind").and_then(|v| v.as_str()),
            Some("viewport-canvas")
        );
    }

    #[test]
    fn lower_titled_shell_maps_title_chrome_to_head_props() {
        let payload = json!({
            "id": "warning",
            "shell": {
                "__call": "panel_contract",
                "__args": {
                    "title": "监督预警",
                    "title_background": {"image": "panel_title_bar"},
                    "title_decor": {
                        "url": "/workspace-app-assets/templates/cockpit/assets/panel/caret-left-filled@3x.svg",
                        "left": "26.2%",
                        "right": "71.2%"
                    },
                    "title_height": "54px",
                    "title_align": "center",
                    "blocks": []
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "data-demo",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "data-demo".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_panel_payload(&payload, "warning", &ctx).expect("panel");
        assert_eq!(
            panel.head_props["background"]["image"],
            json!("panel_title_bar")
        );
        assert!(panel
            .head_props
            .get("carets")
            .and_then(|v| v.get("url"))
            .is_some());
        assert_eq!(panel.head_props["height"], json!("54px"));
        assert_eq!(panel.head_props["align"], json!("center"));
        assert!(panel.head_props.get("title_background").is_none());
    }

    #[test]
    fn lower_section_shell_maps_padding_profile_to_body_props() {
        let payload = json!({
            "id": "enforcement",
            "shell": {
                "__call": "section_shell",
                "__args": {
                    "title": "执法要素",
                    "padding_profile": "dense_strip_100",
                    "body": {
                        "__call": "panel_contract",
                        "__args": {
                            "id": "enforcement-stats",
                            "blocks": []
                        }
                    }
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "pretty-panels",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "pretty-panels".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_panel_payload(&payload, "enforcement", &ctx).expect("panel");
        assert_eq!(panel.body_props["padding"], json!("8px 4px 2px 4px"));
        assert_eq!(panel.body_props["box_sizing"], json!("border-box"));
    }

    #[test]
    fn lower_titled_shell_hoists_expanded_panel_body_padding() {
        let payload = json!({
            "id": "enforcement",
            "shell": {
                "__call": "panel_contract",
                "__args": {
                    "title": "执法要素",
                    "title_height": "54px",
                    "props": {"height": "166px"},
                    "blocks": [{
                        "__call": "panel",
                        "__args": {
                            "id": "panel",
                            "chrome": "bare",
                            "show_heading": false,
                            "props": {
                                "padding": "8px 4px 4px 4px",
                                "background": "transparent",
                                "height": "100%"
                            },
                            "blocks": [{
                                "__call": "panel_contract",
                                "__args": {"id": "enforcement-stats", "blocks": []}
                            }]
                        }
                    }]
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "pretty-panels",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "pretty-panels".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_panel_payload(&payload, "enforcement", &ctx).expect("panel");
        assert_eq!(panel.body_props["padding"], json!("8px 4px 4px 4px"));
        let UiNodeDecl::Panel(wrapper) = &panel.blocks[0] else {
            panic!("expected wrapper panel");
        };
        assert!(wrapper.props.get("padding").is_none());
    }

    #[test]
    fn lower_expanded_titled_shell_macro_loads_blocks_not_body() {
        let payload = json!({
            "id": "warning",
            "slots": [{
                "__call": "panel_slot",
                "__args": {
                    "area": "warning",
                    "shell": {
                        "__call": "panel_contract",
                        "__args": {
                            "title": "监督预警",
                            "title_background": {"image": "panel_title_bar"},
                            "heading_variant": "plain",
                            "props": {"border": "1px solid rgba(52, 82, 108, 0.5)"},
                            "blocks": [{
                                "__ref": "panel_ref",
                                "__args": { "arg0": "content/supervision-stats" }
                            }]
                        }
                    }
                }
            }]
        });
        let mut registry = McgRegistry {
            schema_version: String::new(),
            app_id: "data-demo".to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            nodes: vec![crate::mcg::registry::McgNodeRecord {
                id: GraphNodeId::new(
                    GraphNodeKind::PanelContract,
                    "panel_contract:supervision-stats".to_string(),
                ),
                revision: String::new(),
                state: MaterialState::Ready,
                layer: "test".to_string(),
                payload_ref: None,
                deps: Vec::new(),
                owner_resource_id: None,
                assembly_inputs: Vec::new(),
            }],
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let app_root = tmp.path().join("apps/data-demo");
        let env_dir = app_root.join("env/WS-20260101.0");
        let current = app_root.join("env/current");
        let store = env_dir.join("build/store/content/panel_contract");
        std::fs::create_dir_all(&store).expect("mkdir");
        std::fs::create_dir_all(current.parent().expect("env parent")).expect("env root");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&env_dir, &current).expect("symlink env/current");
        #[cfg(not(unix))]
        std::fs::create_dir_all(&current).expect("mkdir env/current");
        let artifact = json!({
            "payload": {
                "id": "supervision-stats",
                "blocks": [{
                    "__call": "metric_card",
                    "__args": {
                        "id": "supervision_items_card",
                        "template": {"__call": "solid_stack", "__args": {}},
                        "source": {
                            "__ref": "metric_ref",
                            "__args": {
                                "arg0": "supervision_items_count",
                                "bundle": "metrics/supervision-warning.bundle.mei"
                            }
                        }
                    }
                }]
            }
        });
        let hash = "abc123";
        std::fs::write(
            store.join(format!("{hash}.json")),
            serde_json::to_string(&artifact).expect("json"),
        )
        .expect("write");
        registry.nodes[0].payload_ref = Some(PayloadRef::new(
            "panel_contract",
            hash,
            "mei-panel-contract-artifact-v1",
        ));
        let ctx = PanelLowerContext {
            app_root: app_root.as_path(),
            app_id: "data-demo",
            registry: &registry,
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel =
            lower_panel_with_slots(&payload, "warning".into(), Some("warning".into()), &ctx)
                .expect("slot panel");
        assert_eq!(
            panel.blocks.len(),
            1,
            "expanded titled_shell should lower blocks"
        );
        let nested = match &panel.blocks[0] {
            UiNodeDecl::Panel(nested) => nested,
            other => panic!("expected nested panel, got {other:?}"),
        };
        assert_eq!(nested.title.as_deref(), Some("监督预警"));
        assert!(
            !nested.blocks.is_empty(),
            "titled shell should load body panel_ref"
        );
    }

    #[test]
    fn lower_inline_panel_resolves_string_concat_panel_ids() {
        let payload = json!({
            "__call": "panel",
            "__args": {
                "id": {
                    "__binop": "Add",
                    "left": "enforcement_objects",
                    "right": "_body"
                },
                "variant": "container",
                "blocks": []
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "supervision-mini",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "supervision-mini".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_inline_panel(&payload, &ctx).expect("panel");
        assert_eq!(panel.id, "enforcement_objects_body");
    }

    #[test]
    fn lower_inline_panel_expands_nested_blocks() {
        let payload = json!({
            "id": "inspection-stats",
            "layout": {
                "__call": "grid",
                "__args": {
                    "rows": ["250px", "80px"],
                    "columns": ["1fr"],
                    "areas": [["block_upper"], ["block_ai"]],
                    "gap": "6px"
                }
            },
            "blocks": [
                {
                    "__call": "panel",
                    "__args": {
                        "id": "block_upper",
                        "area": "block_upper",
                        "variant": "container",
                        "show_heading": false,
                        "chrome": "bare",
                        "blocks": [{
                            "__call": "metric_card",
                            "__args": {
                                "id": "inspection_total_card",
                                "area": "total",
                                "template": { "__call": "solid_row_accent", "__args": {"width": "165px"} },
                                "source": {
                                    "__ref": "metric_ref",
                                    "__args": {
                                        "arg0": "inspections_total_count",
                                        "bundle": "metrics/inspection-dashboard.bundle.mei"
                                    }
                                }
                            }
                        }]
                    }
                },
                {
                    "__call": "panel",
                    "__args": {
                        "id": "block_ai",
                        "area": "block_ai",
                        "template": {
                            "__call": "panel_contract",
                            "__args": {
                                "variant": "container",
                                "show_heading": false,
                                "chrome": "bare",
                                "props": {"height": "80px"}
                            }
                        },
                        "blocks": [{
                            "__call": "metric_card",
                            "__args": {
                                "id": "ai_compound_main",
                                "area": "main",
                                "template": { "__call": "plain_metric", "__args": {} },
                                "source": {
                                    "__ref": "metric_ref",
                                    "__args": {
                                        "arg0": "ai_recognition_warnings_count",
                                        "bundle": "metrics/inspection-dashboard.bundle.mei"
                                    }
                                }
                            }
                        }]
                    }
                }
            ]
        });
        let ctx = PanelLowerContext {
            app_root: std::path::Path::new("/tmp"),
            app_id: "data-demo",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "data-demo".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let panel = lower_panel_payload(&payload, "inspection-stats", &ctx).expect("panel");
        assert_eq!(panel.blocks.len(), 2, "top-level inline panels");
        let upper = match &panel.blocks[0] {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel block_upper, got {other:?}"),
        };
        assert_eq!(upper.id, "block_upper");
        assert!(
            !upper.blocks.is_empty(),
            "block_upper should contain lowered metric_card blocks"
        );
        let ai = match &panel.blocks[1] {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel block_ai, got {other:?}"),
        };
        assert_eq!(ai.id, "block_ai");
        assert!(
            !ai.blocks.is_empty(),
            "block_ai should contain lowered metric cards"
        );
    }

    #[test]
    fn lower_metric_card_emits_mei_text_slots() {
        let value = json!({
            "__call": "metric_card",
            "__args": {
                "id": "warnings_total_card",
                "area": "warnings",
                "height_px": 86,
                "template": { "__call": "solid_stack", "__args": {} },
                "source": {
                    "__ref": "metric_ref",
                    "__args": { "arg0": "warnings_count", "bundle": "metrics/supervision-warning.bundle.mei" }
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "data-demo",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "data-demo".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let card = lower_metric_card(&value, &ctx).expect("metric card");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        assert_eq!(panel.blocks.len(), 3);
        assert!(panel
            .blocks
            .iter()
            .all(|node| matches!(node, UiNodeDecl::Block(b) if b.use_key == "mei.text")));
        assert_eq!(
            panel.props["__mei_metric_title_ratio"],
            json!("2"),
            "solid_stack template should set title/content ratio"
        );
        assert!(
            panel
                .props
                .get("border")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("rgba(98,190,235")),
            "solid_stack should inherit cockpit card border"
        );
        let value_block = match &panel.blocks[1] {
            UiNodeDecl::Block(block) => block,
            other => panic!("expected value slot block, got {other:?}"),
        };
        assert_eq!(
            value_block.props["content"],
            json!({
                "__ref": "metric",
                "id": "warnings_count",
                "from_dataset": "__world_metrics__::metrics/supervision-warning.bundle.mei",
            })
        );
    }

    #[test]
    fn lower_metric_card_honors_compile_expanded_solid_stack_template() {
        let value = json!({
            "__call": "metric_card",
            "__args": {
                "id": "supervision_items_card",
                "area": "items",
                "height_px": 86,
                "template": {
                    "__call": "panel_contract",
                    "__args": {
                        "layout": {
                            "__call": "layout_metric_stack",
                            "__args": { "title_ratio": 2, "content_ratio": 3 }
                        },
                        "props": {
                            "__mei_metric_template": "stack",
                            "__mei_metric_title_ratio": "2",
                            "__mei_metric_content_ratio": "3",
                            "border": "1px solid rgba(98,190,235,0.35)",
                            "background": { "color": "rgba(98,190,235,0.10)" }
                        }
                    }
                },
                "source": {
                    "__ref": "metric_ref",
                    "__args": {
                        "arg0": "supervision_items_count",
                        "bundle": "metrics/supervision-warning.bundle.mei"
                    }
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "data-demo",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "data-demo".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let card = lower_metric_card(&value, &ctx).expect("metric card");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        assert!(
            panel
                .props
                .get("border")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("98,190,235")),
            "expanded solid_stack template should preserve cockpit border"
        );
        assert_eq!(panel.props["__mei_metric_title_ratio"], json!("2"));
    }

    #[test]
    fn lower_v2_metric_ref_resolves_panel_bundle_constant() {
        let mut constants = BTreeMap::new();
        constants.insert(
            "INSPECTION_BUNDLE".to_string(),
            json!("metrics/inspection-dashboard.bundle.mei"),
        );
        let value = json!({
            "__ref": "metric_ref",
            "__args": {
                "arg0": "inspections_total_count",
                "bundle": { "__var": "INSPECTION_BUNDLE" }
            }
        });
        let lowered = lower_v2_metric_ref(&value, &constants).expect("metric ref");
        assert_eq!(
            lowered,
            json!({
                "__ref": "metric",
                "id": "inspections_total_count",
                "from_dataset": "__world_metrics__::metrics/inspection-dashboard.bundle.mei",
            })
        );
    }

    #[test]
    fn lower_metric_card_resolves_bundle_var_from_panel_constants() {
        let mut constants = BTreeMap::new();
        constants.insert(
            "INSPECTION_BUNDLE".to_string(),
            json!("metrics/inspection-dashboard.bundle.mei"),
        );
        let value = json!({
            "__call": "metric_card",
            "__args": {
                "id": "inspection_total_card",
                "area": "total",
                "height_px": 86,
                "template": { "__call": "solid_stack", "__args": {} },
                "source": {
                    "__ref": "metric_ref",
                    "__args": {
                        "arg0": "inspections_total_count",
                        "bundle": { "__var": "INSPECTION_BUNDLE" }
                    }
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "data-demo",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "data-demo".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: constants,
            assembly_stack_order: None,
        };
        let card = lower_metric_card(&value, &ctx).expect("metric card");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        let value_block = match &panel.blocks[1] {
            UiNodeDecl::Block(block) => block,
            other => panic!("expected value slot block, got {other:?}"),
        };
        assert_eq!(
            value_block.props["content"],
            json!({
                "__ref": "metric",
                "id": "inspections_total_count",
                "from_dataset": "__world_metrics__::metrics/inspection-dashboard.bundle.mei",
            })
        );
    }

    #[test]
    fn lower_metric_static_positional_emits_transparent_shell() {
        let value = json!({
            "__call": "metric",
            "__args": {
                "id": "demo_metric",
                "area": "first",
                "arg0": "监督事项",
                "arg1": "23",
                "arg2": "项",
                "layout_role": "solid_stack",
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "pretty-panels",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "pretty-panels".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let card = lower_metric(&value, &ctx).expect("metric");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        assert_eq!(panel.id, "demo_metric");
        assert_eq!(panel.props["background"], json!("transparent"));
        assert!(
            panel
                .props
                .get("border")
                .and_then(Value::as_str)
                .is_none_or(|border| border == "none"),
            "metric atom should not carry card border"
        );
        assert_eq!(panel.blocks.len(), 3);
    }

    #[test]
    fn lower_metric_with_metric_ref_and_popup() {
        let value = json!({
            "__call": "metric",
            "__args": {
                "id": "supervision_items_card",
                "area": "items",
                "layout_role": "solid_stack",
                "source": {
                    "__ref": "metric_ref",
                    "__args": {
                        "arg0": "supervision_items_count",
                        "bundle": "metrics/supervision-warning.bundle.mei"
                    }
                },
                "popup": {
                    "__ref": "link_ref",
                    "__args": { "arg0": "supervision-mini/home/t2/links/supervision-items-analytics" }
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "supervision-mini",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "supervision-mini".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let card = lower_metric(&value, &ctx).expect("metric");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        let value_block = match &panel.blocks[1] {
            UiNodeDecl::Block(block) => block,
            other => panic!("expected value slot block, got {other:?}"),
        };
        assert!(value_block.props.get("popup").is_some());
    }

    #[test]
    fn lower_metric_compound_top_row_layout_role() {
        let value = json!({
            "__call": "metric",
            "__args": {
                "id": "enforcement_objects_top",
                "area": "top",
                "layout_role": "compound_top_row",
                "source": {"label": "执法对象", "value": "16.4", "unit": "万"},
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "supervision-mini",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "supervision-mini".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let card = lower_metric(&value, &ctx).expect("metric");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        assert_eq!(panel.props["__mei_metric_template"], json!("row"));
    }

    #[test]
    fn panel_constant_paths_include_scene_home_panels() {
        let paths = panel_constant_candidate_paths("home:basemap");
        assert!(
            paths
                .iter()
                .any(|path| path == "src/scene/home/basemap.panel.mei"),
            "expected scene panel path, got {paths:?}"
        );
        let content_paths = panel_constant_candidate_paths("content/gis-map");
        assert!(
            content_paths
                .iter()
                .any(|path| path == "src/content/panels/gis-map.panel.mei"),
            "expected content panel path, got {content_paths:?}"
        );
    }

    #[test]
    fn lower_metric_card_applies_presentation_icon_to_shell() {
        let value = json!({
            "__call": "metric_card",
            "__args": {
                "id": "issue_pending_card",
                "area": "pending",
                "height_px": 74,
                "template": { "__call": "icon_left", "__args": {} },
                "source": {
                    "label": "待办",
                    "value": "4",
                    "unit": "件"
                },
                "presentation": {
                    "icon": "url(/workspace-app-assets/pretty-panels/assets/待办@3x.png)"
                }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "pretty-panels",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "pretty-panels".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let card = lower_metric_card(&value, &ctx).expect("metric card");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        assert_eq!(
            panel.props["background"]["image"],
            json!("url(/workspace-app-assets/pretty-panels/assets/待办@3x.png)")
        );
        assert_eq!(
            panel.props["__mei_metric_presentation"]["icon"],
            json!("url(/workspace-app-assets/pretty-panels/assets/待办@3x.png)")
        );
        let value_block = match &panel.blocks[1] {
            UiNodeDecl::Block(block) => block,
            other => panic!("expected value slot block, got {other:?}"),
        };
        assert_eq!(
            value_block.props["__mei_metric_presentation"]["icon"],
            json!("url(/workspace-app-assets/pretty-panels/assets/待办@3x.png)")
        );
    }

    #[test]
    fn lower_metric_card_presentation_overrides_source_presentation() {
        let value = json!({
            "__call": "metric_card",
            "__args": {
                "id": "issue_doing_card",
                "template": { "__call": "icon_left", "__args": {} },
                "source": {
                    "label": "在办",
                    "value": "10",
                    "unit": "件",
                    "presentation": { "icon": "url(/old.png)" }
                },
                "presentation": { "icon": "url(/new.png)" }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "pretty-panels",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "pretty-panels".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let card = lower_metric_card(&value, &ctx).expect("metric card");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        assert_eq!(panel.props["background"]["image"], json!("url(/new.png)"));
    }

    #[test]
    fn lower_metric_card_strip_icon_left_preserves_geometry_with_presentation() {
        let value = json!({
            "__call": "metric_card",
            "__args": {
                "id": "issue_rate_card",
                "template": { "__call": "strip_icon_left", "__args": {} },
                "presentation": { "icon": "url(/rate.png)" }
            }
        });
        let ctx = PanelLowerContext {
            app_root: Path::new("/tmp"),
            app_id: "pretty-panels",
            registry: &McgRegistry {
                schema_version: String::new(),
                app_id: "pretty-panels".to_string(),
                registry_revision: String::new(),
                updated_at_ms: 0,
                nodes: Vec::new(),
            },
            scene_id: "home",
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let card = lower_metric_card(&value, &ctx).expect("metric card");
        let panel = match card {
            UiNodeDecl::Panel(panel) => panel,
            other => panic!("expected panel, got {other:?}"),
        };
        assert_eq!(panel.props["background"]["image"], json!("url(/rate.png)"));
        assert_eq!(panel.props["background"]["position"], json!("24px center"));
        assert_eq!(panel.props["background"]["size"], json!("48px 48px"));
    }
}
