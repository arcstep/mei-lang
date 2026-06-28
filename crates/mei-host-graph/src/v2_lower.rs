use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mei_lang_kernel::{
    decode_config_ref_value, load_mei_config_for_app, BlockDecl, ConfigRefKind, FrameDecl,
    LayoutDecl, PanelDecl, UiNodeDecl,
};
use serde_json::{json, Map, Value};

use crate::assemble::assembly_key_to_target;
use crate::import::load_block_artifact;
use crate::mcg::registry::McgRegistry;
use crate::types::GraphNodeKind;

pub struct PanelLowerContext<'a> {
    pub app_root: &'a Path,
    pub app_id: &'a str,
    pub registry: &'a McgRegistry,
    pub scene_id: &'a str,
    /// Top-level `NAME = expr` constants from the panel `.mei` source file.
    pub panel_constants: BTreeMap<String, Value>,
}

impl<'a> PanelLowerContext<'a> {
    pub fn with_panel_constants(&self, panel_key: &str) -> Self {
        Self {
            app_root: self.app_root,
            app_id: self.app_id,
            registry: self.registry,
            scene_id: self.scene_id,
            panel_constants: load_panel_file_constants(self.app_root, panel_key),
        }
    }
}

fn normalize_panel_key_for_source(panel_key: &str) -> &str {
    panel_key
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(panel_key)
}

fn panel_source_relative_path(panel_key: &str) -> String {
    let key = normalize_panel_key_for_source(panel_key);
    let basename = key.rsplit('/').next().unwrap_or(key);
    format!("src/content/panels/{basename}.panel.mei")
}

fn load_panel_file_constants(app_root: &Path, panel_key: &str) -> BTreeMap<String, Value> {
    let path = app_root.join(panel_source_relative_path(panel_key));
    std::fs::read_to_string(path.as_path())
        .map(|content| crate::panel_constants::parse_panel_constants_from_source(&content))
        .unwrap_or_default()
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
    if constants.is_empty() {
        return value.clone();
    }
    crate::v2_bundle_constants::resolve_v2_constants(value, &constants)
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
    let area = payload.get("area").and_then(|v| v.as_str()).map(str::to_string);

    if payload.get("slots").and_then(Value::as_array).is_some() {
        return lower_panel_with_slots(payload, id, area, ctx);
    }

    if let Some(shell) = payload.get("shell") {
        return lower_panel_from_shell(payload, shell, id, area, ctx);
    }

    let mut props = json!({});
    merge_card_fields(&mut props, payload);
    if let Some(extra) = payload.get("props").filter(|value| value.is_object()) {
        deep_merge_value(&mut props, extra);
    }
    apply_placement(payload.get("placement"), &mut props);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: payload.get("title").and_then(|v| v.as_str()).map(str::to_string),
        head: None,
        area,
        layout: payload.get("layout").and_then(lower_layout),
        blocks: lower_blocks(payload.get("blocks"), ctx)?,
        slot: None,
        props,
        head_props: lower_head_props(payload),
        body_props: payload
            .get("body_props")
            .cloned()
            .unwrap_or(json!({})),
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
    apply_placement(payload.get("placement"), &mut props);

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
                {
                    lower_titled_shell_panel(shell, slot_id, slot_area, ctx, None)?
                } else {
                    lower_panel_from_generic_shell(payload, shell, slot_id, slot_area, ctx)?
                };
                blocks.push(UiNodeDecl::Panel(slot_panel));
            }
        }
    }

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
        return lower_titled_shell_panel(shell, id, area, ctx, payload.get("placement"));
    }
    match v2_call_name(shell) {
        Some("screen_header") => lower_screen_header_panel(payload, shell, id, area, ctx),
        Some("titled_shell") => lower_titled_shell_panel(
            shell,
            id,
            area,
            ctx,
            payload.get("placement"),
        ),
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
    apply_placement(payload.get("placement"), &mut props);

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

fn lower_titled_shell_panel(
    shell: &Value,
    id: String,
    area: Option<String>,
    ctx: &PanelLowerContext<'_>,
    outer_placement: Option<&Value>,
) -> Result<PanelDecl> {
    let args = v2_call_args(shell).context("titled shell missing __args")?;
    let mut props = titled_shell_template_props(args);
    merge_card_fields(&mut props, args);
    if let Some(extra) = args.get("props").filter(|value| value.is_object()) {
        deep_merge_value(&mut props, extra);
    }
    if let Some(placement) = outer_placement {
        apply_placement(Some(placement), &mut props);
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

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: args.get("title").and_then(|v| v.as_str()).map(str::to_string),
        head: None,
        area,
        layout: args.get("layout").and_then(lower_layout),
        blocks,
        slot: None,
        props,
        head_props,
        body_props: args
            .get("body_props")
            .cloned()
            .unwrap_or(json!({})),
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
    let mut props = args
        .get("props")
        .cloned()
        .unwrap_or(json!({}));
    merge_card_fields(&mut props, args);
    apply_placement(payload.get("placement"), &mut props);

    let mut head_props = lower_head_props(args);
    if let Some(heading) = args.get("heading") {
        if let Some(map) = head_props.as_object_mut() {
            map.insert("heading".to_string(), heading.clone());
        }
    }
    merge_head_props_from_source(&mut head_props, args);
    finalize_panel_head_props(&mut head_props);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: args.get("title").and_then(|v| v.as_str()).map(str::to_string),
        head: None,
        area,
        layout: args.get("layout").and_then(lower_layout),
        blocks: lower_blocks(args.get("blocks"), ctx)?,
        slot: None,
        props,
        head_props,
        body_props: args
            .get("body_props")
            .cloned()
            .unwrap_or(json!({})),
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
            map.insert(key.clone(), value.clone());
        }
    }
}

fn lower_layout(value: &Value) -> Option<LayoutDecl> {
    let layout_type = v2_call_name(value)?.to_string();
    let args = v2_call_args(value).unwrap_or(value);
    let obj = args.as_object()?;
    Some(LayoutDecl {
        layout_type,
        direction: obj.get("direction").and_then(|v| v.as_str()).map(str::to_string),
        columns: obj
            .get("columns")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            }),
        rows: obj
            .get("rows")
            .and_then(|v| v.as_array())
            .map(|items| {
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
        align: obj.get("align").and_then(|v| v.as_str()).map(str::to_string),
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
    if v2_call_name(value).as_deref() == Some("metric_card") {
        return Ok(vec![lower_metric_card(value, ctx)?]);
    }
    if v2_call_name(value).as_deref() == Some("panel") {
        return Ok(vec![UiNodeDecl::Panel(lower_inline_panel(value, ctx)?)]);
    }
    if value.get("use_key").is_some() || value.get("kind").and_then(|v| v.as_str()) == Some("block")
    {
        return Ok(vec![UiNodeDecl::Block(
            serde_json::from_value(value.clone()).context("decode legacy block")?,
        )]);
    }
    Ok(Vec::new())
}

fn lower_inline_panel(value: &Value, ctx: &PanelLowerContext<'_>) -> Result<PanelDecl> {
    let args = v2_call_args(value).context("panel missing __args")?;
    let expanded_template = metric_expanded_template_args(args.get("template"));
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("panel")
        .to_string();
    let area = args.get("area").and_then(|v| v.as_str()).map(str::to_string);

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
            props.as_object_mut()
                .expect("panel props object")
                .insert(key.to_string(), value.clone());
        } else if let Some(expanded) = expanded_template.and_then(|t| t.get(key)) {
            props.as_object_mut()
                .expect("panel props object")
                .insert(key.to_string(), expanded.clone());
        }
    }

    let layout = args
        .get("layout")
        .or_else(|| expanded_template.and_then(|t| t.get("layout")))
        .and_then(lower_layout);

    Ok(PanelDecl {
        kind: "panel".to_string(),
        id,
        title: args.get("title").and_then(|v| v.as_str()).map(str::to_string),
        head: None,
        area,
        layout,
        blocks: lower_blocks(args.get("blocks"), ctx)?,
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
    let preset = metric_template_preset(template_name.as_str());
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

    let title_ratio = metric_ratio_from_props(&props, "__mei_metric_title_ratio", preset.title_ratio);
    let content_ratio =
        metric_ratio_from_props(&props, "__mei_metric_content_ratio", preset.content_ratio);

    let source = resolve_config_refs_in_value(
        &args.get("source").cloned().unwrap_or(json!({})),
        ctx,
    );
    let map = args.get("map").cloned();
    let patch = args.get("patch").cloned();
    let popup = args
        .get("popup")
        .map(|popup| resolve_popup_config(popup, ctx, Some(&source)));
    let template_desc = args
        .get("template")
        .and_then(v2_call_args)
        .and_then(|template| template.get("desc"))
        .and_then(Value::as_str)
        .map(str::to_string);
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
        let desc_text = template_desc.or_else(|| metric_template_desc_text(args.get("template")));
        if let Some(desc) = desc_text.as_deref() {
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

    Ok(UiNodeDecl::Panel(PanelDecl {
        kind: "panel".to_string(),
        id: args
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("metric_card")
            .to_string(),
        title: None,
        head: None,
        area: args.get("area").and_then(|v| v.as_str()).map(str::to_string),
        layout,
        blocks,
        slot: None,
        props,
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
                let role = props.and_then(|p| p.get("metric_role")).and_then(Value::as_str);
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
                source,
                role,
                role,
                template,
                map,
                patch,
                popup,
                args,
                &constants,
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
            columns: Some(vec!["auto".to_string(), "auto".to_string(), "auto".to_string()]),
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
                columns: Some(vec!["auto".to_string(), "auto".to_string(), "auto".to_string()]),
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

fn resolve_link_decl_popup(ctx: &PanelLowerContext<'_>, link_key: &str) -> Option<Value> {
    let payload = load_link_decl_payload(ctx, link_key)?;
    let board_ref = payload.get("board")?;
    let board_key = v2_ref_arg0(board_ref)?;
    let (scene_id, scene_file) = resolve_board_assembly_target(ctx, board_key.as_str())?;
    let params = payload
        .get("default_params")
        .cloned()
        .unwrap_or(json!({}));
    let overlay_size = payload
        .get("overlay_size")
        .and_then(|v| v.as_str())
        .unwrap_or("large");
    Some(json!({
        "mode": "popup",
        "type": payload.get("type").cloned().unwrap_or(json!("popup")),
        "projection": payload.get("projection").cloned().unwrap_or(json!("overlay")),
        "overlay_size": overlay_size,
        "scene_id": scene_id,
        "scene_file": scene_file,
        "scene": {
            "scene_id": scene_id,
            "scene_file": scene_file,
        },
        "params": params,
    }))
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
) -> Option<(String, String)> {
    let node = ctx.registry.nodes.iter().find(|node| {
        node.id.kind == GraphNodeKind::AssemblyView && node.id.key == board_key
    })?;
    let pref = node.payload_ref.as_ref()?;
    let artifact = load_block_artifact(ctx.app_root, pref).ok()??;
    let payload = artifact.get("payload")?;
    let scene_id = payload.get("scene").and_then(|v| v.as_str())?.to_string();
    let scene_file = assembly_key_to_target(board_key);
    Some((scene_id, scene_file))
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
                if let Some(lowered) =
                    lower_v2_metric_ref(&value, &combined_panel_constants(ctx))
                {
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
            if v2_ref_name(&value) == Some("asset_ref") {
                return resolve_asset_value(&value, ctx.app_id);
            }
            let mut out = Map::new();
            for (key, entry) in map {
                out.insert(
                    key.clone(),
                    resolve_config_refs_in_value(&entry, ctx),
                );
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
    props.insert("padding".to_string(), json!(match density {
        "compact" => "4px 3px",
        "roomy" => "8px 5px",
        _ => "6px 4px",
    }));
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
            return json!(format!(
                "/workspace-app-assets/{app_id}/assets/{path}"
            ));
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
    let props = args
        .get("props")
        .map(|raw| resolve_config_refs_in_value(raw, ctx))
        .unwrap_or(json!({}));
    Ok(BlockDecl {
        kind: "block".to_string(),
        use_key,
        id: args.get("id").and_then(|v| v.as_str()).map(str::to_string),
        title: args.get("title").and_then(|v| v.as_str()).map(str::to_string),
        area: args.get("area").and_then(|v| v.as_str()).map(str::to_string),
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
        if let Some(node) = registry.nodes.iter().find(|node| {
            node.id.kind == GraphNodeKind::PanelContract && node.id.key == key
        }) {
            return Some(node);
        }
    }
    None
}

fn load_panel_contract_payload(ctx: &PanelLowerContext<'_>, ref_path: &str) -> Result<Value> {
    let node = find_panel_contract_node(ctx.registry, ref_path, ctx.scene_id)
        .with_context(|| format!("panel contract not found for ref `{ref_path}`"))?;
    let pref = node
        .payload_ref
        .as_ref()
        .context("panel contract missing payload ref")?;
    let artifact = load_block_artifact(ctx.app_root, pref)?
        .with_context(|| format!("panel artifact missing for ref `{ref_path}`"))?;
    Ok(artifact.get("payload").cloned().unwrap_or(json!({})))
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
        assert_eq!(
            frame.props["viewport"]["design_width"].as_i64(),
            Some(1920)
        );
    }

    #[test]
    fn lower_component_block_from_v2_ir() {
        let value = json!({
            "__call": "component",
            "__args": {
                "arg0": "cockpit.header-brand",
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
        };
        let block = lower_component(&value, &ctx).expect("component");
        assert_eq!(block.use_key, "cockpit.header-brand");
        assert_eq!(block.props["title"], json!("Demo"));
    }

    #[test]
    fn panel_contract_lookup_resolves_content_ref_basename() {
        let keys = panel_contract_lookup_keys("content/realtime-table", "home");
        assert!(keys.iter().any(|key| key == "panel_contract:realtime-table"));
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
        };
        let panel = lower_panel_payload(&payload, "warning", &ctx).expect("panel");
        assert_eq!(
            panel.head_props["background"]["image"],
            json!("panel_title_bar")
        );
        assert!(
            panel
                .head_props
                .get("carets")
                .and_then(|v| v.get("url"))
                .is_some()
        );
        assert_eq!(panel.head_props["height"], json!("54px"));
        assert_eq!(panel.head_props["align"], json!("center"));
        assert!(panel.head_props.get("title_background").is_none());
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
        let store = app_root.join("build/active/store/content/panel_contract");
        std::fs::create_dir_all(&store).expect("mkdir");
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
        };
        let panel = lower_panel_with_slots(&payload, "warning".into(), Some("warning".into()), &ctx)
            .expect("slot panel");
        assert_eq!(panel.blocks.len(), 1, "expanded titled_shell should lower blocks");
        let nested = match &panel.blocks[0] {
            UiNodeDecl::Panel(nested) => nested,
            other => panic!("expected nested panel, got {other:?}"),
        };
        assert_eq!(nested.title.as_deref(), Some("监督预警"));
        assert!(!nested.blocks.is_empty(), "titled shell should load body panel_ref");
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
        assert!(!ai.blocks.is_empty(), "block_ai should contain lowered metric cards");
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
}
