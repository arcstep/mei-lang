use std::path::Path;

use serde::Serialize;

use crate::readiness::scope_gate::{check_scope_gate_for_access_entry, ScopeGateReport};
use crate::readiness::types::ScopeCoords;

#[derive(Debug, Clone, Serialize)]
pub struct AccessEntry {
    pub app_id: String,
    pub scene_id: String,
    pub target_file: String,
}

impl From<&ScopeCoords> for AccessEntry {
    fn from(scope: &ScopeCoords) -> Self {
        Self {
            app_id: scope.app_id.clone(),
            scene_id: scope.scene_id.clone(),
            target_file: scope.target_file.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReachabilityReport {
    pub access_entry: AccessEntry,
    pub shell_ready: bool,
    pub data_ready: bool,
    pub access_ready: bool,
    pub navigation_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigation_key: Option<String>,
    pub shell_blockers: Vec<String>,
    pub data_blockers: Vec<String>,
    pub scope_gate: ScopeGateReport,
}

pub fn resolve_access_entry(source_root: &Path) -> AccessEntry {
    legacy_resolve_access_entry(source_root)
}

pub fn legacy_resolve_access_entry(source_root: &Path) -> AccessEntry {
    use mei_lang_kernel::{discover_apps, load_workspace_config, resolve_app_id};
    let cfg = load_workspace_config(source_root);
    let app_raw = cfg
        .deploy
        .access_entry
        .default_app
        .as_deref()
        .or(cfg.workspace.default_app.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            discover_apps(source_root)
                .ok()
                .and_then(|apps| apps.first().map(|app| app.id.clone()))
        })
        .unwrap_or_else(|| "zhifa".to_string());
    let app_id = resolve_app_id(source_root, app_raw.as_str());
    let scene_id = cfg
        .deploy
        .access_entry
        .default_scene
        .as_deref()
        .unwrap_or("home")
        .to_string();
    let target_file = mei_lang_kernel::canonical_app_source_rel_path(
        cfg.deploy
            .access_entry
            .target_file
            .as_deref()
            .unwrap_or("scenes/home.mei"),
    );
    AccessEntry {
        app_id,
        scene_id,
        target_file,
    }
}

pub fn check_reachability(source_root: &Path, _snapshot_root: Option<&Path>) -> ReachabilityReport {
    let entry = resolve_access_entry(source_root);
    let gate = check_scope_gate_for_access_entry(source_root, &entry);
    let shell_blockers = gate
        .blockers
        .iter()
        .filter(|blocker| blocker.starts_with("L3:") || blocker.starts_with("L2:"))
        .cloned()
        .collect::<Vec<_>>();
    let data_blockers = gate
        .blockers
        .iter()
        .filter(|blocker| blocker.starts_with("L4:"))
        .map(|blocker| blocker.trim_start_matches("L4:").to_string())
        .collect::<Vec<_>>();
    ReachabilityReport {
        access_entry: entry,
        shell_ready: gate.shell_ready,
        data_ready: gate.data_ready,
        access_ready: gate.access_ready,
        navigation_ready: gate.navigation_ready,
        navigation_key: gate.navigation_key.clone(),
        shell_blockers,
        data_blockers,
        scope_gate: gate,
    }
}

pub fn shell_ready_for_access_entry(source_root: &Path) -> bool {
    let entry = resolve_access_entry(source_root);
    check_scope_gate_for_access_entry(source_root, &entry).access_ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn legacy_access_entry_prefers_first_discovered_app_over_zhifa_default() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::write(
            ws.join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"ws-demo"}}"#,
        )
        .expect("write workspace");
        fs::create_dir_all(ws.join("apps/hello/src")).expect("mkdir app");
        fs::write(ws.join("apps/hello/src/main.mei"), "app(id=hello)").expect("write main");
        let entry = legacy_resolve_access_entry(ws);
        assert_eq!(entry.app_id, "hello");
    }
}
