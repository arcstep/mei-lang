use std::path::Path;

use super::io::load_workspace_config;
use super::types::{
    WorkspaceStockCatalogAppConfig, WorkspaceStockCatalogKindConfig, DEFAULT_STOCK_CATALOG_APP_ID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockCatalogKind {
    Components,
    Templates,
    Authoring,
}

pub fn stock_catalog_kind_config(source_root: &Path, kind: StockCatalogKind) -> WorkspaceStockCatalogKindConfig {
    let cfg = load_workspace_config(source_root);
    match kind {
        StockCatalogKind::Components => cfg.stock.catalog.components.clone(),
        StockCatalogKind::Templates => cfg.stock.catalog.templates.clone(),
        StockCatalogKind::Authoring => cfg.stock.catalog.authoring.clone(),
    }
}

pub fn stock_catalog_enabled(source_root: &Path, kind: StockCatalogKind) -> bool {
    stock_catalog_kind_config(source_root, kind).enabled
}

pub fn stock_catalog_app_config(source_root: &Path) -> WorkspaceStockCatalogAppConfig {
    load_workspace_config(source_root).stock.catalog_app
}

pub fn stock_catalog_app_id(source_root: &Path) -> String {
    let id = stock_catalog_app_config(source_root).id;
    let trimmed = id.trim();
    if trimmed.is_empty() {
        DEFAULT_STOCK_CATALOG_APP_ID.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn is_stock_catalog_app(app_id: &str) -> bool {
    app_id.trim() == DEFAULT_STOCK_CATALOG_APP_ID
        || app_id.trim().starts_with("_stock-catalog")
}

pub fn is_stock_catalog_app_for_root(source_root: &Path, app_id: &str) -> bool {
    app_id.trim() == stock_catalog_app_id(source_root).as_str()
}

pub fn stock_path_excluded(source_root: &Path, kind: StockCatalogKind, rel_path: &str) -> bool {
    let config = stock_catalog_kind_config(source_root, kind);
    if !config.enabled {
        return true;
    }
    let normalized = normalize_stock_relative_path(rel_path);
    config
        .exclude
        .iter()
        .any(|pattern| glob_matches(pattern.trim(), normalized.as_str()))
}

/// Normalize legacy `.stock/` prefixes to `stock/` for comparisons.
pub fn normalize_stock_relative_path(rel_path: &str) -> String {
    let mut normalized = rel_path.trim().replace('\\', "/");
    while normalized.starts_with("./") {
        normalized = normalized.trim_start_matches("./").to_string();
    }
    if normalized.starts_with(".stock/") {
        normalized = format!("stock/{}", normalized.trim_start_matches(".stock/"));
    }
    normalized
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if pattern == "**" {
        return true;
    }
    let regex = glob_pattern_to_regex(pattern);
    regex.is_match(path)
}

fn glob_pattern_to_regex(pattern: &str) -> regex::Regex {
    let mut out = String::from("^");
    let bytes = pattern.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'*' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
            out.push_str(".*");
            index += 2;
            if index < bytes.len() && bytes[index] == b'/' {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'*' {
            out.push_str("[^/]*");
            index += 1;
            continue;
        }
        if bytes[index] == b'?' {
            out.push_str("[^/]");
            index += 1;
            continue;
        }
        let ch = bytes[index] as char;
        if ".^$+{}[]|()\\".contains(ch) {
            out.push('\\');
        }
        out.push(ch);
        index += 1;
    }
    out.push('$');
    regex::Regex::new(&out).unwrap_or_else(|_| regex::Regex::new("$^").expect("empty regex"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_dot_stock_prefix() {
        assert_eq!(
            normalize_stock_relative_path(".stock/templates/cockpit/main.mei"),
            "stock/templates/cockpit/main.mei"
        );
    }

    #[test]
    fn glob_exclude_assets_directory() {
        assert!(glob_matches("**/assets/**", "cockpit/assets/foo.mei"));
        assert!(!glob_matches("**/assets/**", "cockpit/main.mei"));
    }
}
