use std::path::PathBuf;

pub use mei_lang_kernel::{
    coerce_rows_to_schema, compile_app_from_root, compile_app_from_root_with_options,
    evaluate_runtime_metric_defs, load_xlsx_table_snapshot, resolve_runtime_metric_def_key,
    CompileOptions, MetricShape, Severity, UiNodeDecl, UiTreeNode,
};

/// Optional private workspace via `MEI_TEST_WORKSPACE` only (never sibling default).
pub fn source_root() -> Option<PathBuf> {
    let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
    let path = PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() || !path.is_dir() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

pub fn zhifa_app_root() -> Option<PathBuf> {
    Some(mei_lang_kernel::resolve_app_root(&source_root()?, "zhifa"))
}
