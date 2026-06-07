mod handlers;
mod resource_query;
mod types;

pub use handlers::{
    sim_step_api, world_asset_api, world_assets_api, world_context_api, world_runtime_api,
};

pub(crate) use resource_query::{
    default_resource_query_tools, query_resource_dataset, query_resource_dataset_metric,
    query_resource_get, query_resource_list, query_resource_runtime_peek,
    RESOURCE_QUERY_SCHEMA_VERSION,
};
pub(crate) use types::{
    ResourceInventoryItem, ResourceInventorySnapshot, WorldAssetListResponse, WorldContextSnapshot,
    WorldScope,
};

use std::path::Path;

use anyhow::Result;
use mei_lang_toolchain as toolchain;

use crate::AppState;

pub(crate) fn build_world_context_snapshot(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldContextSnapshot> {
    toolchain::build_world_context_snapshot(source_root, app_id, scope)
}

pub(crate) fn build_world_context_snapshot_cached(
    state: &AppState,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldContextSnapshot> {
    toolchain::build_world_context_snapshot(&state.source_root, app_id, scope)
}

pub(crate) fn query_world_assets(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    kind: Option<&str>,
    limit: Option<usize>,
) -> Result<WorldAssetListResponse> {
    toolchain::query_world_assets(source_root, app_id, scope, kind, limit)
}

pub(crate) fn query_world_asset(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
) -> Result<toolchain::WorldAssetGetResponse> {
    toolchain::query_world_asset(source_root, app_id, scope, id)
}

pub(crate) fn query_world_runtime(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    trace_limit: Option<usize>,
) -> Result<toolchain::WorldRuntimePeekResponse> {
    toolchain::query_world_runtime(source_root, app_id, scope, trace_limit)
}
