use std::collections::{BTreeMap, BTreeSet};

use crate::compile::reachability_tree::{ReachabilityTreeNode, ReachabilityTreeRoot};
use crate::model::{
    BuildNodeId, BuildTemplateIndex, ComponentAsset, ExperienceNodeManifest, PanelDecl,
    SceneContract, TemplateCatalogEntry, UiNodeDecl,
};

pub struct BuildTemplateIndexResult {
    pub index: BuildTemplateIndex,
    pub tree_root: ReachabilityTreeRoot,
}

pub fn build_template_index(
    component_assets: &[ComponentAsset],
    scene_contracts_by_id: &BTreeMap<String, SceneContract>,
    _node_manifest: &BTreeMap<String, ExperienceNodeManifest>,
) -> BuildTemplateIndexResult {
    let mut templates = BTreeMap::<String, TemplateCatalogEntry>::new();
    let mut use_key_consumers = BTreeMap::<String, BTreeSet<String>>::new();

    for contract in scene_contracts_by_id.values() {
        for panel in &contract.panels {
            collect_panel_use_keys(panel, &mut use_key_consumers);
        }
    }

    for asset in component_assets {
        let category = categorize_template_key(asset.key.as_str());
        let mut consumers: Vec<String> = use_key_consumers
            .get(asset.key.as_str())
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        consumers.sort();
        let variants = related_variant_keys(asset.key.as_str(), component_assets);
        let agent_hint = Some(agent_hint_for(category, asset.key.as_str(), asset.script.as_str()));
        templates.insert(
            asset.key.clone(),
            TemplateCatalogEntry {
                template_key: asset.key.clone(),
                template_file: asset.script.clone(),
                category: category.to_string(),
                props_schema: default_props_schema(category),
                variants,
                consumers,
                agent_hint,
            },
        );
    }

    let mut by_category = BTreeMap::<String, Vec<ReachabilityTreeNode>>::new();
    for entry in templates.values() {
        by_category
            .entry(entry.category.clone())
            .or_default()
            .push(ReachabilityTreeNode {
                id: format!("template-{}", entry.template_key),
                node_id: BuildNodeId::template(entry.template_key.as_str()).encode(),
                kind: "template".to_string(),
                label: entry.template_key.clone(),
                badges: vec![entry.template_file.clone()],
                children: Vec::new(),
                ..Default::default()
            });
    }

    let mut tree_children = Vec::new();
    for (category, nodes) in by_category {
        tree_children.push(ReachabilityTreeNode {
            id: format!("template-group-{category}"),
            node_id: String::new(),
            kind: "template_group".to_string(),
            label: category,
            badges: Vec::new(),
            children: nodes,
            ..Default::default()
        });
    }

    let index = BuildTemplateIndex { templates };
    let tree_root = ReachabilityTreeRoot {
        group: "templates".to_string(),
        label: "Components".to_string(),
        default_open: false,
        children: tree_children,
    };

    BuildTemplateIndexResult { index, tree_root }
}

/// Rebuild Templates reachability group from compile-time index (e.g. stale snapshot fallback).
pub fn template_tree_root_from_index(index: &BuildTemplateIndex) -> ReachabilityTreeRoot {
    let mut by_category = BTreeMap::<String, Vec<ReachabilityTreeNode>>::new();
    for entry in index.templates.values() {
        by_category
            .entry(entry.category.clone())
            .or_default()
            .push(ReachabilityTreeNode {
                id: format!("template-{}", entry.template_key),
                node_id: BuildNodeId::template(entry.template_key.as_str()).encode(),
                kind: "template".to_string(),
                label: entry.template_key.clone(),
                badges: vec![entry.template_file.clone()],
                children: Vec::new(),
                ..Default::default()
            });
    }
    let mut tree_children = Vec::new();
    for (category, nodes) in by_category {
        tree_children.push(ReachabilityTreeNode {
            id: format!("template-group-{category}"),
            node_id: String::new(),
            kind: "template_group".to_string(),
            label: category,
            badges: Vec::new(),
            children: nodes,
            ..Default::default()
        });
    }
    ReachabilityTreeRoot {
        group: "templates".to_string(),
        label: "Components".to_string(),
        default_open: false,
        children: tree_children,
    }
}

fn collect_panel_use_keys(panel: &PanelDecl, out: &mut BTreeMap<String, BTreeSet<String>>) {
    for ui_node in &panel.blocks {
        match ui_node {
            UiNodeDecl::Block(block) => {
                let consumer = block
                    .title
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| {
                        block
                            .id
                            .clone()
                            .unwrap_or_else(|| block.use_key.clone())
                    });
                out.entry(block.use_key.clone())
                    .or_default()
                    .insert(consumer);
            }
            UiNodeDecl::Panel(nested) => collect_panel_use_keys(nested, out),
            _ => {}
        }
    }
}

fn categorize_template_key(key: &str) -> &'static str {
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

fn related_variant_keys(key: &str, assets: &[ComponentAsset]) -> Vec<String> {
    let family = key.rsplit('.').next().unwrap_or(key);
    let mut variants: Vec<String> = assets
        .iter()
        .filter(|asset| asset.key.contains(family) && asset.key != key)
        .map(|asset| asset.key.clone())
        .collect();
    variants.sort();
    variants.dedup();
    variants
}

fn default_props_schema(category: &str) -> Vec<String> {
    match category {
        "metric_card" => vec![
            "metric (__ref metric)".to_string(),
            "title (optional)".to_string(),
            "value / unit overrides".to_string(),
        ],
        "panel_shell" => vec![
            "title".to_string(),
            "body blocks".to_string(),
        ],
        "table" => vec![
            "dataset / rowset".to_string(),
            "columns".to_string(),
        ],
        _ => vec!["props (component-specific)".to_string()],
    }
}

fn agent_hint_for(category: &str, key: &str, script: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_index_lists_metric_card_assets() {
        let assets = vec![ComponentAsset {
            key: "cockpit.metric-card".to_string(),
            tag: "div".to_string(),
            script: "templates/cockpit/metric-card.mei".to_string(),
        }];
        let result = build_template_index(&assets, &BTreeMap::new(), &BTreeMap::new());
        let entry = result
            .index
            .templates
            .get("cockpit.metric-card")
            .expect("template");
        assert_eq!(entry.category, "metric_card");
        assert!(entry.agent_hint.as_deref().is_some_and(|hint| hint.contains("metric-card")));
        assert!(!result.tree_root.children.is_empty());
    }
}
