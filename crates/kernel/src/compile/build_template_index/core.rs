use super::{
    agent_hint_for, categorize_template_key, collect_panel_template_usage, collect_panel_use_keys,
    default_props_schema, related_variant_keys,
};

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use walkdir::WalkDir;

use crate::compile::reachability_tree::{ReachabilityTreeNode, ReachabilityTreeRoot};
use crate::mei_config::{resolve_templates_root, stock_path_excluded, StockCatalogKind};
use crate::model::{
    BuildNodeId, BuildTemplateIndex, CompiledApp, ComponentAsset, ExperienceNodeManifest,
    SceneContract, TemplateCatalogEntry, TemplateConsumerAnchor,
};
use crate::workspace::load_component_assets;

pub struct BuildTemplateIndexResult {
    pub index: BuildTemplateIndex,
    pub tree_root: ReachabilityTreeRoot,
}

/// Union workspace component manifests with compile-time `component_assets` (deduped by use_key).
pub fn merged_component_catalog(source_root: &Path, compiled: &CompiledApp) -> Vec<ComponentAsset> {
    let mut merged = BTreeMap::<String, ComponentAsset>::new();
    if let Ok(map) = load_component_assets(source_root) {
        merged.extend(map);
    }
    for asset in &compiled.component_assets {
        merged.insert(asset.key.clone(), asset.clone());
    }
    merged.into_values().collect()
}

/// Resolve catalog entry for build preview: compiled index first, then workspace stock manifest.
pub fn template_entry_for_preview(
    compiled: &CompiledApp,
    template_key: &str,
) -> Option<TemplateCatalogEntry> {
    if let Some(entry) = compiled.build_template_index.lookup(template_key) {
        return Some(entry.clone());
    }
    let source_root = crate::mei_config::resolve_workspace_source_root_from_app_root(Path::new(
        compiled.app_root.as_str(),
    ));
    let asset = merged_component_catalog(source_root.as_path(), compiled)
        .into_iter()
        .find(|asset| asset.key == template_key)?;
    build_template_index(&[asset], &BTreeMap::new(), &BTreeMap::new())
        .index
        .templates
        .get(template_key)
        .cloned()
}

pub fn build_template_index(
    component_assets: &[ComponentAsset],
    scene_contracts_by_id: &BTreeMap<String, SceneContract>,
    _node_manifest: &BTreeMap<String, ExperienceNodeManifest>,
) -> BuildTemplateIndexResult {
    let mut templates = BTreeMap::<String, TemplateCatalogEntry>::new();
    let mut use_key_consumers = BTreeMap::<String, BTreeSet<String>>::new();
    let mut consumer_anchors = BTreeMap::<String, Vec<TemplateConsumerAnchor>>::new();

    for (scene_id, contract) in scene_contracts_by_id {
        for panel in &contract.panels {
            collect_panel_use_keys(panel, &mut use_key_consumers);
            collect_panel_template_usage(
                scene_id.as_str(),
                panel,
                panel.id.as_str(),
                &mut consumer_anchors,
            );
        }
    }

    for asset in component_assets {
        let category = categorize_template_key(asset.key.as_str());
        let mut consumers: Vec<String> = use_key_consumers
            .get(asset.key.as_str())
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        consumers.sort();
        let anchors = consumer_anchors
            .remove(asset.key.as_str())
            .unwrap_or_default();
        let variants = related_variant_keys(asset.key.as_str(), component_assets);
        let agent_hint = Some(agent_hint_for(
            category,
            asset.key.as_str(),
            asset.script.as_str(),
        ));
        templates.insert(
            asset.key.clone(),
            TemplateCatalogEntry {
                template_key: asset.key.clone(),
                template_file: asset.script.clone(),
                category: category.to_string(),
                props_schema: default_props_schema(category),
                variants,
                consumers,
                consumer_anchors: anchors,
                agent_hint,
                preview_mei: asset.preview_mei.clone(),
            },
        );
    }

    let mut by_pack = BTreeMap::<String, Vec<ReachabilityTreeNode>>::new();
    for asset in component_assets {
        let entry = templates.get(asset.key.as_str()).expect("catalog entry");
        let mut badges = vec![entry
            .preview_mei
            .clone()
            .unwrap_or_else(|| entry.template_file.clone())];
        if entry.preview_mei.is_none() {
            badges.push("preview:unavailable".to_string());
        }
        by_pack
            .entry(asset.pack_path.clone())
            .or_default()
            .push(ReachabilityTreeNode {
                id: format!("component-{}", entry.template_key),
                node_id: if crate::compile::build_experience::is_template_file_node_key(
                    entry.template_key.as_str(),
                ) {
                    BuildNodeId::template(entry.template_key.as_str()).encode()
                } else {
                    BuildNodeId::component(entry.template_key.as_str()).encode()
                },
                kind: if crate::compile::build_experience::is_template_file_node_key(
                    entry.template_key.as_str(),
                ) {
                    "template_file".to_string()
                } else {
                    "component".to_string()
                },
                label: entry.template_key.clone(),
                badges,
                children: Vec::new(),
                ..Default::default()
            });
    }

    let mut tree_children = Vec::new();
    for (pack_path, mut nodes) in by_pack {
        nodes.sort_by(|left, right| left.label.cmp(&right.label));
        tree_children.push(ReachabilityTreeNode {
            id: format!("component-pack-{pack_path}"),
            node_id: String::new(),
            kind: "component_pack".to_string(),
            label: pack_path.clone(),
            badges: Vec::new(),
            children: nodes,
            ..Default::default()
        });
    }
    tree_children.sort_by(|left, right| left.label.cmp(&right.label));

    let index = BuildTemplateIndex { templates };
    let tree_root = ReachabilityTreeRoot {
        group: "templates".to_string(),
        label: "Components".to_string(),
        default_open: false,
        children: tree_children,
    };

    BuildTemplateIndexResult { index, tree_root }
}

/// Workspace `stock/templates/**/*.mei` as a separate reachability group (authoring file tree).
pub fn build_stock_template_files_root(source_root: &Path) -> ReachabilityTreeRoot {
    let templates_root = resolve_templates_root(source_root);
    let templates_prefix = templates_root
        .strip_prefix(source_root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .filter(|rel| !rel.is_empty())
        .unwrap_or_else(|| "stock/templates".to_string());
    let mut by_folder = BTreeMap::<String, Vec<ReachabilityTreeNode>>::new();
    if templates_root.is_dir() {
        for entry in WalkDir::new(&templates_root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let file_name = entry.file_name().to_string_lossy();
            if !file_name.ends_with(".mei") {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&templates_root)
                .ok()
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| file_name.to_string());
            if stock_path_excluded(source_root, StockCatalogKind::Templates, rel.as_str()) {
                continue;
            }
            if rel.starts_with("assets/") || rel.contains("/assets/") {
                continue;
            }
            let folder = rel
                .rsplit_once('/')
                .map(|(dir, _)| dir.to_string())
                .unwrap_or_else(|| ".".to_string());
            let template_path = format!("{templates_prefix}/{rel}");
            by_folder
                .entry(folder)
                .or_default()
                .push(ReachabilityTreeNode {
                    id: format!("template-file-{rel}"),
                    node_id: BuildNodeId::template(rel.as_str()).encode(),
                    kind: "template_file".to_string(),
                    label: rel.clone(),
                    badges: vec![template_path.clone()],
                    children: Vec::new(),
                    ..Default::default()
                });
        }
    }
    let mut children = Vec::new();
    for (folder, mut nodes) in by_folder {
        nodes.sort_by(|left, right| left.label.cmp(&right.label));
        if folder == "." {
            children.extend(nodes);
        } else {
            children.push(ReachabilityTreeNode {
                id: format!("template-files-group-{folder}"),
                node_id: String::new(),
                kind: "template_group".to_string(),
                label: folder,
                badges: Vec::new(),
                children: nodes,
                ..Default::default()
            });
        }
    }
    children.sort_by(|left, right| left.label.cmp(&right.label));
    ReachabilityTreeRoot {
        group: "template_files".to_string(),
        label: "Templates".to_string(),
        default_open: false,
        children,
    }
}

pub fn template_primary_consumer<'a>(
    compiled: &'a CompiledApp,
    template_key: &str,
) -> Option<&'a TemplateConsumerAnchor> {
    let entry = compiled.build_template_index.lookup(template_key)?;
    template_primary_consumer_from_entry(entry, compiled.active_scene.as_deref())
}

pub fn template_primary_consumer_from_entry<'a>(
    entry: &'a TemplateCatalogEntry,
    active_scene: Option<&str>,
) -> Option<&'a TemplateConsumerAnchor> {
    if entry.consumer_anchors.is_empty() {
        return None;
    }
    if let Some(active) = active_scene
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(found) = entry
            .consumer_anchors
            .iter()
            .find(|anchor| anchor.scene_id == active)
        {
            return Some(found);
        }
    }
    Some(&entry.consumer_anchors[0])
}

pub fn preview_target_for_template_consumer(
    compiled: &CompiledApp,
    template_key: &str,
) -> Option<String> {
    let anchor = template_primary_consumer(compiled, template_key)?;
    crate::compile::build_experience::preview_target_for_scene_id(
        compiled,
        anchor.scene_id.as_str(),
    )
}

pub fn preview_scene_id_for_template_consumer(
    compiled: &CompiledApp,
    template_key: &str,
) -> Option<String> {
    template_primary_consumer(compiled, template_key).map(|anchor| anchor.scene_id.clone())
}
