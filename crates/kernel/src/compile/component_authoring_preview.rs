use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::mei_config::{
    load_workspace_config, resolve_authoring_root, resolve_workspace_path,
    resolve_workspace_source_root_from_app_root,
};
use crate::model::CompiledApp;

#[derive(Debug, Deserialize)]
struct ComponentContractsFile {
    components: Vec<ComponentContractEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ComponentContractEntry {
    id: String,
    #[serde(default)]
    preferred_example_ids: Vec<String>,
}

static KERNEL_CONTRACTS: OnceLock<ComponentContractsFile> = OnceLock::new();

fn kernel_contracts() -> &'static ComponentContractsFile {
    KERNEL_CONTRACTS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../knowledge/editor-runtime/components/component-contracts.json"
        ))
        .expect("component-contracts.json must parse")
    })
}

fn workspace_contracts(source_root: &Path) -> Option<ComponentContractsFile> {
    let cfg = load_workspace_config(source_root);
    let rel = cfg.stock_contracts_rel()?;
    let path = resolve_workspace_path(source_root, rel);
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn merged_contract_entries(source_root: &Path) -> Vec<ComponentContractEntry> {
    let mut by_id = HashMap::<String, ComponentContractEntry>::new();
    for entry in &kernel_contracts().components {
        by_id.insert(entry.id.clone(), entry.clone());
    }
    if let Some(overlay) = workspace_contracts(source_root) {
        for entry in overlay.components {
            by_id.insert(entry.id.clone(), entry);
        }
    }
    by_id.into_values().collect()
}

/// Map `example_chart_baseline` → `chart-baseline.mei`.
pub fn example_id_to_mei_filename(example_id: &str) -> Option<String> {
    let stem = example_id.strip_prefix("example_")?;
    if stem.is_empty() {
        return None;
    }
    Some(format!("{stem}.mei").replace('_', "-"))
}

pub fn preferred_example_id_for_component(
    source_root: &Path,
    component_key: &str,
) -> Option<String> {
    let key = component_key.trim();
    if key.is_empty() {
        return None;
    }
    let entries = merged_contract_entries(source_root);
    for entry in &entries {
        if entry.id == key {
            return entry.preferred_example_ids.first().cloned();
        }
    }
    let family = key.split_once('.').map(|(prefix, _)| prefix)?;
    let wildcard = format!("{family}.*");
    for entry in &entries {
        if entry.id == wildcard {
            return entry.preferred_example_ids.first().cloned();
        }
    }
    None
}

fn workspace_example_rel(source_root: &Path, filename: &str) -> Option<String> {
    let examples_root = resolve_authoring_root(source_root).join("examples");
    if !examples_root.join(filename).is_file() {
        return None;
    }
    let authoring_prefix = resolve_authoring_root(source_root)
        .strip_prefix(source_root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .filter(|rel| !rel.is_empty())
        .unwrap_or_else(|| "stock/authoring".to_string());
    Some(format!("{authoring_prefix}/examples/{filename}"))
}

/// Workspace-relative path like `stock/authoring/examples/chart-baseline.mei` when available.
pub fn component_authoring_example_workspace_path(
    compiled: &CompiledApp,
    component_key: &str,
) -> Option<String> {
    let app_root = Path::new(compiled.app_root.as_str());
    let source_root = resolve_workspace_source_root_from_app_root(app_root);
    let example_id = preferred_example_id_for_component(source_root.as_path(), component_key)?;
    let filename = example_id_to_mei_filename(example_id.as_str())?;
    workspace_example_rel(source_root.as_path(), filename.as_str())
}

pub fn scene_contract_contains_use_key(
    contract: &crate::model::SceneContract,
    use_key: &str,
) -> bool {
    contract
        .panels
        .iter()
        .any(|panel| panel_contains_use_key(panel, use_key))
}

fn panel_contains_use_key(panel: &crate::model::UiNodeDecl, use_key: &str) -> bool {
    panel
        .blocks
        .iter()
        .any(|node| node_contains_use_key(node, use_key))
}

fn node_contains_use_key(node: &crate::model::UiTreeNode, use_key: &str) -> bool {
    match node {
        crate::model::UiTreeNode::Block(block) => block.use_key == use_key,
        crate::model::UiTreeNode::Panel(nested) => panel_contains_use_key(nested, use_key),
        crate::model::UiTreeNode::PanelRefEmbed(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_area_resolves_chart_baseline_example() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let example_id =
            preferred_example_id_for_component(tmp.path(), "chart.area").expect("example id");
        assert_eq!(example_id, "example_chart_baseline");
        assert_eq!(
            example_id_to_mei_filename(example_id.as_str()).as_deref(),
            Some("chart-baseline.mei")
        );
    }

    #[test]
    fn chart_bar_resolves_chart_wildcard_example() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            preferred_example_id_for_component(tmp.path(), "chart.bar"),
            Some("example_chart_baseline".to_string())
        );
    }
}
