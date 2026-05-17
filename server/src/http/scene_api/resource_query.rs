use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

use super::types::{
    ResourceQueryToolSpec, WorldAssetGetResponse, WorldAssetListResponse, WorldRuntimePeekResponse,
    WorldScope,
};
use super::world::{
    query_world_asset, query_world_assets, query_world_dataset, query_world_runtime,
};

pub(crate) const RESOURCE_QUERY_SCHEMA_VERSION: &str = "resource-query-v2";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct ResourceToolScope {
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub entry_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
}

#[allow(dead_code)]
impl ResourceToolScope {
    pub fn to_world_scope(&self) -> WorldScope {
        WorldScope {
            scene_id: self.scene_id.clone(),
            entry_id: self.entry_id.clone(),
            target_file: self.target_file.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ResourceListToolInput {
    pub scope: ResourceToolScope,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ResourceGetToolInput {
    pub scope: ResourceToolScope,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ResourceRuntimePeekToolInput {
    pub scope: ResourceToolScope,
    #[serde(default)]
    pub trace_limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DatasetQueryToolInput {
    pub scope: ResourceToolScope,
    pub id: String,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub filters: BTreeMap<String, String>,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub(crate) fn default_resource_query_tools() -> Vec<ResourceQueryToolSpec> {
    vec![
        ResourceQueryToolSpec {
            id: "dataset_query".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose:
                "按 dataset 资源 id 查询有界结果（schema+filters+metric ids+sample rows）；对应 LLM 工具名 dataset_query"
                    .to_string(),
            input: "{id: string, search?: string, filters?: object, columns?: string[], limit?: number, scene_id?, entry_id?, target_file?}"
                .to_string(),
            output:
                "bounded: {dataset{schema_preview,filters,metric_ids}, sample_rows, truncation, usage_hint}; defaults: first 10 rows + first 10 columns + cell text truncation."
                    .to_string(),
        },
    ]
}

pub(crate) fn query_resource_list(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    kind: Option<&str>,
    limit: Option<usize>,
) -> Result<WorldAssetListResponse> {
    query_world_assets(source_root, app_id, scope, kind, limit)
}

pub(crate) fn query_resource_get(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
) -> Result<WorldAssetGetResponse> {
    query_world_asset(source_root, app_id, scope, id)
}

pub(crate) fn query_resource_runtime_peek(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    trace_limit: Option<usize>,
) -> Result<WorldRuntimePeekResponse> {
    query_world_runtime(source_root, app_id, scope, trace_limit)
}

pub(crate) fn query_resource_dataset(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    columns: Option<&[String]>,
    limit: Option<usize>,
) -> Result<Value> {
    query_world_dataset(
        source_root,
        app_id,
        scope,
        id,
        search,
        filters,
        columns,
        limit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_query_tool_ids_are_stable() {
        let ids = default_resource_query_tools()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["dataset_query".to_string()]);
    }
}
