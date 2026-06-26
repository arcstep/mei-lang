use std::path::Path;

use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{load_workspace_config, CompileOptions};
use serde::Serialize;

use crate::graph::feature::graph_registry_dedup_enabled;
use crate::graph::integration::try_assemble_scope_from_scene_payload;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::navigation::{
    match_request_to_navigation, match_request_to_navigation_with_opts, resolve_default_scope_with_opts,
    NavigationResolveOpts,
};
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::types::{GraphNodeKind, MaterialState};
use crate::http::pages::AppQuery;
use crate::readiness::reachability::AccessEntry;
use crate::readiness::types::{ScopeCoords, UiMode};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeGateReport {
    pub scope: ScopeCoords,
    pub navigation_ready: bool,
    pub assembly_ready: bool,
    pub shell_ready: bool,
    pub data_ready: bool,
    pub access_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigation_key: Option<String>,
    pub blockers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_revision: Option<String>,
    // Legacy aliases for existing callers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_scene: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_target: Option<String>,
}

impl ScopeGateReport {
    fn from_parts(
        scope: ScopeCoords,
        navigation_ready: bool,
        assembly_ready: bool,
        shell_ready: bool,
        data_ready: bool,
        access_ready: bool,
        navigation_key: Option<String>,
        compile_revision: Option<String>,
        blockers: Vec<String>,
    ) -> Self {
        Self {
            resolved_scene: Some(scope.scene_id.clone()),
            resolved_target: Some(scope.target_file.clone()),
            scope,
            navigation_ready,
            assembly_ready,
            shell_ready,
            data_ready,
            access_ready,
            navigation_key,
            compile_revision,
            blockers,
        }
    }
}

pub fn resolve_scope_gate(
    source_root: &Path,
    app_id: &str,
    route_mode: UiRouteMode,
    scene_id: Option<&str>,
    query: &AppQuery,
) -> ScopeGateReport {
    let nav_match =
        match_request_to_navigation(source_root, app_id, route_mode, scene_id, query);
    check_scope_gate_for_coords(source_root, nav_match, None)
}

/// Align L3 assembly checks with compile options (build `file=` may differ from MRG default target).
pub fn resolve_scope_gate_for_compile(
    source_root: &Path,
    app_id: &str,
    route_mode: UiRouteMode,
    compile_options: &CompileOptions,
    query: &AppQuery,
) -> ScopeGateReport {
    let scene = compile_options
        .scene
        .as_deref()
        .or(query.scene.as_deref());
    let target = compile_options
        .preview_target
        .as_deref()
        .or(query.file.as_deref());
    let aligned = AppQuery {
        file: target.map(str::to_string),
        scene: scene.map(str::to_string),
        tab: query.tab.clone(),
        diag_filter: query.diag_filter.clone(),
        world_metric: query.world_metric.clone(),
        world_dataset: query.world_dataset.clone(),
        explain: query.explain.clone(),
        node: query.node.clone(),
        scope: query.scope.clone(),
        focus: query.focus.clone(),
        chrome: query.chrome.clone(),
        catalog: query.catalog.clone(),
        pack: query.pack.clone(),
    };
    let nav_match =
        match_request_to_navigation(source_root, app_id, route_mode, scene, &aligned);
    let assembly_scene = scene
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| nav_match.scope.scene_id.clone());
    let assembly_target = target
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            let nav_target = nav_match.scope.target_file.trim();
            if !nav_target.is_empty() {
                nav_match.scope.target_file.clone()
            } else if !assembly_scene.is_empty() {
                mei_lang_kernel::canonical_app_source_rel_path(&format!(
                    "scenes/{}.mei",
                    assembly_scene.trim()
                ))
            } else {
                String::new()
            }
        });
    let assembly_scope = ScopeCoords::new(
        app_id,
        crate::readiness::types::UiMode::from_route_mode(route_mode),
        assembly_scene,
        assembly_target,
    );
    check_scope_gate_for_coords(source_root, nav_match, Some(assembly_scope))
}

#[allow(dead_code)]
pub fn check_scope_gate(
    source_root: &Path,
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
) -> ScopeGateReport {
    check_scope_gate_silent(source_root, app_id, scene_id, target_file, false)
}

pub fn check_scope_gate_silent(
    source_root: &Path,
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    silent: bool,
) -> ScopeGateReport {
    let opts = NavigationResolveOpts { silent };
    let query = AppQuery {
        file: target_file.map(str::to_string),
        scene: scene_id.map(str::to_string),
        tab: None,
        diag_filter: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        node: None,
        scope: None,
        focus: None,
        chrome: None,
        catalog: None,
        pack: None,
    };
    let nav_match = if scene_id.is_some() || target_file.is_some() {
        match_request_to_navigation_with_opts(
            source_root,
            app_id,
            UiRouteMode::Build,
            scene_id,
            &query,
            opts,
        )
    } else {
        resolve_default_scope_with_opts(source_root, app_id, UiMode::Build, opts)
    };
    check_scope_gate_for_coords(source_root, nav_match, None)
}

fn check_scope_gate_for_coords(
    source_root: &Path,
    nav_match: crate::graph::mrg::navigation::NavigationMatch,
    assembly_scope: Option<ScopeCoords>,
) -> ScopeGateReport {
    let scope = assembly_scope.unwrap_or_else(|| nav_match.scope.clone());
    let app_id = scope.app_id.as_str();
    let scene = scope.scene_id.clone();
    let target = scope.target_file.clone();
    let navigation_ready = nav_match.navigation_ready();
    let navigation_key = nav_match.navigation_key();
    let mut blockers = Vec::new();

    if !navigation_ready {
        if nav_match.legacy_fallback {
            blockers.push("L2:navigation missing in MRG registry (legacy scope fallback)".to_string());
        } else if let Some(entry) = nav_match.entry.as_ref() {
            blockers.push(format!(
                "L2:navigation {} state={:?}",
                entry.key, entry.state
            ));
        } else {
            blockers.push("L2:navigation entry missing".to_string());
        }
    }

    let scene_arg = scene.as_str().trim();
    let scene_for_assemble = if scene_arg.is_empty() {
        None
    } else {
        Some(scene_arg)
    };

    let (assembly_ready, shell_ready, compile_revision) =
        if try_assemble_scope_from_scene_payload(
            source_root,
            app_id,
            scene_for_assemble,
            target.as_str(),
        )
        .is_some()
        {
            (true, true, None)
        } else {
            let entry = AccessEntry {
                app_id: app_id.to_string(),
                scene_id: scene.clone(),
                target_file: target.clone(),
            };
            if let Some(blocker) = check_mcg_scene_payload_ready(source_root, &entry) {
                blockers.push(format!("L3:{blocker}"));
            } else {
                blockers.push(format!(
                    "L3:scope artifact missing for app={app_id} scene={scene} target={target}"
                ));
            }
            (false, false, None)
        };

    let data_blockers = check_mrg_data_ready(source_root, app_id);
    let data_ready = data_blockers.is_empty();
    blockers.extend(data_blockers.into_iter().map(|b| format!("L4:{b}")));

    let require_data = load_workspace_config(source_root)
        .deploy
        .reachability_gate
        .require_mrg_critical_ready
        .unwrap_or(false);
    let access_ready = shell_ready
        && (!require_data || data_ready)
        && navigation_ready;

    ScopeGateReport::from_parts(
        scope,
        navigation_ready,
        assembly_ready,
        shell_ready,
        data_ready,
        access_ready,
        navigation_key,
        compile_revision,
        blockers,
    )
}

fn check_mcg_scene_payload_ready(source_root: &Path, entry: &AccessEntry) -> Option<String> {
    if !graph_registry_dedup_enabled() {
        return None;
    }
    let cfg = load_workspace_config(source_root);
    if !cfg
        .deploy
        .reachability_gate
        .require_mcg_assembly_ready
        .unwrap_or(true)
    {
        return None;
    }
    let registry = McgRegistryWriter::load(source_root, &entry.app_id);
    let lookup_keys = mei_lang_kernel::app_source_rel_path_lookup_keys(entry.target_file.as_str());
    let node = lookup_keys.iter().find_map(|key| {
        registry.nodes.iter().find(|node| {
            node.id.kind == GraphNodeKind::ScenePayload && node.id.key == *key
        })
    });
    let Some(node) = node else {
        return Some(format!(
            "MCG scene_payload:{} missing from registry",
            mei_lang_kernel::canonical_app_source_rel_path(entry.target_file.as_str())
        ));
    };
    if node.state == MaterialState::Ready {
        None
    } else {
        Some(format!(
            "MCG scene_payload:{} state={:?}",
            entry.target_file, node.state
        ))
    }
}

fn check_mrg_data_ready(source_root: &Path, app_id: &str) -> Vec<String> {
    let cfg = load_workspace_config(source_root);
    if !cfg
        .deploy
        .reachability_gate
        .require_mrg_critical_ready
        .unwrap_or(false)
    {
        return Vec::new();
    }
    if !graph_registry_dedup_enabled() {
        return vec!["MRG registry dedup disabled".to_string()];
    }
    let registry = MrgRegistryWriter::load(source_root, app_id);
    registry
        .slots
        .iter()
        .filter_map(|slot| {
            if slot.state != MaterialState::Ready {
                return Some(format!(
                    "MRG slot {} state={:?}",
                    slot.slot_id.node.stable_key(),
                    slot.state
                ));
            }
            if crate::graph::mrg::slots::resolve_slot_payload_path(source_root, app_id, slot).is_none()
            {
                return Some(format!(
                    "MRG slot {} payload missing (CAS and legacy path)",
                    slot.slot_id.node.stable_key()
                ));
            }
            None
        })
        .collect()
}

pub fn check_scope_gate_for_access_entry(source_root: &Path, entry: &AccessEntry) -> ScopeGateReport {
    let query = AppQuery {
        file: Some(entry.target_file.clone()),
        scene: Some(entry.scene_id.clone()),
        tab: None,
        diag_filter: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        node: None,
        scope: None,
        focus: None,
        chrome: None,
        catalog: None,
        pack: None,
    };
    resolve_scope_gate(
        source_root,
        entry.app_id.as_str(),
        UiRouteMode::App,
        Some(entry.scene_id.as_str()),
        &query,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_app::UiRouteMode;
    use mei_lang_kernel::CompileOptions;
    use std::fs;

    use crate::http::pages::AppQuery;

    #[test]
    fn scope_gate_report_serializes_layer_fields() {
        let report = ScopeGateReport::from_parts(
            ScopeCoords::new("hello", UiMode::Build, "home", "scenes/home.mei"),
            true,
            true,
            true,
            true,
            true,
            Some("default_build".to_string()),
            None,
            Vec::new(),
        );
        let json = serde_json::to_value(&report).expect("json");
        assert_eq!(json["navigationReady"], true);
        assert_eq!(json["accessReady"], true);
    }

    #[test]
    fn compile_gate_infers_target_from_mrg_when_scene_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("runtime/platform/graphs/demo")).expect("mkdir");
        fs::write(
            ws.join("runtime/platform/graphs/demo/mrg-registry.json"),
            r#"{"schemaVersion":"mei-mrg-registry-v1","appId":"demo","registryRevision":"x","updatedAtMs":0,"nodes":[{"id":{"kind":"navigation","key":"build:home"},"url":"/apps/build/demo?scene=home&file=scenes/home.mei","sceneId":"home","targetFile":"scenes/home.mei","state":"ready"}],"slots":[],"edges":[]}"#,
        )
        .expect("write mrg");
        let compile_options = CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        };
        let query = AppQuery {
            file: None,
            scene: None,
            tab: None,
            diag_filter: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            node: Some("scene:home".to_string()),
            scope: None,
            focus: None,
            chrome: None,
            catalog: None,
            pack: None,
        };
        let gate = resolve_scope_gate_for_compile(
            ws,
            "demo",
            UiRouteMode::Build,
            &compile_options,
            &query,
        );
        assert_eq!(gate.resolved_target.as_deref(), Some("src/scenes/home.mei"));
        assert!(
            !gate
                .blockers
                .iter()
                .any(|blocker| blocker == "L3:MCG scene_payload: missing from registry"),
            "unexpected empty-target MCG lookup: {:?}",
            gate.blockers
        );
    }
}
