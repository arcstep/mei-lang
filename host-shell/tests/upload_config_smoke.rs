//! Upload + Config ops path smoke (kernel + workspace layout).

use std::collections::BTreeMap;
use std::path::PathBuf;

use mei_lang_kernel::{
    load_mei_config_for_app, resolve_app_root, OpsConfigPatch, MEI_CONFIG_FILENAME,
};
use serde_json::json;

fn ws_demo_v2_root() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspaces/ws-demo-v2");
    root.canonicalize().ok().filter(|path| path.is_dir())
}

#[test]
fn upload_rel_configured_separately_from_assets() {
    let Some(workspace) = ws_demo_v2_root() else {
        return;
    };
    let app_root = resolve_app_root(workspace.as_path(), "data-demo");
    if !app_root.is_dir() {
        return;
    }
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace.as_path()));
    let upload_rel = config.paths.upload.as_deref().unwrap_or("").trim();
    assert!(!upload_rel.is_empty(), "paths.upload should be configured");
    assert_ne!(upload_rel, "assets", "upload root should not alias assets");
}

#[test]
fn ops_patch_accepts_sources_upload_path_shape() {
    let mut sources = BTreeMap::new();
    sources.insert(
        "demo_xlsx".to_string(),
        json!({
            "kind": "xlsx",
            "path": "upload/demo.xlsx"
        }),
    );
    let patch = OpsConfigPatch {
        sources: Some(sources),
        ..Default::default()
    };
    assert!(!patch.is_empty());
    let sources = patch.sources.as_ref().expect("sources");
    let demo = sources.get("demo_xlsx").expect("demo source");
    assert_eq!(
        demo.get("path").and_then(|v| v.as_str()),
        Some("upload/demo.xlsx")
    );
}

#[test]
fn config_route_targets_mei_config_json() {
    assert_eq!(MEI_CONFIG_FILENAME, "app.config.json");
}
