use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde_json::{json, Value};

use super::types::{MeiConfig, OpsConfig};

/// Live scene theme definition from app `ops.themes[theme_id]`.
pub fn resolve_live_ops_theme_value(config: &MeiConfig, theme_id: &str) -> Option<Value> {
    let id = theme_id.trim();
    if id.is_empty() {
        return None;
    }
    config.ops.themes.get(id).cloned()
}

/// Stable digest for compile revision: app config fields that affect compile structure/data.
/// Excludes `ops.themes` so runtime theme edits do not invalidate AOT artifacts.
pub fn mei_config_compile_revision_digest(config: &MeiConfig) -> String {
    let payload = json!({
        "schemaVersion": config.schema_version,
        "entry": config.entry,
        "paths": config.paths,
        "features": config.features,
        "ops": ops_config_for_compile_revision(&config.ops),
    });
    let canonical = serde_json::to_string(&payload).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("digest:{:016x}", hasher.finish())
}

fn ops_config_for_compile_revision(ops: &OpsConfig) -> Value {
    json!({
        "sources": ops.sources,
        "basemaps": ops.basemaps,
        "params": ops.params,
    })
}

/// Revision token for live scene theme overlay (changes when `ops.themes` changes).
pub fn ops_themes_revision_digest(config: &MeiConfig) -> String {
    let payload = json!({ "themes": config.ops.themes });
    let canonical = serde_json::to_string(&payload).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("themes:{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::types::{AppEntryConfig, AppFeaturesConfig, AppPathsConfig, OpsSourceEntry};
    use std::collections::BTreeMap;

    fn sample_config(themes: BTreeMap<String, Value>) -> MeiConfig {
        MeiConfig {
            schema_version: 1,
            entry: AppEntryConfig {
                main: "main.mei".to_string(),
            },
            paths: AppPathsConfig::default(),
            features: AppFeaturesConfig::default(),
            ops: OpsConfig {
                themes,
                sources: BTreeMap::from([(
                    "demo".to_string(),
                    OpsSourceEntry {
                        kind: "xlsx".to_string(),
                        path: "upload/demo.xlsx".to_string(),
                        sheet: None,
                        header_row: Some(1),
                        preview_rows: None,
                        page_size: None,
                        max_page_size: None,
                        table: None,
                        query: None,
                        connection: None,
                    },
                )]),
                basemaps: BTreeMap::new(),
                params: BTreeMap::new(),
                layout_tuning: None,
            },
            ..Default::default()
        }
    }

    #[test]
    fn compile_revision_digest_ignores_theme_only_changes() {
        let mut themes_a = BTreeMap::new();
        themes_a.insert(
            "cockpit".to_string(),
            json!({"font": {"2": "14px"}}),
        );
        let mut themes_b = BTreeMap::new();
        themes_b.insert(
            "cockpit".to_string(),
            json!({"font": {"2": "16px"}}),
        );
        let first = mei_config_compile_revision_digest(&sample_config(themes_a));
        let second = mei_config_compile_revision_digest(&sample_config(themes_b));
        assert_eq!(first, second);
    }

    #[test]
    fn compile_revision_digest_changes_when_source_path_changes() {
        let themes = BTreeMap::new();
        let mut first = sample_config(themes.clone());
        let mut second = sample_config(themes);
        first.ops.sources.get_mut("demo").unwrap().path = "upload/a.xlsx".to_string();
        second.ops.sources.get_mut("demo").unwrap().path = "upload/b.xlsx".to_string();
        assert_ne!(
            mei_config_compile_revision_digest(&first),
            mei_config_compile_revision_digest(&second)
        );
    }

    #[test]
    fn resolve_live_ops_theme_value_reads_ops_table() {
        let mut themes = BTreeMap::new();
        themes.insert("cockpit".to_string(), json!({"tokens": {"color": {"x": "#111"}}}));
        let config = sample_config(themes);
        let value = resolve_live_ops_theme_value(&config, "cockpit").expect("theme");
        assert_eq!(
            value
                .pointer("/tokens/color/x")
                .and_then(Value::as_str),
            Some("#111")
        );
    }
}
