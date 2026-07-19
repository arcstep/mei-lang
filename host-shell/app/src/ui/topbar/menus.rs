use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use mei_lang_kernel::{
    discover_stock_catalog_packs, load_workspace_config, stock_catalog_app_config, WorkspaceConfig,
};

use super::super::{TopbarMenuConfig, TopbarMenuContext};

pub const DEFAULT_BRAND_TITLE: &str = "MeiLang";
pub const DEFAULT_BRAND_LOGO_HREF: &str = "/app-assets/favicon.svg";

fn read_topbar_menu_json(path: &Path) -> Option<TopbarMenuConfig> {
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<TopbarMenuConfig>(&raw).ok()
}

fn menu_from_workspace_config(config: &WorkspaceConfig) -> Option<TopbarMenuConfig> {
    if config.menu.is_null() {
        return None;
    }
    serde_json::from_value::<TopbarMenuConfig>(config.menu.clone()).ok()
}

fn load_topbar_menu_from_dir(dir: &Path) -> Option<TopbarMenuConfig> {
    let workspace = load_workspace_config(dir);
    if let Some(menu) = menu_from_workspace_config(&workspace) {
        return Some(menu);
    }
    read_topbar_menu_json(&dir.join("_menu.json"))
}

fn workspace_display_label(source_root: &Path) -> Option<String> {
    let workspace = load_workspace_config(source_root);
    workspace
        .workspace
        .label
        .or(workspace.workspace.id)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Resolve `workspace.brand.logo` to a safe workspace-relative path under `assets/`.
pub fn resolve_workspace_brand_logo_rel(source_root: &Path, logo: &str) -> Option<String> {
    let trimmed = logo.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::RootDir))
    {
        return None;
    }
    let first = candidate.components().next().and_then(|part| match part {
        Component::Normal(value) => Some(value.to_string_lossy().to_string()),
        _ => None,
    })?;
    if first != "assets" {
        return None;
    }
    let absolute = source_root.join(&candidate);
    let Ok(canonical_root) = source_root.canonicalize() else {
        return None;
    };
    let Ok(canonical_file) = absolute.canonicalize() else {
        return None;
    };
    if !canonical_file.starts_with(&canonical_root) || !canonical_file.is_file() {
        return None;
    }
    Some(candidate.to_string_lossy().replace('\\', "/"))
}

pub fn resolve_workspace_brand(source_root: &Path) -> (String, String) {
    let workspace = load_workspace_config(source_root);
    let title = workspace
        .workspace
        .brand
        .title_trimmed()
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_BRAND_TITLE.to_string());
    let logo_href = workspace
        .workspace
        .brand
        .logo_trimmed()
        .and_then(|logo| resolve_workspace_brand_logo_rel(source_root, logo))
        .map(|rel| format!("/workspace-assets/{rel}"))
        .unwrap_or_else(|| DEFAULT_BRAND_LOGO_HREF.to_string());
    (title, logo_href)
}

/// Load workspace + segment topbar menu configuration for SSR chrome.
pub fn load_topbar_menu_context(source_root: &Path) -> TopbarMenuContext {
    let mut by_segment = BTreeMap::new();
    let workspace_label = workspace_display_label(source_root);
    let (brand_title, brand_logo_href) = resolve_workspace_brand(source_root);
    let root = load_topbar_menu_from_dir(source_root);
    let stock_packs = discover_stock_catalog_packs(source_root).ok();
    let stock_component_packs = stock_packs
        .as_ref()
        .map(|discovery| discovery.component_packs.clone())
        .unwrap_or_default();
    let stock_template_packs = stock_packs
        .as_ref()
        .map(|discovery| discovery.template_packs.clone())
        .unwrap_or_default();
    let stock_catalog_app_id = stock_packs
        .as_ref()
        .map(|discovery| discovery.catalog_app_id.clone());
    let stock_catalog_app_title = stock_packs
        .as_ref()
        .map(|_| {
            stock_catalog_app_config(source_root)
                .title
                .trim()
                .to_string()
        })
        .filter(|title| !title.is_empty());
    let Ok(entries) = fs::read_dir(source_root) else {
        return TopbarMenuContext {
            root,
            by_segment,
            workspace_label,
            brand_title: Some(brand_title),
            brand_logo_href: Some(brand_logo_href),
            stock_component_packs,
            stock_template_packs,
            stock_catalog_app_id,
            stock_catalog_app_title,
        };
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }
        if let Some(config) = load_topbar_menu_from_dir(&entry.path()) {
            by_segment.insert(name, config);
        }
    }
    TopbarMenuContext {
        root,
        by_segment,
        workspace_label,
        brand_title: Some(brand_title),
        brand_logo_href: Some(brand_logo_href),
        stock_component_packs,
        stock_template_packs,
        stock_catalog_app_id,
        stock_catalog_app_title,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn brand_logo_must_stay_under_assets() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mei-brand-logo-{stamp}"));
        let assets = dir.join("assets");
        fs::create_dir_all(&assets).expect("assets");
        fs::write(assets.join("logo.svg"), b"<svg></svg>").expect("logo");
        assert_eq!(
            resolve_workspace_brand_logo_rel(dir.as_path(), "assets/logo.svg").as_deref(),
            Some("assets/logo.svg")
        );
        assert!(resolve_workspace_brand_logo_rel(dir.as_path(), "../etc/passwd").is_none());
        assert!(resolve_workspace_brand_logo_rel(dir.as_path(), "secret/logo.svg").is_none());
        assert!(resolve_workspace_brand_logo_rel(dir.as_path(), "assets/missing.svg").is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
