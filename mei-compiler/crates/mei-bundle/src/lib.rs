//! MeiLang compile exchange bundle (`.meibundle`).

mod digest;
mod io;

pub use digest::compute_workspace_digest;
pub use io::{
    bundle_stats, read_bundle, write_bundle, write_bundle_from_outcome, write_debug_sidecar,
    BundleStats, ReadBundleError, WriteBundleError,
};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mei_graph::{CompileOutcome, GraphBlock};
use serde::{Deserialize, Serialize};

pub const BUNDLE_SCHEMA_VERSION: &str = "mei-compile-bundle-v1";
pub const GRAPH_SCHEMA_VERSION: &str = "mei-compiler-graph-v2";
pub const MANIFEST_PATH: &str = "manifest.json";
pub const BLOCKS_ZST_PATH: &str = "blocks.json.zst";
pub const BLOCKS_MEDIA_TYPE: &str = "application/json+zstd";

/// Deduped compile output for exchange (no per-file block duplication).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeiCompileExchange {
    pub app_id: String,
    pub syntax_version: String,
    pub graph_schema_version: String,
    pub blocks: Vec<GraphBlock>,
    pub sources: Vec<MeiBundleSourceIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeiBundleSourceIndex {
    pub source_file: String,
    pub block_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeiBundleManifest {
    pub bundle_schema_version: String,
    pub compiler_version: String,
    pub syntax_version: String,
    pub graph_schema_version: String,
    pub app_id: String,
    pub compiled_at_ms: u64,
    pub workspace_digest: String,
    pub block_count: usize,
    pub blocks_media_type: String,
    pub blocks_path: String,
    pub index_by_kind: BTreeMap<String, usize>,
    pub sources: Vec<MeiBundleSourceIndex>,
}

pub fn exchange_from_outcome(outcome: &CompileOutcome) -> MeiCompileExchange {
    let sources = outcome
        .files
        .iter()
        .map(|file| MeiBundleSourceIndex {
            source_file: file.source_file.clone(),
            block_ids: file.blocks.iter().map(|b| b.block_id.clone()).collect(),
        })
        .collect();
    MeiCompileExchange {
        app_id: outcome.app_id.clone(),
        syntax_version: outcome.syntax_version.clone(),
        graph_schema_version: GRAPH_SCHEMA_VERSION.to_string(),
        blocks: outcome.blocks.clone(),
        sources,
    }
}

pub fn index_by_kind(blocks: &[GraphBlock]) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    for block in blocks {
        *index.entry(block.kind.clone()).or_default() += 1;
    }
    index
}

pub fn build_manifest(
    exchange: &MeiCompileExchange,
    workspace_digest: &str,
    compiler_version: &str,
    compiled_at_ms: u64,
) -> MeiBundleManifest {
    MeiBundleManifest {
        bundle_schema_version: BUNDLE_SCHEMA_VERSION.to_string(),
        compiler_version: compiler_version.to_string(),
        syntax_version: exchange.syntax_version.clone(),
        graph_schema_version: exchange.graph_schema_version.clone(),
        app_id: exchange.app_id.clone(),
        compiled_at_ms,
        workspace_digest: workspace_digest.to_string(),
        block_count: exchange.blocks.len(),
        blocks_media_type: BLOCKS_MEDIA_TYPE.to_string(),
        blocks_path: BLOCKS_ZST_PATH.to_string(),
        index_by_kind: index_by_kind(&exchange.blocks),
        sources: exchange.sources.clone(),
    }
}

pub fn default_bundle_path(workspace: &Path, app_id: &str) -> std::path::PathBuf {
    let app_root = resolve_v2_app_root(workspace, app_id);
    let bundle_name = format!("{app_id}.meibundle");
    let primary = bundle_output_path(workspace, app_id);
    if primary.is_file() {
        return primary;
    }
    let active = app_root.join("build/active/exchange").join(&bundle_name);
    if active.is_file() {
        return active;
    }
    if let Some(fallback) = find_latest_env_bundle(app_root.as_path(), bundle_name.as_str()) {
        return fallback;
    }
    primary
}

/// Compiler output must always target `env/current`, even before that
/// generation has its first bundle. Read paths may fall back to an older
/// bundle, but write paths must never cross generation boundaries.
pub fn bundle_output_path(workspace: &Path, app_id: &str) -> std::path::PathBuf {
    let app_root = resolve_v2_app_root(workspace, app_id);
    resolve_v2_app_build_root(app_root.as_path())
        .join("exchange")
        .join(format!("{app_id}.meibundle"))
}

fn find_latest_env_bundle(app_root: &Path, bundle_name: &str) -> Option<PathBuf> {
    let env_root = app_root.join("env");
    let entries = std::fs::read_dir(env_root).ok()?;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let file_name = path.file_name()?.to_string_lossy();
        if !file_name.starts_with("WS-") {
            continue;
        }
        let candidate = path.join("build/exchange").join(bundle_name);
        if candidate.is_file() {
            candidates.push(candidate);
        }
    }
    candidates.sort();
    candidates.pop()
}

fn resolve_v2_app_root(workspace: &Path, app_id: &str) -> PathBuf {
    workspace.join("apps").join(app_id.trim())
}

fn resolve_v2_app_build_root(app_root: &Path) -> PathBuf {
    let current = app_root.join("env/current");
    if current.is_symlink() {
        if let Ok(target) = std::fs::read_link(&current) {
            let env_dir = if target.is_absolute() {
                target
            } else if let Some(parent) = current.parent() {
                parent.join(target)
            } else {
                target
            };
            let build = env_dir.join("build");
            if build.is_dir() {
                return build;
            }
        }
    }
    panic!(
        "missing env/current for app {} (run build prepare first)",
        app_root.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_bundle_path_uses_env_current_build_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        let app_root = workspace.join("apps/demo");
        let env_dir = app_root.join("env/WS-20260228.0");
        fs::create_dir_all(env_dir.join("build/exchange")).expect("mkdirs");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("WS-20260228.0", app_root.join("env/current"))
                .expect("symlink current");
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir("WS-20260228.0", app_root.join("env/current"))
                .expect("symlink current");
        }

        let path = default_bundle_path(workspace, "demo");
        assert_eq!(path, env_dir.join("build/exchange/demo.meibundle"));
    }

    #[test]
    fn default_bundle_path_falls_back_to_latest_env_generation() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        let app_root = workspace.join("apps/demo");
        let older = app_root.join("env/WS-20260228.0");
        let newer = app_root.join("env/WS-20260301.0");
        fs::create_dir_all(older.join("build/exchange")).expect("older exchange");
        fs::create_dir_all(newer.join("build")).expect("newer build");
        fs::write(older.join("build/exchange/demo.meibundle"), b"bundle").expect("older bundle");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("WS-20260301.0", app_root.join("env/current"))
                .expect("symlink current");
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir("WS-20260301.0", app_root.join("env/current"))
                .expect("symlink current");
        }

        let path = default_bundle_path(workspace, "demo");
        assert_eq!(path, older.join("build/exchange/demo.meibundle"));
    }

    #[test]
    fn bundle_output_path_targets_empty_current_generation() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        let app_root = workspace.join("apps/demo");
        let older = app_root.join("env/WS-20260228.0");
        let current = app_root.join("env/WS-20260301.0");
        fs::create_dir_all(older.join("build/exchange")).expect("older exchange");
        fs::create_dir_all(current.join("build")).expect("current build");
        fs::write(older.join("build/exchange/demo.meibundle"), b"bundle").expect("older bundle");
        #[cfg(unix)]
        std::os::unix::fs::symlink("WS-20260301.0", app_root.join("env/current"))
            .expect("symlink current");
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir("WS-20260301.0", app_root.join("env/current"))
            .expect("symlink current");

        assert_eq!(
            bundle_output_path(workspace, "demo"),
            current.join("build/exchange/demo.meibundle")
        );
        assert_eq!(
            default_bundle_path(workspace, "demo"),
            older.join("build/exchange/demo.meibundle")
        );
    }
}
