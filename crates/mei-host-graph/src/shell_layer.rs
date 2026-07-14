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

/// True when topbar is the bootstrap stub (empty mei-shell-topbar header).
pub fn is_placeholder_shell_document(doc: &ShellLayerDocument) -> bool {
    let top = doc.topbar_html.trim();
    if top.is_empty() {
        return true;
    }
    top.contains("class=\"mei-shell-topbar\"") && top.len() < 240
}

pub fn store_shell_layer_document(
    app_id: &str,
    scene_id: &str,
    route_mode: &str,
    tab: &str,
    chrome: &str,
    auth_sig: Option<u64>,
    document: &ShellLayerDocument,
) {
    let cache_key = shell_cache_key(
        app_id,
        scene_id,
        route_mode,
        tab,
        chrome,
        auth_sig,
        SHELL_LAYER_SCHEMA,
    );
    let bytes = serde_json::to_vec(document).unwrap_or_default();
    let content_hash = crate::content_store::content_hash_bytes(bytes.as_slice());
    store_layer(
        cache_key,
        format!("shell.{route_mode}").as_str(),
        content_hash.as_str(),
        bytes.as_slice(),
    );
}

pub fn build_shell_layer_document(route_mode: &str, tab: &str, chrome: &str) -> ShellLayerDocument {
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
    app_id: &str,
    scene_id: &str,
    route_mode: &str,
    tab: &str,
    chrome: &str,
    auth_sig: Option<u64>,
) -> (ShellLayerDocument, bool) {
    ensure_shell_layer_rendered(app_id, scene_id, route_mode, tab, chrome, auth_sig, || {
        build_shell_layer_document(route_mode, tab, chrome)
    })
}

pub fn ensure_shell_layer_rendered<F>(
    app_id: &str,
    scene_id: &str,
    route_mode: &str,
    tab: &str,
    chrome: &str,
    auth_sig: Option<u64>,
    render: F,
) -> (ShellLayerDocument, bool)
where
    F: FnOnce() -> ShellLayerDocument,
{
    let cache_key = shell_cache_key(
        app_id,
        scene_id,
        route_mode,
        tab,
        chrome,
        auth_sig,
        SHELL_LAYER_SCHEMA,
    );
    if let Some(bytes) = take_layer(cache_key.as_str()) {
        if crate::schema_gate::layer_bytes_match_schema(bytes.as_slice(), SHELL_LAYER_SCHEMA) {
            if let Ok(doc) = serde_json::from_slice::<ShellLayerDocument>(bytes.as_slice()) {
                if crate::schema_gate::document_schema_ok(
                    doc.schema_version.as_str(),
                    SHELL_LAYER_SCHEMA,
                ) && !is_placeholder_shell_document(&doc)
                {
                    return (doc, true);
                }
            }
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

pub fn shell_layer_json(
    app_id: &str,
    scene_id: &str,
    route_mode: &str,
    tab: &str,
    chrome: &str,
) -> serde_json::Value {
    let (doc, _) = ensure_shell_layer_cached(app_id, scene_id, route_mode, tab, chrome, None);
    json!(doc)
}

#[cfg(test)]
mod gate_c_tests {
    use super::*;
    use crate::layer_store::store_layer;
    use crate::view_artifact::shell_cache_key;

    #[test]
    fn shell_cache_miss_on_wrong_schema_then_rebuilds() {
        let app_id = "gate-c-shell";
        let scene_id = "home";
        let route_mode = "app";
        let tab = "scene";
        let chrome = "full";
        let cache_key = shell_cache_key(
            app_id,
            scene_id,
            route_mode,
            tab,
            chrome,
            None,
            SHELL_LAYER_SCHEMA,
        );
        let stale = br#"{"schema_version":"shell-v0","route_mode":"app","tab":"scene","chrome":"full","topbar_html":"<header class=\"mei-shell-topbar mei-shell-rich\">stale shell content that is long enough to avoid placeholder detection path when schema matched</header>","statusbar_html":""}"#;
        store_layer(
            cache_key,
            "shell.app",
            "stale-hash",
            stale,
        );

        let rich = ShellLayerDocument {
            schema_version: SHELL_LAYER_SCHEMA.to_string(),
            route_mode: route_mode.to_string(),
            tab: tab.to_string(),
            chrome: chrome.to_string(),
            topbar_html: "<header class=\"mei-shell-topbar mei-shell-rich\">rebuilt shell content long enough to not be treated as placeholder bootstrap stub</header>".to_string(),
            statusbar_html: String::new(),
        };
        let (doc, hit) = ensure_shell_layer_rendered(
            app_id,
            scene_id,
            route_mode,
            tab,
            chrome,
            None,
            || rich.clone(),
        );
        assert!(!hit, "wrong schema must miss and rebuild");
        assert_eq!(doc.schema_version, SHELL_LAYER_SCHEMA);

        let (doc2, hit2) = ensure_shell_layer_rendered(
            app_id,
            scene_id,
            route_mode,
            tab,
            chrome,
            None,
            || panic!("must hit cache after rebuild"),
        );
        assert!(hit2);
        assert_eq!(doc2.schema_version, SHELL_LAYER_SCHEMA);
    }
}
