use std::{fs, path::Path};

use mei_lang_app::{TopbarMenuConfig, TopbarMenuContext};
use mei_lang_kernel::{segment_mei_config_path, MeiConfig};
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

fn menu_from_mei_config(path: &Path) -> Option<TopbarMenuConfig> {
    let config = MeiConfig::load_or_default(path);
    if config.menu.is_null() {
        return None;
    }
    match serde_json::from_value::<TopbarMenuConfig>(config.menu) {
        Ok(menu) => Some(menu),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to parse .mei-config.json menu");
            None
        }
    }
}

fn load_topbar_menu_from_dir(dir: &Path) -> Option<TopbarMenuConfig> {
    let modern = segment_mei_config_path(dir);
    if let Some(menu) = menu_from_mei_config(&modern) {
        return Some(menu);
    }
    read_topbar_menu_json(&dir.join("_menu.json"))
}

pub(crate) fn load_segment_topbar_menus(source_root: &Path) -> TopbarMenuContext {
    let mut by_segment = BTreeMap::new();
    let root = load_topbar_menu_from_dir(source_root);
    let Ok(entries) = fs::read_dir(source_root) else {
        return TopbarMenuContext { root, by_segment };
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
    TopbarMenuContext { root, by_segment }
}
