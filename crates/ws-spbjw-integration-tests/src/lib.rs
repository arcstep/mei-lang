use std::path::{Path, PathBuf};

pub use mei_lang_kernel::{
    coerce_rows_to_schema, compile_app_from_root, compile_app_from_root_with_options,
    evaluate_runtime_metric_defs, load_xlsx_table_snapshot, resolve_runtime_metric_def_key,
    CompileOptions, MetricShape, UiNodeDecl, Severity, UiTreeNode,
};

/// `ws-spbjw` workspace root (sibling repo under mei-projects).
pub fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-spbjw")
        .canonicalize()
        .expect("ws-spbjw source root")
}

pub fn zhifa_app_root() -> PathBuf {
    mei_lang_kernel::resolve_app_root(&source_root(), "zhifa")
}
