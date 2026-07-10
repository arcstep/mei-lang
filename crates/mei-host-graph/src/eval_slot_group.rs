//! `eval.slot_group[*]` artifact helpers.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{BlockDecl, CompiledApp, DataMode, MetricShape, UiNodeDecl, UiTreeNode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::compose_chrome::{
    build_head_chrome, build_panel_shell, section_id_for_head_scope, should_export_panel_shell,
    ThemeResolveContext,
};
use crate::content_store::{put_if_absent, EVAL_SLOT_GROUP_KIND};
use crate::layer_store::{layer_entry_meta, store_layer, take_layer};
use crate::mrg::registry::MrgRegistryWriter;
use crate::semantic_cache::SemanticCacheCore;
use crate::structure_full::build_structure_full_document;
use crate::structure_full::slot_group_id_for_node;
use crate::view_artifact::StructureFullNode;
use crate::types::{MaterialState, PayloadRef};
use crate::view_artifact::eval_slot_group_cache_key;

pub const EVAL_SLOT_GROUP_SCHEMA: &str = "eval-slot-group-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalSlotGroupDocument {
    pub schema_version: String,
    pub slot_group_id: String,
    pub data_mode: String,
    pub slots: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_seed: Option<Value>,
}

pub fn collect_slot_groups(structure: &crate::view_artifact::StructureFullDocument) -> Vec<String> {
    let mut groups = BTreeMap::new();
    groups.insert("scene:default".to_string(), ());
    for node in &structure.nodes {
        if matches!(node.ui_role.as_str(), "slot" | "section" | "content") {
            groups.insert(slot_group_id_for_node(node), ());
        }
    }
    groups.into_keys().collect()
}

fn material_state_slug(state: &MaterialState) -> &'static str {
    match state {
        MaterialState::Missing => "missing",
        MaterialState::Warming => "warming",
        MaterialState::Ready => "ready",
        MaterialState::Stale => "stale",
        MaterialState::Failed => "failed",
    }
}

fn delivery_class_for_metric(
    manifest: Option<&crate::mrg::client_bootstrap::ClientBootstrapManifest>,
    metric_id: &str,
) -> String {
    manifest
        .and_then(|doc| {
            doc.metrics
                .iter()
                .find(|metric| metric.id == metric_id)
                .map(|metric| match metric.contract.shape {
                    MetricShape::Dataframe => "dataframe_page1".to_string(),
                    MetricShape::Scalar => "metric_scalar".to_string(),
                    _ => "metric_scalar".to_string(),
                })
        })
        .unwrap_or_else(|| "metric_scalar".to_string())
}

fn slot_mount_json(
    slot: &crate::mrg::registry::MrgSlotRecord,
    data_mode: &str,
    manifest: Option<&crate::mrg::client_bootstrap::ClientBootstrapManifest>,
) -> Value {
    let metric_id = slot
        .slot_id
        .node
        .key
        .split("::")
        .nth(1)
        .unwrap_or(slot.owner_resource_id.as_str())
        .to_string();
    json!({
        "metric_id": metric_id,
        "slot_key": format!("{}::{}", slot.slot_id.node.key, slot.slot_id.scope_key),
        "owner_resource_id": slot.owner_resource_id,
        "payload_ref": slot.payload_ref,
        "state": material_state_slug(&slot.state),
        "data_mode": data_mode,
        "client_eligible": slot.client_eligible,
        "delivery_class": delivery_class_for_metric(manifest, metric_id.as_str()),
    })
}

fn scene_mounts(
    workspace_root: Option<&Path>,
    app_id: &str,
    scene_id: &str,
    data_mode: &str,
) -> Vec<Value> {
    let Some(workspace_root) = workspace_root else {
        return Vec::new();
    };
    let manifest =
        crate::mrg::client_bootstrap::read_client_bootstrap(workspace_root, app_id, scene_id);
    let registry = MrgRegistryWriter::load(workspace_root, app_id);
    registry
        .slots
        .iter()
        .filter(|slot| slot.slot_id.scope_key == scene_id)
        .map(|slot| slot_mount_json(slot, data_mode, manifest.as_ref()))
        .collect()
}

fn block_id_matches(block: &BlockDecl, label: &str) -> bool {
    block
        .id
        .as_deref()
        .is_some_and(|id| id.trim() == label)
}

fn push_block_mount(block: &BlockDecl, out: &mut Vec<Value>) {
    let mut entry = json!({
        "use_key": block.use_key,
        "props": block.props,
    });
    if let Some(obj) = entry.as_object_mut() {
        if let Some(area) = block.area.as_ref().map(|a| a.trim()).filter(|a| !a.is_empty()) {
            obj.insert("area".to_string(), Value::String(area.to_string()));
        }
        if let Some(id) = block.id.as_ref().map(|a| a.trim()).filter(|a| !a.is_empty()) {
            obj.insert("block_id".to_string(), Value::String(id.to_string()));
        }
    }
    out.push(entry);
}

/// Leaf content scopes end with `…/<area>/<use_key>` (e.g. `…/active/mei.text`).
fn scope_component_area(preview_scope: &str) -> Option<String> {
    let parts: Vec<&str> = preview_scope
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let last = *parts.last()?;
    if !last.contains('.') {
        return None;
    }
    if parts.len() < 2 {
        return None;
    }
    let area = parts[parts.len() - 2].trim();
    if area.is_empty() || area.contains('.') {
        return None;
    }
    Some(area.to_string())
}

fn mount_area_str(mount: &Value) -> &str {
    mount
        .get("area")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
}

/// When a parent panel exports every sibling `mei.text` / `chart.*`, keep only the
/// mount that belongs to this leaf's grid area (or nested panel id).
fn narrow_mounts_for_content_scope(
    node: &StructureFullNode,
    contract: &mei_lang_kernel::SceneContract,
    mounts: Vec<Value>,
) -> Vec<Value> {
    if mounts.len() <= 1 {
        return mounts;
    }
    let Some(area) = scope_component_area(&node.preview_scope) else {
        return mounts;
    };
    let by_area: Vec<Value> = mounts
        .iter()
        .filter(|m| {
            let mount_area = mount_area_str(m);
            mount_area == area
        })
        .cloned()
        .collect();
    if by_area.len() == 1 {
        return by_area;
    }
    if !by_area.is_empty() {
        return by_area;
    }
    // Nested panel(id|area = leaf area) with auto-area child block.
    if let Some(panel) = find_panel_in_contract(contract, area.as_str()) {
        let mut local = Vec::new();
        let key = node
            .content_kind
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .or_else(|| node.use_keys.first().map(String::as_str));
        if let Some(key) = key {
            collect_component_mounts_for_use_key(panel, key, &mut local);
        }
        if !local.is_empty() {
            return local;
        }
    }
    mounts
}

fn push_panel_blocks(panel: &UiNodeDecl, out: &mut Vec<Value>) {
    if let Some(head) = panel.head.as_ref() {
        if let UiTreeNode::Block(block) = head.as_ref() {
            push_block_mount(block, out);
        }
    }
    // Direct blocks only. Nested panels (e.g. compound-metric children) have their
    // own structure content nodes and must not be re-exported on the parent.
    for child in &panel.blocks {
        match child {
            UiTreeNode::Block(block) => push_block_mount(block, out),
            UiTreeNode::Panel(_) | UiTreeNode::PanelRefEmbed(_) => {}
        }
    }
}

fn panel_is_metric_card(panel: &UiNodeDecl) -> bool {
    panel
        .props
        .get("__mei_metric_card")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn push_metric_card_shell_mount(panel: &UiNodeDecl, out: &mut Vec<Value>) {
    if !panel_is_metric_card(panel) {
        return;
    }
    out.push(json!({
        "use_key": "metric-card",
        "mount_role": "shell",
        "props": panel.props,
    }));
}

fn block_use_key_matches(block: &BlockDecl, use_key: &str) -> bool {
    block.use_key.trim() == use_key.trim()
}

fn collect_component_mounts_for_use_key(panel: &UiNodeDecl, use_key: &str, out: &mut Vec<Value>) {
    let use_key = use_key.trim();
    if use_key.is_empty() {
        return;
    }
    for child in &panel.blocks {
        match child {
            UiTreeNode::Block(block) if block_use_key_matches(block, use_key) => {
                push_block_mount(block, out);
            }
            UiTreeNode::Panel(nested) => collect_component_mounts_for_use_key(nested, use_key, out),
            UiTreeNode::Block(_) | UiTreeNode::PanelRefEmbed(_) => {}
        }
    }
}

fn collect_component_mounts_for_label(panel: &UiNodeDecl, label: &str, out: &mut Vec<Value>) {
    if panel.id == label {
        push_metric_card_shell_mount(panel, out);
        push_panel_blocks(panel, out);
        return;
    }
    if let Some(head) = panel.head.as_ref() {
        if let UiTreeNode::Block(block) = head.as_ref() {
            if block_id_matches(block, label) {
                push_block_mount(block, out);
            }
        }
    }
    for child in &panel.blocks {
        match child {
            UiTreeNode::Block(block) if block_id_matches(block, label) => {
                push_block_mount(block, out);
            }
            UiTreeNode::Panel(nested) => collect_component_mounts_for_label(nested, label, out),
            UiTreeNode::Block(_) | UiTreeNode::PanelRefEmbed(_) => {}
        }
    }
}

fn is_duplicate_metric_card_leaf_scope(scope: &str) -> bool {
    let scope = scope.trim().to_ascii_lowercase();
    scope.ends_with("/label/mei.text")
        || scope.ends_with("/value/mei.text")
        || scope.ends_with("/unit/mei.text")
}

fn is_ambiguous_mount_label(label: &str) -> bool {
    matches!(
        label.trim().to_ascii_lowercase().as_str(),
        "label" | "value" | "unit" | "icon" | "head" | "mei.text" | "panel" | ""
    )
}

fn panel_lookup_label(node: &StructureFullNode) -> String {
    if let Some(hint) = metric_card_panel_hint_from_scope(&node.preview_scope) {
        if !hint.is_empty() {
            return hint;
        }
    }
    let label = node.label.trim();
    if !label.is_empty() && !is_ambiguous_mount_label(label) {
        return label.to_string();
    }
    let scope_label = node
        .preview_scope
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim();
    if !scope_label.is_empty() && !is_ambiguous_mount_label(scope_label) {
        return scope_label.to_string();
    }
    String::new()
}

fn author_panel_props_for_shell(
    workspace_root: &Path,
    compiled: &CompiledApp,
    panel_id: &str,
) -> Option<Value> {
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, compiled.app_id.as_str());
    let registry = crate::mcg::registry::McgRegistryWriter::load(workspace_root, compiled.app_id.as_str());
    let scene_id = compiled
        .active_scene
        .as_deref()
        .unwrap_or("home");
    let ctx = crate::v2_lower::PanelLowerContext {
        app_root: app_root.as_path(),
        app_id: compiled.app_id.as_str(),
        registry: &registry,
        scene_id,
        panel_constants: std::collections::BTreeMap::new(),
        assembly_stack_order: None,
    };
    for ref_path in [
        format!("content/{panel_id}"),
        panel_id.to_string(),
        format!("supervision-mini/home/t1/r-right-rail/s-warning/content/{panel_id}"),
    ] {
        if let Ok(payload) = crate::v2_lower::load_content_panel_payload(&ctx, ref_path.as_str()) {
            if let Some(raw_props) = payload.get("props") {
                let props = crate::v2_lower::resolve_panel_props_for_shell(raw_props, &ctx);
                if props.get("background").is_some() {
                    return Some(props);
                }
            }
        }
    }
    None
}

fn panel_decl_for_shell_export(
    panel: &UiNodeDecl,
    author_props: Option<&Value>,
) -> UiNodeDecl {
    let mut exported = panel.clone();
    // Flatten unresolved `props = base | shell_props` before shell export.
    exported.props = flatten_merged_panel_props(&exported.props);
    let Some(exported_props) = exported.props.as_object_mut() else {
        return exported;
    };
    let current_bg = exported_props.get("background");
    let needs_author_bg = match current_bg {
        None => true,
        Some(Value::String(bg)) => bg.eq_ignore_ascii_case("transparent"),
        Some(Value::Object(map)) => {
            let color = map
                .get("color")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let image = map.get("image");
            let has_image = match image {
                Some(Value::Array(items)) => items.iter().any(|item| {
                    item.as_str().is_some_and(|s| !s.trim().is_empty()) || item.is_object()
                }),
                Some(Value::String(s)) => !s.trim().is_empty(),
                Some(_) => true,
                None => false,
            };
            !has_image && (color.is_empty() || color.eq_ignore_ascii_case("transparent"))
        }
        Some(_) => false,
    };
    if !needs_author_bg {
        return exported;
    }
    if let Some(author_props) = author_props {
        if let Some(background) = author_props.get("background") {
            exported_props.insert("background".to_string(), background.clone());
            for key in [
                "padding",
                "margin",
                "width",
                "height",
                "min_height",
                "overflow",
                "border",
                "radius",
                "__mei_slot_frame_bg",
            ] {
                if let Some(value) = author_props.get(key) {
                    exported_props.insert(key.to_string(), value.clone());
                }
            }
            return exported;
        }
    }
    if exported_props
        .get("__mei_layout_fill")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        exported_props.insert("background".to_string(), json!("panel_glow_bg"));
    }
    exported
}

fn flatten_merged_panel_props(props: &Value) -> Value {
    if props.get("__binop").and_then(Value::as_str) != Some("Merge") {
        return props.clone();
    }
    let mut merged = props
        .get("left")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(right) = props.get("right") {
        if let (Some(base), Some(overlay)) = (merged.as_object_mut(), right.as_object()) {
            for (key, value) in overlay {
                base.insert(key.clone(), value.clone());
            }
        }
    }
    merged
}

fn metric_card_panel_hint_from_scope(scope: &str) -> Option<String> {
    // Prefer the deepest segment so nested scopes under a compound card
    // (e.g. …/ai_compound_card/main) do not resolve to the ancestor shell.
    for segment in scope.split('/').rev() {
        if let Some(id) = segment.strip_suffix("_card_content") {
            if !id.is_empty() {
                return Some(format!("{id}_card"));
            }
        }
        // slot_metric_shell pattern: …/foo_content → outer shell panel id `foo`
        // (background / __mei_slot_frame_bg live on the shell, not the inner metric).
        if let Some(id) = segment.strip_suffix("_content") {
            if !id.is_empty() && !matches!(id, "label" | "value" | "unit" | "desc" | "content") {
                return Some(id.to_string());
            }
        }
        if segment.ends_with("_card") && !segment.ends_with("_card_content") {
            return Some(segment.to_string());
        }
    }
    None
}

/// Panel shell (slot-frame background) must bind only to the panel that owns
/// the frame — not to nested compound areas (main/rtop/rbottom) or child cards.
fn panel_shell_lookup_matches_node(node: &StructureFullNode, panel_id: &str) -> bool {
    let panel_id = panel_id.trim();
    if panel_id.is_empty() {
        return false;
    }
    if node.content_kind.as_deref() == Some("compound-metric") {
        return node
            .preview_scope
            .rsplit('/')
            .map(str::trim)
            .any(|segment| segment == panel_id);
    }
    let leaf = node
        .preview_scope
        .rsplit('/')
        .next()
        .map(str::trim)
        .unwrap_or("");
    leaf == panel_id
        || leaf == format!("{panel_id}_content")
        || leaf == format!("{panel_id}_card_content")
        || leaf.strip_suffix("_content") == Some(panel_id)
}

fn metric_card_lookup_label(node: &StructureFullNode) -> String {
    let label = panel_lookup_label(node);
    if label.ends_with("_card_content") || label.ends_with("_content") {
        if let Some(hint) = metric_card_panel_hint_from_scope(&node.preview_scope) {
            return hint;
        }
    }
    label
}

fn metric_card_content_panel_lookup(node: &StructureFullNode) -> Option<String> {
    if node.ui_role != "content" {
        return None;
    }
    let kind = node.content_kind.as_deref()?.to_ascii_lowercase();
    if !matches!(kind.as_str(), "stack" | "stack_desc" | "row") {
        return None;
    }
    node.preview_scope
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|segment| !segment.is_empty() && !is_ambiguous_mount_label(segment))
        .map(|segment| segment.to_string())
}

fn content_panel_lookup_label(node: &StructureFullNode) -> String {
    if node.content_kind.as_deref() == Some("compound-metric") {
        if let Some(id) = node
            .preview_scope
            .rsplit('/')
            .map(str::trim)
            .find(|segment| !segment.is_empty() && !is_ambiguous_mount_label(segment))
        {
            return id.to_string();
        }
    }
    let resolved = metric_card_lookup_label(node);
    if resolved.is_empty() {
        panel_lookup_label(node)
    } else {
        resolved
    }
}

fn panel_background_is_transparent(background: &Value) -> bool {
    match background {
        Value::String(raw) => raw.eq_ignore_ascii_case("transparent"),
        Value::Object(map) => {
            let color = map
                .get("color")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            color.is_empty() || color.eq_ignore_ascii_case("transparent")
        }
        _ => false,
    }
}

fn panel_has_slot_frame_shell(panel: &UiNodeDecl) -> bool {
    panel
        .props
        .get("__mei_slot_frame_bg")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || panel
            .props
            .get("background")
            .is_some_and(|background| !panel_background_is_transparent(background))
}

fn compound_metric_shell_panel<'a>(
    contract: &'a mei_lang_kernel::SceneContract,
    content_panel_id: &str,
) -> Option<&'a UiNodeDecl> {
    if let Some(panel) = find_panel_in_contract(contract, content_panel_id) {
        if panel_has_slot_frame_shell(panel) {
            return Some(panel);
        }
    }
    if let Some(shell_id) = content_panel_id.strip_suffix("_body") {
        if let Some(shell) = find_panel_in_contract(contract, shell_id) {
            if panel_has_slot_frame_shell(shell) {
                return Some(shell);
            }
        }
    }
    None
}

fn find_panel_by_id<'a>(panel: &'a UiNodeDecl, target: &str) -> Option<&'a UiNodeDecl> {
    if panel.id == target {
        return Some(panel);
    }
    for child in &panel.blocks {
        if let UiTreeNode::Panel(nested) = child {
            if let Some(found) = find_panel_by_id(nested, target) {
                return Some(found);
            }
        }
    }
    None
}

fn find_panel_in_contract<'a>(
    contract: &'a mei_lang_kernel::SceneContract,
    target: &str,
) -> Option<&'a UiNodeDecl> {
    contract
        .panels
        .iter()
        .find_map(|panel| find_panel_by_id(panel, target))
}

fn is_section_head_text_slot(node: &StructureFullNode) -> bool {
    let scope = node.preview_scope.to_ascii_lowercase();
    scope.ends_with("/title_zone/mei.text")
        || scope.ends_with("/head/mei.text")
        || (scope.ends_with("/title_zone") && node.content_kind.as_deref() == Some("mei.text"))
        || (scope.ends_with("/head") && node.content_kind.as_deref() == Some("mei.text"))
}

fn component_mounts_for_content_node(compiled: &CompiledApp, node: &StructureFullNode) -> Vec<Value> {
    if node.ui_role != "content" {
        return Vec::new();
    }
    if is_duplicate_metric_card_leaf_scope(&node.preview_scope) {
        return Vec::new();
    }
    if is_section_head_text_slot(node) {
        return Vec::new();
    }
    // Compound hosts only provide layout + panel_shell. Child metric cards export
    // their own mounts; aggregating here double-paints label/value/unit on the parent.
    if node.content_kind.as_deref() == Some("compound-metric") {
        return Vec::new();
    }
    let label = node.label.trim();
    let Some(contract) = compiled.scene_contract.as_ref() else {
        return Vec::new();
    };
    let panel_hint = metric_card_panel_hint_from_scope(&node.preview_scope);
    let mut mounts = Vec::new();
    if let Some(panel_id) = metric_card_content_panel_lookup(node) {
        if let Some(panel) = find_panel_in_contract(contract, panel_id.as_str()) {
            collect_component_mounts_for_label(panel, panel_id.as_str(), &mut mounts);
        }
        if mounts.is_empty() {
            for panel in &contract.panels {
                collect_component_mounts_for_label(panel, panel_id.as_str(), &mut mounts);
                if !mounts.is_empty() {
                    break;
                }
            }
        }
        if !mounts.is_empty() {
            return narrow_mounts_for_content_scope(node, contract, mounts);
        }
    }
    if let Some(panel_id) = panel_hint.as_deref() {
        if let Some(panel) = find_panel_in_contract(contract, panel_id) {
            collect_component_mounts_for_label(panel, panel_id, &mut mounts);
            if !mounts.is_empty() {
                return narrow_mounts_for_content_scope(node, contract, mounts);
            }
        }
    }
    let lookup_label = metric_card_lookup_label(node);
    if is_ambiguous_mount_label(label) {
        if let Some(panel_id) = panel_hint.as_deref() {
            if let Some(panel) = find_panel_in_contract(contract, panel_id) {
                collect_component_mounts_for_label(panel, panel_id, &mut mounts);
            }
        }
    } else if !lookup_label.is_empty() {
        if let Some(panel) = find_panel_in_contract(contract, lookup_label.as_str()) {
            collect_component_mounts_for_label(panel, lookup_label.as_str(), &mut mounts);
        } else {
            for panel in &contract.panels {
                collect_component_mounts_for_label(panel, lookup_label.as_str(), &mut mounts);
            }
        }
    }
    if mounts.is_empty() {
        // Only abort use_key fallback when a concrete panel was found but produced
        // no mounts. Display labels like "分组柱图" are not panel ids — falling
        // through lets chart.column / map.* keep their authored props.
        let panel_found = !lookup_label.is_empty()
            && !is_ambiguous_mount_label(label)
            && find_panel_in_contract(contract, lookup_label.as_str()).is_some();
        if panel_found {
            return narrow_mounts_for_content_scope(node, contract, mounts);
        }
        for use_key in &node.use_keys {
            let key = use_key.trim();
            if key.is_empty() || key == "metric-card" {
                continue;
            }
            // Content-group parents (chart-summary / metric-summary / …) aggregate
            // child use_keys. Only the leaf whose content_kind matches should export
            // component mounts — otherwise chart.column mounts land on the layout
            // host and get mounted into the first metric-card slot.
            if let Some(kind) = node.content_kind.as_deref() {
                let kind = kind.trim();
                if !kind.is_empty() && kind != key {
                    continue;
                }
            }
            if is_ambiguous_mount_label(label) {
                if let Some(panel_id) = panel_hint.as_deref() {
                    if let Some(panel) = find_panel_in_contract(contract, panel_id) {
                        collect_component_mounts_for_use_key(panel, key, &mut mounts);
                    }
                }
                continue;
            }
            // Prefer panels whose id appears in preview_scope (deepest match first).
            // A naive full-scene scan would bind penalty chart.column props onto
            // the inspection chart slot.
            let mut scope_panels: Vec<&UiNodeDecl> = Vec::new();
            for segment in node.preview_scope.split('/') {
                let segment = segment.trim();
                if segment.is_empty() || segment.contains('.') {
                    continue;
                }
                if let Some(panel) = find_panel_in_contract(contract, segment) {
                    if !scope_panels.iter().any(|p| p.id == panel.id) {
                        scope_panels.push(panel);
                    }
                }
            }
            let mut found = false;
            for panel in scope_panels.iter().rev() {
                let mut local = Vec::new();
                collect_component_mounts_for_use_key(panel, key, &mut local);
                if !local.is_empty() {
                    mounts.extend(local);
                    found = true;
                    break;
                }
            }
            if !found {
                // Last resort: unique use_key across the contract.
                let mut all = Vec::new();
                for panel in &contract.panels {
                    collect_component_mounts_for_use_key(panel, key, &mut all);
                }
                if all.len() == 1 {
                    mounts.extend(all);
                }
            }
        }
    }
    narrow_mounts_for_content_scope(node, contract, mounts)
}

pub fn build_eval_slot_group_document(
    compiled: &CompiledApp,
    structure: &crate::view_artifact::StructureFullDocument,
    slot_group_id: &str,
    data_mode: DataMode,
    workspace_root: Option<&Path>,
) -> EvalSlotGroupDocument {
    let mode_slug = data_mode.slug();
    let scene_id = compiled
        .active_scene
        .clone()
        .unwrap_or_else(|| structure.scene_id.clone());
    let scene_mounts = scene_mounts(workspace_root, compiled.app_id.as_str(), scene_id.as_str(), mode_slug);
    let theme_ctx = workspace_root.and_then(|root| ThemeResolveContext::from_compiled(root, compiled));
    let mut slots = BTreeMap::new();
    for node in &structure.nodes {
        if slot_group_id_for_node(node) != slot_group_id {
            continue;
        }
        let scope_key = if node.preview_scope.trim().is_empty() {
            "scene:default".to_string()
        } else {
            node.preview_scope.clone()
        };
        let mut entry = json!({
            "node_id": node.node_id,
            "ui_role": node.ui_role,
            "label": node.label,
            "content_kind": node.content_kind,
            "panel_id": node.panel_id,
            "use_keys": node.use_keys,
        });
        if node.ui_role == "content" {
            let component_mounts = component_mounts_for_content_node(compiled, node);
            if !component_mounts.is_empty() {
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert("component_mounts".to_string(), Value::Array(component_mounts));
                }
            }
        }
        if let (Some(ctx), Some(contract)) = (theme_ctx.as_ref(), compiled.scene_contract.as_ref()) {
            if matches!(node.ui_role.as_str(), "section" | "slot" | "content") {
                let panel_lookup = content_panel_lookup_label(node);
                if !panel_lookup.is_empty()
                    && panel_shell_lookup_matches_node(node, panel_lookup.as_str())
                {
                    let panel = if node.content_kind.as_deref() == Some("compound-metric") {
                        compound_metric_shell_panel(contract, panel_lookup.as_str()).or_else(|| {
                            find_panel_in_contract(contract, panel_lookup.as_str())
                        })
                    } else {
                        find_panel_in_contract(contract, panel_lookup.as_str())
                    };
                    if let Some(panel) = panel {
                        if should_export_panel_shell(panel) {
                            let author_props = workspace_root.and_then(|root| {
                                author_panel_props_for_shell(root, compiled, panel_lookup.as_str())
                            });
                            let export_panel =
                                panel_decl_for_shell_export(panel, author_props.as_ref());
                            if let Some(obj) = entry.as_object_mut() {
                                obj.insert(
                                    "panel_shell".to_string(),
                                    build_panel_shell(&export_panel, ctx),
                                );
                            }
                        }
                    }
                }
            }
        }
        if let Some(section_id) = section_id_for_head_scope(scope_key.as_str()) {
            if let (Some(ctx), Some(contract)) = (theme_ctx.as_ref(), compiled.scene_contract.as_ref()) {
                if let Some(panel) = find_panel_in_contract(contract, section_id.as_str()) {
                    let chrome = build_head_chrome(panel, ctx);
                    if !chrome.is_null() {
                        if let Some(obj) = entry.as_object_mut() {
                            obj.insert("head_chrome".to_string(), chrome);
                        }
                    }
                }
            }
        }
        if slot_group_id == "scene:default" {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("mounts".to_string(), Value::Array(scene_mounts.clone()));
            }
        }
        slots.insert(scope_key, entry);
    }
    if slots.is_empty() && slot_group_id == "scene:default" {
        slots.insert(
            "scene:default".to_string(),
            json!({
                "scene_id": scene_id,
                "app_id": compiled.app_id,
                "mounts": scene_mounts,
            }),
        );
    }
    EvalSlotGroupDocument {
        schema_version: EVAL_SLOT_GROUP_SCHEMA.to_string(),
        slot_group_id: slot_group_id.to_string(),
        data_mode: mode_slug.to_string(),
        slots,
        bootstrap_seed: if slot_group_id == "scene:default" {
            workspace_root.and_then(|root| {
                crate::mrg::client_bootstrap::client_bootstrap_eval_seed_json(
                    root,
                    compiled.app_id.as_str(),
                    scene_id.as_str(),
                )
            })
        } else {
            None
        },
    }
}

pub fn persist_eval_slot_group(app_root: &Path, document: &EvalSlotGroupDocument) -> Result<PayloadRef> {
    let bytes = serde_json::to_vec(document)?;
    let put = put_if_absent(app_root, EVAL_SLOT_GROUP_KIND, &bytes)?;
    Ok(PayloadRef::new(
        EVAL_SLOT_GROUP_KIND,
        put.content_hash,
        EVAL_SLOT_GROUP_SCHEMA,
    ))
}

pub fn ensure_eval_slot_group_cached(
    workspace_root: &Path,
    compiled: &CompiledApp,
    semantic_core: &SemanticCacheCore,
    slot_group_id: &str,
    data_mode: DataMode,
    layout_policy_revision: &str,
) -> Result<(EvalSlotGroupDocument, PayloadRef, bool)> {
    let cache_key = eval_slot_group_cache_key(
        semantic_core,
        slot_group_id,
        data_mode.slug(),
        "default",
    );
    if let Some(bytes) = take_layer(cache_key.as_str()) {
        let doc: EvalSlotGroupDocument = serde_json::from_slice(bytes.as_slice())?;
        let content_hash = layer_entry_meta(cache_key.as_str())
            .map(|(_, hash)| hash)
            .filter(|hash| !hash.is_empty())
            .unwrap_or_else(|| "cached".to_string());
        let pref = PayloadRef::new(EVAL_SLOT_GROUP_KIND, content_hash.as_str(), EVAL_SLOT_GROUP_SCHEMA);
        return Ok((doc, pref, true));
    }
    let structure = build_structure_full_document(compiled, layout_policy_revision);
    let document = build_eval_slot_group_document(
        compiled,
        &structure,
        slot_group_id,
        data_mode,
        Some(workspace_root),
    );
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, compiled.app_id.as_str());
    let pref = persist_eval_slot_group(app_root.as_path(), &document)?;
    let bytes = serde_json::to_vec(&document)?;
    store_layer(
        cache_key,
        EVAL_SLOT_GROUP_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    Ok((document, pref, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_artifact::StructureFullNode;
    use mei_lang_kernel::{BlockDecl, UiNodeDecl, UiTreeNode};
    use serde_json::json;

    #[test]
    fn compound_metric_shell_panel_prefers_slot_frame_parent() {
        let shell = UiNodeDecl {
            kind: "panel".to_string(),
            id: "enforcement_objects".to_string(),
            title: None,
            head: None,
            area: Some("compound".to_string()),
            layout: None,
            blocks: vec![UiTreeNode::Panel(UiNodeDecl {
                kind: "panel".to_string(),
                id: "enforcement_objects_body".to_string(),
                title: None,
                head: None,
                area: Some("content".to_string()),
                layout: None,
                blocks: vec![],
                slot: None,
                props: json!({"background": "transparent"}),
                head_props: json!({}),
                body_props: json!({}),
                base: None,
                import_scope: None,
            })],
            slot: None,
            props: json!({
                "__mei_slot_frame_bg": true,
                "background": {
                    "image": "url(/workspace-app-assets/templates/cockpit/assets/metrics/metric-bg-target@3x.svg)"
                }
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let contract = mei_lang_kernel::SceneContract {
            scene: mei_lang_kernel::SceneDecl {
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
                shared: json!({}),
                local_nav: json!({}),
                params: json!({}),
                capabilities: json!({}),
                bindings: json!({}),
                examples: json!({}),
                access_export: true,
            },
            themes: vec![],
            shared: json!({}),
            world: None,
            flow: None,
            frame: None,
            panels: vec![shell],
        };
        let resolved = compound_metric_shell_panel(&contract, "enforcement_objects_body")
            .expect("shell panel");
        assert_eq!(resolved.id, "enforcement_objects");
        assert_eq!(
            resolved
                .props
                .get("__mei_slot_frame_bg")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn content_panel_lookup_prefers_scope_id_for_compound_metric() {
        let node = StructureFullNode {
            node_id: "compound".to_string(),
            ui_role: "content".to_string(),
            preview_scope: "t1/right_rail/enforcement/enforcement-compound".to_string(),
            label: "执法对象".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("compound-metric".to_string()),
            panel_id: None,
            use_keys: vec!["metric-card".to_string()],
            frame_viewport: None,
        };
        assert_eq!(
            content_panel_lookup_label(&node),
            "enforcement-compound"
        );
    }

    #[test]
    fn compound_metric_parent_does_not_export_child_component_mounts() {
        let node = StructureFullNode {
            node_id: "compound".to_string(),
            ui_role: "content".to_string(),
            preview_scope: "t1/left_rail/inspection/inspection-stats/block_ai/ai_compound_card"
                .to_string(),
            label: "ai_compound_card".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("compound-metric".to_string()),
            panel_id: None,
            use_keys: vec!["metric-card".to_string(), "row".to_string()],
            frame_viewport: None,
        };
        let child = UiNodeDecl {
            kind: "panel".to_string(),
            id: "ai_compound_card_main".to_string(),
            title: None,
            head: None,
            area: Some("main".to_string()),
            layout: None,
            blocks: vec![
                UiTreeNode::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "mei.text".to_string(),
                    id: None,
                    title: None,
                    area: Some("label".to_string()),
                    props: json!({"metric_role": "label", "content": "AI执法识别"}),
                    base: None,
                    layout: None,
                    blocks: Vec::new(),
                    component: None,
                    placement: None,
                    interactions: Vec::new(),
                    lifecycle: None,
                    constraints: None,
                    data: None,
                }),
                UiTreeNode::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "mei.text".to_string(),
                    id: None,
                    title: None,
                    area: Some("value".to_string()),
                    props: json!({"metric_role": "value", "content": "34"}),
                    base: None,
                    layout: None,
                    blocks: Vec::new(),
                    component: None,
                    placement: None,
                    interactions: Vec::new(),
                    lifecycle: None,
                    constraints: None,
                    data: None,
                }),
            ],
            slot: None,
            props: json!({"__mei_metric_card": true}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let panel = UiNodeDecl {
            kind: "panel".to_string(),
            id: "ai_compound_card".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiTreeNode::Panel(child)],
            slot: None,
            props: json!({}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let compiled = CompiledApp {
            app_id: "pretty-panels".to_string(),
            title: "pretty-panels".to_string(),
            app_root: "/tmp/pretty-panels".to_string(),
            scene_routes: vec![],
            active_scene: Some("home".to_string()),
            active_target_file: "src/scene/home/assembly.mei".to_string(),
            file_tree: vec![],
            scene_contract: Some(mei_lang_kernel::SceneContract {
                scene: mei_lang_kernel::SceneDecl {
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
                    shared: json!({}),
                    local_nav: json!({}),
                    params: json!({}),
                    capabilities: json!({}),
                    bindings: json!({}),
                    examples: json!({}),
                    access_export: true,
                },
                themes: vec![],
                shared: json!({}),
                world: None,
                flow: None,
                frame: None,
                panels: vec![panel],
            }),
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: vec![],
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: vec![],
            diagnostics: vec![],
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        };
        let mounts = component_mounts_for_content_node(&compiled, &node);
        assert!(
            mounts.is_empty(),
            "compound-metric parent must not aggregate child mei.text mounts"
        );
    }

    #[test]
    fn slot_groups_include_scene_default() {
        let structure = crate::view_artifact::StructureFullDocument {
            schema_version: "structure-full-v1".to_string(),
            app_id: "demo".to_string(),
            scene_id: "home".to_string(),
            semantic_revision: "rev".to_string(),
            scene_roots: vec![],
            nodes: vec![StructureFullNode {
                node_id: "n1".to_string(),
                ui_role: "slot".to_string(),
                preview_scope: "panel:left".to_string(),
                label: "left".to_string(),
                parent_id: None,
                children: vec![],
                plane: None,
                content_kind: None,
                panel_id: None,
                use_keys: vec![],
                frame_viewport: None,
            }],
            frame_viewport: None,
        };
        let groups = collect_slot_groups(&structure);
        assert!(groups.iter().any(|group| group == "scene:default"));
        assert!(groups.iter().any(|group| group == "scope:panel:left"));
    }

    #[test]
    fn component_mounts_resolve_map_block_by_use_key() {
        let panel = UiNodeDecl {
            kind: "panel".to_string(),
            id: "gis-map".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiTreeNode::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "map.maplibre".to_string(),
                id: Some("map".to_string()),
                title: None,
                area: Some("map".to_string()),
                props: json!({"mapSpec": {"layers": []}}),
                base: None,
                layout: None,
                blocks: Vec::new(),
                component: None,
                placement: None,
                interactions: Vec::new(),
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            slot: None,
            props: json!({}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let mut mounts = Vec::new();
        collect_component_mounts_for_use_key(&panel, "map.maplibre", &mut mounts);
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0]["use_key"], "map.maplibre");
        assert!(mounts[0]["props"]["mapSpec"].is_object());
    }

    #[test]
    fn component_mounts_include_metric_card_shell_when_panel_matches() {
        let panel = UiNodeDecl {
            kind: "panel".to_string(),
            id: "supervision_items_card".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiTreeNode::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "mei.text".to_string(),
                id: Some("label".to_string()),
                title: None,
                area: Some("label".to_string()),
                props: json!({"metric_role": "label", "content": {"text": "督办事项"}}),
                base: None,
                layout: None,
                blocks: Vec::new(),
                component: None,
                placement: None,
                interactions: Vec::new(),
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            slot: None,
            props: json!({
                "__mei_metric_card": true,
                "__mei_metric_template": "stack",
                "border": "1px solid rgba(34,211,238,0.35)",
                "background": "rgba(15,23,42,0.72)",
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let mut mounts = Vec::new();
        collect_component_mounts_for_label(&panel, "supervision_items_card", &mut mounts);
        assert!(mounts.len() >= 2);
        assert_eq!(mounts[0]["use_key"], "metric-card");
        assert_eq!(mounts[0]["mount_role"], "shell");
        assert_eq!(mounts[0]["props"]["__mei_metric_template"], "stack");
    }

    #[test]
    fn component_mounts_skip_duplicate_metric_card_leaf_scopes() {
        let node = StructureFullNode {
            node_id: "leaf".to_string(),
            ui_role: "content".to_string(),
            preview_scope: "t1/left/enforcement_units_card_content/value/mei.text".to_string(),
            label: "value".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("mei.text".to_string()),
            panel_id: None,
            use_keys: vec!["mei.text".to_string()],
            frame_viewport: None,
        };
        let compiled = CompiledApp {
            app_id: "pretty-panels".to_string(),
            title: "pretty-panels".to_string(),
            app_root: "/tmp/pretty-panels".to_string(),
            scene_routes: vec![],
            active_scene: Some("home".to_string()),
            active_target_file: "src/scene/home/assembly.mei".to_string(),
            file_tree: vec![],
            scene_contract: Some(mei_lang_kernel::SceneContract {
                scene: mei_lang_kernel::SceneDecl {
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
                    shared: json!({}),
                    local_nav: json!({}),
                    params: json!({}),
                    capabilities: json!({}),
                    bindings: json!({}),
                    examples: json!({}),
                    access_export: true,
                },
                themes: vec![],
                shared: json!({}),
                world: None,
                flow: None,
                frame: None,
                panels: vec![UiNodeDecl {
                    kind: "panel".to_string(),
                    id: "enforcement_units_card".to_string(),
                    title: None,
                    head: None,
                    area: None,
                    layout: None,
                    blocks: vec![UiTreeNode::Block(BlockDecl {
                        kind: "block".to_string(),
                        use_key: "mei.text".to_string(),
                        id: Some("head".to_string()),
                        title: None,
                        area: None,
                        props: json!({"content": "典型案例"}),
                        base: None,
                        layout: None,
                        blocks: Vec::new(),
                        component: None,
                        placement: None,
                        interactions: Vec::new(),
                        lifecycle: None,
                        constraints: None,
                        data: None,
                    })],
                    slot: None,
                    props: json!({"__mei_metric_card": true}),
                    head_props: json!({}),
                    body_props: json!({}),
                    base: None,
                    import_scope: None,
                }],
            }),
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: vec![],
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: vec![],
            diagnostics: vec![],
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        };
        let mounts = component_mounts_for_content_node(&compiled, &node);
        assert!(mounts.is_empty(), "duplicate metric leaf scopes must not inherit mounts");
    }

    #[test]
    fn component_mounts_resolve_chart_column_by_use_key_when_label_is_display_name() {
        let node = StructureFullNode {
            node_id: "n-chart".to_string(),
            ui_role: "content".to_string(),
            preview_scope: "t1/left_rail/inspection/inspection-stats/block_counts/inspection_counts_layout/chart/content_zone/chart.column".to_string(),
            label: "分组柱图".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("chart.column".to_string()),
            panel_id: None,
            use_keys: vec!["chart.column".to_string()],
            frame_viewport: None,
        };
        let compiled = CompiledApp {
            app_id: "pretty-panels".to_string(),
            title: "pretty-panels".to_string(),
            app_root: "/tmp/pretty-panels".to_string(),
            scene_routes: vec![],
            active_scene: Some("home".to_string()),
            active_target_file: "src/scene/home/assembly.mei".to_string(),
            file_tree: vec![],
            scene_contract: Some(mei_lang_kernel::SceneContract {
                scene: mei_lang_kernel::SceneDecl {
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
                    shared: json!({}),
                    local_nav: json!({}),
                    params: json!({}),
                    capabilities: json!({}),
                    bindings: json!({}),
                    examples: json!({}),
                    access_export: true,
                },
                themes: vec![],
                shared: json!({}),
                world: None,
                flow: None,
                frame: None,
                panels: vec![UiNodeDecl {
                    kind: "panel".to_string(),
                    id: "inspection_counts_chart".to_string(),
                    title: None,
                    head: None,
                    area: Some("chart".to_string()),
                    layout: None,
                    blocks: vec![UiTreeNode::Block(BlockDecl {
                        kind: "block".to_string(),
                        use_key: "chart.column".to_string(),
                        id: None,
                        title: None,
                        area: Some("auto".to_string()),
                        props: json!({
                            "compact": true,
                            "chartHeight": 140,
                            "title": "",
                            "showLegend": true,
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
                    })],
                    slot: None,
                    props: json!({}),
                    head_props: json!({}),
                    body_props: json!({}),
                    base: None,
                    import_scope: None,
                }],
            }),
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: vec![],
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: vec![],
            diagnostics: vec![],
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        };
        let mounts = component_mounts_for_content_node(&compiled, &node);
        assert_eq!(mounts.len(), 1, "chart.column must export mounts via use_key fallback");
        assert_eq!(mounts[0]["use_key"], "chart.column");
        assert_eq!(mounts[0]["props"]["compact"], true);
        assert_eq!(mounts[0]["props"]["chartHeight"], 140);
        assert_eq!(
            mounts[0]["props"]["title"],
            "",
            "must bind inspection chart, not another chart.column in the scene"
        );
    }

    #[test]
    fn component_mounts_prefer_scope_ancestor_panel_for_duplicate_use_keys() {
        let node = StructureFullNode {
            node_id: "n-chart".to_string(),
            ui_role: "content".to_string(),
            preview_scope: "t1/left_rail/inspection/inspection-stats/block_counts/inspection_counts_layout/chart/content_zone/chart.column".to_string(),
            label: "分组柱图".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("chart.column".to_string()),
            panel_id: None,
            use_keys: vec!["chart.column".to_string()],
            frame_viewport: None,
        };
        let inspection_layout = UiNodeDecl {
            kind: "panel".to_string(),
            id: "inspection_counts_layout".to_string(),
            title: None,
            head: None,
            area: Some("block_counts".to_string()),
            layout: None,
            blocks: vec![UiTreeNode::Panel(UiNodeDecl {
                kind: "panel".to_string(),
                id: "inspection_counts_chart".to_string(),
                title: None,
                head: None,
                area: Some("chart".to_string()),
                layout: None,
                blocks: vec![UiTreeNode::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "chart.column".to_string(),
                    id: None,
                    title: None,
                    area: Some("auto".to_string()),
                    props: json!({
                        "compact": true,
                        "chartHeight": 100,
                        "title": "",
                        "barGradient": "cockpit-year-duo",
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
                })],
                slot: None,
                props: json!({}),
                head_props: json!({}),
                body_props: json!({}),
                base: None,
                import_scope: None,
            })],
            slot: None,
            props: json!({}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let penalty_layout = UiNodeDecl {
            kind: "panel".to_string(),
            id: "penalty_counts_layout".to_string(),
            title: None,
            head: None,
            area: Some("block_counts".to_string()),
            layout: None,
            blocks: vec![UiTreeNode::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "chart.column".to_string(),
                id: None,
                title: None,
                area: Some("party_bars".to_string()),
                props: json!({
                    "compact": true,
                    "chartHeight": 140,
                    "title": "2025罚没居前当事人（元）",
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
            })],
            slot: None,
            props: json!({}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        // Put penalty first so a naive full-scan would pick the wrong props.
        let compiled = CompiledApp {
            app_id: "pretty-panels".to_string(),
            title: "pretty-panels".to_string(),
            app_root: "/tmp/pretty-panels".to_string(),
            scene_routes: vec![],
            active_scene: Some("home".to_string()),
            active_target_file: "src/scene/home/assembly.mei".to_string(),
            file_tree: vec![],
            scene_contract: Some(mei_lang_kernel::SceneContract {
                scene: mei_lang_kernel::SceneDecl {
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
                    shared: json!({}),
                    local_nav: json!({}),
                    params: json!({}),
                    capabilities: json!({}),
                    bindings: json!({}),
                    examples: json!({}),
                    access_export: true,
                },
                themes: vec![],
                shared: json!({}),
                world: None,
                flow: None,
                frame: None,
                panels: vec![penalty_layout, inspection_layout],
            }),
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: vec![],
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: vec![],
            diagnostics: vec![],
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        };
        let mounts = component_mounts_for_content_node(&compiled, &node);
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0]["props"]["chartHeight"], 100);
        assert_eq!(mounts[0]["props"]["title"], "");
        assert_eq!(mounts[0]["props"]["barGradient"], "cockpit-year-duo");
    }

    #[test]
    fn component_mounts_skip_chart_on_chart_summary_parent_node() {
        let node = StructureFullNode {
            node_id: "n-layout".to_string(),
            ui_role: "content".to_string(),
            preview_scope: "t1/left_rail/inspection/inspection-stats/block_counts/inspection_counts_layout".to_string(),
            label: "检查统计".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("chart-summary".to_string()),
            panel_id: None,
            use_keys: vec![
                "chart.column".to_string(),
                "metric-summary".to_string(),
                "row".to_string(),
            ],
            frame_viewport: None,
        };
        let panel = UiNodeDecl {
            kind: "panel".to_string(),
            id: "inspection_counts_layout".to_string(),
            title: None,
            head: None,
            area: Some("block_counts".to_string()),
            layout: None,
            blocks: vec![UiTreeNode::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "chart.column".to_string(),
                id: None,
                title: None,
                area: Some("chart".to_string()),
                props: json!({"compact": true, "chartHeight": 100, "title": ""}),
                base: None,
                layout: None,
                blocks: Vec::new(),
                component: None,
                placement: None,
                interactions: Vec::new(),
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            slot: None,
            props: json!({}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let compiled = CompiledApp {
            app_id: "pretty-panels".to_string(),
            title: "pretty-panels".to_string(),
            app_root: "/tmp/pretty-panels".to_string(),
            scene_routes: vec![],
            active_scene: Some("home".to_string()),
            active_target_file: "src/scene/home/assembly.mei".to_string(),
            file_tree: vec![],
            scene_contract: Some(mei_lang_kernel::SceneContract {
                scene: mei_lang_kernel::SceneDecl {
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
                    shared: json!({}),
                    local_nav: json!({}),
                    params: json!({}),
                    capabilities: json!({}),
                    bindings: json!({}),
                    examples: json!({}),
                    access_export: true,
                },
                themes: vec![],
                shared: json!({}),
                world: None,
                flow: None,
                frame: None,
                panels: vec![panel],
            }),
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: vec![],
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: vec![],
            diagnostics: vec![],
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        };
        let mounts = component_mounts_for_content_node(&compiled, &node);
        assert!(
            mounts.is_empty(),
            "chart-summary parent must not export chart.column mounts"
        );
    }

    #[test]
    fn component_mounts_narrow_sibling_mei_text_by_grid_area() {
        let node = StructureFullNode {
            node_id: "n-active".to_string(),
            ui_role: "content".to_string(),
            preview_scope: "t1/left_rail/list/event-list/active/mei.text".to_string(),
            label: "active · EVT-1".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("mei.text".to_string()),
            panel_id: None,
            use_keys: vec!["mei.text".to_string()],
            frame_viewport: None,
        };
        let panel = UiNodeDecl {
            kind: "panel".to_string(),
            id: "event-list".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![
                UiTreeNode::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "mei.text".to_string(),
                    id: None,
                    title: None,
                    area: Some("active".to_string()),
                    props: json!({"content": "进行中 · EVT-1"}),
                    base: None,
                    layout: None,
                    blocks: Vec::new(),
                    component: None,
                    placement: None,
                    interactions: Vec::new(),
                    lifecycle: None,
                    constraints: None,
                    data: None,
                }),
                UiTreeNode::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "mei.text".to_string(),
                    id: None,
                    title: None,
                    area: Some("archived_a".to_string()),
                    props: json!({"content": "已归档 · EVT-2"}),
                    base: None,
                    layout: None,
                    blocks: Vec::new(),
                    component: None,
                    placement: None,
                    interactions: Vec::new(),
                    lifecycle: None,
                    constraints: None,
                    data: None,
                }),
            ],
            slot: None,
            props: json!({}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let compiled = CompiledApp {
            app_id: "thunder".to_string(),
            title: "thunder".to_string(),
            app_root: "/tmp/thunder".to_string(),
            scene_routes: vec![],
            active_scene: Some("home".to_string()),
            active_target_file: "src/scene/home/assembly.mei".to_string(),
            file_tree: vec![],
            scene_contract: Some(mei_lang_kernel::SceneContract {
                scene: mei_lang_kernel::SceneDecl {
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
                    shared: json!({}),
                    local_nav: json!({}),
                    params: json!({}),
                    capabilities: json!({}),
                    bindings: json!({}),
                    examples: json!({}),
                    access_export: true,
                },
                themes: vec![],
                shared: json!({}),
                world: None,
                flow: None,
                frame: None,
                panels: vec![panel],
            }),
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: vec![],
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: vec![],
            diagnostics: vec![],
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        };
        let mounts = component_mounts_for_content_node(&compiled, &node);
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0]["area"], "active");
        assert_eq!(mounts[0]["props"]["content"], "进行中 · EVT-1");
    }

    #[test]
    fn metric_card_panel_hint_maps_card_content_scope_to_panel_id() {
        assert_eq!(
            metric_card_panel_hint_from_scope(
                "t1/left_rail/enforcement/enforcement_strip_layout/first/enforcement_units_card_content"
            )
            .as_deref(),
            Some("enforcement_units_card")
        );
        assert_eq!(
            metric_card_panel_hint_from_scope(
                "t1/right_rail/warning/supervision-stats/items/supervision_items_card"
            )
            .as_deref(),
            Some("supervision_items_card")
        );
        // Nested compound areas must resolve to the deepest card segment, not
        // an ancestor compound shell id earlier in the path.
        assert_eq!(
            metric_card_panel_hint_from_scope(
                "t1/left_rail/inspection/inspection-stats/block_ai/ai_compound_card/main"
            )
            .as_deref(),
            Some("ai_compound_card")
        );
        // Leaf `*_main_content` maps to outer shell id `*_main`, not the
        // compound ancestor — panel_shell_lookup_matches_node must still reject
        // applying the compound frame onto the leaf.
        assert_eq!(
            metric_card_panel_hint_from_scope(
                "t1/left_rail/inspection/inspection-stats/block_ai/ai_compound_card/main/ai_compound_card_main_content"
            )
            .as_deref(),
            Some("ai_compound_card_main")
        );
        assert_eq!(
            metric_card_panel_hint_from_scope(
                "t1/right_rail/warning/supervision-stats/triptych/supervision_triptych/first/supervision_triptych_first_content"
            )
            .as_deref(),
            Some("supervision_triptych_first")
        );
    }

    #[test]
    fn panel_shell_lookup_only_matches_compound_host_not_nested_slots() {
        let host = StructureFullNode {
            node_id: "compound".to_string(),
            ui_role: "content".to_string(),
            preview_scope: "t1/left_rail/inspection/inspection-stats/block_ai/ai_compound_card"
                .to_string(),
            label: "ai_compound_card".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("compound-metric".to_string()),
            panel_id: None,
            use_keys: vec![],
            frame_viewport: None,
        };
        let nested_slot = StructureFullNode {
            node_id: "main".to_string(),
            ui_role: "slot".to_string(),
            preview_scope:
                "t1/left_rail/inspection/inspection-stats/block_ai/ai_compound_card/main"
                    .to_string(),
            label: "main".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: None,
            panel_id: None,
            use_keys: vec![],
            frame_viewport: None,
        };
        assert!(panel_shell_lookup_matches_node(&host, "ai_compound_card"));
        assert!(!panel_shell_lookup_matches_node(
            &nested_slot,
            "ai_compound_card"
        ));
        let nested_content = StructureFullNode {
            node_id: "main_content".to_string(),
            ui_role: "content".to_string(),
            preview_scope:
                "t1/left_rail/inspection/inspection-stats/block_ai/ai_compound_card/main/ai_compound_card_main_content"
                    .to_string(),
            label: "AI执法识别".to_string(),
            parent_id: None,
            children: vec![],
            plane: None,
            content_kind: Some("stack".to_string()),
            panel_id: None,
            use_keys: vec!["metric-card".to_string()],
            frame_viewport: None,
        };
        assert!(!panel_shell_lookup_matches_node(
            &nested_content,
            "ai_compound_card"
        ));
    }
}
