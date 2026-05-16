use anyhow::{anyhow, Result};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::{
    compile_app, initial_runtime_state, project_runtime_view, render_runtime_html, runtime_step,
    RuntimeIntent, RuntimeSceneView, RuntimeState, RuntimeTraceItem,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

use crate::{AppError, AppState};

#[derive(Debug, Deserialize)]
pub struct SimStepRequest {
    #[serde(default)]
    pub state: Option<RuntimeState>,
    pub intent: RuntimeIntent,
}

#[derive(Debug, Serialize)]
pub struct SimStepResponse {
    pub state: RuntimeState,
    pub scene_view: RuntimeSceneView,
    #[serde(default)]
    pub trace_delta: Vec<RuntimeTraceItem>,
    pub html: String,
}

#[derive(Debug, Clone)]
struct WorldRuntimeBundle {
    entry_target: String,
    contract: mei_lang_kernel::SceneContract,
    state: RuntimeState,
    scene_view: RuntimeSceneView,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldQueryCapabilitySummary {
    pub id: String,
    pub status: String,
    pub purpose: String,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldRuntimeSummary {
    pub phase: String,
    pub result: String,
    pub countdown: i64,
    pub scene_view_entities: usize,
    pub scene_view_cells: usize,
    pub available_actions: Vec<String>,
    pub recent_trace_messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldSnapshotSummary {
    pub scene_id: String,
    pub world_id: Option<String>,
    pub world_resource_count: usize,
    pub world_entity_count: usize,
    pub world_topology: Option<String>,
    pub world_resource_kind_counts: BTreeMap<String, usize>,
    pub world_entity_kind_counts: BTreeMap<String, usize>,
    pub world_key_resource_ids: Vec<String>,
    pub world_key_entity_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldContextSnapshot {
    pub app_id: String,
    pub entry_target: String,
    pub world_snapshot: WorldSnapshotSummary,
    pub runtime_summary: WorldRuntimeSummary,
    pub query_capabilities: Vec<WorldQueryCapabilitySummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldAssetListItem {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldAssetListResponse {
    pub app_id: String,
    pub scene_id: String,
    pub query_kind: String,
    pub total: usize,
    pub items: Vec<WorldAssetListItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldAssetGetResponse {
    pub app_id: String,
    pub scene_id: String,
    pub id: String,
    pub kind: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldRuntimePeekResponse {
    pub app_id: String,
    pub scene_id: String,
    pub phase: String,
    pub result: String,
    pub countdown: i64,
    pub available_actions: Vec<String>,
    pub recent_trace_messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct WorldAssetListQuery {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct WorldAssetGetQuery {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct WorldRuntimePeekQuery {
    #[serde(default)]
    pub trace_limit: Option<usize>,
}

fn normalize_asset_kind(kind: Option<&str>) -> String {
    match kind.map(|value| value.trim().to_ascii_lowercase()) {
        Some(value) if value == "entity" => "entity".to_string(),
        Some(value) if value == "resource" => "resource".to_string(),
        Some(value) if value == "cell" => "cell".to_string(),
        _ => "all".to_string(),
    }
}

fn normalize_limit(limit: Option<usize>, default: usize, max: usize) -> usize {
    limit.unwrap_or(default).clamp(1, max)
}

fn load_world_runtime_bundle(source_root: &Path, app_id: &str) -> Result<WorldRuntimeBundle> {
    let compiled = compile_app(source_root, app_id)?;
    let contract = compiled
        .scene_contract
        .ok_or_else(|| anyhow!("app `{}` does not provide a scene contract", app_id))?;
    let state = initial_runtime_state(&contract, 1);
    let scene_view = project_runtime_view(&contract, &state);
    Ok(WorldRuntimeBundle {
        entry_target: compiled.entry_target,
        contract,
        state,
        scene_view,
    })
}

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

fn recent_trace_messages(state: &RuntimeState, trace_limit: usize) -> Vec<String> {
    state
        .trace_events
        .iter()
        .rev()
        .take(trace_limit)
        .map(|item| item.message.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn default_world_query_capabilities() -> Vec<WorldQueryCapabilitySummary> {
    vec![
        WorldQueryCapabilitySummary {
            id: "world.asset.list".to_string(),
            status: "phase1_context".to_string(),
            purpose: "按类型查看 world 里的核心资产清单（entity/resource/cell）".to_string(),
            input: "{kind?: entity|resource|cell, limit?: number}".to_string(),
            output: "{items: [{id, kind, label_or_title, tags?}], total}".to_string(),
        },
        WorldQueryCapabilitySummary {
            id: "world.asset.get".to_string(),
            status: "phase2_planned_tool".to_string(),
            purpose: "按资产 id 查看单个对象详情".to_string(),
            input: "{id: string}".to_string(),
            output: "{id, kind, fields, relations?}".to_string(),
        },
        WorldQueryCapabilitySummary {
            id: "world.runtime.peek".to_string(),
            status: "phase1_context".to_string(),
            purpose: "查看运行态关键信息（phase/result/actions/trace）".to_string(),
            input: "{include?: [state|actions|trace], trace_limit?: number}".to_string(),
            output: "{phase, result, available_actions, recent_trace_messages}".to_string(),
        },
    ]
}

pub(crate) fn build_world_context_snapshot(
    source_root: &Path,
    app_id: &str,
) -> Result<WorldContextSnapshot> {
    let bundle = load_world_runtime_bundle(source_root, app_id)?;
    let world = bundle.contract.world.clone();

    let (resource_kind_counts, key_resource_ids, world_resource_count) = if let Some(world) = &world
    {
        let mut counts = BTreeMap::new();
        let mut ids = Vec::new();
        for item in &world.resources {
            *counts.entry(item.kind.clone()).or_insert(0) += 1;
            if ids.len() < 20 {
                ids.push(item.id.clone());
            }
        }
        (counts, ids, world.resources.len())
    } else {
        (BTreeMap::new(), Vec::new(), 0)
    };

    let (entity_kind_counts, key_entity_ids, world_entity_count) = if let Some(world) = &world {
        let mut counts = BTreeMap::new();
        let mut ids = Vec::new();
        for item in &world.entities {
            *counts.entry(item.kind.clone()).or_insert(0) += 1;
            if ids.len() < 20 {
                ids.push(item.id.clone());
            }
        }
        (counts, ids, world.entities.len())
    } else {
        (BTreeMap::new(), Vec::new(), 0)
    };

    let world_topology = world.as_ref().and_then(|item| {
        item.topology.as_ref().map(|topology| {
            format!(
                "grid(rows={}, cols={}, cells={})",
                topology.rows,
                topology.cols,
                topology.cells.len()
            )
        })
    });

    let recent_trace_messages = recent_trace_messages(&bundle.state, 5);

    Ok(WorldContextSnapshot {
        app_id: app_id.to_string(),
        entry_target: bundle.entry_target,
        world_snapshot: WorldSnapshotSummary {
            scene_id: bundle.contract.scene.id.clone(),
            world_id: world.as_ref().and_then(|item| item.id.clone()),
            world_resource_count,
            world_entity_count,
            world_topology,
            world_resource_kind_counts: resource_kind_counts,
            world_entity_kind_counts: entity_kind_counts,
            world_key_resource_ids: key_resource_ids,
            world_key_entity_ids: key_entity_ids,
        },
        runtime_summary: WorldRuntimeSummary {
            phase: bundle.state.phase.clone(),
            result: bundle.state.result.clone(),
            countdown: bundle.state.countdown,
            scene_view_entities: bundle.scene_view.entities.len(),
            scene_view_cells: bundle.scene_view.cells.len(),
            available_actions: bundle
                .scene_view
                .available_actions
                .iter()
                .take(20)
                .cloned()
                .collect(),
            recent_trace_messages,
        },
        query_capabilities: default_world_query_capabilities(),
    })
}

pub async fn world_context_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
) -> Result<Json<WorldContextSnapshot>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let snapshot =
        build_world_context_snapshot(&state.source_root, app_id).map_err(AppError::from)?;
    Ok(Json(snapshot))
}

pub async fn world_assets_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Query(query): Query<WorldAssetListQuery>,
) -> Result<Json<WorldAssetListResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let bundle = load_world_runtime_bundle(&state.source_root, app_id).map_err(AppError::from)?;
    let kind = normalize_asset_kind(query.kind.as_deref());
    let limit = normalize_limit(query.limit, 20, 200);
    let (total, items) = collect_world_asset_items(&bundle, &kind, limit);
    Ok(Json(WorldAssetListResponse {
        app_id: app_id.to_string(),
        scene_id: bundle.contract.scene.id.clone(),
        query_kind: kind,
        total,
        items,
    }))
}

pub async fn world_asset_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Query(query): Query<WorldAssetGetQuery>,
) -> Result<Json<WorldAssetGetResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let bundle = load_world_runtime_bundle(&state.source_root, app_id).map_err(AppError::from)?;
    let target_id = query.id.trim();
    if target_id.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "query parameter `id` is required",
        ));
    }

    if let Some(world) = &bundle.contract.world {
        if let Some(item) = world.resources.iter().find(|item| item.id == target_id) {
            return Ok(Json(WorldAssetGetResponse {
                app_id: app_id.to_string(),
                scene_id: bundle.contract.scene.id.clone(),
                id: item.id.clone(),
                kind: "resource".to_string(),
                payload: serde_json::to_value(item).unwrap_or(Value::Null),
            }));
        }
        if let Some(item) = world.entities.iter().find(|item| item.id == target_id) {
            return Ok(Json(WorldAssetGetResponse {
                app_id: app_id.to_string(),
                scene_id: bundle.contract.scene.id.clone(),
                id: item.id.clone(),
                kind: "entity".to_string(),
                payload: serde_json::to_value(item).unwrap_or(Value::Null),
            }));
        }
        if let Some(topology) = &world.topology {
            if let Some(item) = topology.cells.iter().find(|item| item.id == target_id) {
                return Ok(Json(WorldAssetGetResponse {
                    app_id: app_id.to_string(),
                    scene_id: bundle.contract.scene.id.clone(),
                    id: item.id.clone(),
                    kind: "cell".to_string(),
                    payload: serde_json::to_value(item).unwrap_or(Value::Null),
                }));
            }
        }
    }

    Err(AppError::status(
        StatusCode::NOT_FOUND,
        format!("world asset `{target_id}` not found"),
    ))
}

pub async fn world_runtime_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Query(query): Query<WorldRuntimePeekQuery>,
) -> Result<Json<WorldRuntimePeekResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let bundle = load_world_runtime_bundle(&state.source_root, app_id).map_err(AppError::from)?;
    let trace_limit = normalize_limit(query.trace_limit, 5, 50);
    Ok(Json(WorldRuntimePeekResponse {
        app_id: app_id.to_string(),
        scene_id: bundle.contract.scene.id.clone(),
        phase: bundle.state.phase.clone(),
        result: bundle.state.result.clone(),
        countdown: bundle.state.countdown,
        available_actions: bundle
            .scene_view
            .available_actions
            .iter()
            .take(20)
            .cloned()
            .collect(),
        recent_trace_messages: recent_trace_messages(&bundle.state, trace_limit),
    }))
}

pub async fn sim_step_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Json(request): Json<SimStepRequest>,
) -> Result<Json<SimStepResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let compiled = compile_app(&state.source_root, app_id).map_err(AppError::from)?;
    let contract = compiled.scene_contract.ok_or_else(|| {
        AppError::msg(format!(
            "app `{}` does not provide a scene contract",
            app_id
        ))
    })?;
    let current_state = request
        .state
        .clone()
        .unwrap_or_else(|| initial_runtime_state(&contract, 1));
    let next_state = runtime_step(&contract, request.state, &request.intent);
    let trace_delta = if next_state.trace_events.len() > current_state.trace_events.len() {
        next_state.trace_events[current_state.trace_events.len()..].to_vec()
    } else if request.intent.kind == "sync" {
        next_state.trace_events.clone()
    } else {
        Vec::new()
    };
    let scene_view = project_runtime_view(&contract, &next_state);
    let html = render_runtime_html(&scene_view, &next_state);
    Ok(Json(SimStepResponse {
        state: next_state,
        scene_view,
        trace_delta,
        html,
    }))
}
