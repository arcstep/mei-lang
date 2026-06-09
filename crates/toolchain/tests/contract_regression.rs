use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use mei_lang_kernel::RuntimeIntent;
use mei_lang_kernel::{set_mei_package_root, CompileOptions};
use mei_lang_toolchain::{
    build_world_context_snapshot, capability_catalog_descriptor_for_package_root,
    clear_compile_cache_for_app, compile_app_with_cache, compile_report, query_world_dataset,
    query_world_dataset_metrics, resolve_components_root, runtime_sim_step,
    RESOURCE_QUERY_SCHEMA_VERSION,
};

fn package_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("mei-lang package root");
        set_mei_package_root(root.clone());
        root
    })
    .clone()
}

fn workspaces_root() -> PathBuf {
    let _ = package_root();
    if let Ok(raw) = std::env::var("MEI_TEST_SOURCE_ROOT") {
        return PathBuf::from(raw)
            .canonicalize()
            .expect("MEI_TEST_SOURCE_ROOT");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-dev")
        .canonicalize()
        .expect("workspaces/ws-dev root")
}

fn standalone_fixture_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(build_standalone_fixture).clone()
}

fn build_standalone_fixture() -> PathBuf {
    let source = workspaces_root();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_millis();
    let fixture_root = std::env::temp_dir().join(format!(
        "mei_toolchain_standalone_fixture_{}_{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(&fixture_root).expect("create fixture root");
    copy_dir_recursive(
        source.join("examples/core/01-single-file-doc"),
        fixture_root.join("core-smoke-app"),
    );
    copy_dir_recursive(
        source.join("examples/ds/01-dataset-baseline"),
        fixture_root.join("ds-smoke-app"),
    );
    copy_dir_recursive(
        source.join(".stock/components"),
        fixture_root.join(".stock/components"),
    );
    fixture_root
}

fn copy_dir_recursive(src: PathBuf, dst: PathBuf) {
    fs::create_dir_all(&dst).expect("create destination directory");
    for entry in fs::read_dir(src).expect("read directory") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(path, target);
        } else {
            fs::copy(path, target).expect("copy file");
        }
    }
}

const DATASET_APP: &str = "examples/ds/01-dataset-baseline";
const METRIC_APP: &str = "examples/ds/04-data-table-features";
const RUNTIME_APP: &str = "examples/sim/01-fire-baseline";

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

#[test]
fn query_world_dataset_contract_shape_is_stable() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let payload = query_world_dataset(
        &root,
        DATASET_APP,
        None,
        "sales_data",
        None,
        &BTreeMap::new(),
        None,
        None,
        None,
    )
    .expect("dataset query");
    assert_eq!(payload["id"], "sales_data");
    assert!(payload["sample_rows"].is_array());
    assert!(payload["dataset"]["schema_preview"].is_array());
    assert!(payload["observation"]["exposure"]["query_schema_version"]
        .as_str()
        .is_some_and(|version| version.contains(RESOURCE_QUERY_SCHEMA_VERSION)));
    assert!(payload["perf"]["total_ms"].as_u64().is_some());
}

#[test]
fn query_world_dataset_metrics_contract_shape_is_stable() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, METRIC_APP);
    let payload = query_world_dataset_metrics(
        &root,
        METRIC_APP,
        None,
        "orders",
        &["orders_overview".to_string()],
        None,
        &BTreeMap::new(),
        None,
        &[],
    )
    .expect("metric query");
    assert_eq!(payload["dataset_id"], "orders");
    assert!(payload["metrics"].is_array());
    assert!(!payload["metrics"].as_array().unwrap().is_empty());
    assert!(payload["analysis_contracts"].is_object() || payload["analysis_contracts"].is_array());
    assert!(payload["observation"]["compile"]["compile_ms"]
        .as_u64()
        .is_some());
    assert!(payload["perf"]["metric_eval_ms"].as_u64().is_some());
}

#[test]
fn runtime_sim_step_returns_scene_view_and_html() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, RUNTIME_APP);
    let result = runtime_sim_step(
        &root,
        RUNTIME_APP,
        None,
        RuntimeIntent {
            kind: "sync".to_string(),
            target: None,
        },
    )
    .expect("runtime sim");
    assert!(!result.html.is_empty());
    assert!(!result.scene_view.scene_id.is_empty());
}

#[test]
fn standalone_source_root_core_smoke_check_works() {
    let root = standalone_fixture_root();
    clear_compile_cache_for_app(&root, "core-smoke-app");
    let report =
        compile_report(&root, "core-smoke-app", CompileOptions::default()).expect("compile report");
    assert!(!report.revision_token.is_empty());
    assert!(!report
        .compiled
        .diagnostics
        .iter()
        .any(|item| matches!(item.severity, mei_lang_kernel::Severity::Error)));
}

#[test]
fn standalone_source_root_ds_smoke_query_dataset_works() {
    let root = standalone_fixture_root();
    clear_compile_cache_for_app(&root, "ds-smoke-app");
    let payload = query_world_dataset(
        &root,
        "ds-smoke-app",
        None,
        "sales_data",
        None,
        &BTreeMap::new(),
        None,
        Some(5),
        None,
    )
    .expect("standalone dataset query");
    assert_eq!(payload["id"], "sales_data");
    assert!(payload["sample_rows"].is_array());
}

#[test]
fn capability_catalog_includes_platform_assets_and_profiles() {
    let root = package_root();
    let descriptor = capability_catalog_descriptor_for_package_root(&root);
    assert_eq!(descriptor["schema_version"], "mei-capability-catalog-v1");
    assert!(descriptor["ai_profiles"].is_array());
    assert_eq!(descriptor["ai_profiles"].as_array().unwrap().len(), 2);
    assert!(descriptor["platform_assets"]["component_packs"].is_array());
    assert!(
        !descriptor["platform_assets"]["component_packs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(descriptor["platform_assets"]["template_packs"].is_array());
    assert!(
        descriptor["platform_assets"]["template_packs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "cockpit")
    );
}

#[test]
fn world_context_snapshot_includes_world_catalog_lines() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let snapshot = build_world_context_snapshot(&root, DATASET_APP, None).expect("world snapshot");
    let lines = snapshot.prompt_catalog_lines;
    assert!(
        lines.iter().any(|line| line.contains("[World — catalog]")),
        "prompt catalog should include [World — catalog]"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("[World — query tooling]")),
        "prompt catalog should include [World — query tooling]"
    );
}
