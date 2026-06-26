use std::{fs, path::Path};

use mei_lang_app::{TopbarMenuConfig, TopbarMenuContext};
use mei_lang_kernel::{
    discover_stock_catalog_packs, load_workspace_config, stock_catalog_app_config, WorkspaceConfig,
};
use std::collections::BTreeMap;

fn read_topbar_menu_json(path: &Path) -> Option<TopbarMenuConfig> {
    if !path.is_file() {
        return None;
    }
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read topbar menu file");
            return None;
        }
    };
    match serde_json::from_str::<TopbarMenuConfig>(&raw) {
        Ok(config) => Some(config),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to parse topbar menu json");
            None
        }
    }
}

fn menu_from_workspace_config(config: &WorkspaceConfig) -> Option<TopbarMenuConfig> {
    if config.menu.is_null() {
        return None;
    }
    match serde_json::from_value::<TopbarMenuConfig>(config.menu.clone()) {
        Ok(menu) => Some(menu),
        Err(error) => {
            tracing::warn!(%error, "failed to parse workspace config menu");
            None
        }
    }
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

pub(crate) fn load_segment_topbar_menus(source_root: &Path) -> TopbarMenuContext {
    let mut by_segment = BTreeMap::new();
    let workspace_label = workspace_display_label(source_root);
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
    let stock_catalog_app_id = stock_packs.as_ref().map(|discovery| discovery.catalog_app_id.clone());
    let stock_catalog_app_title = stock_packs.as_ref().map(|_| {
        stock_catalog_app_config(source_root)
            .title
            .trim()
            .to_string()
    }).filter(|title| !title.is_empty());
    let Ok(entries) = fs::read_dir(source_root) else {
        return TopbarMenuContext {
            root,
            by_segment,
            workspace_label,
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
        stock_component_packs,
        stock_template_packs,
        stock_catalog_app_id,
        stock_catalog_app_title,
    }
}
