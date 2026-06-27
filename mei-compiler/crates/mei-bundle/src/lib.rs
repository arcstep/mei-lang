//! MeiLang compile exchange bundle (`.meibundle`).

mod digest;
mod io;

pub use digest::compute_workspace_digest;
pub use io::{
    bundle_stats, read_bundle, write_bundle, write_bundle_from_outcome, BundleStats,
    ReadBundleError, WriteBundleError,
};

use std::collections::BTreeMap;
use std::path::Path;

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
    workspace
        .join("apps")
        .join(app_id)
        .join(".mei")
        .join("compile")
        .join(format!("{app_id}.meibundle"))
}
