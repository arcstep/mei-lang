use mei_lang_kernel::{decode_theme_ref_token, resolve_workspace_shell_theme, CompiledApp, SceneContract, WorkspaceConfig};
use serde_json::Value;

use super::{deep_merge_value, resolve_shared_refs};
use super::parse_builtin::builtin_theme;
use super::parse_tokens::{
    collect_scene_css_vars, collect_shell_css_vars, scene_css_vars_style, shell_css_vars_style,
    theme_decl_value,
};

#[derive(Debug, Clone)]
pub(crate) struct ThemeResolved {
    pub(crate) id: String,
    pub(crate) frame: Value,
    pub(crate) panel: Value,
    pub(crate) panel_bare: Value,
    pub(crate) panel_head: Value,
    pub(crate) panel_body: Value,
    /// 兼容：`theme.heading` 已合并进 `panel_head`（保留字段供调试/后续消费）。
    #[allow(dead_code)]
    pub(crate) heading: Value,
    /// scene/profile/theme 合并后的只读共享参数。
    pub(crate) shared: Value,
    /// 合并后的 `theme.components`，供宿主组件通过 `_mei.components` 读取。
    pub(crate) components: Value,
    pub(crate) css_vars: Vec<(String, String)>,
}

pub(crate) fn resolve_theme(scene_contract: &SceneContract) -> ThemeResolved {
    let mut theme_id = scene_contract
        .scene
        .theme
        .as_deref()
        .and_then(decode_theme_ref_token)
        .or_else(|| scene_contract.scene.theme.clone())
        .or_else(|| scene_contract.scene.profile.clone())
        .unwrap_or_else(|| "page".to_string());
    let mut theme = builtin_theme(theme_id.as_str());
    if theme.is_none() {
        theme_id = "page".to_string();
        theme = builtin_theme("page");
    }
    let mut theme = theme.unwrap_or_else(|| serde_json::json!({}));
    if let Some(custom) = scene_contract
        .themes
        .iter()
        .find(|item| item.id == theme_id)
        .or_else(|| scene_contract.themes.first())
    {
        theme = deep_merge_value(&theme, &theme_decl_value(custom));
        if theme_id != custom.id {
            theme_id = custom.id.clone();
        }
    }
    let mut shared = theme_field(&theme, "shared");
    if !scene_contract.shared.is_null() {
        shared = deep_merge_value(&shared, &scene_contract.shared);
    }
    let frame = resolve_shared_refs(&theme_field(&theme, "frame"), &shared);
    let panel = resolve_shared_refs(&theme_field(&theme, "panel"), &shared);
    let panel_bare = resolve_shared_refs(&theme_field(&theme, "panel_bare"), &shared);
    let panel_head = resolve_shared_refs(&merge_panel_head_theme(&theme), &shared);
    let panel_body = resolve_shared_refs(&theme_field(&theme, "panel_body"), &shared);
    let heading = resolve_shared_refs(&theme_field(&theme, "heading"), &shared);
    let css_vars = collect_scene_css_vars(&theme);
    let components = resolve_shared_refs(
        &theme
            .as_object()
            .and_then(|map| map.get("components"))
            .cloned()
            .filter(|value| !value.is_null())
            .unwrap_or_else(|| serde_json::json!({})),
        &shared,
    );
    ThemeResolved {
        id: theme_id,
        frame,
        panel,
        panel_bare,
        panel_head,
        panel_body,
        heading,
        shared,
        components,
        css_vars,
    }
}

/// Builtin page shell CSS variables (fallback when workspace shell theme is unset).
pub fn default_shell_body_theme_style() -> String {
    let theme = builtin_theme("page").unwrap_or_else(|| serde_json::json!({}));
    let vars = collect_shell_css_vars(&theme);
    shell_css_vars_style("page", &vars)
}

/// Inject shell-track CSS variables on `<body>` from workspace config.
pub fn shell_body_theme_style(workspace: &WorkspaceConfig) -> String {
    let mut theme_id = workspace
        .ops
        .shell_theme
        .clone()
        .unwrap_or_else(|| "page".to_string());
    let mut theme = builtin_theme("page").unwrap_or_else(|| serde_json::json!({}));
    if let Some(custom) = resolve_workspace_shell_theme(workspace) {
        theme = deep_merge_value(&theme, &custom);
        if let Some(id) = workspace.ops.shell_theme.as_deref() {
            theme_id = id.to_string();
        }
    }
    let vars = collect_shell_css_vars(&theme);
    shell_css_vars_style(theme_id.as_str(), &vars)
}

/// Shell vars on `<body>` plus scene vars for body-mounted cockpit/access overlays.
pub fn page_body_theme_style(workspace: &WorkspaceConfig, compiled: Option<&CompiledApp>) -> String {
    let mut style = shell_body_theme_style(workspace);
    if let Some(compiled) = compiled {
        style.push_str(&scene_viewport_theme_style(compiled));
    }
    style
}

/// Scene viewport theme CSS variables.
pub fn scene_viewport_theme_style(compiled: &CompiledApp) -> String {
    if let Some(contract) = compiled.scene_contract.as_ref() {
        return scene_css_vars_style(&resolve_theme(contract));
    }
    scene_css_vars_style(&resolve_builtin_only("page"))
}

fn resolve_builtin_only(theme_id: &str) -> ThemeResolved {
    let mut theme_id = theme_id.to_string();
    let mut theme = builtin_theme(theme_id.as_str());
    if theme.is_none() {
        theme_id = "page".to_string();
        theme = builtin_theme("page");
    }
    let theme = theme.unwrap_or_else(|| serde_json::json!({}));
    let shared = theme_field(&theme, "shared");
    let css_vars = collect_scene_css_vars(&theme);
    let components = theme
        .as_object()
        .and_then(|map| map.get("components"))
        .cloned()
        .filter(|value| !value.is_null())
        .unwrap_or_else(|| serde_json::json!({}));
    ThemeResolved {
        id: theme_id,
        frame: resolve_shared_refs(&theme_field(&theme, "frame"), &shared),
        panel: resolve_shared_refs(&theme_field(&theme, "panel"), &shared),
        panel_bare: resolve_shared_refs(&theme_field(&theme, "panel_bare"), &shared),
        panel_head: resolve_shared_refs(&merge_panel_head_theme(&theme), &shared),
        panel_body: resolve_shared_refs(&theme_field(&theme, "panel_body"), &shared),
        heading: resolve_shared_refs(&theme_field(&theme, "heading"), &shared),
        shared: shared.clone(),
        components: resolve_shared_refs(&components, &shared),
        css_vars,
    }
}

use super::parse_builtin::{merge_panel_head_theme, theme_field};

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::ThemeDecl;
    use serde_json::json;

    #[test]
    fn theme_decl_value_preserves_metric_role_fonts() {
        let decl = ThemeDecl {
            kind: "theme".to_string(),
            id: "cockpit".to_string(),
            frame: json!({}),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            font: json!({"4": "20px"}),
            metric_label: json!({"font": "2"}),
            metric_value: json!({"font": "4"}),
            metric_unit: json!({"font": "1"}),
            metric_desc: json!({}),
            metric_sub_label: json!({"font_size": "12px"}),
            metric_sub_value: json!({"font_size": "18px", "text_align": "right"}),
            metric_sub_unit: json!({}),
            chart_title: json!({"font": "3"}),
            chart_label: json!({}),
            table_head: json!({}),
            table_body: json!({}),
            filter_panel: json!({}),
            tokens: json!({}),
            shared: json!({}),
            components: json!({}),
        };
        let merged = theme_decl_value(&decl);
        let vars = collect_scene_css_vars(&merged);
        assert!(vars
            .iter()
            .any(|(k, v)| k == "--mei-chart-title-font-size" && v.contains("--mei-font-3")));
        assert!(vars
            .iter()
            .any(|(k, v)| k == "--mei-metric-value-font-size" && v.contains("--mei-font-4")));
        assert!(vars
            .iter()
            .any(|(k, v)| k == "--mei-metric-sub-value-font-size" && v == "18px"));
        assert!(vars
            .iter()
            .any(|(k, v)| k == "--mei-metric-sub-value-text-align" && v == "right"));
    }

    #[test]
    fn collect_theme_css_vars_prefers_font_over_font_size() {
        let merged = json!({
            "font": {"4": "18px"},
            "metric_value": {
                "font": "4",
                "font_size": "28px",
            },
        });
        let vars = collect_scene_css_vars(&merged);
        let value_sizes: Vec<_> = vars
            .iter()
            .filter(|(k, _)| k == "--mei-metric-value-font-size")
            .collect();
        assert_eq!(value_sizes.len(), 1);
        assert!(value_sizes[0].1.contains("--mei-font-4"));
        assert!(!value_sizes[0].1.contains("28px"));
    }

    #[test]
    fn collect_shell_css_vars_uses_shell_prefix_only() {
        let theme = json!({
            "font": {"2": "13px"},
            "tokens": {
                "shell": {"bg": "#111", "text": "#eee", "family_ui": "sans-serif"},
                "color": {"text_primary": "#abc", "panel_bg": "rgba(0,0,0,.5)"}
            }
        });
        let vars = collect_shell_css_vars(&theme);
        assert!(vars.iter().any(|(k, _)| k == "--mei-shell-font-2"));
        assert!(vars.iter().any(|(k, _)| k == "--mei-shell-bg"));
        assert!(vars.iter().any(|(k, _)| k == "--mei-shell-color-text-primary"));
        assert!(!vars.iter().any(|(k, _)| k.starts_with("--mei-color-")));
        assert!(!vars.iter().any(|(k, _)| k.starts_with("--mei-font-")));
    }

    #[test]
    fn collect_scene_css_vars_skips_shell_partition() {
        let theme = json!({
            "font": {"2": "14px"},
            "tokens": {
                "shell": {"bg": "#111"},
                "color": {"text_primary": "#def"}
            }
        });
        let vars = collect_scene_css_vars(&theme);
        assert!(vars.iter().any(|(k, _)| k == "--mei-color-text-primary"));
        assert!(!vars.iter().any(|(k, _)| k.contains("shell")));
    }

    #[test]
    fn shell_body_and_scene_viewport_styles_use_separate_var_tracks() {
        use std::path::Path;

        use mei_lang_kernel::{compile_app_from_root_with_options, load_workspace_config, CompileOptions};

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../workspaces/ws-hello")
            .canonicalize()
            .expect("ws-hello");
        let app_root = source_root.join("hello");
        let workspace = load_workspace_config(&source_root);
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions::default(),
        )
        .expect("compile ws-hello");
        let shell_style = shell_body_theme_style(&workspace);
        assert!(
            shell_style.contains("--mei-shell-color-"),
            "body shell style should inject shell color vars"
        );
        assert!(
            !shell_style.contains("--mei-color-surface"),
            "shell-only style must not inject scene surface vars"
        );
        let page_style = page_body_theme_style(&workspace, Some(&compiled));
        assert!(
            page_style.contains("--mei-color-"),
            "page body should also inject scene vars for body-mounted overlays"
        );
        let scene_style = scene_viewport_theme_style(&compiled);
        assert!(
            scene_style.contains("--mei-color-"),
            "viewport should inject scene color vars"
        );
        assert!(
            !scene_style.contains("--mei-shell-color-"),
            "viewport must not inject shell vars: {scene_style}"
        );
    }
}
