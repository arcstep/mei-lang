//! Stock catalog app route collection for generated `_stock-catalog` main.mei.

use std::fs;
use std::path::{Path, PathBuf};

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::compile::preview_target_relative_to_app;
use crate::mei_config::{
    is_stock_catalog_app, is_stock_catalog_app_for_root, resolve_app_root, resolve_templates_root,
    stock_catalog_app_config, stock_path_excluded, StockCatalogKind,
};
use crate::model::{BuildNodeId, CompiledApp, CompiledSceneRoute};
use crate::workspace::load_component_assets;

/// Parse `scene_ref(...)` routes from a catalog app's generated `main.mei` without full compile.
pub fn catalog_scene_routes_from_app_root(app_root: &Path) -> Vec<CompiledSceneRoute> {
    let main_path = crate::mei_config::resolve_app_main_path(app_root);
    let content = fs::read_to_string(&main_path).unwrap_or_default();
    parse_scene_routes_from_main_mei(content.as_str())
}

fn parse_scene_routes_from_main_mei(content: &str) -> Vec<CompiledSceneRoute> {
    let pattern = regex::Regex::new(
        r#"scene_ref\s*\(\s*id\s*=\s*"([^"]+)"\s*,\s*scene_file\s*=\s*"([^"]+)""#,
    )
    .expect("scene_ref route regex");
    let mut routes = Vec::new();
    for capture in pattern.captures_iter(content) {
        let scene_id = capture.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let target_file = capture.get(2).map(|m| m.as_str()).unwrap_or("").trim();
        if scene_id.is_empty() || target_file.is_empty() {
            continue;
        }
        routes.push(CompiledSceneRoute {
            scene_id: scene_id.to_string(),
            frame_id: None,
            target_file: target_file.to_string(),
            kind: "file_ref".to_string(),
            title: None,
            is_default: routes.is_empty(),
            access_export: true,
        });
    }
    routes
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StockCatalogPackDiscovery {
    pub catalog_app_id: String,
    pub component_packs: Vec<String>,
    pub template_packs: Vec<String>,
}

/// Discover component pack paths and template top-level folders for stock catalog navigation.
pub fn discover_stock_catalog_packs(source_root: &Path) -> Result<StockCatalogPackDiscovery> {
    let cfg = stock_catalog_app_config(source_root);
    let mut component_packs = std::collections::BTreeSet::new();
    for asset in load_component_assets(source_root)?.values() {
        let pack = asset.pack_path.trim();
        if pack.is_empty() || pack == "vendor" {
            continue;
        }
        component_packs.insert(pack.to_string());
    }
    let mut template_packs = std::collections::BTreeSet::new();
    let templates_root = resolve_templates_root(source_root);
    if templates_root.is_dir() {
        for entry in WalkDir::new(&templates_root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let file_name = entry.file_name().to_string_lossy();
            if !file_name.ends_with(".mei") {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&templates_root)
                .ok()
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| file_name.to_string());
            if stock_path_excluded(source_root, StockCatalogKind::Templates, rel.as_str()) {
                continue;
            }
            if rel.starts_with("assets/") || rel.contains("/assets/") {
                continue;
            }
            let top = rel.split('/').next().unwrap_or("").trim();
            if !top.is_empty() {
                template_packs.insert(top.to_string());
            }
        }
    }
    Ok(StockCatalogPackDiscovery {
        catalog_app_id: cfg.id,
        component_packs: component_packs.into_iter().collect(),
        template_packs: template_packs.into_iter().collect(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StockCatalogRouteEntry {
    pub route_id: String,
    pub target_rel: String,
    pub kind: StockCatalogRouteKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StockCatalogRouteKind {
    Component,
    Template,
}

pub fn stock_catalog_app_root(source_root: &Path) -> PathBuf {
    resolve_app_root(
        source_root,
        stock_catalog_app_config(source_root).id.as_str(),
    )
}

pub fn collect_stock_catalog_routes(source_root: &Path) -> Result<Vec<StockCatalogRouteEntry>> {
    let catalog_root = stock_catalog_app_root(source_root);
    let stub = catalog_app_stub_compiled(catalog_root.as_path());
    let mut routes = Vec::new();

    let assets = load_component_assets(source_root)?;
    for asset in assets.values() {
        let Some(workspace_preview) = asset
            .preview_mei
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(target_rel) = preview_target_relative_to_app(&stub, workspace_preview) else {
            continue;
        };
        if !mei_file_contains_scene(catalog_root.as_path(), target_rel.as_str())? {
            continue;
        }
        routes.push(StockCatalogRouteEntry {
            route_id: asset.key.clone(),
            target_rel,
            kind: StockCatalogRouteKind::Component,
        });
    }

    let templates_root = resolve_templates_root(source_root);
    if templates_root.is_dir() {
        for entry in WalkDir::new(&templates_root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let file_name = entry.file_name().to_string_lossy();
            if !file_name.ends_with(".mei") {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&templates_root)
                .ok()
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| file_name.to_string());
            if stock_path_excluded(source_root, StockCatalogKind::Templates, rel.as_str()) {
                continue;
            }
            if rel.starts_with("assets/") || rel.contains("/assets/") {
                continue;
            }
            let templates_prefix = templates_root
                .strip_prefix(source_root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| "stock/templates".to_string());
            let workspace_path = format!("{templates_prefix}/{rel}");
            let Some(target_rel) = preview_target_relative_to_app(&stub, workspace_path.as_str())
            else {
                continue;
            };
            if !mei_file_contains_scene(catalog_root.as_path(), target_rel.as_str())? {
                continue;
            }
            let route_id = Path::new(rel.as_str())
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(rel.as_str())
                .to_string();
            routes.push(StockCatalogRouteEntry {
                route_id,
                target_rel,
                kind: StockCatalogRouteKind::Template,
            });
        }
    }

    routes.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    routes.dedup_by(|left, right| left.route_id == right.route_id);
    Ok(routes)
}

fn catalog_app_stub_compiled(app_root: &Path) -> CompiledApp {
    let source_root = crate::mei_config::resolve_workspace_source_root_from_app_root(app_root);
    let app_id = stock_catalog_app_config(source_root.as_path()).id;
    CompiledApp {
        app_id: app_id.clone(),
        title: stock_catalog_app_config(source_root.as_path()).title,
        app_root: app_root.to_string_lossy().to_string(),
        scene_routes: Vec::new(),
        active_scene: None,
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
        active_target_file: String::new(),
        file_tree: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: BTreeMap::new(),
        scene_bindings_by_id: BTreeMap::new(),
        scene_examples_by_id: BTreeMap::new(),
        scene_projection_assembly_by_id: BTreeMap::new(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    }
}

fn mei_file_contains_scene(app_root: &Path, rel_path: &str) -> Result<bool> {
    let abs = resolve_mei_abs(app_root, rel_path);
    let content = match fs::read_to_string(&abs) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    Ok(content.trim().contains("scene("))
}

fn resolve_mei_abs(app_root: &Path, rel_path: &str) -> PathBuf {
    if rel_path.starts_with("../") {
        let mut base = app_root.to_path_buf();
        for part in rel_path.split('/') {
            if part == ".." {
                let _ = base.pop();
            } else if !part.is_empty() && part != "." {
                base.push(part);
            }
        }
        base
    } else {
        app_root.join(rel_path)
    }
}

pub fn render_stock_catalog_main_mei(
    source_root: &Path,
    routes: &[StockCatalogRouteEntry],
) -> Result<String> {
    let cfg = stock_catalog_app_config(source_root);
    let app_id = cfg.id.trim();
    let title = cfg.title.trim();
    let default_stage = routes
        .first()
        .map(|route| route.route_id.as_str())
        .unwrap_or("home");
    let mut out = String::from(
        "# GENERATED — do not edit; run `mei-toolchain workspace stock catalog-app sync`\n\n",
    );
    out.push_str(&format!(
        "app(\n    id = \"{app_id}\",\n    title = \"{title}\",\n    default_stage = \"{default_stage}\",\n)\n\n"
    ));
    for route in routes {
        out.push_str(&format!(
            "app_add_scene(\n    scene = scene_ref(id = \"{}\", scene_file = \"{}\"),\n)\n\n",
            route.route_id, route.target_rel
        ));
    }
    Ok(out)
}

pub fn catalog_app_needs_sync(source_root: &Path) -> Result<bool> {
    let catalog_root = stock_catalog_app_root(source_root);
    let main_path = crate::mei_config::resolve_app_main_path(catalog_root.as_path());
    if !main_path.is_file() {
        return Ok(true);
    }
    let routes = collect_stock_catalog_routes(source_root)?;
    let expected = render_stock_catalog_main_mei(source_root, routes.as_slice())?;
    let current = fs::read_to_string(&main_path)
        .with_context(|| format!("read catalog main {}", main_path.display()))?;
    Ok(current != expected)
}

pub fn is_catalog_build_app(source_root: &Path, app_id: &str) -> bool {
    is_stock_catalog_app_for_root(source_root, app_id)
}

pub fn catalog_scene_route_for_build_node<'a>(
    compiled: &'a CompiledApp,
    node: &BuildNodeId,
) -> Option<&'a CompiledSceneRoute> {
    if !is_stock_catalog_app(compiled.app_id.as_str()) {
        return None;
    }
    let key = node.key.trim();
    if key.is_empty() {
        return None;
    }
    compiled
        .scene_routes
        .iter()
        .find(|route| catalog_route_matches_node_key(route, key))
}

fn catalog_route_matches_node_key(route: &CompiledSceneRoute, key: &str) -> bool {
    if route.scene_id.trim() == key {
        return true;
    }
    let stem = Path::new(route.target_file.as_str())
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if stem == key {
        return true;
    }
    let normalized = route.target_file.replace('\\', "/");
    normalized.ends_with(key)
        || normalized.ends_with(&format!("/{key}"))
        || (key.ends_with(".mei") && normalized.ends_with(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_scene_routes_from_generated_main_mei() {
        let content = r#"
app_add_scene(
    scene = scene_ref(id = "analytics-drilldown-board", scene_file = "../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei"),
)
app_add_scene(
    scene = scene_ref(id = "chart.area", scene_file = "../../stock/components/chart/echarts/previews/chart.area.mei"),
)
"#;
        let routes = parse_scene_routes_from_main_mei(content);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].scene_id, "analytics-drilldown-board");
        assert_eq!(
            routes[0].target_file,
            "../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei"
        );
    }

    #[test]
    fn catalog_scene_route_resolves_preview_target_for_scene_node() {
        use crate::compile::catalog_preview_target_for_build_node;

        let Some(raw) = std::env::var("MEI_TEST_WORKSPACE").ok() else {
            eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
            return;
        };
        let source_root = PathBuf::from(raw.trim());
        if source_root.as_os_str().is_empty() || !source_root.is_dir() {
            eprintln!("skip: MEI_TEST_WORKSPACE is not a directory");
            return;
        }
        let app_root = source_root.join("apps/_stock-catalog");
        if !app_root.is_dir() {
            eprintln!("skip: apps/_stock-catalog missing under MEI_TEST_WORKSPACE");
            return;
        }
        let node = BuildNodeId::scene("analytics-drilldown-board");
        let target = catalog_preview_target_for_build_node(app_root.as_path(), &node)
            .expect("preview target");
        assert_eq!(
            target,
            "../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei"
        );
    }

    #[test]
    fn render_main_mei_includes_app_add_scene_blocks() {
        let routes = vec![StockCatalogRouteEntry {
            route_id: "chart.pie".to_string(),
            target_rel: "../../stock/components/chart/echarts/previews/chart.pie.mei".to_string(),
            kind: StockCatalogRouteKind::Component,
        }];
        let root = std::env::temp_dir().join(format!(
            "mei_catalog_render_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("mkdir");
        let cfg_path = root.join("workspace.json");
        fs::write(
            &cfg_path,
            r#"{"schemaVersion":2,"stock":{"catalogApp":{"id":"_stock-catalog","title":"Stock Catalog","buildOnly":true}}}"#,
        )
        .expect("write workspace.json");
        let rendered =
            render_stock_catalog_main_mei(root.as_path(), routes.as_slice()).expect("render");
        assert!(rendered.contains("app_add_scene("));
        assert!(rendered.contains("id = \"chart.pie\""));
        assert!(rendered.contains("chart.pie.mei"));
        let _ = fs::remove_dir_all(root);
    }
}
