use super::support::*;

#[test]
fn compile_service_reports_cache_hit_on_second_request() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let components = resolve_components_root(&root);
    let options = CompileOptions::default();
    let first = compile_app_with_cache(&root, DATASET_APP, options.clone(), components.as_path())
        .map_err(|failure| failure.error)
        .expect("first");
    let second = compile_app_with_cache(&root, DATASET_APP, options, components.as_path())
        .map_err(|failure| failure.error)
        .expect("second");
    assert!(second.cache_hit, "second compile should hit cache");
    assert_eq!(first.compile_revision, second.compile_revision);
}

#[test]
fn clear_compile_cache_for_app_invalidates_cache_hit() {
    let root = workspaces_root();
    let components = resolve_components_root(&root);
    let options = CompileOptions::default();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let _ = compile_app_with_cache(&root, DATASET_APP, options.clone(), components.as_path())
        .map_err(|failure| failure.error)
        .expect("warm");
    let cleared = clear_compile_cache_for_app(&root, DATASET_APP);
    assert!(cleared >= 1, "expected at least one cache entry cleared");
    let after_clear = compile_app_with_cache(&root, DATASET_APP, options, components.as_path())
        .map_err(|failure| failure.error)
        .expect("after clear");
    assert!(
        !after_clear.cache_hit,
        "compile after clear should miss cache"
    );
}

#[test]
fn compile_report_revision_matches_cached_outcome() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let report = compile_report(&root, DATASET_APP, CompileOptions::default()).expect("report");
    assert!(!report.revision_token.is_empty());
    let cached = compile_app_with_cache(
        &root,
        DATASET_APP,
        CompileOptions::default(),
        resolve_components_root(&root).as_path(),
    )
    .map_err(|failure| failure.error)
    .expect("cached");
    assert_eq!(report.revision_token, cached.compile_revision);
    let second =
        compile_report(&root, DATASET_APP, CompileOptions::default()).expect("second report");
    assert!(second.cache_hit);
    assert_eq!(report.revision_token, second.revision_token);
}
