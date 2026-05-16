use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::types::{
    ResourceQueryToolSpec, WorldAssetGetResponse, WorldAssetListResponse, WorldRuntimePeekResponse,
    WorldScope,
};
use super::world::{query_world_asset, query_world_assets, query_world_runtime};

pub(crate) const RESOURCE_QUERY_SCHEMA_VERSION: &str = "resource-query-v1";

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

pub(crate) fn default_resource_query_tools() -> Vec<ResourceQueryToolSpec> {
    vec![
        ResourceQueryToolSpec {
            id: "resource.list".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "按 scope 与类型查看资源清单（entity/resource/cell）".to_string(),
            input: "{scope: {scene_id, entry_id, target_file}, kind?: entity|resource|cell, limit?: number}"
                .to_string(),
            output:
                "{items: [{id, kind, label_or_title, tags?}], total}; endpoint: GET /api/world/assets/*app_id?scene_id=..."
                    .to_string(),
        },
        ResourceQueryToolSpec {
            id: "resource.get".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "按资源 id 查看单个对象详情".to_string(),
            input: "{scope: {scene_id, entry_id, target_file}, id: string}".to_string(),
            output:
                "{id, kind, fields, relations?}; endpoint: GET /api/world/asset/*app_id?id=...&scene_id=..."
                    .to_string(),
        },
        ResourceQueryToolSpec {
            id: "resource.runtime.peek".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "查看运行态关键信息（phase/result/actions/trace）".to_string(),
            input: "{scope: {scene_id, entry_id, target_file}, trace_limit?: number}".to_string(),
            output:
                "{phase, result, available_actions, recent_trace_messages}; endpoint: GET /api/world/runtime/*app_id?scene_id=..."
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_query_tool_ids_are_stable() {
        let ids = default_resource_query_tools()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "resource.list".to_string(),
                "resource.get".to_string(),
                "resource.runtime.peek".to_string()
            ]
        );
    }
}
