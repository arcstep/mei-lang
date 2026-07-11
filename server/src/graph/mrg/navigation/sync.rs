use std::path::Path;

use mei_lang_kernel::{
    load_workspace_config, resolve_app_id, resolve_app_root, resolve_default_scene_from_root,
};

use crate::graph::mrg::registry::{MrgRegistry, MrgRegistryWriter};
use crate::graph::mrg::warmup::record_navigation_edge;
use crate::graph::types::{stable_hash, MaterialState};

/// Minimal scene×target pair for MRG navigation sync (prebuild compile scopes).
#[derive(Debug, Clone)]
pub struct CompileScopeNav {
    pub scene_id: String,
    pub target_file: String,
}

pub fn sync_navigation_registry(
    source_root: &Path,
    app_id: &str,
    scene_routes: &[(String, String)],
) -> anyhow::Result<()> {
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    let cfg = load_workspace_config(source_root);
    let canonical_app = resolve_app_id(source_root, app_id);
    let app_root = resolve_app_root(source_root, app_id);
    let default_scene = resolve_default_scene_from_root(app_root.as_path())
        .ok()
        .flatten()
        .map(|scene| scene.trim().to_string())
        .filter(|scene| !scene.is_empty())
        .or_else(|| {
            cfg.deploy
                .access_entry
                .default_scene
                .as_deref()
                .map(str::trim)
                .filter(|scene| !scene.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            scene_routes
                .first()
                .map(|(scene_id, _)| scene_id.trim().to_string())
        })
        .filter(|scene| !scene.is_empty())
        .unwrap_or_else(|| "home".to_string());

    for (scene_id, target_file) in scene_routes {
        let scene_id = scene_id.trim();
        let target_file = target_file.trim();
        if scene_id.is_empty() || target_file.is_empty() {
            continue;
        }
        let access_url = format!("/apps/app/{canonical_app}/scene/{scene_id}");
        let layout_url = format!(
            "/apps/{canonical_app}/layout?scene={scene_id}&file={}",
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
            &format!("layout:{scene_id}"),
            &layout_url,
            scene_id,
            target_file,
        );
        if scene_id != default_scene.as_str() {
            record_navigation_edge(&mut registry, default_scene.as_str(), scene_id);
        }
    }

    let default_target = scene_routes
        .iter()
        .find(|(scene_id, _)| scene_id.trim() == default_scene.as_str())
        .map(|(_, target)| target.as_str())
        .or_else(|| {
            cfg.deploy
                .access_entry
                .target_file
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty())
        })
        .or_else(|| scene_routes.first().map(|(_, target)| target.as_str()))
        .unwrap_or("src/scenes/home.mei");
    upsert_navigation_node(
        &mut registry,
        "default_access",
        &format!("/apps/app/{canonical_app}/scene/{default_scene}"),
        default_scene.as_str(),
        default_target,
    );
    upsert_navigation_node(
        &mut registry,
        "default_layout",
        &format!("/apps/{canonical_app}/layout"),
        default_scene.as_str(),
        default_target,
    );

    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)
}

/// Register every prebuild compile scope (scene×target) so L2 gate refresh hits MRG navigation.
pub fn sync_navigation_for_compile_scopes(
    source_root: &Path,
    app_id: &str,
    scopes: &[CompileScopeNav],
) -> anyhow::Result<()> {
    if scopes.is_empty() {
        return Ok(());
    }
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    let cfg = load_workspace_config(source_root);
    let canonical_app = resolve_app_id(source_root, app_id);
    let mut seen = std::collections::BTreeSet::new();

    for scope in scopes {
        let scene_id = scope.scene_id.trim();
        let target_file = scope.target_file.trim();
        if scene_id.is_empty() || target_file.is_empty() {
            continue;
        }
        let dedupe_key = format!("{scene_id}\0{target_file}");
        if !seen.insert(dedupe_key) {
            continue;
        }
        let access_url = format!("/apps/app/{canonical_app}/scene/{scene_id}");
        let layout_url = format!(
            "/apps/{canonical_app}/layout?scene={scene_id}&file={}",
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
            &format!("layout:{scene_id}"),
            &layout_url,
            scene_id,
            target_file,
        );
        let scope_key = format!(
            "scope:{scene_id}:{}",
            stable_hash(&format!("{scene_id}@{target_file}"))
        );
        upsert_navigation_node(
            &mut registry,
            scope_key.as_str(),
            &access_url,
            scene_id,
            target_file,
        );
    }

    let default_scene =
        resolve_default_scene_from_root(resolve_app_root(source_root, app_id).as_path())
            .ok()
            .flatten()
            .map(|scene| scene.trim().to_string())
            .filter(|scene| !scene.is_empty())
            .or_else(|| {
                cfg.deploy
                    .access_entry
                    .default_scene
                    .as_deref()
                    .map(str::trim)
                    .filter(|scene| !scene.is_empty())
                    .map(str::to_string)
            })
            .or_else(|| scopes.first().map(|scope| scope.scene_id.clone()))
            .unwrap_or_else(|| "home".to_string());

    for scope in scopes {
        let scene_id = scope.scene_id.trim();
        if scene_id.is_empty() || scene_id == default_scene.as_str() {
            continue;
        }
        record_navigation_edge(&mut registry, default_scene.as_str(), scene_id);
    }

    let default_target = scopes
        .iter()
        .find(|scope| scope.scene_id.trim() == default_scene.as_str())
        .map(|scope| scope.target_file.as_str())
        .or_else(|| {
            cfg.deploy
                .access_entry
                .target_file
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty())
        })
        .or_else(|| scopes.first().map(|scope| scope.target_file.as_str()))
        .unwrap_or("src/scenes/home.mei");
    upsert_navigation_node(
        &mut registry,
        "default_access",
        &format!("/apps/app/{canonical_app}/scene/{default_scene}"),
        default_scene.as_str(),
        default_target,
    );
    upsert_navigation_node(
        &mut registry,
        "default_layout",
        &format!("/apps/{canonical_app}/layout"),
        default_scene.as_str(),
        default_target,
    );

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
    registry.upsert_navigation_node(key, url, scene_id, target_file, MaterialState::Ready);
}

fn urlencoding_path_segment(value: &str) -> String {
    value.replace(' ', "%20")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sync_compile_scopes_writes_default_access() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("runtime/platform/graphs/catalog")).expect("mkdir");
        fs::create_dir_all(ws.join("apps/demo/src/scenes")).expect("mkdir app");
        fs::write(
            ws.join("apps/demo/src/main.mei"),
            r#"app(id=demo, default_scene=home)
app_add_scene(scene=scene_ref(id="home", scene_file="scenes/home.mei"))"#,
        )
        .expect("write main");
        sync_navigation_for_compile_scopes(
            ws,
            "demo",
            &[CompileScopeNav {
                scene_id: "home".to_string(),
                target_file: "src/scenes/home.mei".to_string(),
            }],
        )
        .expect("sync");
        let registry = MrgRegistryWriter::load(ws, "demo");
        let default_access = registry
            .navigation_by_key("default_access")
            .expect("default_access");
        assert_eq!(default_access.scene_id, "home");
        assert_eq!(default_access.target_file, "src/scenes/home.mei");
    }

    #[test]
    fn sync_default_layout_uses_main_mei_default_scene() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("runtime/platform/graphs/catalog")).expect("mkdir");
        fs::create_dir_all(ws.join("apps/catalog/src")).expect("mkdir app");
        fs::write(
            ws.join("apps/catalog/src/main.mei"),
            r#"app(id=catalog, default_scene=analytics-drilldown-board)
app_add_scene(scene=scene_ref(id="analytics-drilldown-board", scene_file="../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei"))
app_add_scene(scene=scene_ref(id="chart.rose", scene_file="../../stock/components/chart/echarts/previews/chart.rose.mei"))"#,
        )
        .expect("write main");
        sync_navigation_registry(
            ws,
            "catalog",
            &[
                (
                    "analytics-drilldown-board".to_string(),
                    "../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei"
                        .to_string(),
                ),
                (
                    "chart.rose".to_string(),
                    "../../stock/components/chart/echarts/previews/chart.rose.mei".to_string(),
                ),
            ],
        )
        .expect("sync");
        let registry = MrgRegistryWriter::load(ws, "catalog");
        let default_layout = registry
            .navigation_by_key("default_layout")
            .expect("default_layout");
        assert_eq!(default_layout.scene_id, "analytics-drilldown-board");
        assert_eq!(
            default_layout.target_file,
            "../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei"
        );
    }
}
