use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde_json::{json, Value};

use super::types::{MeiConfig, OpsConfig, WorkspaceConfig};

/// Live scene theme definition from app `ops.themes[theme_id]` (legacy / layout host).
pub fn resolve_live_ops_theme_value(config: &MeiConfig, theme_id: &str) -> Option<Value> {
    let id = theme_id.trim();
    if id.is_empty() {
        return None;
    }
    config.ops.themes.get(id).cloned()
}

/// Workspace scene color library entry.
pub fn resolve_workspace_scene_theme_value(
    workspace: &WorkspaceConfig,
    theme_id: &str,
) -> Option<Value> {
    let id = theme_id.trim();
    if id.is_empty() {
        return None;
    }
    workspace.ops.scene_themes.get(id).cloned()
}

/// Catalog entries for Admin theme select (`id` + display `label`).
pub fn list_workspace_scene_theme_catalog(workspace: &WorkspaceConfig) -> Vec<Value> {
    workspace
        .ops
        .scene_themes
        .iter()
        .map(|(id, theme)| {
            let label = theme
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id.as_str());
            json!({ "id": id, "label": label, "value": id })
        })
        .collect()
}

fn theme_library_keys(
    workspace: Option<&WorkspaceConfig>,
    app: &MeiConfig,
) -> BTreeMap<String, ()> {
    let mut keys = BTreeMap::new();
    if let Some(workspace) = workspace {
        for id in workspace.ops.scene_themes.keys() {
            keys.insert(id.clone(), ());
        }
    }
    if keys.is_empty() {
        for id in app.ops.themes.keys() {
            keys.insert(id.clone(), ());
        }
    }
    keys
}

fn library_contains(workspace: Option<&WorkspaceConfig>, app: &MeiConfig, id: &str) -> bool {
    if let Some(workspace) = workspace {
        if !workspace.ops.scene_themes.is_empty() {
            return workspace.ops.scene_themes.contains_key(id);
        }
    }
    app.ops.themes.contains_key(id)
}

/// Active Scene Theme Profile id.
///
/// Priority:
/// 1. `ops.theme_selection.active` (must exist in workspace sceneThemes when library present)
/// 2. scene `theme_ref` / compiled scene.theme
/// 3. workspace `ops.sceneThemeDefault` / `cockpit` / first library id
pub fn resolve_active_scene_theme_id(
    workspace: Option<&WorkspaceConfig>,
    app: &MeiConfig,
    scene_theme: Option<&str>,
) -> String {
    if let Some(id) = app
        .ops
        .extensions
        .get("theme_selection")
        .and_then(|value| value.get("active"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if library_contains(workspace, app, id) {
            return id.to_string();
        }
    }
    if let Some(id) = scene_theme.map(str::trim).filter(|id| !id.is_empty()) {
        if library_contains(workspace, app, id) {
            return id.to_string();
        }
    }
    if let Some(default) = workspace
        .and_then(|ws| ws.ops.scene_theme_default.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if library_contains(workspace, app, default) {
            return default.to_string();
        }
    }
    let keys = theme_library_keys(workspace, app);
    if keys.contains_key("cockpit") {
        return "cockpit".to_string();
    }
    keys.keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "cockpit".to_string())
}

/// Backward-compatible wrapper (app-local themes only).
pub fn resolve_active_scene_theme_id_app_only(
    config: &MeiConfig,
    scene_theme: Option<&str>,
) -> String {
    resolve_active_scene_theme_id(None, config, scene_theme)
}

fn deep_merge_value(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            let mut out = base_map.clone();
            for (key, value) in overlay_map {
                let merged = match out.get(key) {
                    Some(existing) => deep_merge_value(existing, value),
                    None => value.clone(),
                };
                out.insert(key.clone(), merged);
            }
            Value::Object(out)
        }
        (_, overlay) => overlay.clone(),
    }
}

fn app_font_scale(app: &MeiConfig) -> Option<&Value> {
    app.ops.extensions.get("font_scale")
}

fn app_layout_value(app: &MeiConfig) -> Option<Value> {
    if let Some(layout) = app.ops.extensions.get("layout") {
        if !layout.is_null() {
            return Some(layout.clone());
        }
    }
    if let Some(layout) = app
        .ops
        .themes
        .get("_layout")
        .and_then(|theme| theme.get("layout").cloned())
    {
        return Some(layout);
    }
    app.ops
        .themes
        .values()
        .find_map(|theme| theme.get("layout").cloned())
}

/// Assemble the effective scene theme:
/// workspace sceneThemes[id] (colors) ⊕ app layout ⊕ app font_scale.
/// Falls back to app.ops.themes[id] when workspace library is empty.
pub fn resolve_assembled_scene_theme(
    workspace: Option<&WorkspaceConfig>,
    app: &MeiConfig,
    theme_id: &str,
) -> Option<Value> {
    let id = theme_id.trim();
    if id.is_empty() {
        return None;
    }
    let mut theme = if let Some(workspace) = workspace {
        if !workspace.ops.scene_themes.is_empty() {
            resolve_workspace_scene_theme_value(workspace, id)
                .or_else(|| resolve_live_ops_theme_value(app, id))
        } else {
            resolve_live_ops_theme_value(app, id)
        }
    } else {
        resolve_live_ops_theme_value(app, id)
    }?;

    if let Some(layout) = app_layout_value(app) {
        theme = deep_merge_value(&theme, &json!({ "layout": layout }));
    }
    // Legacy: app theme may still carry role maps / frame chrome; merge non-color leftovers lightly.
    if let Some(app_theme) = resolve_live_ops_theme_value(app, id) {
        if workspace
            .map(|ws| !ws.ops.scene_themes.is_empty())
            .unwrap_or(false)
        {
            // Prefer workspace colors; keep app-only keys such as metric_* role maps if absent.
            theme = deep_merge_value(&app_theme, &theme);
            if let Some(layout) = app_layout_value(app) {
                theme = deep_merge_value(&theme, &json!({ "layout": layout }));
            }
        }
    }
    if let Some(scale) = app_font_scale(app) {
        theme = deep_merge_value(&theme, &json!({ "font": scale }));
    }
    Some(theme)
}

/// Theme tokens revision: workspace library + app selection + font_scale + layout.
pub fn ops_active_theme_revision_digest(
    workspace: Option<&WorkspaceConfig>,
    app: &MeiConfig,
) -> String {
    let active = resolve_active_scene_theme_id(workspace, app, None);
    let payload = json!({
        "workspace_scene_themes": workspace.map(|ws| &ws.ops.scene_themes),
        "workspace_default": workspace.and_then(|ws| ws.ops.scene_theme_default.as_deref()),
        "app_themes": app.ops.themes,
        "theme_selection": app.ops.extensions.get("theme_selection"),
        "font_scale": app.ops.extensions.get("font_scale"),
        "layout": app.ops.extensions.get("layout"),
        "active": active,
    });
    let canonical = serde_json::to_string(&payload).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("themes:{:016x}", hasher.finish())
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

/// Revision token for live scene theme overlay (app-local themes table).
pub fn ops_themes_revision_digest(config: &MeiConfig) -> String {
    let payload = json!({ "themes": config.ops.themes });
    let canonical = serde_json::to_string(&payload).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("themes:{:016x}", hasher.finish())
}

/// Helper for callers that still pass only app config.
pub fn ops_active_theme_revision_digest_app_only(config: &MeiConfig) -> String {
    ops_active_theme_revision_digest(None, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::types::{
        AppEntryConfig, AppFeaturesConfig, AppPathsConfig, OpsSourceEntry, WorkspaceOpsConfig,
        WorkspaceProfile,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn sample_app(themes: BTreeMap<String, Value>) -> MeiConfig {
        let sources = BTreeMap::from([(
            "demo".to_string(),
            OpsSourceEntry {
                kind: "xlsx".to_string(),
                path: "upload/demo.xlsx".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
            },
        )]);
        MeiConfig {
            schema_version: 1,
            entry: AppEntryConfig {
                main: "main.mei".to_string(),
            },
            paths: AppPathsConfig::default(),
            features: AppFeaturesConfig::default(),
            ops: OpsConfig {
                sources,
                themes,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn sample_workspace(scene_themes: BTreeMap<String, Value>) -> WorkspaceConfig {
        WorkspaceConfig {
            schema_version: 2,
            workspace: WorkspaceProfile {
                id: Some("demo".to_string()),
                ..Default::default()
            },
            ops: WorkspaceOpsConfig {
                scene_themes,
                scene_theme_default: Some("cockpit".to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn compile_revision_digest_ignores_theme_only_changes() {
        let mut themes_a = BTreeMap::new();
        themes_a.insert("cockpit".to_string(), json!({"font": {"2": "14px"}}));
        let mut themes_b = BTreeMap::new();
        themes_b.insert("cockpit".to_string(), json!({"font": {"2": "16px"}}));
        let first = mei_config_compile_revision_digest(&sample_app(themes_a));
        let second = mei_config_compile_revision_digest(&sample_app(themes_b));
        assert_eq!(first, second);
    }

    #[test]
    fn resolve_active_prefers_selection_in_workspace_library() {
        let mut scene_themes = BTreeMap::new();
        scene_themes.insert(
            "tech-bright".to_string(),
            json!({"label": "亮色科技蓝", "tokens": {"color": {"surface_bg": "#111"}}}),
        );
        scene_themes.insert("cockpit".to_string(), json!({"label": "经典"}));
        let workspace = sample_workspace(scene_themes);
        let mut app = sample_app(BTreeMap::new());
        app.ops.extensions.insert(
            "theme_selection".to_string(),
            json!({ "active": "tech-bright" }),
        );
        assert_eq!(
            resolve_active_scene_theme_id(Some(&workspace), &app, Some("cockpit")),
            "tech-bright"
        );
    }

    #[test]
    fn assembled_theme_applies_font_scale_and_layout() {
        let mut scene_themes = BTreeMap::new();
        scene_themes.insert(
            "cockpit".to_string(),
            json!({
                "tokens": {"color": {"surface_bg": "rgb(1,2,3)"}},
                "metric_label": {"font": "7", "color": "text_muted"}
            }),
        );
        let workspace = sample_workspace(scene_themes);
        let mut app = sample_app(BTreeMap::new());
        app.ops
            .extensions
            .insert("font_scale".to_string(), json!({"7": "48px", "1": "16px"}));
        app.ops.extensions.insert(
            "layout".to_string(),
            json!({"home/T1": {"headerHeight": "72px"}}),
        );
        let theme =
            resolve_assembled_scene_theme(Some(&workspace), &app, "cockpit").expect("theme");
        assert_eq!(
            theme
                .pointer("/tokens/color/surface_bg")
                .and_then(Value::as_str),
            Some("rgb(1,2,3)")
        );
        assert_eq!(
            theme.pointer("/font/7").and_then(Value::as_str),
            Some("48px")
        );
        assert_eq!(
            theme
                .get("layout")
                .and_then(|layout| layout.get("home/T1"))
                .and_then(|row| row.get("headerHeight"))
                .and_then(Value::as_str),
            Some("72px")
        );
    }

    #[test]
    fn catalog_exposes_labels() {
        let mut scene_themes = BTreeMap::new();
        scene_themes.insert("tech-bright".to_string(), json!({"label": "亮色科技蓝"}));
        let workspace = sample_workspace(scene_themes);
        let catalog = list_workspace_scene_theme_catalog(&workspace);
        assert_eq!(catalog.len(), 1);
        assert_eq!(
            catalog[0].get("label").and_then(Value::as_str),
            Some("亮色科技蓝")
        );
    }
}
