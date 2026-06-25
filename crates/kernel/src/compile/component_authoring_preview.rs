use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::mei_config::{resolve_authoring_root, resolve_workspace_source_root_from_app_root};
use crate::model::CompiledApp;

#[derive(Debug, Deserialize)]
struct ComponentContractsFile {
    components: Vec<ComponentContractEntry>,
}

#[derive(Debug, Deserialize)]
struct ComponentContractEntry {
    id: String,
    #[serde(default)]
    preferred_example_ids: Vec<String>,
}

static CONTRACTS: OnceLock<ComponentContractsFile> = OnceLock::new();

fn contracts() -> &'static ComponentContractsFile {
    CONTRACTS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../knowledge/editor-runtime/components/component-contracts.json"
        ))
        .expect("component-contracts.json must parse")
    })
}

fn package_authoring_examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stock/authoring/examples")
}

/// Map `example_chart_baseline` → `chart-baseline.mei`.
pub fn example_id_to_mei_filename(example_id: &str) -> Option<String> {
    let stem = example_id.strip_prefix("example_")?;
    if stem.is_empty() {
        return None;
    }
    Some(format!("{stem}.mei").replace('_', "-"))
}

pub fn preferred_example_id_for_component(component_key: &str) -> Option<&'static str> {
    let key = component_key.trim();
    if key.is_empty() {
        return None;
    }
    let file = contracts();
    for entry in &file.components {
        if entry.id == key {
            return entry.preferred_example_ids.first().map(String::as_str);
        }
    }
    let family = key.split_once('.').map(|(prefix, _)| prefix)?;
    let wildcard = format!("{family}.*");
    for entry in &file.components {
        if entry.id == wildcard {
            return entry.preferred_example_ids.first().map(String::as_str);
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
    let example_id = preferred_example_id_for_component(component_key)?;
    let filename = example_id_to_mei_filename(example_id)?;
    let app_root = Path::new(compiled.app_root.as_str());
    let source_root = resolve_workspace_source_root_from_app_root(app_root);
    if let Some(rel) = workspace_example_rel(source_root.as_path(), filename.as_str()) {
        return Some(rel);
    }
    let package_file = package_authoring_examples_root().join(&filename);
    if package_file.is_file() {
        return super::build_experience::preview_target_for_absolute_path(compiled, &package_file);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_area_resolves_chart_baseline_example() {
        assert_eq!(
            preferred_example_id_for_component("chart.area"),
            Some("example_chart_baseline")
        );
        assert_eq!(
            example_id_to_mei_filename("example_chart_baseline").as_deref(),
            Some("chart-baseline.mei")
        );
    }

    #[test]
    fn chart_bar_resolves_chart_wildcard_example() {
        assert_eq!(
            preferred_example_id_for_component("chart.bar"),
            Some("example_chart_baseline")
        );
    }

    #[test]
    fn cockpit_header_brand_resolves_cockpit_panel_example() {
        assert_eq!(
            preferred_example_id_for_component("cockpit.header-brand"),
            Some("example_cockpit_panel")
        );
    }
}
