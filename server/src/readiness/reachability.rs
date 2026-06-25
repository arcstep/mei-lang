use std::path::Path;

use mei_lang_kernel::{
    bundle_snapshot_root_from_env, load_workspace_config, resolve_app_id, CompileOptions,
    MEI_BUNDLE_SNAPSHOT_ROOT_ENV,
};
use mei_lang_toolchain::{self as toolchain, WorldScope};
use serde::Serialize;

use crate::graph::feature::graph_registry_dedup_enabled;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::types::{GraphNodeKind, MaterialState};

#[derive(Debug, Clone, Serialize)]
pub struct AccessEntry {
    pub app_id: String,
    pub scene_id: String,
    pub target_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShellReadyReport {
    pub shell_ready: bool,
    pub blockers: Vec<String>,
    pub access_entry: AccessEntry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReachabilityReport {
    pub access_entry: AccessEntry,
    pub shell_ready: bool,
    pub data_ready: bool,
    pub access_ready: bool,
    pub shell_blockers: Vec<String>,
    pub data_blockers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_snapshot_root: Option<String>,
}

struct SnapshotEnvGuard {
    previous: Option<String>,
}

impl SnapshotEnvGuard {
    fn install(snapshot_root: Option<&Path>) -> Self {
        let previous = std::env::var(MEI_BUNDLE_SNAPSHOT_ROOT_ENV).ok();
        match snapshot_root {
            Some(root) => std::env::set_var(
                MEI_BUNDLE_SNAPSHOT_ROOT_ENV,
                root.to_string_lossy().as_ref(),
            ),
            None => std::env::remove_var(MEI_BUNDLE_SNAPSHOT_ROOT_ENV),
        }
        Self { previous }
    }
}

impl Drop for SnapshotEnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(MEI_BUNDLE_SNAPSHOT_ROOT_ENV, value),
            None => std::env::remove_var(MEI_BUNDLE_SNAPSHOT_ROOT_ENV),
        }
    }
}

pub fn resolve_access_entry(source_root: &Path) -> AccessEntry {
    let cfg = load_workspace_config(source_root);
    let app_raw = cfg
        .deploy
        .access_entry
        .default_app
        .as_deref()
        .or(cfg.workspace.default_app.as_deref())
        .unwrap_or("zhifa");
    let app_id = resolve_app_id(source_root, app_raw);
    let scene_id = cfg
        .deploy
        .access_entry
        .default_scene
        .as_deref()
        .unwrap_or("home")
        .to_string();
    let target_file = cfg
        .deploy
        .access_entry
        .target_file
        .as_deref()
        .unwrap_or("scenes/home.mei")
        .to_string();
    AccessEntry {
        app_id,
        scene_id,
        target_file,
    }
}

pub fn check_shell_ready(source_root: &Path, entry: &AccessEntry) -> ShellReadyReport {
    let components_root = toolchain::resolve_components_root(source_root);
    let options = CompileOptions {
        scene: Some(entry.scene_id.clone()),
        preview_target: Some(entry.target_file.clone()),
        ..Default::default()
    };
    if let Some(outcome) = toolchain::load_compile_artifact_only(
        source_root,
        &entry.app_id,
        &options,
        components_root.as_path(),
    ) {
        return ShellReadyReport {
            shell_ready: true,
            blockers: Vec::new(),
            access_entry: entry.clone(),
            compile_revision: Some(outcome.compile_revision),
        };
    }

    let scope = WorldScope {
        scene_id: Some(entry.scene_id.clone()),
        target_file: Some(entry.target_file.clone()),
    };
    if toolchain::probe_compiled_app_manifest_identity(source_root, &entry.app_id, &scope).is_some()
    {
        return ShellReadyReport {
            shell_ready: true,
            blockers: Vec::new(),
            access_entry: entry.clone(),
            compile_revision: None,
        };
    }

    ShellReadyReport {
        shell_ready: false,
        blockers: vec![format!(
            "missing compiled_app artifact for {}/scene/{}/{}",
            entry.app_id, entry.scene_id, entry.target_file
        )],
        access_entry: entry.clone(),
        compile_revision: None,
    }
}

fn check_mcg_assembly_ready(source_root: &Path, entry: &AccessEntry) -> Option<String> {
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
    let node = registry.nodes.iter().find(|node| {
        node.id.kind == GraphNodeKind::AssemblyView && node.id.key == entry.scene_id
    });
    let Some(node) = node else {
        return Some(format!(
            "MCG assembly_view:{} missing from registry",
            entry.scene_id
        ));
    };
    if node.state == MaterialState::Ready {
        None
    } else {
        Some(format!(
            "MCG assembly_view:{} state={:?}",
            entry.scene_id, node.state
        ))
    }
}

fn check_mrg_critical_ready(source_root: &Path, entry: &AccessEntry) -> Vec<String> {
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
    let registry = MrgRegistryWriter::load(source_root, &entry.app_id);
    registry
        .slots
        .iter()
        .filter(|slot| slot.state != MaterialState::Ready)
        .map(|slot| {
            format!(
                "MRG slot {} state={:?}",
                slot.slot_id.node.stable_key(),
                slot.state
            )
        })
        .collect()
}

pub fn check_reachability(
    source_root: &Path,
    snapshot_root: Option<&Path>,
) -> ReachabilityReport {
    let _guard = SnapshotEnvGuard::install(snapshot_root);
    let entry = resolve_access_entry(source_root);
    let mut shell = check_shell_ready(source_root, &entry);
    if let Some(blocker) = check_mcg_assembly_ready(source_root, &entry) {
        if shell.shell_ready {
            shell.shell_ready = false;
        }
        shell.blockers.push(blocker);
    }
    let data_blockers = check_mrg_critical_ready(source_root, &entry);
    let data_ready = data_blockers.is_empty();
    let shell_ready = shell.shell_ready;
    ReachabilityReport {
        access_entry: entry,
        shell_ready,
        data_ready,
        access_ready: shell_ready && data_ready,
        shell_blockers: shell.blockers,
        data_blockers,
        bundle_snapshot_root: snapshot_root
            .map(|path| path.display().to_string())
            .or_else(|| bundle_snapshot_root_from_env().map(|path| path.display().to_string())),
    }
}

pub fn shell_ready_for_access_entry(source_root: &Path) -> bool {
    let entry = resolve_access_entry(source_root);
    check_shell_ready(source_root, &entry).shell_ready
}
