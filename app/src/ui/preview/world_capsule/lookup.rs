use mei_lang_kernel::{
    resolve_runtime_metric_def_key, CompiledApp, DatasetView,
    MetricContract, MetricShape, WorldSemanticExplainBlock, WorldSemanticMetric,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::ui::preview::nodes::component_html;
use crate::ui::preview::resolve::{
    attach_host_meta, dataset_for_host_ssr, with_runtime_ref, HostMetaOptions,
    RuntimeSceneAnchor,
};
use crate::ui::compile_status::{
    is_world_capsule_target, normalize_target_path, world_capsule_companion_scene,
};

pub(super) fn component_tag(compiled: &CompiledApp, use_key: &str) -> String {
    compiled
        .component_assets
        .iter()
        .find(|asset| asset.key == use_key)
        .map(|asset| asset.tag.clone())
        .unwrap_or_else(|| match use_key {
            "dataset.table" => "mei-dataset-table".to_string(),
            _ => "mei-missing-component".to_string(),
        })
}

pub(super) fn runtime_scene_anchor(compiled: &CompiledApp, file_path: &str) -> RuntimeSceneAnchor {
    let file_path = normalize_target_path(file_path);
    if !is_world_capsule_target(&file_path) {
        return RuntimeSceneAnchor {
            scene_id: compiled
                .active_scene
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| "default".to_string()),
            scene_path: Some(file_path),
        };
    }

    let active_target = normalize_target_path(&compiled.active_target_file);
    if active_target == file_path {
        let mut anchor = RuntimeSceneAnchor::from_compiled(compiled);
        anchor.scene_path = Some(file_path);
        return anchor;
    }

    let scene_id = compiled
        .scene_routes
        .iter()
        .find(|route| normalize_target_path(&route.target_file) == file_path)
        .map(|route| route.scene_id.clone())
        .or_else(|| {
            world_capsule_companion_scene(&file_path).and_then(|companion| {
                compiled
                    .scene_routes
                    .iter()
                    .find(|route| normalize_target_path(&route.target_file) == companion)
                    .map(|route| route.scene_id.clone())
            })
        })
        .unwrap_or_else(|| "default".to_string());

    RuntimeSceneAnchor {
        scene_id,
        scene_path: Some(file_path),
    }
}

pub(super) fn table_host_html(compiled: &CompiledApp, app_path: &str, file_path: &str, data: Value) -> String {
    let props = attach_host_meta(
        json!({
            "data": data,
            "paging": { "defaultPageSize": 20 },
            "toolbar": { "search": true },
        }),
        compiled,
        app_path,
        &json!({}),
        Some(file_path),
        HostMetaOptions::default(),
    );
    let tag = component_tag(compiled, "dataset.table");
    component_html(tag.as_str(), &props)
}

pub(super) fn prepare_dataset_table_data(
    dataset: &DatasetView,
    resolved_id: &str,
    anchor: &RuntimeSceneAnchor,
    host_ssr_slim_payload: bool,
) -> Value {
    let data = if host_ssr_slim_payload {
        dataset_for_host_ssr(dataset)
    } else {
        serde_json::to_value(dataset).unwrap_or(Value::Null)
    };
    with_runtime_ref(
        data,
        anchor.runtime_ref_extra("data", resolved_id, None, None),
    )
}

pub(super) fn find_world_metrics_dataset<'a>(
    compiled: &'a CompiledApp,
    file_path: &str,
) -> Option<&'a DatasetView> {
    let namespaced = format!("__world_metrics__::{file_path}::metrics");
    compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__" || resource.id == namespaced)
        .and_then(|resource| resource.dataset.as_ref())
}

pub(super) const SCALAR_ROWSET_SUFFIX: &str = "__scalar_rowset__";

pub(super) fn canonical_parent_metric_key(
    dataset: &DatasetView,
    resource_id: &str,
    parent_metric_id: &str,
) -> String {
    resolve_runtime_metric_def_key(resource_id, parent_metric_id, &dataset.runtime_metric_defs)
        .unwrap_or_else(|| parent_metric_id.to_string())
}

pub(super) fn tabular_node_id_from_analysis_contract(
    dataset: &DatasetView,
    resource_id: &str,
    parent_metric_id: &str,
    explain_block_id: &str,
) -> Option<String> {
    let parent_key = canonical_parent_metric_key(dataset, resource_id, parent_metric_id);
    let contract = dataset.runtime_analysis_contracts.get(&parent_key)?;
    let blocks = contract.get("blocks")?.as_array()?;
    for block in blocks {
        let Some(block_obj) = block.as_object() else {
            continue;
        };
        if block_obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            != Some(explain_block_id)
        {
            continue;
        }
        return block_obj
            .get("node_id")
            .or_else(|| block_obj.get("metric_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
    None
}

pub(super) fn tabular_metric_lookup_candidates(
    parent_metric_id: &str,
    explain_block_id: Option<&str>,
    explain_block: Option<&WorldSemanticExplainBlock>,
    dataset: Option<&DatasetView>,
    resource_id: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(block_id) = explain_block_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(dataset) = dataset {
            if let Some(node_id) = tabular_node_id_from_analysis_contract(
                dataset,
                resource_id,
                parent_metric_id,
                block_id,
            ) {
                candidates.push(node_id);
            }
        }
        candidates.push(format!("{parent_metric_id}::{block_id}"));
        let role = explain_block
            .and_then(|block| block.support_role.as_deref())
            .unwrap_or_else(|| explain_block.map(|block| block.kind.as_str()).unwrap_or(""));
        if role == "detail" {
            candidates.push(format!("{parent_metric_id}::{SCALAR_ROWSET_SUFFIX}"));
        }
        candidates.push(block_id.to_string());
    } else {
        candidates.push(parent_metric_id.to_string());
    }
    let mut seen = BTreeMap::<String, ()>::new();
    candidates
        .into_iter()
        .filter(|candidate| {
            if candidate.is_empty() || seen.contains_key(candidate) {
                false
            } else {
                seen.insert(candidate.clone(), ());
                true
            }
        })
        .collect()
}

pub(super) fn resolve_explain_block_id<'a>(
    metric_meta: &'a WorldSemanticMetric,
    explain_block_id: Option<&'a str>,
) -> Option<&'a str> {
    let raw = explain_block_id?.trim();
    if raw.is_empty() {
        return None;
    }
    if metric_meta.explain.iter().any(|block| block.id == raw) {
        return Some(raw);
    }
    if let Some(suffix) = raw.strip_prefix("data_product_") {
        if let Ok(index) = suffix.parse::<usize>() {
            return metric_meta
                .explain
                .get(index)
                .map(|block| block.id.as_str());
        }
    }
    Some(raw)
}

pub(super) fn lookup_metric_contract<'a>(
    compiled: &'a CompiledApp,
    dataset: &'a DatasetView,
    resource_id: &str,
    lookup_candidates: &[String],
    parent_metric_id: Option<&str>,
) -> Option<&'a MetricContract> {
    for candidate in lookup_candidates {
        if let Some(entry) = compiled.world_metrics.get(candidate.as_str()) {
            if metric_contract_is_tabular(&entry.metric) {
                return Some(&entry.metric);
            }
        }
    }
    for candidate in lookup_candidates {
        if let Some(canonical) = resolve_runtime_metric_def_key(
            resource_id,
            candidate.as_str(),
            &dataset.runtime_metric_defs,
        ) {
            if let Some(metric) = dataset.metrics.get(&canonical) {
                return Some(metric);
            }
        }
        if let Some(metric) = dataset.metrics.get(candidate.as_str()) {
            return Some(metric);
        }
    }
    let parent = parent_metric_id
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    for candidate in lookup_candidates {
        let metric_id = candidate.as_str();
        if metric_id == parent {
            continue;
        }
        if let Some(metric) = dataset.metrics.iter().find_map(|(key, metric)| {
            if key == metric_id
                || key.ends_with(&format!("::{metric_id}"))
                || (metric_id.contains("::") && key == metric_id)
            {
                Some(metric)
            } else {
                None
            }
        }) {
            return Some(metric);
        }
    }
    None
}

pub(super) fn metric_scalar_display(metric: &MetricContract) -> String {
    if let Some(value) = metric.value.get("value") {
        return value.to_string().trim_matches('"').to_string();
    }
    if metric.value.is_number() || metric.value.is_string() {
        return metric.value.to_string().trim_matches('"').to_string();
    }
    String::new()
}

pub(super) fn metric_contract_is_tabular(contract: &MetricContract) -> bool {
    matches!(
        contract.shape,
        MetricShape::Table | MetricShape::Dataframe | MetricShape::Series
    ) || contract.value.is_array()
}

