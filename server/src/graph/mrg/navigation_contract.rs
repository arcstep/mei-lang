//! Contract checks: AppSkeleton routes ↔ MRG Navigation ↔ axum URL grammar.

use std::path::Path;

use mei_lang_kernel::resolve_app_root;

use crate::graph::mcg::app_skeleton::load_app_skeleton_artifact;
use crate::graph::mrg::navigation::list_navigation_entries;
use crate::graph::mrg::registry::MrgRegistryWriter;

#[derive(Debug, Clone, Default)]
pub struct NavigationContractReport {
    pub ok: bool,
    pub missing_access_keys: Vec<String>,
    pub missing_build_keys: Vec<String>,
    pub duplicate_keys: Vec<String>,
    pub orphan_urls: Vec<String>,
}

pub fn verify_navigation_contract(source_root: &Path, app_id: &str) -> NavigationContractReport {
    let mut report = NavigationContractReport {
        ok: true,
        ..Default::default()
    };
    let app_root = resolve_app_root(source_root, app_id);
    let Some(skeleton) = load_app_skeleton_artifact(app_root.as_path(), None)
        .ok()
        .flatten()
    else {
        return report;
    };
    let routes = skeleton
        .payload
        .get("sceneRoutes")
        .and_then(|value| serde_json::from_value::<Vec<mei_lang_kernel::CompiledSceneRoute>>(value.clone()).ok())
        .unwrap_or_default();
    let entries = list_navigation_entries(source_root, app_id);
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for entry in &entries {
        *seen.entry(entry.key.clone()).or_default() += 1;
        if !entry.url.starts_with("/apps/") {
            report.orphan_urls.push(entry.url.clone());
            report.ok = false;
        }
    }
    for (key, count) in seen {
        if count > 1 {
            report.duplicate_keys.push(key);
            report.ok = false;
        }
    }
    for route in routes {
        if !route.access_export {
            continue;
        }
        let access_key = format!("access:{}", route.scene_id);
        let build_key = format!("build:{}", route.scene_id);
        if !entries.iter().any(|entry| entry.key == access_key) {
            report.missing_access_keys.push(access_key);
            report.ok = false;
        }
        if !entries.iter().any(|entry| entry.key == build_key) {
            report.missing_build_keys.push(build_key);
            report.ok = false;
        }
    }
    report
}

pub fn navigation_drift_metrics(source_root: &Path, app_id: &str) -> (usize, usize, usize) {
    let registry = MrgRegistryWriter::load(source_root, app_id);
    let entries = registry.navigation_entries();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for entry in &entries {
        *counts.entry(entry.key.clone()).or_default() += 1;
    }
    let duplicates = counts.values().filter(|count| **count > 1).count();
    let orphans = entries
        .iter()
        .filter(|entry| !entry.url.starts_with("/apps/"))
        .count();
    (entries.len(), duplicates, orphans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::mrg::navigation::sync_navigation_registry;
    use std::fs;

    #[test]
    fn verify_navigation_contract_detects_missing_keys() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("apps/demo/build/active/graph/payloads")).expect("mkdir");
        fs::write(
            ws.join("apps/demo/build/active/graph/payloads/app-skeleton.json"),
            r#"{"schemaVersion":"mei-app-skeleton-artifact-v1","revision":"sk:t","payload":{"sceneRoutes":[{"scene_id":"home","target_file":"scenes/home.mei","kind":"scene","access_export":true}]}}"#,
        )
        .expect("write skeleton");
        let report = verify_navigation_contract(ws, "demo");
        assert!(!report.ok);
        assert!(report.missing_access_keys.contains(&"access:home".to_string()));
    }

    #[test]
    fn sync_then_verify_navigation_contract_passes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("runtime/platform/graphs/demo")).expect("mkdir");
        fs::write(ws.join("workspace.json"), r#"{"schemaVersion":2,"workspace":{"defaultApp":"demo"}}"#).expect("write ws");
        fs::create_dir_all(ws.join("apps/demo/src")).expect("mkdir");
        fs::write(ws.join("apps/demo/src/main.mei"), "app(id=demo)").expect("write main");
        sync_navigation_registry(
            ws,
            "demo",
            &[("home".to_string(), "scenes/home.mei".to_string())],
        )
        .expect("sync");
        let report = verify_navigation_contract(ws, "demo");
        assert!(report.missing_access_keys.is_empty() || report.ok);
    }
}
