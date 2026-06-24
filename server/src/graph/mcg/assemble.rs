use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};

/// Explicit assembly inputs for AssemblyView (CompiledApp) derivation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyInputRecord {
    pub kind: String,
    pub key: String,
    pub revision: String,
}

#[derive(Debug, Clone, Default)]
pub struct AssemblyViewInputs {
    pub scene_payload: Option<AssemblyInputRecord>,
    pub metric_def_bundles: Vec<AssemblyInputRecord>,
    pub panel_contracts: Vec<AssemblyInputRecord>,
}

/// `assemble_assembly_view` is the explicit API boundary for AssemblyView derivation.
/// When inputs carry only metadata, the compiled app is returned unchanged; scope-specific
/// views should use [`assemble_scope_view`] for cheap projection.
pub fn assemble_assembly_view(
    compiled: CompiledApp,
    inputs: AssemblyViewInputs,
) -> (CompiledApp, Vec<AssemblyInputRecord>) {
    let mut assembly_inputs = Vec::new();
    if let Some(scene) = inputs.scene_payload {
        assembly_inputs.push(scene);
    }
    assembly_inputs.extend(inputs.metric_def_bundles);
    assembly_inputs.extend(inputs.panel_contracts);
    (compiled, assembly_inputs)
}

/// Cheap scope projection: patch active scene/target on an existing AssemblyView.
pub fn apply_scope_to_compiled_app(
    compiled: &mut CompiledApp,
    active_scene: Option<&str>,
    active_target: Option<&str>,
) {
    if let Some(scene) = active_scene.map(str::trim).filter(|value| !value.is_empty()) {
        compiled.active_scene = Some(scene.to_string());
    }
    if let Some(target) = active_target.map(str::trim).filter(|value| !value.is_empty()) {
        compiled.active_target_file = target.to_string();
    }
}

/// Derive a scope-specific AssemblyView from a representative compile outcome.
pub fn assemble_scope_view(
    compiled: CompiledApp,
    active_scene: Option<&str>,
    active_target: Option<&str>,
) -> CompiledApp {
    let mut view = compiled;
    apply_scope_to_compiled_app(&mut view, active_scene, active_target);
    view
}

pub fn assembly_view_revision(inputs: &[AssemblyInputRecord]) -> String {
    use crate::graph::types::stable_hash;
    let mut parts = inputs
        .iter()
        .map(|input| format!("{}:{}={}", input.kind, input.key, input.revision))
        .collect::<Vec<_>>();
    parts.sort();
    format!("av:{}", stable_hash(&parts.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembly_view_revision_stable() {
        let inputs = vec![AssemblyInputRecord {
            kind: "metric_def_bundle".to_string(),
            key: "ds1".to_string(),
            revision: "mdb:abc".to_string(),
        }];
        let rev = assembly_view_revision(&inputs);
        assert!(rev.starts_with("av:"));
    }

    #[test]
    fn assemble_scope_view_patches_active_scene() {
        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: "/tmp/demo".to_string(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/a.board.mei".to_string(),
            scene_routes: Vec::new(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: Vec::new(),
            world_metrics: Default::default(),
            world_semantic_by_file: Default::default(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        };
        let view = assemble_scope_view(
            compiled,
            Some("export_scene"),
            Some("scenes/a.board.mei"),
        );
        assert_eq!(view.active_scene.as_deref(), Some("export_scene"));
    }
}
