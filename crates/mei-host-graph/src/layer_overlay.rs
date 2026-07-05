//! `theme.tokens` and `layout.overlay` artifact helpers.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

pub fn persist_layout_overlay(app_root: &Path, document: &LayoutOverlayDocument) -> Result<PayloadRef> {
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
    let document = default_theme_tokens();
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, app_id);
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

    #[test]
    fn session_overlay_key_differs_from_persisted() {
        let persisted = layout_overlay_persisted_cache_key("layout0");
        let session = layout_overlay_session_cache_key("demo", "sess-1", "digest-a");
        assert_ne!(persisted, session);
    }
}
