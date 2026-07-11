use super::prelude::*;
use super::*;

pub(crate) fn empty_query_state() -> mei_lang_kernel::QueryState {
    let filters = BTreeMap::<String, String>::new();
    let normalized_filters = mei_lang_datasets::normalize_query_filters(&filters);
    let normalized_search = mei_lang_datasets::normalize_query_search(None);
    query_state_from_request(&normalized_filters, normalized_search.as_deref(), None)
}

pub(crate) fn artifact_scene_context(compiled: &CompiledApp) -> (String, Option<String>) {
    let scene_id = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene_id| !scene_id.is_empty())
        .map(str::to_string)
        .or_else(|| {
            compiled
                .scene_routes
                .iter()
                .find(|route| route.target_file == compiled.active_target_file)
                .map(|route| route.scene_id.clone())
        })
        .unwrap_or_else(|| "default".to_string());
    let scene_path = compiled.active_target_file.trim().to_string();
    let scene_path = if scene_path.is_empty() {
        None
    } else {
        Some(scene_path)
    };
    (scene_id, scene_path)
}

pub(crate) fn artifact_scene_context_for_resource(
    compiled: &CompiledApp,
    resource_id: &str,
) -> (String, Option<String>) {
    let Some(target_file) =
        mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(resource_id)
    else {
        return artifact_scene_context(compiled);
    };
    let lookup_keys = mei_lang_kernel::app_source_rel_path_lookup_keys(target_file.as_str());
    let scene_id = lookup_keys
        .iter()
        .find_map(|key| {
            compiled
                .scene_routes
                .iter()
                .find(|route| route.target_file == *key)
                .map(|route| route.scene_id.clone())
        })
        .or_else(|| compiled.active_scene.clone())
        .unwrap_or_else(|| "default".to_string());
    let scene_path = lookup_keys
        .into_iter()
        .find(|key| {
            compiled
                .scene_routes
                .iter()
                .any(|route| route.target_file == *key)
        })
        .or_else(|| {
            Some(mei_lang_kernel::canonical_app_source_rel_path(
                target_file.as_str(),
            ))
        });
    (scene_id, scene_path)
}

pub(crate) fn scope_identity_key(scene_id: &str, scene_path: Option<&str>) -> String {
    compile_scope_key_from_parts(
        Some(scene_id),
        scene_path.map(str::trim).filter(|value| !value.is_empty()),
    )
}

pub(crate) fn request_metric_scope_token(metric_ids: &[String]) -> String {
    if metric_ids.is_empty() {
        "*".to_string()
    } else {
        metric_scope_cache_key(metric_ids)
    }
}

pub(crate) fn logical_metric_workset_id(
    app_id: &str,
    owner_resource_id: &str,
    metric_ids: &[String],
) -> String {
    format!(
        "workset|app={app_id}|owner={owner_resource_id}|metrics={}",
        request_metric_scope_token(metric_ids)
    )
}

pub(crate) fn summarize_metric_ids(metric_ids: &[String]) -> String {
    if metric_ids.is_empty() {
        return "*".to_string();
    }
    let mut preview = metric_ids
        .iter()
        .take(6)
        .map(|metric_id| short_metric_id(metric_id).to_string())
        .collect::<Vec<_>>();
    if metric_ids.len() > 6 {
        preview.push(format!("+{} more", metric_ids.len() - 6));
    }
    preview.join(", ")
}

pub(crate) fn materialization_identity(
    logical_node_id: &str,
    scope_id: &str,
    dependency_revision_key: &str,
    compile_revision: &str,
) -> String {
    format!(
        "{logical_node_id}|scope={scope_id}|dependency={dependency_revision_key}|compile={compile_revision}"
    )
}
