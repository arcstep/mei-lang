//! Shared test helpers for in-repo conformance fixtures.
//!
//! - [`conformance_workspace`]: always points at a materialized copy of
//!   `tests/fixtures/ws-conformance` (must exist; CI fails if missing).
//! - [`optional_external_workspace`]: only `MEI_TEST_WORKSPACE`; never defaults
//!   to a monorepo sibling path.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use mei_bundle::{compute_workspace_digest, write_bundle_from_outcome};
use mei_graph::compile_app;
use mei_host_core::HostContext;
use mei_host_graph::{clear_assemble_cache_for_app, import_bundle, ImportOptions};
use mei_lang_kernel::prepare_dev_build_generation;
use mei_lang_toolchain::materialize_workspace_stock;

static CONFORMANCE_WS: OnceLock<PathBuf> = OnceLock::new();
static IMPORTED_APPS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Walk up from `start` until `tests/fixtures/ws-conformance/workspace.json` is found.
pub fn mei_lang_root_from(start: &Path) -> PathBuf {
    let mut cur = start.to_path_buf();
    for _ in 0..10 {
        let marker = cur.join("tests/fixtures/ws-conformance/workspace.json");
        if marker.is_file() {
            return cur;
        }
        if !cur.pop() {
            break;
        }
    }
    panic!(
        "mei-lang root with tests/fixtures/ws-conformance not found from {}",
        start.display()
    );
}

pub fn mei_lang_root() -> PathBuf {
    if let Ok(raw) = std::env::var("MEI_PACKAGE_ROOT") {
        let p = PathBuf::from(raw);
        if p.join("tests/fixtures/ws-conformance/workspace.json")
            .is_file()
        {
            return p;
        }
    }
    mei_lang_root_from(Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn fixture_source_root() -> PathBuf {
    let root = mei_lang_root().join("tests/fixtures/ws-conformance");
    assert!(
        root.join("workspace.json").is_file(),
        "missing conformance fixture at {}",
        root.display()
    );
    root
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap_or_else(|e| {
        panic!("create {}: {e}", dst.display());
    });
    for entry in fs::read_dir(src).unwrap_or_else(|e| panic!("read {}: {e}", src.display())) {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap_or_else(|e| {
                panic!("copy {} -> {}: {e}", path.display(), target.display());
            });
        }
    }
}

/// Materialized conformance workspace (apps + platform stock). Cached per process.
pub fn conformance_workspace() -> PathBuf {
    CONFORMANCE_WS
        .get_or_init(|| {
            let package = mei_lang_root();
            let src = fixture_source_root();
            let dest = std::env::temp_dir().join(format!(
                "mei-ws-conformance-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            ));
            if dest.exists() {
                let _ = fs::remove_dir_all(&dest);
            }
            copy_dir_recursive(&src, &dest);
            materialize_workspace_stock(&dest, &package, true).unwrap_or_else(|e| {
                panic!("materialize stock into {}: {e:#}", dest.display());
            });
            dest
        })
        .clone()
}

/// Optional private workspace for business probes. Never defaults to sibling paths.
pub fn optional_external_workspace() -> Option<PathBuf> {
    let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
    let path = PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() {
        return None;
    }
    if !path.is_dir() {
        eprintln!(
            "skip: MEI_TEST_WORKSPACE is not a directory: {}",
            path.display()
        );
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

/// Compile app from conformance workspace, write temp bundle, import into registry.
pub fn ensure_imported(app_id: &str) -> PathBuf {
    let workspace = conformance_workspace();
    let mut guard = IMPORTED_APPS.lock().expect("import lock");
    let imported = guard.get_or_insert_with(HashSet::new);
    if imported.contains(app_id) {
        return workspace;
    }
    prepare_dev_build_generation(workspace.as_path(), &[app_id.to_string()]).unwrap_or_else(|e| {
        panic!("prepare_dev_build_generation {app_id}: {e:#}");
    });
    let outcome = compile_app(workspace.as_path(), app_id).unwrap_or_else(|e| {
        panic!("compile {app_id} in conformance fixture: {e:#}");
    });
    let digest = compute_workspace_digest(workspace.as_path(), app_id, "stock/templates");
    let temp_dir = std::env::temp_dir().join("mei-conformance-bundles");
    fs::create_dir_all(&temp_dir).expect("bundle temp dir");
    let bundle_path = temp_dir.join(format!("{app_id}.meibundle"));
    write_bundle_from_outcome(
        &outcome,
        digest.as_str(),
        env!("CARGO_PKG_VERSION"),
        bundle_path.as_path(),
        false,
    )
    .unwrap_or_else(|e| panic!("write bundle {app_id}: {e:#}"));
    let ctx = HostContext::new(workspace.clone(), app_id);
    import_bundle(
        &ctx,
        &ImportOptions {
            bundle_path: Some(bundle_path),
        },
    )
    .unwrap_or_else(|e| panic!("import {app_id}: {e:#}"));
    clear_assemble_cache_for_app(app_id);
    imported.insert(app_id.to_string());
    workspace
}

pub const APP_STRUCTURE: &str = "fx-structure";
pub const APP_DATA: &str = "fx-data";
pub const APP_ADMIN_MEI: &str = "fx-admin-mei";
pub const APP_DECK_MINIMAL: &str = "fx-deck-minimal";
pub const APP_DUAL_STAGE: &str = "fx-dual-stage";
pub const APP_NARRATION_JOURNEY: &str = "fx-narration-journey";
pub const APP_PAGE_REPORT: &str = "fx-page-report";

pub const APP_DIAG_LINK_TARGET: &str = "fx-diag-link-target";
pub const APP_DIAG_LINK_PARAM: &str = "fx-diag-link-param";
pub const APP_DIAG_GRID_TRACK: &str = "fx-diag-grid-track";
pub const APP_DIAG_UNKNOWN_COMPONENT: &str = "fx-diag-unknown-component";
pub const APP_DIAG_WARMUP_FOCUS: &str = "fx-diag-warmup-focus";
pub const APP_DIAG_FILTER_KEY: &str = "fx-diag-filter-key";
pub const APP_DIAG_DEFAULT_FILTERS: &str = "fx-diag-default-filters";
