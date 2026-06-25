use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use walkdir::WalkDir;

use crate::compile::block_instance_id;
use crate::compile::reachability_tree::{ReachabilityTreeNode, ReachabilityTreeRoot};
use crate::mei_config::{resolve_templates_root, stock_path_excluded, StockCatalogKind};
use crate::model::{
    BlockDecl, BuildNodeId, BuildTemplateIndex, CompiledApp, ComponentAsset,
    ExperienceNodeManifest, PanelDecl, SceneContract, TemplateCatalogEntry, TemplateConsumerAnchor,
    UiNodeDecl,
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
            },
        );
    }

    let mut by_category = BTreeMap::<String, Vec<ReachabilityTreeNode>>::new();
    for entry in templates.values() {
        by_category
            .entry(entry.category.clone())
            .or_default()
            .push(ReachabilityTreeNode {
                id: format!("component-{}", entry.template_key),
                node_id: if super::build_experience::is_template_file_node_key(entry.template_key.as_str())
                {
                    BuildNodeId::template(entry.template_key.as_str()).encode()
                } else {
                    BuildNodeId::component(entry.template_key.as_str()).encode()
                },
                kind: if super::build_experience::is_template_file_node_key(entry.template_key.as_str())
                {
                    "template_file".to_string()
                } else {
                    "component".to_string()
                },
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
    super::build_experience::preview_target_for_scene_id(compiled, anchor.scene_id.as_str())
}

pub fn preview_scene_id_for_template_consumer(
    compiled: &CompiledApp,
    template_key: &str,
) -> Option<String> {
    template_primary_consumer(compiled, template_key).map(|anchor| anchor.scene_id.clone())
}

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

/// Primary build preview: compile the template `.mei` itself (built-in preview scene + sample data).
pub fn authoring_preview_target_for_template(
    compiled: &CompiledApp,
    template_key: &str,
) -> Option<String> {
    let workspace_path = authoring_template_workspace_path(compiled, template_key)?;
    let rel =
        super::build_experience::preview_target_relative_to_app(compiled, workspace_path.as_str())?;
    if !template_file_supports_authoring_preview(compiled, rel.as_str()) {
        return None;
    }
    Some(rel)
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

fn authoring_template_workspace_path(compiled: &CompiledApp, template_key: &str) -> Option<String> {
    if super::build_experience::is_template_file_node_key(template_key) {
        return super::build_experience::template_file_preview_target(compiled, template_key);
    }
    let entry = template_entry_for_preview(compiled, template_key)?;
    let file = entry.template_file.as_str();
    if file.ends_with(".mei") {
        Some(file.to_string())
    } else {
        super::component_authoring_preview::component_authoring_example_workspace_path(
            compiled,
            template_key,
        )
    }
}

pub fn preview_target_for_template_file_consumer(
    compiled: &CompiledApp,
    template_file_key: &str,
) -> Option<String> {
    let anchor = template_primary_consumer_for_template_file(compiled, template_file_key)?;
    super::build_experience::preview_target_for_scene_id(compiled, anchor.scene_id.as_str())
}

pub fn preview_scene_id_for_template_file_consumer(
    compiled: &CompiledApp,
    template_file_key: &str,
) -> Option<String> {
    template_primary_consumer_for_template_file(compiled, template_file_key)
        .map(|anchor| anchor.scene_id.clone())
}

fn collect_panel_use_keys(panel: &PanelDecl, out: &mut BTreeMap<String, BTreeSet<String>>) {
    for ui_node in &panel.blocks {
        match ui_node {
            UiNodeDecl::Block(block) => {
                let consumer = block_consumer_label(block);
                out.entry(block.use_key.clone())
                    .or_default()
                    .insert(consumer);
            }
            UiNodeDecl::Panel(nested) => collect_panel_use_keys(nested, out),
            _ => {}
        }
    }
}

fn collect_panel_template_usage(
    scene_id: &str,
    panel: &PanelDecl,
    panel_path: &str,
    out: &mut BTreeMap<String, Vec<TemplateConsumerAnchor>>,
) {
    for (ordinal, ui_node) in panel.blocks.iter().enumerate() {
        match ui_node {
            UiNodeDecl::Block(block) => {
                out.entry(block.use_key.clone())
                    .or_default()
                    .push(TemplateConsumerAnchor {
                        scene_id: scene_id.to_string(),
                        panel_path: panel_path.to_string(),
                        block_id: block_instance_id(block, ordinal),
                        label: block_consumer_label(block),
                    });
            }
            UiNodeDecl::Panel(nested) => {
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
        "panel_shell" => vec!["title".to_string(), "body blocks".to_string()],
        "table" => vec!["dataset / rowset".to_string(), "columns".to_string()],
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
        assert!(entry
            .agent_hint
            .as_deref()
            .is_some_and(|hint| hint.contains("metric-card")));
        assert!(!result.tree_root.children.is_empty());
    }

    #[test]
    fn template_index_collects_consumer_anchors() {
        use crate::model::{SceneDecl, UiNodeDecl};
        let assets = vec![ComponentAsset {
            key: "cockpit.header-brand".to_string(),
            tag: "div".to_string(),
            script: "templates/cockpit/header-brand.mei".to_string(),
        }];
        let mut contracts = BTreeMap::new();
        contracts.insert(
            "home".to_string(),
            SceneContract {
                scene: SceneDecl {
                    kind: "scene".to_string(),
                    id: "home".to_string(),
                    profile: None,
                    state: serde_json::Value::Null,
                    world: None,
                    flow: None,
                    frame: None,
                    theme: None,
                    summary: None,
                    goal: None,
                    shared: serde_json::Value::Null,
                    local_nav: serde_json::Value::Null,
                    params: serde_json::Value::Null,
                    bindings: serde_json::Value::Null,
                    examples: serde_json::Value::Null,
                    access_export: true,
                },
                themes: Vec::new(),
                shared: serde_json::Value::Null,
                world: None,
                flow: None,
                frame: None,
                panels: vec![PanelDecl {
                    id: "header".to_string(),
                    blocks: vec![UiNodeDecl::Block(BlockDecl {
                        kind: "block".to_string(),
                        use_key: "cockpit.header-brand".to_string(),
                        id: None,
                        title: None,
                        area: None,
                        props: serde_json::Value::Null,
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
                    ..Default::default()
                }],
            },
        );
        let result = build_template_index(&assets, &contracts, &BTreeMap::new());
        let entry = result
            .index
            .templates
            .get("cockpit.header-brand")
            .expect("template");
        assert_eq!(entry.consumer_anchors.len(), 1);
        assert_eq!(entry.consumer_anchors[0].scene_id, "home");
        assert_eq!(entry.consumer_anchors[0].panel_path, "header");
    }

    #[test]
    fn js_component_authoring_preview_targets_stock_example() {
        use std::path::Path;

        use crate::compile::{
            compile_app_from_root_with_options, compile_coordinate_for_node, BuildPreviewKind,
            CompileOptions,
        };
        use crate::model::BuildNodeId;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-spbjw");
        let app_root = source_root.join("zhifa");
        let examples_root = source_root.join("stock/authoring/examples/chart-baseline.mei");
        if !app_root.is_dir() || !examples_root.is_file() {
            return;
        }
        let compiled =
            compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
                .expect("compile zhifa");
        let node = BuildNodeId::component("chart.area");
        let target = authoring_preview_target_for_template(&compiled, "chart.area");
        assert!(
            target
                .as_deref()
                .is_some_and(|file| file.contains("chart-baseline.mei")),
            "expected chart baseline example, got {target:?}"
        );
        let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
        assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
        assert!(
            coord.preview_target.contains("chart-baseline.mei"),
            "coord target should be example mei, got {}",
            coord.preview_target
        );
        let preview_compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: coord.preview_target.clone().into(),
            },
        )
        .expect("compile chart.area authoring preview");
        let errors: Vec<_> = preview_compiled
            .diagnostics
            .iter()
            .filter(|diag| matches!(diag.severity, crate::Severity::Error))
            .collect();
        assert!(
            errors.is_empty(),
            "chart.area authoring preview should compile cleanly: {errors:?}"
        );
        assert!(
            preview_compiled.scene_contract.is_some(),
            "chart.area authoring preview should yield scene contract"
        );
    }

    #[test]
    fn ws_hello_chart_area_authoring_preview_coordinate() {
        use std::path::Path;

        use crate::compile::{
            compile_app_from_root_with_options, compile_coordinate_for_node, BuildPreviewKind,
            CompileOptions,
        };
        use crate::model::BuildNodeId;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root")
            .join("workspaces")
            .join("ws-hello");
        let app_root = source_root.join("apps").join("hello");
        if !app_root.is_dir() {
            return;
        }
        let compiled =
            compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
                .expect("compile ws-hello hello");
        let node = BuildNodeId::component("chart.area");
        let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
        assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
        assert!(
            coord.preview_target.contains("chart-baseline.mei"),
            "expected chart baseline example, got {}",
            coord.preview_target
        );
    }

    #[test]
    fn template_preview_targets_primary_consumer_scene() {
        use std::path::Path;

        use crate::compile::{
            compile_app_from_root_with_options, compile_coordinate_for_node, BuildPreviewKind,
            CompileOptions,
        };
        use crate::model::BuildNodeId;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-spbjw");
        let app_root = source_root.join("zhifa");
        if !app_root.is_dir() {
            return;
        }
        let compiled =
            compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
                .expect("compile zhifa");
        let node = BuildNodeId::template("cockpit.header-brand");
        let target = preview_target_for_template_consumer(&compiled, "cockpit.header-brand");
        assert!(
            target.as_deref().is_some_and(|file| file.contains("home")),
            "expected home scene file, got {target:?}"
        );
        let scene = preview_scene_id_for_template_consumer(&compiled, "cockpit.header-brand");
        assert_eq!(scene.as_deref(), Some("home"));
        let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
        let cockpit_example = source_root.join("stock/authoring/examples/cockpit-panel.mei");
        if cockpit_example.is_file() {
            assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
            assert!(coord.preview_target.contains("cockpit-panel.mei"));
        } else {
            assert_eq!(coord.preview_kind, BuildPreviewKind::SceneCapsule);
        }
    }

    #[test]
    fn template_file_authoring_preview_targets_template_mei() {
        use std::collections::BTreeMap;

        use crate::model::{
            BuildTemplateIndex, CompiledApp, CompiledSceneRoute, TemplateCatalogEntry,
        };

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("workspaces")
            .join("ws-spbjw");
        let app_root = source_root.join("zhifa");
        let template_mei = source_root.join("stock/templates/cockpit/main.mei");
        if !app_root.is_dir() || !template_mei.is_file() {
            return;
        }

        let mut templates = BTreeMap::new();
        templates.insert(
            "cockpit.main".to_string(),
            TemplateCatalogEntry {
                template_key: "cockpit.main".to_string(),
                template_file: "stock/templates/cockpit/main.mei".to_string(),
                category: "component".to_string(),
                props_schema: Vec::new(),
                variants: Vec::new(),
                consumers: vec!["home/header".to_string()],
                consumer_anchors: vec![TemplateConsumerAnchor {
                    scene_id: "home".to_string(),
                    panel_path: "header".to_string(),
                    block_id: "cockpit.main~0".to_string(),
                    label: "Header".to_string(),
                }],
                agent_hint: None,
            },
        );
        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: app_root.display().to_string(),
            scene_routes: vec![CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "scenes/home.mei".to_string(),
                kind: "file_ref".to_string(),
                title: Some("Home".to_string()),
                is_default: true,
                access_export: true,
            }],
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: BuildTemplateIndex { templates },
        };
        assert_eq!(
            authoring_preview_target_for_template(&compiled, "cockpit/main.mei").as_deref(),
            Some("../stock/templates/cockpit/main.mei")
        );
        assert_eq!(
            preview_scene_id_for_template_file_consumer(&compiled, "cockpit/main.mei").as_deref(),
            Some("home")
        );
        assert_eq!(
            preview_target_for_template_file_consumer(&compiled, "cockpit/main.mei").as_deref(),
            Some("scenes/home.mei")
        );
    }
}
