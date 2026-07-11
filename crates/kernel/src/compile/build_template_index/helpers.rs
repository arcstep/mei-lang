use super::{template_entry_for_preview, template_primary_consumer_from_entry};

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::compile::block_instance_id;
use crate::model::{
    BlockDecl, CompiledApp, ComponentAsset, TemplateCatalogEntry, TemplateConsumerAnchor,
    UiNodeDecl, UiTreeNode,
};

fn normalize_template_file_key(raw: &str) -> String {
    let mut value = raw.trim().replace('\\', "/");
    while let Some(rest) = value.strip_prefix("./") {
        value = rest.to_string();
    }
    while let Some(rest) = value.strip_prefix('/') {
        value = rest.to_string();
    }
    for prefix in [".stock/templates/", "templates/"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest.to_string();
            break;
        }
    }
    value
}

fn template_entries_for_file<'a>(
    compiled: &'a CompiledApp,
    template_file_key: &str,
) -> Vec<&'a TemplateCatalogEntry> {
    let wanted = normalize_template_file_key(template_file_key);
    if wanted.is_empty() {
        return Vec::new();
    }
    compiled
        .build_template_index
        .templates
        .values()
        .filter(|entry| normalize_template_file_key(entry.template_file.as_str()) == wanted)
        .collect()
}

fn template_primary_consumer_for_template_file<'a>(
    compiled: &'a CompiledApp,
    template_file_key: &str,
) -> Option<&'a TemplateConsumerAnchor> {
    let active_scene = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());
    let mut fallback: Option<&TemplateConsumerAnchor> = None;
    for entry in template_entries_for_file(compiled, template_file_key) {
        if let Some(anchor) = template_primary_consumer_from_entry(entry, active_scene) {
            if active_scene.is_some_and(|scene| anchor.scene_id == scene) {
                return Some(anchor);
            }
            if fallback.is_none() {
                fallback = Some(anchor);
            }
        }
    }
    fallback
}

/// Primary build preview: pack-local preview scene or template `.mei`.
pub fn authoring_preview_target_for_template(
    compiled: &CompiledApp,
    template_key: &str,
) -> Option<String> {
    if crate::compile::build_experience::is_template_file_node_key(template_key) {
        let workspace_path =
            crate::compile::build_experience::template_file_preview_target(compiled, template_key)?;
        let rel = crate::compile::build_experience::preview_target_relative_to_app(
            compiled,
            workspace_path.as_str(),
        )?;
        if !template_file_supports_authoring_preview(compiled, rel.as_str()) {
            return None;
        }
        return Some(rel);
    }
    let entry = template_entry_for_preview(compiled, template_key)?;
    if entry.template_file.ends_with(".mei") {
        let rel = crate::compile::build_experience::preview_target_relative_to_app(
            compiled,
            entry.template_file.as_str(),
        )?;
        if !template_file_supports_authoring_preview(compiled, rel.as_str()) {
            return None;
        }
        return Some(rel);
    }
    crate::compile::component_pack_preview::component_pack_preview_relative_to_app_for_key(
        compiled,
        template_key,
    )
}

fn template_file_supports_authoring_preview(compiled: &CompiledApp, rel_path: &str) -> bool {
    let app_root = Path::new(compiled.app_root.as_str());
    let abs = if rel_path.starts_with("../") {
        let mut base = app_root.to_path_buf();
        for part in rel_path.split('/') {
            if part == ".." {
                if !base.pop() {
                    return false;
                }
            } else if !part.is_empty() && part != "." {
                base.push(part);
            }
        }
        base
    } else {
        app_root.join(rel_path)
    };
    let content = match std::fs::read_to_string(abs) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let trimmed = content.trim();
    !trimmed.is_empty() && trimmed.contains("scene(")
}

pub fn preview_target_for_template_file_consumer(
    compiled: &CompiledApp,
    template_file_key: &str,
) -> Option<String> {
    let anchor = template_primary_consumer_for_template_file(compiled, template_file_key)?;
    crate::compile::build_experience::preview_target_for_scene_id(
        compiled,
        anchor.scene_id.as_str(),
    )
}

pub fn preview_scene_id_for_template_file_consumer(
    compiled: &CompiledApp,
    template_file_key: &str,
) -> Option<String> {
    template_primary_consumer_for_template_file(compiled, template_file_key)
        .map(|anchor| anchor.scene_id.clone())
}

pub(super) fn collect_panel_use_keys(
    panel: &UiNodeDecl,
    out: &mut BTreeMap<String, BTreeSet<String>>,
) {
    for ui_node in &panel.blocks {
        match ui_node {
            UiTreeNode::Block(block) => {
                let consumer = block_consumer_label(block);
                out.entry(block.use_key.clone())
                    .or_default()
                    .insert(consumer);
            }
            UiTreeNode::Panel(nested) => collect_panel_use_keys(nested, out),
            _ => {}
        }
    }
}

pub(super) fn collect_panel_template_usage(
    scene_id: &str,
    panel: &UiNodeDecl,
    panel_path: &str,
    out: &mut BTreeMap<String, Vec<TemplateConsumerAnchor>>,
) {
    for (ordinal, ui_node) in panel.blocks.iter().enumerate() {
        match ui_node {
            UiTreeNode::Block(block) => {
                out.entry(block.use_key.clone())
                    .or_default()
                    .push(TemplateConsumerAnchor {
                        scene_id: scene_id.to_string(),
                        panel_path: panel_path.to_string(),
                        block_id: block_instance_id(block, ordinal),
                        label: block_consumer_label(block),
                    });
            }
            UiTreeNode::Panel(nested) => {
                let nested_path = format!("{panel_path}/{}", nested.id);
                collect_panel_template_usage(scene_id, nested, nested_path.as_str(), out);
            }
            _ => {}
        }
    }
}

fn block_consumer_label(block: &BlockDecl) -> String {
    block
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| block.id.clone().unwrap_or_else(|| block.use_key.clone()))
}

pub(super) fn categorize_template_key(key: &str) -> &'static str {
    if key.contains("metric-card") || key.contains("metric_card") {
        "metric_card"
    } else if key.contains("panel") {
        "panel_shell"
    } else if key.contains("table") {
        "table"
    } else if key.contains("chart") {
        "chart"
    } else {
        "component"
    }
}

pub(super) fn related_variant_keys(key: &str, assets: &[ComponentAsset]) -> Vec<String> {
    let family = key.split('.').next().unwrap_or(key);
    let prefix = format!("{family}.");
    let mut variants: Vec<String> = assets
        .iter()
        .filter(|asset| asset.key.starts_with(prefix.as_str()) && asset.key.as_str() != key)
        .map(|asset| asset.key.clone())
        .collect();
    variants.sort();
    variants.dedup();
    variants
}

pub(super) fn default_props_schema(category: &str) -> Vec<String> {
    match category {
        "metric_card" => vec![
            "metric (__ref metric)".to_string(),
            "title (optional)".to_string(),
            "value / unit overrides".to_string(),
        ],
        "panel_shell" => vec!["title".to_string(), "body blocks".to_string()],
        "table" => vec!["dataset / rowset".to_string(), "columns".to_string()],
        _ => vec!["props (component-specific)".to_string()],
    }
}

pub(super) fn agent_hint_for(category: &str, key: &str, script: &str) -> String {
    match category {
        "metric_card" => format!(
            "选用 `{key}`（`{script}`）展示单指标卡；新建变体请复制 stock metric-card 模板并调整 props.metric 绑定；在 scene block 中设置 use_key=`{key}` 或 metric_card_ref。"
        ),
        "panel_shell" => format!(
            "选用 `{key}` 作为 titled panel 外壳；通过 panel_ref / panel(base=panel_ref) 挂载到 layout scene。"
        ),
        _ => format!("模板 `{key}` 位于 `{script}`；在 block 上设置 use_key=`{key}` 引用。"),
    }
}
