use super::prelude::*;
use super::*;

pub fn ensure_stock_catalog_app_synced(
    source_root: &Path,
) -> Result<Option<crate::catalog_app::SyncStockCatalogAppReport>> {
    use crate::catalog_app::sync_stock_catalog_app;
    use mei_lang_kernel::catalog_app_needs_sync;

    if !catalog_app_needs_sync(source_root)? {
        return Ok(None);
    }
    Ok(Some(sync_stock_catalog_app(source_root)?))
}
pub fn doctor_workspace_stock(
    source_root: &Path,
    package_root: &Path,
) -> Result<StockDoctorReport> {
    let mut missing_trees = Vec::new();
    let mut orphan_paths = Vec::new();
    let mut manifest_drift = Vec::new();
    let mut warnings = Vec::new();

    let components_root = resolve_components_root(source_root);
    let templates_root = resolve_templates_root(source_root);
    // components + templates are required; authoring is optional (retired).
    for (label, path) in [
        ("components", &components_root),
        ("templates", &templates_root),
    ] {
        if !stock_tree_ready(path) {
            missing_trees.push(format!("{label}: {}", path.display()));
        }
    }

    let legacy_stock = source_root.join(LEGACY_STOCK_DIR);
    if legacy_stock.is_dir() {
        orphan_paths.push(legacy_stock.display().to_string());
    }
    let root_authoring = source_root.join("authoring");
    if root_authoring.is_dir() {
        orphan_paths.push(root_authoring.display().to_string());
    }
    let nested_authoring = resolve_stock_root(source_root).join("authoring/authoring");
    if nested_authoring.is_dir() {
        orphan_paths.push(nested_authoring.display().to_string());
    }

    let manifest_path = resolve_stock_root(source_root).join(STOCK_MANIFEST_FILENAME);
    if manifest_path.is_file() {
        let raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?;
        match serde_json::from_str::<StockManifest>(&raw) {
            Ok(manifest) => {
                if manifest.bootstrap_source != BOOTSTRAP_SOURCE_PLATFORM_DEFAULT {
                    manifest_drift.push(format!(
                        "bootstrapSource={} (expected {BOOTSTRAP_SOURCE_PLATFORM_DEFAULT})",
                        manifest.bootstrap_source
                    ));
                }
                let expected_package_root = package_root
                    .canonicalize()
                    .unwrap_or_else(|_| package_root.to_path_buf());
                let recorded = PathBuf::from(&manifest.package_root);
                let recorded_canonical = recorded.canonicalize().unwrap_or(recorded);
                if recorded_canonical != expected_package_root {
                    manifest_drift.push(format!(
                        "packageRoot={} (expected {})",
                        manifest.package_root,
                        expected_package_root.display()
                    ));
                }
            }
            Err(error) => warnings.push(format!(
                "failed to parse {}: {error}",
                manifest_path.display()
            )),
        }
    } else if stock_tree_ready(&components_root) || stock_tree_ready(&templates_root) {
        warnings.push(format!(
            "missing {} under {}",
            STOCK_MANIFEST_FILENAME,
            resolve_stock_root(source_root).display()
        ));
    }

    let missing_component_previews =
        mei_lang_kernel::audit_component_preview_coverage(source_root).unwrap_or_default();
    let workspace = mei_lang_kernel::load_workspace_config(source_root);
    // Pack previews are optional when authoring catalog is disabled (gold-sample workspaces).
    let preview_required = workspace.stock.catalog.authoring.enabled;
    if !preview_required && !missing_component_previews.is_empty() {
        warnings.push(format!(
            "component pack previews absent for {} keys (ok: stock.catalog.authoring.enabled=false)",
            missing_component_previews.len()
        ));
    }
    let missing_component_previews = if preview_required {
        missing_component_previews
    } else {
        Vec::new()
    };
    let mut catalog_app_drift = Vec::new();
    if preview_required {
        if mei_lang_kernel::catalog_app_needs_sync(source_root).unwrap_or(true) {
            catalog_app_drift.push(
                "apps/_stock-catalog out of sync; run `mei-toolchain workspace stock catalog-app sync`"
                    .to_string(),
            );
        }
        catalog_app_drift.extend(check_stock_catalog_menu_config(source_root));
    } else if mei_lang_kernel::catalog_app_needs_sync(source_root).unwrap_or(false) {
        warnings.push(
            "apps/_stock-catalog drift ignored (stock.catalog.authoring.enabled=false)".to_string(),
        );
    }

    let ok = missing_trees.is_empty()
        && orphan_paths.is_empty()
        && manifest_drift.is_empty()
        && missing_component_previews.is_empty()
        && catalog_app_drift.is_empty();
    Ok(StockDoctorReport {
        ok,
        missing_trees,
        orphan_paths,
        manifest_drift,
        warnings,
        missing_component_previews,
        catalog_app_drift,
    })
}

pub(crate) fn check_stock_catalog_menu_config(source_root: &Path) -> Vec<String> {
    let workspace = mei_lang_kernel::load_workspace_config(source_root);
    let groups = workspace
        .menu
        .get("groups")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let group_ids: Vec<String> = groups
        .iter()
        .filter_map(|group| {
            group
                .get("id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect();
    let items = workspace
        .menu
        .get("items")
        .and_then(|value| value.as_array());
    let mut has_stock_catalog_single = false;
    let mut has_legacy_catalog_only = false;
    if let Some(items) = items {
        for item in items {
            let app_id = item
                .get("app_id")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if app_id != "_stock-catalog" {
                continue;
            }
            let label = item
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let catalog = item
                .get("catalog")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let pack = item
                .get("pack")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if label.eq_ignore_ascii_case("stock catalog") {
                has_stock_catalog_single = true;
            }
            if !catalog.is_empty() && pack.is_empty() {
                has_legacy_catalog_only = true;
            }
        }
    }
    let discovery = mei_lang_kernel::discover_stock_catalog_packs(source_root).ok();
    let mut out = Vec::new();
    if has_stock_catalog_single {
        out.push(
            "remove legacy Stock Catalog topbar item; use 组件/模板 first-level groups with per-pack entries"
                .to_string(),
        );
    }
    if has_legacy_catalog_only {
        out.push(
            "remove catalog-only _stock-catalog menu items without pack=; packs are auto-discovered from stock/"
                .to_string(),
        );
    }
    if !group_ids.iter().any(|id| id == "components") {
        out.push("workspace menu.groups should include id=components (label 组件)".to_string());
    }
    if !group_ids.iter().any(|id| id == "templates") {
        out.push("workspace menu.groups should include id=templates (label 模板)".to_string());
    }
    if let Some(discovery) = discovery {
        if !discovery.component_packs.is_empty() && !group_ids.iter().any(|id| id == "components") {
            out.push(format!(
                "discovered {} component packs but menu group components is missing",
                discovery.component_packs.len()
            ));
        }
        if !discovery.template_packs.is_empty() && !group_ids.iter().any(|id| id == "templates") {
            out.push(format!(
                "discovered {} template packs but menu group templates is missing",
                discovery.template_packs.len()
            ));
        }
    }
    out
}
