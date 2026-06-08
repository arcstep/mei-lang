use axum::http::StatusCode;
use mei_lang_kernel::{CompileOptions, FilterIntent, QueryState};
use std::collections::BTreeMap;

use crate::AppError;

use super::super::util::is_script_target;

/// Request coordinates for scene-first dataset/metric APIs.
#[derive(Debug, Clone, Default)]
pub struct SceneQueryCoords {
    pub scene_id: Option<String>,
    /// Legacy source locator; used when `scene_id` is absent to derive compile context.
    pub target: Option<String>,
}

impl SceneQueryCoords {
    pub fn from_parts(scene_id: Option<String>, target: Option<String>) -> Self {
        Self {
            scene_id: scene_id
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            target: target
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }
}

pub fn strict_scene_query_coords(
    scene_id: Option<String>,
    target: Option<String>,
    request_kind: &str,
) -> Result<SceneQueryCoords, AppError> {
    let coords = SceneQueryCoords::from_parts(scene_id, target);
    if coords.scene_id.is_some() {
        return Ok(coords);
    }
    let detail = coords
        .target
        .as_deref()
        .map(|target| format!(" (received legacy target `{target}` without scene_id)"))
        .unwrap_or_default();
    Err(AppError::status(
        StatusCode::BAD_REQUEST,
        format!(
            "{request_kind} requires `scene_id`; target-only runtime requests are no longer supported{detail}"
        ),
    ))
}

fn normalize_contract_filters(filters: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    filters
        .iter()
        .filter_map(|(dimension, value)| {
            let dimension = dimension.trim();
            let value = value.trim();
            if dimension.is_empty() || value.is_empty() {
                return None;
            }
            Some((dimension.to_string(), value.to_string()))
        })
        .collect()
}

fn normalize_contract_search(search: Option<&str>) -> Option<String> {
    search
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn strict_runtime_query_contract(
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
    query_state: Option<&QueryState>,
    filter_intents: &[FilterIntent],
    request_kind: &str,
) -> Result<(), AppError> {
    let normalized_filters = normalize_contract_filters(filters);
    let normalized_search = normalize_contract_search(search);
    let requires_query_state =
        !normalized_filters.is_empty() || normalized_search.is_some() || !filter_intents.is_empty();
    if requires_query_state && query_state.is_none() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "{request_kind} with `filters`, `search`, or `filter_intents` requires `query_state`"
            ),
        ));
    }
    let Some(state) = query_state else {
        return Ok(());
    };
    let state_filters = normalize_contract_filters(&state.filters);
    if !normalized_filters.is_empty()
        && !state_filters.is_empty()
        && normalized_filters != state_filters
    {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "{request_kind} received conflicting `filters` and `query_state.filters`; fail-fast migration rejects mixed query truth"
            ),
        ));
    }
    let state_search = normalize_contract_search(state.search.as_deref());
    if normalized_search.is_some() && state_search.is_some() && normalized_search != state_search {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "{request_kind} received conflicting `search` and `query_state.search`; fail-fast migration rejects mixed query truth"
            ),
        ));
    }
    Ok(())
}

pub fn strict_dataset_query_mode_contract(
    metric_id: Option<&str>,
    filter_intents: &[FilterIntent],
) -> Result<(), AppError> {
    let metric_id = metric_id.map(str::trim).filter(|value| !value.is_empty());
    if metric_id.is_some() || filter_intents.is_empty() {
        return Ok(());
    }
    Err(AppError::status(
        StatusCode::BAD_REQUEST,
        "plain dataset row queries do not accept `filter_intents`; keep row filtering in `query_state`, or use metric/dataframe query for EvalScope-aware evaluation",
    ))
}

pub fn compile_options_from_coords(coords: &SceneQueryCoords) -> CompileOptions {
    let preview_target = coords
        .target
        .as_deref()
        .filter(|target| is_script_target(target))
        .map(ToString::to_string);
    CompileOptions {
        scene: coords.scene_id.clone(),
        preview_target,
    }
}
