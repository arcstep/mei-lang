mod bundle;
mod inventory;
mod query;
mod shrink;
mod snapshot;

pub use bundle::{default_resource_query_tools, load_world_runtime_bundle, normalize_path};
pub use query::{query_world_asset, query_world_assets, query_world_runtime};
pub use snapshot::build_world_context_snapshot;

#[cfg(test)]
mod tests {
    use super::bundle::{app_relative_mei_for_preview, normalize_path};
    use super::inventory::extract_ref_tokens_from_source;

    #[test]
    fn app_relative_preview_target_keeps_app_relative_mei() {
        assert_eq!(
            app_relative_mei_for_preview("demo", "demo/scenes/home.mei").as_deref(),
            Some("scenes/home.mei")
        );
    }

    #[test]
    fn normalize_path_strips_relative_prefix() {
        assert_eq!(normalize_path("./foo\\bar.mei"), "foo/bar.mei");
    }

    #[test]
    fn extract_ref_tokens_collects_typed_and_legacy_refs() {
        let source = r#"
scene(id="s1", world = world_ref(scene_file = "worlds/home.mei"))
panel_ref("overview")
world_file_ref(path = "legacy.mei")
"#;
        let refs = extract_ref_tokens_from_source(source);
        assert!(refs.contains(&"world_ref".to_string()));
        assert!(refs.contains(&"panel_ref".to_string()));
        assert!(refs.contains(&"world_file_ref".to_string()));
    }
}
