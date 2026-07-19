//! `theme.tokens` and `layout.overlay` artifact helpers.

use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::load_mei_config_for_app;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::content_store::{put_if_absent, LAYOUT_OVERLAY_KIND, THEME_TOKENS_KIND};
use crate::layer_store::{store_layer, take_layer};
use crate::types::PayloadRef;
use crate::view_artifact::{
    layout_overlay_persisted_cache_key, layout_overlay_session_cache_key, theme_tokens_cache_key,
};

pub const THEME_TOKENS_SCHEMA: &str = "theme-tokens-v1";
pub const LAYOUT_OVERLAY_SCHEMA: &str = "layout-overlay-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ThemeTokensDocument {
    pub schema_version: String,
    pub colors: Value,
    pub fonts: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LayoutOverlayDocument {
    pub schema_version: String,
    pub patches: Value,
}

pub fn default_theme_tokens() -> ThemeTokensDocument {
    ThemeTokensDocument {
        schema_version: THEME_TOKENS_SCHEMA.to_string(),
        colors: json!({}),
        fonts: json!({}),
    }
}

pub fn theme_tokens_document_from_theme(theme: &Value) -> ThemeTokensDocument {
    let mut colors = Map::new();
    let mut fonts = Map::new();
    if let Some(font_map) = theme.get("font").and_then(Value::as_object) {
        for (key, value) in font_map {
            if let Some(raw) = value.as_str() {
                fonts.insert(key.clone(), Value::String(raw.to_string()));
            }
        }
    }
    for role in ["label", "value", "unit", "desc"] {
        push_metric_role_font_var(theme, &format!("metric_{role}"), &format!("metric-{role}"), &mut colors);
    }
    for role in ["label", "value", "unit"] {
        push_metric_role_font_var(
            theme,
            &format!("metric_sub_{role}"),
            &format!("metric-sub-{role}"),
            &mut colors,
        );
    }
    if let Some(token_colors) = theme
        .get("tokens")
        .and_then(|v| v.get("color"))
        .and_then(Value::as_object)
    {
        for (key, value) in token_colors {
            if let Some(raw) = value.as_str() {
                let css_key = format!("color-{}", key.replace('_', "-"));
                colors.insert(css_key, Value::String(raw.to_string()));
            }
        }
    }
    if let Some(gradients) = theme
        .get("tokens")
        .and_then(|v| v.get("gradient"))
        .and_then(Value::as_object)
    {
        for (key, value) in gradients {
            if let Some(raw) = value.as_str() {
                let css_key = format!("gradient-{}", key.replace('_', "-"));
                colors.insert(css_key, Value::String(raw.to_string()));
            }
        }
    }
    ThemeTokensDocument {
        schema_version: THEME_TOKENS_SCHEMA.to_string(),
        colors: Value::Object(colors),
        fonts: Value::Object(fonts),
    }
}

fn push_metric_role_font_var(
    theme: &Value,
    theme_key: &str,
    css_prefix: &str,
    colors: &mut Map<String, Value>,
) {
    let Some(entry) = theme.get(theme_key) else {
        return;
    };
    let Some(raw) = entry.get("font").and_then(Value::as_str).map(str::trim) else {
        return;
    };
    if raw.is_empty() {
        return;
    }
    let resolved = if raw.chars().all(|c| c.is_ascii_digit()) {
        format!("var(--mei-font-{raw})")
    } else if raw.starts_with("var(") || raw.ends_with("px") || raw.ends_with("rem") {
        raw.to_string()
    } else {
        format!("var(--mei-font-{raw})")
    };
    colors.insert(
        format!("{css_prefix}-font-size"),
        Value::String(resolved),
    );
}

pub fn layout_overlay_from_draft(draft: Option<&Value>) -> LayoutOverlayDocument {
    LayoutOverlayDocument {
        schema_version: LAYOUT_OVERLAY_SCHEMA.to_string(),
        patches: draft.cloned().unwrap_or(Value::Null),
    }
}

pub fn persist_theme_tokens(app_root: &Path, document: &ThemeTokensDocument) -> Result<PayloadRef> {
    let bytes = serde_json::to_vec(document)?;
    let put = put_if_absent(app_root, THEME_TOKENS_KIND, &bytes)?;
    Ok(PayloadRef::new(
        THEME_TOKENS_KIND,
        put.content_hash,
        THEME_TOKENS_SCHEMA,
    ))
}

pub fn persist_layout_overlay(
    app_root: &Path,
    document: &LayoutOverlayDocument,
) -> Result<PayloadRef> {
    let bytes = serde_json::to_vec(document)?;
    let put = put_if_absent(app_root, LAYOUT_OVERLAY_KIND, &bytes)?;
    Ok(PayloadRef::new(
        LAYOUT_OVERLAY_KIND,
        put.content_hash,
        LAYOUT_OVERLAY_SCHEMA,
    ))
}

pub fn ensure_theme_tokens_cached(
    workspace_root: &Path,
    app_id: &str,
    theme_digest: &str,
) -> Result<(ThemeTokensDocument, bool)> {
    let cache_key = theme_tokens_cache_key(theme_digest);
    if let Some(bytes) = take_layer(cache_key.as_str()) {
        let doc: ThemeTokensDocument = serde_json::from_slice(bytes.as_slice())?;
        return Ok((doc, true));
    }
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, app_id);
    let mei_config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    let theme = mei_config
        .ops
        .themes
        .get("cockpit")
        .or_else(|| mei_config.ops.themes.values().next())
        .cloned()
        .unwrap_or(Value::Null);
    let document = if theme.is_object() {
        theme_tokens_document_from_theme(&theme)
    } else {
        default_theme_tokens()
    };
    let pref = persist_theme_tokens(app_root.as_path(), &document)?;
    let bytes = serde_json::to_vec(&document)?;
    store_layer(
        cache_key,
        THEME_TOKENS_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    Ok((document, false))
}

pub fn ensure_layout_overlay_cached(
    workspace_root: &Path,
    app_id: &str,
    layout_policy_revision: &str,
    draft_session: Option<&str>,
    draft_digest: Option<&str>,
    draft: Option<&Value>,
) -> Result<(LayoutOverlayDocument, bool)> {
    let cache_key = if let (Some(session), Some(digest)) = (draft_session, draft_digest) {
        if !digest.is_empty() {
            layout_overlay_session_cache_key(app_id, session, digest)
        } else {
            layout_overlay_persisted_cache_key(layout_policy_revision)
        }
    } else {
        layout_overlay_persisted_cache_key(layout_policy_revision)
    };
    if let Some(bytes) = take_layer(cache_key.as_str()) {
        let doc: LayoutOverlayDocument = serde_json::from_slice(bytes.as_slice())?;
        return Ok((doc, true));
    }
    let document = layout_overlay_from_draft(draft);
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, app_id);
    let pref = persist_layout_overlay(app_root.as_path(), &document)?;
    let bytes = serde_json::to_vec(&document)?;
    store_layer(
        cache_key,
        LAYOUT_OVERLAY_KIND,
        pref.content_hash.as_str(),
        bytes.as_slice(),
    );
    Ok((document, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_overlay_key_differs_from_persisted() {
        let persisted = layout_overlay_persisted_cache_key("layout0");
        let session = layout_overlay_session_cache_key("demo", "sess-1", "digest-a");
        assert_ne!(persisted, session);
    }

    #[test]
    fn theme_tokens_include_metric_role_font_vars() {
        let theme = json!({
            "font": { "3": "26px", "6": "40px", "7": "48px" },
            "metric_value": { "font": "6" },
            "metric_label": { "font": "7" },
            "metric_unit": { "font": "1" },
            "metric_sub_value": { "font": "7" },
        });
        let doc = theme_tokens_document_from_theme(&theme);
        let colors = doc.colors.as_object().expect("colors object");
        assert_eq!(
            colors.get("metric-value-font-size").and_then(|v| v.as_str()),
            Some("var(--mei-font-6)")
        );
        assert_eq!(
            colors.get("metric-label-font-size").and_then(|v| v.as_str()),
            Some("var(--mei-font-7)")
        );
        assert_eq!(
            colors.get("metric-sub-value-font-size").and_then(|v| v.as_str()),
            Some("var(--mei-font-7)")
        );
        assert_eq!(
            doc.fonts
                .as_object()
                .and_then(|m| m.get("6"))
                .and_then(|v| v.as_str()),
            Some("40px")
        );
    }
}
