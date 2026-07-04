//! Regression tests for layered view artifacts.

use mei_host_graph::{
    build_semantic_cache_core, eval_slot_group_cache_key, structure_full_cache_key,
    ui_role_depth_rank, ComposeRequest, SceneViewManifest, STRUCTURE_FULL_SCHEMA,
};

#[test]
fn structure_full_schema_is_frozen() {
    assert_eq!(STRUCTURE_FULL_SCHEMA, "structure-full-v1");
    assert_eq!(ui_role_depth_rank("region"), 1);
    assert_eq!(ui_role_depth_rank("content"), 3);
}

#[test]
fn projection_depth_does_not_change_structure_key() {
    let core = build_semantic_cache_core("demo", "home", None, "r", "c", "g", "e");
    let key = structure_full_cache_key(&core, "layout0");
    assert!(!key.contains("plane_region"));
    assert!(!key.contains("review_projection"));
}

#[test]
fn data_mode_only_affects_eval_layer_key() {
    let core = build_semantic_cache_core("demo", "home", None, "r", "c", "g", "e");
    let eval = eval_slot_group_cache_key(&core, "scope:panel:left", "eval", "default");
    let fixture = eval_slot_group_cache_key(&core, "scope:panel:left", "fixture", "default");
    assert_ne!(eval, fixture);
}

#[test]
fn manifest_compose_defaults_are_view_only() {
    let core = build_semantic_cache_core("demo", "home", None, "r", "c", "g", "e");
    let manifest = SceneViewManifest {
        schema_version: "scene-view-manifest-v1".to_string(),
        app_id: "demo".to_string(),
        scene_id: "home".to_string(),
        semantic_core: core,
        revision_digest: "digest".to_string(),
        layers: std::collections::BTreeMap::new(),
        compose_defaults: Some(ComposeRequest {
            review_projection: Some("plane_region".to_string()),
            ..Default::default()
        }),
    };
    let serialized = serde_json::to_string(&manifest).expect("serialize");
    assert!(serialized.contains("plane_region"));
    assert!(!serialized.contains("structure_full"));
}
