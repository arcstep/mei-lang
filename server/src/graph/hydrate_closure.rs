//! Scope-closure hydrate: load only MCG ScenePayload seeds required by the active scope.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use mei_lang_kernel::CompiledApp;

use crate::graph::integration::{
    board_catalog_fallback_targets, capsule_paths_for_prebuild_hydrate,
    hydrate_metric_defs_from_mcg_cas, load_scene_payload_compiled_from_mcg,
    merge_compiled_runtime_catalog, world_capsule_path_for_scene,
};
use crate::graph::mcg::registry::{McgRegistry, McgRegistryWriter};
use mei_lang_kernel::resolve_app_root;

const CLOSURE_HYDRATE_ENV: &str = "MEI_MCG_CLOSURE_HYDRATE";

pub fn closure_hydrate_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var(CLOSURE_HYDRATE_ENV)
            .ok()
            .map(|value| {
                let trimmed = value.trim();
                !matches!(trimmed, "0" | "false" | "FALSE" | "no" | "off")
            })
            .unwrap_or(true)
    })
}

pub fn closure_seed_scene_files(
    compiled: &CompiledApp,
    mcg: &McgRegistry,
    metric_ids: &[String],
    owner_resource_ids: &[String],
) -> BTreeSet<String> {
    let mut seeds = BTreeSet::new();
    let target = compiled.active_target_file.trim();
    if !target.is_empty() {
        seeds.insert(mei_lang_kernel::canonical_app_source_rel_path(target));
        for fallback in board_catalog_fallback_targets(target) {
            seeds.insert(fallback);
        }
    }
    for capsule in crate::graph::integration::embedded_capsule_targets(compiled, mcg) {
        if let Some(world) = world_capsule_path_for_scene(capsule.as_str()) {
            seeds.insert(world);
        }
        seeds.insert(capsule);
    }
    for capsule in capsule_paths_for_prebuild_hydrate(metric_ids, owner_resource_ids) {
        if let Some(world) = world_capsule_path_for_scene(capsule.as_str()) {
            seeds.insert(world);
        }
        seeds.insert(capsule);
    }
    seeds
}

pub fn hydrate_assembled_scope_closure(
    source_root: &Path,
    app_id: &str,
    compiled: &mut CompiledApp,
    metric_ids: &[String],
    owner_resource_ids: &[String],
) {
    let app_root = resolve_app_root(source_root, app_id);
    let mcg = McgRegistryWriter::load(source_root, app_id);
    let active = mei_lang_kernel::canonical_app_source_rel_path(compiled.active_target_file.trim());
    for seed in closure_seed_scene_files(compiled, &mcg, metric_ids, owner_resource_ids) {
        if seed == active {
            continue;
        }
        if seed.ends_with(".board.mei") {
            continue;
        }
        if let Some(donor) =
            load_scene_payload_compiled_from_mcg(app_root.as_path(), &mcg, seed.as_str())
        {
            merge_compiled_runtime_catalog(compiled, &donor);
        }
        if seed.ends_with(".mei") && !seed.ends_with(".world.mei") {
            if let Some(world_capsule) = world_capsule_path_for_scene(seed.as_str()) {
                if let Some(donor) = load_scene_payload_compiled_from_mcg(
                    app_root.as_path(),
                    &mcg,
                    world_capsule.as_str(),
                ) {
                    merge_compiled_runtime_catalog(compiled, &donor);
                }
            }
        }
    }
    hydrate_metric_defs_from_mcg_cas(app_root.as_path(), &mcg, compiled);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::CompiledApp;

    fn minimal_compiled(target: &str) -> CompiledApp {
        CompiledApp {
            app_id: "demo".to_string(),
            title: String::new(),
            app_root: String::new(),
            active_scene: Some("home".to_string()),
            active_target_file: target.to_string(),
            file_tree: Vec::new(),
            scene_routes: Vec::new(),
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
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        }
    }

    #[test]
    fn closure_seed_board_includes_fallback_catalog_targets() {
        let compiled = minimal_compiled("scenes/x.board.mei");
        let mcg = McgRegistry::default();
        let seeds = closure_seed_scene_files(&compiled, &mcg, &[], &[]);
        assert!(seeds.contains("scenes/x.board.mei"));
        assert!(
            seeds.len() <= 5,
            "board closure seeds should stay small: {:?}",
            seeds
        );
    }

    #[test]
    fn closure_hydrate_enabled_by_default() {
        assert!(closure_hydrate_enabled());
    }
}
