//! `eval.slot_group[*]` artifact helpers.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{BlockDecl, CompiledApp, DataMode, MetricShape, PanelDecl, UiNodeDecl};
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
    out.push(json!({
        "use_key": block.use_key,
        "props": block.props,
    }));
}

fn push_panel_blocks(panel: &PanelDecl, out: &mut Vec<Value>) {
    if let Some(head) = panel.head.as_ref() {
        if let UiNodeDecl::Block(block) = head.as_ref() {
            push_block_mount(block, out);
        }
    }
    for child in &panel.blocks {
        match child {
            UiNodeDecl::Block(block) => push_block_mount(block, out),
            UiNodeDecl::Panel(nested) => push_panel_blocks(nested, out),
            UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}

fn panel_is_metric_card(panel: &PanelDecl) -> bool {
    panel
        .props
        .get("__mei_metric_card")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn push_metric_card_shell_mount(panel: &PanelDecl, out: &mut Vec<Value>) {
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

fn collect_component_mounts_for_use_key(panel: &PanelDecl, use_key: &str, out: &mut Vec<Value>) {
    let use_key = use_key.trim();
    if use_key.is_empty() {
        return;
    }
    for child in &panel.blocks {
        match child {
            UiNodeDecl::Block(block) if block_use_key_matches(block, use_key) => {
                push_block_mount(block, out);
            }
            UiNodeDecl::Panel(nested) => collect_component_mounts_for_use_key(nested, use_key, out),
            UiNodeDecl::Block(_) | UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}

fn collect_component_mounts_for_label(panel: &PanelDecl, label: &str, out: &mut Vec<Value>) {
    if panel.id == label {
        push_metric_card_shell_mount(panel, out);
        push_panel_blocks(panel, out);
        return;
    }
    if let Some(head) = panel.head.as_ref() {
        if let UiNodeDecl::Block(block) = head.as_ref() {
            if block_id_matches(block, label) {
                push_block_mount(block, out);
            }
        }
    }
    for child in &panel.blocks {
        match child {
            UiNodeDecl::Block(block) if block_id_matches(block, label) => {
                push_block_mount(block, out);
            }
            UiNodeDecl::Panel(nested) => collect_component_mounts_for_label(nested, label, out),
            UiNodeDecl::Block(_) | UiNodeDecl::PanelRefEmbed(_) => {}
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
        "label" | "value" | "unit" | "icon" | "head" | "mei.text" | ""
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
        if let Ok(payload) = crate::v2_lower::load_panel_contract_payload(&ctx, ref_path.as_str()) {
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
    panel: &PanelDecl,
    author_props: Option<&Value>,
) -> PanelDecl {
    let mut exported = panel.clone();
    let Some(exported_props) = exported.props.as_object_mut() else {
        return exported;
    };
    let current_bg = exported_props.get("background").and_then(Value::as_str);
    let needs_author_bg = current_bg.is_none()
        || current_bg.is_some_and(|bg| bg.eq_ignore_ascii_case("transparent"));
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

fn metric_card_panel_hint_from_scope(scope: &str) -> Option<String> {
    for segment in scope.split('/') {
        if let Some(id) = segment.strip_suffix("_card_content") {
            if !id.is_empty() {
                return Some(format!("{id}_card"));
            }
        }
        if segment.ends_with("_card") && !segment.ends_with("_card_content") {
            return Some(segment.to_string());
        }
    }
    None
}

fn metric_card_lookup_label(node: &StructureFullNode) -> String {
    let label = panel_lookup_label(node);
    if label.ends_with("_card_content") {
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

fn panel_contract_lookup_label(node: &StructureFullNode) -> String {
    if node.content_kind.as_deref() == Some("compound-metric") {
        if let Some(id) = node
            .preview_scope
            .rsplit('/')
            .next()
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
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

fn find_panel_by_id<'a>(panel: &'a PanelDecl, target: &str) -> Option<&'a PanelDecl> {
    if panel.id == target {
        return Some(panel);
    }
    for child in &panel.blocks {
        if let UiNodeDecl::Panel(nested) = child {
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
) -> Option<&'a PanelDecl> {
    contract
        .panels
        .iter()
        .find_map(|panel| find_panel_by_id(panel, target))
}

fn component_mounts_for_content_node(compiled: &CompiledApp, node: &StructureFullNode) -> Vec<Value> {
    if node.ui_role != "content" {
        return Vec::new();
    }
    if is_duplicate_metric_card_leaf_scope(&node.preview_scope) {
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
            if !mounts.is_empty() {
                return mounts;
            }
        }
        for panel in &contract.panels {
            collect_component_mounts_for_label(panel, panel_id.as_str(), &mut mounts);
            if !mounts.is_empty() {
                return mounts;
            }
        }
    }
    if let Some(panel_id) = panel_hint.as_deref() {
        if let Some(panel) = find_panel_in_contract(contract, panel_id) {
            collect_component_mounts_for_label(panel, panel_id, &mut mounts);
            if !mounts.is_empty() {
                return mounts;
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
        for use_key in &node.use_keys {
            let key = use_key.trim();
            if key.is_empty() || key == "metric-card" {
                continue;
            }
            if is_ambiguous_mount_label(label) {
                if let Some(panel_id) = panel_hint.as_deref() {
                    if let Some(panel) = find_panel_in_contract(contract, panel_id) {
                        collect_component_mounts_for_use_key(panel, key, &mut mounts);
                    }
                }
                continue;
            }
            for panel in &contract.panels {
                collect_component_mounts_for_use_key(panel, key, &mut mounts);
            }
        }
    }
    mounts
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
                let panel_lookup = panel_contract_lookup_label(node);
                if !panel_lookup.is_empty() {
                    if let Some(panel) = find_panel_in_contract(contract, panel_lookup.as_str()) {
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
    use mei_lang_kernel::{BlockDecl, PanelDecl, UiNodeDecl};
    use serde_json::json;

    #[test]
    fn panel_contract_lookup_prefers_scope_id_for_compound_metric() {
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
            panel_contract_lookup_label(&node),
            "enforcement-compound"
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
        let panel = PanelDecl {
            kind: "panel".to_string(),
            id: "gis-map".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
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
        let panel = PanelDecl {
            kind: "panel".to_string(),
            id: "supervision_items_card".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
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
                panels: vec![PanelDecl {
                    kind: "panel".to_string(),
                    id: "enforcement_units_card".to_string(),
                    title: None,
                    head: None,
                    area: None,
                    layout: None,
                    blocks: vec![UiNodeDecl::Block(BlockDecl {
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
            build_board_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        };
        let mounts = component_mounts_for_content_node(&compiled, &node);
        assert!(mounts.is_empty(), "duplicate metric leaf scopes must not inherit mounts");
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
    }
}
