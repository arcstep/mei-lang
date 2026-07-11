use std::collections::BTreeMap;

use crate::mei_config::canonical_app_source_rel_path;
use crate::model::CompiledApp;

fn rekey_path_map<T: Clone>(map: &mut BTreeMap<String, T>, old: &str, new: &str) {
    if old == new {
        return;
    }
    if let Some(value) = map.remove(old) {
        map.entry(new.to_string()).or_insert(value);
    }
}

/// Normalize authored `.mei` paths on a compile result so MCG/MRG/scope keys use `src/`.
pub fn canonicalize_compiled_app_source_paths(compiled: &mut CompiledApp) {
    compiled.active_target_file =
        canonical_app_source_rel_path(compiled.active_target_file.as_str());
    for route in &mut compiled.scene_routes {
        route.target_file = canonical_app_source_rel_path(route.target_file.as_str());
    }

    let path_keys = compiled
        .world_semantic_by_file
        .keys()
        .chain(compiled.scene_local_nav_by_target.keys())
        .cloned()
        .collect::<Vec<_>>();
    for old in path_keys {
        let new = canonical_app_source_rel_path(old.as_str());
        rekey_path_map(
            &mut compiled.world_semantic_by_file,
            old.as_str(),
            new.as_str(),
        );
        rekey_path_map(
            &mut compiled.scene_local_nav_by_target,
            old.as_str(),
            new.as_str(),
        );
    }
}
