//! Minimal `shell.*` layer producer (tab/chrome/route chrome descriptor).

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::layer_store::{store_layer, take_layer};
use crate::view_artifact::shell_cache_key;

pub const SHELL_LAYER_SCHEMA: &str = "shell-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellLayerDocument {
    pub schema_version: String,
    pub route_mode: String,
    pub tab: String,
    pub chrome: String,
    pub topbar_html: String,
    #[serde(default)]
    pub statusbar_html: String,
}

pub fn build_shell_layer_document(
    route_mode: &str,
    tab: &str,
    chrome: &str,
) -> ShellLayerDocument {
    let topbar_html = format!(
        r#"<header class="mei-shell-topbar" data-tab="{tab}" data-chrome="{chrome}" data-route-mode="{route_mode}"></header>"#
    );
    ShellLayerDocument {
        schema_version: SHELL_LAYER_SCHEMA.to_string(),
        route_mode: route_mode.to_string(),
        tab: tab.to_string(),
        chrome: chrome.to_string(),
        topbar_html,
        statusbar_html: String::new(),
    }
}

pub fn ensure_shell_layer_cached(
    route_mode: &str,
    tab: &str,
    chrome: &str,
    auth_sig: Option<u64>,
) -> (ShellLayerDocument, bool) {
    ensure_shell_layer_rendered(route_mode, tab, chrome, auth_sig, || {
        build_shell_layer_document(route_mode, tab, chrome)
    })
}

pub fn ensure_shell_layer_rendered<F>(
    route_mode: &str,
    tab: &str,
    chrome: &str,
    auth_sig: Option<u64>,
    render: F,
) -> (ShellLayerDocument, bool)
where
    F: FnOnce() -> ShellLayerDocument,
{
    let cache_key = shell_cache_key(route_mode, tab, chrome, auth_sig, SHELL_LAYER_SCHEMA);
    if let Some(bytes) = take_layer(cache_key.as_str()) {
        if let Ok(doc) = serde_json::from_slice::<ShellLayerDocument>(bytes.as_slice()) {
            return (doc, true);
        }
    }
    let document = render();
    let bytes = serde_json::to_vec(&document).unwrap_or_default();
    let content_hash = crate::content_store::content_hash_bytes(bytes.as_slice());
    let artifact_id = format!("shell.{route_mode}");
    store_layer(
        cache_key,
        artifact_id.as_str(),
        content_hash.as_str(),
        bytes.as_slice(),
    );
    (document, false)
}

pub fn shell_layer_json(route_mode: &str, tab: &str, chrome: &str) -> serde_json::Value {
    let (doc, _) = ensure_shell_layer_cached(route_mode, tab, chrome, None);
    json!(doc)
}
