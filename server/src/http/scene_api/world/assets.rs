use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use super::bundle::load_world_runtime_bundle;
use super::json_shrink::{json_serialized_len, shrink_json_for_llm, LLM_RESOURCE_GET_BUDGET_CHARS};
use super::summaries::summarize_resource_decl;
use super::util::{normalize_asset_kind, normalize_limit};
use crate::http::scene_api::types::{
    WorldAssetGetResponse, WorldAssetListItem, WorldAssetListResponse, WorldRuntimeBundle,
    WorldScope,
};

fn collect_world_asset_items(
    bundle: &WorldRuntimeBundle,
    kind: &str,
    limit: usize,
) -> (usize, Vec<WorldAssetListItem>) {
    let mut items = Vec::new();
    let mut total = 0usize;

    if matches!(kind, "all" | "resource") {
        if let Some(world) = &bundle.contract.world {
            total += world.resources.len();
            for item in &world.resources {
                if items.len() >= limit {
                    break;
                }
                items.push(WorldAssetListItem {
                    id: item.id.clone(),
                    kind: "resource".to_string(),
                    label: None,
                    title: item.title.clone(),
                    status: None,
                    tags: Vec::new(),
                });
            }
        }
    }

    if matches!(kind, "all" | "entity") {
        if let Some(world) = &bundle.contract.world {
            total += world.entities.len();
            for item in &world.entities {
                if items.len() >= limit {
                    break;
                }
                items.push(WorldAssetListItem {
                    id: item.id.clone(),
                    kind: "entity".to_string(),
                    label: item.label.clone(),
                    title: None,
                    status: item.status.clone(),
                    tags: Vec::new(),
                });
            }
        }
    }

    if matches!(kind, "all" | "cell") {
        if let Some(world) = &bundle.contract.world {
            if let Some(topology) = &world.topology {
                total += topology.cells.len();
                for cell in &topology.cells {
                    if items.len() >= limit {
                        break;
                    }
                    items.push(WorldAssetListItem {
                        id: cell.id.clone(),
                        kind: "cell".to_string(),
                        label: None,
                        title: cell.surface_kind.clone(),
                        status: cell.hazard_state.clone(),
                        tags: cell.tags.clone(),
                    });
                }
            }
        }
    }

    (total, items)
}

pub(crate) fn query_world_assets(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    kind: Option<&str>,
    limit: Option<usize>,
) -> Result<WorldAssetListResponse> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let normalized_kind = normalize_asset_kind(kind);
    let normalized_limit = normalize_limit(limit, 20, 200);
    let (total, items) = collect_world_asset_items(&bundle, &normalized_kind, normalized_limit);
    Ok(WorldAssetListResponse {
        app_id: app_id.to_string(),
        scene_id: bundle.contract.scene.id.clone(),
        query_kind: normalized_kind,
        total,
        items,
    })
}

pub(crate) fn query_world_asset(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
) -> Result<WorldAssetGetResponse> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let target_id = id.trim();
    if target_id.is_empty() {
        return Err(anyhow!("query parameter `id` is required"));
    }

    if let Some(world) = &bundle.contract.world {
        if let Some(item) = world.resources.iter().find(|item| item.id == target_id) {
            let mut payload = summarize_resource_decl(item);
            if json_serialized_len(&payload) > LLM_RESOURCE_GET_BUDGET_CHARS {
                payload = shrink_json_for_llm(&payload, LLM_RESOURCE_GET_BUDGET_CHARS);
            }
            return Ok(WorldAssetGetResponse {
                app_id: app_id.to_string(),
                scene_id: bundle.contract.scene.id.clone(),
                id: item.id.clone(),
                kind: "resource".to_string(),
                payload,
            });
        }
        if let Some(item) = world.entities.iter().find(|item| item.id == target_id) {
            let raw = serde_json::to_value(item).unwrap_or(Value::Null);
            let payload = shrink_json_for_llm(&raw, LLM_RESOURCE_GET_BUDGET_CHARS);
            return Ok(WorldAssetGetResponse {
                app_id: app_id.to_string(),
                scene_id: bundle.contract.scene.id.clone(),
                id: item.id.clone(),
                kind: "entity".to_string(),
                payload,
            });
        }
        if let Some(topology) = &world.topology {
            if let Some(item) = topology.cells.iter().find(|item| item.id == target_id) {
                let raw = serde_json::to_value(item).unwrap_or(Value::Null);
                let payload = shrink_json_for_llm(&raw, LLM_RESOURCE_GET_BUDGET_CHARS);
                return Ok(WorldAssetGetResponse {
                    app_id: app_id.to_string(),
                    scene_id: bundle.contract.scene.id.clone(),
                    id: item.id.clone(),
                    kind: "cell".to_string(),
                    payload,
                });
            }
        }
    }

    Err(anyhow!("world asset `{target_id}` not found"))
}
