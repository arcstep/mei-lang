use std::path::Path;

use mei_lang_kernel::{load_workspace_config, resolve_app_id};

use crate::graph::mrg::registry::{MrgRegistry, MrgRegistryWriter};
use crate::graph::mrg::warmup::record_navigation_edge;
use crate::graph::types::MaterialState;

pub fn sync_navigation_registry(
    source_root: &Path,
    app_id: &str,
    scene_routes: &[(String, String)],
) -> anyhow::Result<()> {
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    let cfg = load_workspace_config(source_root);
    let canonical_app = resolve_app_id(source_root, app_id);
    let default_scene = cfg
        .deploy
        .access_entry
        .default_scene
        .as_deref()
        .unwrap_or("home")
        .to_string();

    for (scene_id, target_file) in scene_routes {
        let scene_id = scene_id.trim();
        let target_file = target_file.trim();
        if scene_id.is_empty() || target_file.is_empty() {
            continue;
        }
        let access_url = format!("/apps/app/{canonical_app}/scene/{scene_id}");
        let build_url = format!(
            "/apps/build/{canonical_app}?scene={scene_id}&file={}",
            urlencoding_path_segment(target_file)
        );
        upsert_navigation_node(
            &mut registry,
            &format!("access:{scene_id}"),
            &access_url,
            scene_id,
            target_file,
        );
        upsert_navigation_node(
            &mut registry,
            &format!("build:{scene_id}"),
            &build_url,
            scene_id,
            target_file,
        );
        if scene_id != default_scene.as_str() {
            record_navigation_edge(&mut registry, default_scene.as_str(), scene_id);
        }
    }

    if let Some(default_app) = cfg
        .deploy
        .access_entry
        .default_app
        .as_deref()
        .or(cfg.workspace.default_app.as_deref())
    {
        if resolve_app_id(source_root, default_app) == canonical_app {
            let scene = cfg
                .deploy
                .access_entry
                .default_scene
                .as_deref()
                .unwrap_or("home");
            let target = cfg
                .deploy
                .access_entry
                .target_file
                .as_deref()
                .unwrap_or("scenes/home.mei");
            upsert_navigation_node(
                &mut registry,
                "default_access",
                &format!("/apps/app/{canonical_app}/scene/{scene}"),
                scene,
                target,
            );
            upsert_navigation_node(
                &mut registry,
                "default_build",
                &format!("/apps/build/{canonical_app}"),
                scene,
                target,
            );
        }
    }

    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)
}

fn upsert_navigation_node(
    registry: &mut MrgRegistry,
    key: &str,
    url: &str,
    scene_id: &str,
    target_file: &str,
) {
    registry.upsert_navigation_node(
        key,
        url,
        scene_id,
        target_file,
        MaterialState::Ready,
    );
}

fn urlencoding_path_segment(value: &str) -> String {
    value.replace(' ', "%20")
}
