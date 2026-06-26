pub use std::collections::BTreeMap;
pub use std::fs;
pub use std::path::PathBuf;
pub use std::process::Command;
pub use std::sync::OnceLock;
pub use std::time::{SystemTime, UNIX_EPOCH};

pub use mei_lang_kernel::RuntimeIntent;
pub use mei_lang_kernel::{set_mei_package_root, CompileOptions};
pub use mei_lang_toolchain::{
    build_world_context_snapshot, capability_catalog_descriptor_for_package_root,
    capability_catalog_descriptor_for_workspace_root, clear_compile_cache_for_app,
    compile_app_with_cache, compile_report, create_app_skeleton,
    doctor_editor_runtime_for_package_root, doctor_editor_runtime_for_workspace_root,
    editor_runtime_descriptor_for_package_root, export_knowledge_bundle_for_package_root,
    export_knowledge_bundle_for_workspace_root, init_workspace_profile,
    install_editor_runtime_support_files, query_world_dataset, query_world_dataset_metrics,
    resolve_components_root, runtime_sim_step, scaffold_editor_runtime_tooling,
    workspace_runtime_status_for_workspace_root, RESOURCE_QUERY_SCHEMA_VERSION,
};

pub fn package_root() -> PathBuf {
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

pub fn workspaces_root() -> PathBuf {
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

pub fn standalone_fixture_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(build_standalone_fixture).clone()
}

pub fn build_standalone_fixture() -> PathBuf {
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
    fs::write(
        fixture_root.join("workspace.json"),
        r#"{"schemaVersion":2,"paths":{"apps":"apps","components":"stock/components"}}"#,
    )
    .expect("write workspace.json");
    copy_dir_recursive(
        source.join("apps/examples-core-01-single-file-doc"),
        fixture_root.join("apps/core-smoke-app"),
    );
    copy_dir_recursive(
        source.join("apps/examples-ds-01-dataset-baseline"),
        fixture_root.join("apps/ds-smoke-app"),
    );
    copy_dir_recursive(
        source.join("stock/components"),
        fixture_root.join("stock/components"),
    );
    fixture_root
}

pub fn copy_dir_recursive(src: PathBuf, dst: PathBuf) {
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

pub const DATASET_APP: &str = "examples-ds-01-dataset-baseline";
pub const METRIC_APP: &str = "examples-ds-04-data-table-features";
pub const RUNTIME_APP: &str = "examples-sim-01-fire-baseline";

