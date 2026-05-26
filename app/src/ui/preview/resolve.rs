use std::collections::BTreeMap;

use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_dataset_resource_id, resolve_dataset_selector_value,
    scene_payload_cache_epoch, CompiledApp, LoadedResource, RuntimeResourceIndex, SceneContract,
};
use serde_json::{json, Value};

use super::theme::resolve_shared_refs;

/// Scene anchor injected into `__mei_runtime_ref` for scene-qualified runtime APIs.
#[derive(Debug, Clone)]
pub(super) struct RuntimeSceneAnchor {
    pub scene_id: String,
    pub scene_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct MetricDrilldownMeta {
    drilldown_scene: Option<String>,
    drilldown_target_scene_id: Option<String>,
    drilldown_enabled: Option<bool>,
    explain_kind: Option<String>,
    drilldown_tabs: Vec<String>,
    drilldown_title: Option<String>,
    drilldown_note: Option<String>,
    drilldown_table_metric_id: Option<String>,
    drilldown_dataset_id: Option<String>,
    drilldown_layout_preset: Option<String>,
    drilldown_columns: Vec<String>,
    drilldown_headers: Vec<String>,
    drilldown_basis_refs: Vec<String>,
    drilldown_detail_fields: Vec<String>,
    drilldown_recommended_dimensions: Vec<String>,
    drilldown_ratio_numerator: Option<String>,
    drilldown_ratio_denominator: Option<String>,
    drilldown_ratio_formula: Option<String>,
    drilldown_tab_metrics: serde_json::Map<String, Value>,
    explain_metrics: serde_json::Map<String, Value>,
    explain_composition_by: Vec<String>,
    explain_trend_field: Option<String>,
    explain_trend_grain: Option<String>,
    explain_detail_dataset: Option<String>,
}

impl MetricDrilldownMeta {
    fn is_empty(&self) -> bool {
        self.drilldown_scene.is_none()
            && self.drilldown_target_scene_id.is_none()
            && self.drilldown_enabled.is_none()
            && self.explain_kind.is_none()
            && self.drilldown_tabs.is_empty()
            && self.drilldown_title.is_none()
            && self.drilldown_note.is_none()
            && self.drilldown_table_metric_id.is_none()
            && self.drilldown_dataset_id.is_none()
            && self.drilldown_layout_preset.is_none()
            && self.drilldown_columns.is_empty()
            && self.drilldown_headers.is_empty()
            && self.drilldown_basis_refs.is_empty()
            && self.drilldown_detail_fields.is_empty()
            && self.drilldown_recommended_dimensions.is_empty()
            && self.drilldown_ratio_numerator.is_none()
            && self.drilldown_ratio_denominator.is_none()
            && self.drilldown_ratio_formula.is_none()
            && self.drilldown_tab_metrics.is_empty()
            && self.explain_metrics.is_empty()
            && self.explain_composition_by.is_empty()
            && self.explain_trend_field.is_none()
            && self.explain_trend_grain.is_none()
            && self.explain_detail_dataset.is_none()
    }
}

impl RuntimeSceneAnchor {
    pub fn from_compiled(compiled: &CompiledApp) -> Self {
        let scene_id = compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
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
        Self {
            scene_id,
            scene_path: if scene_path.is_empty() {
                None
            } else {
                Some(scene_path)
            },
        }
    }

    fn runtime_ref_extra(
        &self,
        kind: &str,
        dataset_id: &str,
        metric_id: Option<&str>,
        drilldown: Option<&MetricDrilldownMeta>,
    ) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("kind".to_string(), Value::String(kind.to_string()));
        obj.insert("scene_id".to_string(), Value::String(self.scene_id.clone()));
        if let Some(path) = self.scene_path.as_deref().filter(|s| !s.is_empty()) {
            obj.insert("scene_path".to_string(), Value::String(path.to_string()));
        }
        obj.insert(
            "dataset_id".to_string(),
            Value::String(dataset_id.to_string()),
        );
        if let Some(mid) = metric_id.filter(|s| !s.is_empty()) {
            obj.insert("metric_id".to_string(), Value::String(mid.to_string()));
        }
        if let Some(meta) = drilldown.filter(|m| !m.is_empty()) {
            if let Some(scene) = meta.drilldown_scene.as_deref().filter(|s| !s.is_empty()) {
                obj.insert(
                    "drilldown_scene".to_string(),
                    Value::String(scene.to_string()),
                );
            }
            if let Some(scene_id) = meta
                .drilldown_target_scene_id
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "drilldown_target_scene_id".to_string(),
                    Value::String(scene_id.to_string()),
                );
            }
            if let Some(enabled) = meta.drilldown_enabled {
                obj.insert("drilldown_enabled".to_string(), Value::Bool(enabled));
            }
            if let Some(kind_value) = meta.explain_kind.as_deref().filter(|s| !s.is_empty()) {
                obj.insert(
                    "explain_kind".to_string(),
                    Value::String(kind_value.to_string()),
                );
            }
            if !meta.drilldown_tabs.is_empty() {
                obj.insert(
                    "drilldown_tabs".to_string(),
                    Value::Array(
                        meta.drilldown_tabs
                            .iter()
                            .map(|tab| Value::String(tab.clone()))
                            .collect(),
                    ),
                );
            }
            if let Some(title) = meta.drilldown_title.as_deref().filter(|s| !s.is_empty()) {
                obj.insert(
                    "drilldown_title".to_string(),
                    Value::String(title.to_string()),
                );
            }
            if let Some(note) = meta.drilldown_note.as_deref().filter(|s| !s.is_empty()) {
                obj.insert(
                    "drilldown_note".to_string(),
                    Value::String(note.to_string()),
                );
            }
            if let Some(metric_id) = meta
                .drilldown_table_metric_id
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "drilldown_table_metric_id".to_string(),
                    Value::String(metric_id.to_string()),
                );
            }
            if let Some(dataset_id) = meta
                .drilldown_dataset_id
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "drilldown_dataset_id".to_string(),
                    Value::String(dataset_id.to_string()),
                );
            }
            if let Some(layout_preset) = meta
                .drilldown_layout_preset
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "drilldown_layout_preset".to_string(),
                    Value::String(layout_preset.to_string()),
                );
            }
            if !meta.drilldown_columns.is_empty() {
                obj.insert(
                    "drilldown_columns".to_string(),
                    Value::Array(
                        meta.drilldown_columns
                            .iter()
                            .map(|column| Value::String(column.clone()))
                            .collect(),
                    ),
                );
            }
            if !meta.drilldown_headers.is_empty() {
                obj.insert(
                    "drilldown_headers".to_string(),
                    Value::Array(
                        meta.drilldown_headers
                            .iter()
                            .map(|header| Value::String(header.clone()))
                            .collect(),
                    ),
                );
            }
            if !meta.drilldown_basis_refs.is_empty() {
                obj.insert(
                    "drilldown_basis_refs".to_string(),
                    Value::Array(
                        meta.drilldown_basis_refs
                            .iter()
                            .map(|basis| Value::String(basis.clone()))
                            .collect(),
                    ),
                );
            }
            if !meta.drilldown_detail_fields.is_empty() {
                obj.insert(
                    "drilldown_detail_fields".to_string(),
                    Value::Array(
                        meta.drilldown_detail_fields
                            .iter()
                            .map(|field| Value::String(field.clone()))
                            .collect(),
                    ),
                );
            }
            if !meta.drilldown_recommended_dimensions.is_empty() {
                obj.insert(
                    "drilldown_recommended_dimensions".to_string(),
                    Value::Array(
                        meta.drilldown_recommended_dimensions
                            .iter()
                            .map(|dimension| Value::String(dimension.clone()))
                            .collect(),
                    ),
                );
            }
            if let Some(numerator) = meta
                .drilldown_ratio_numerator
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "drilldown_ratio_numerator".to_string(),
                    Value::String(numerator.to_string()),
                );
            }
            if let Some(denominator) = meta
                .drilldown_ratio_denominator
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "drilldown_ratio_denominator".to_string(),
                    Value::String(denominator.to_string()),
                );
            }
            if let Some(formula) = meta
                .drilldown_ratio_formula
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "drilldown_ratio_formula".to_string(),
                    Value::String(formula.to_string()),
                );
            }
            if !meta.drilldown_tab_metrics.is_empty() {
                obj.insert(
                    "drilldown_tab_metrics".to_string(),
                    Value::Object(meta.drilldown_tab_metrics.clone()),
                );
            }
            if !meta.explain_metrics.is_empty() {
                obj.insert(
                    "explain_metrics".to_string(),
                    Value::Object(meta.explain_metrics.clone()),
                );
            }
            if let Some(title) = meta.drilldown_title.as_deref().filter(|s| !s.is_empty()) {
                obj.insert(
                    "explain_title".to_string(),
                    Value::String(title.to_string()),
                );
            }
            if let Some(note) = meta.drilldown_note.as_deref().filter(|s| !s.is_empty()) {
                obj.insert("explain_note".to_string(), Value::String(note.to_string()));
            }
            if !meta.drilldown_basis_refs.is_empty() {
                obj.insert(
                    "explain_basis_refs".to_string(),
                    Value::Array(
                        meta.drilldown_basis_refs
                            .iter()
                            .map(|basis| Value::String(basis.clone()))
                            .collect(),
                    ),
                );
            }
            if !meta.drilldown_detail_fields.is_empty() {
                obj.insert(
                    "explain_detail_fields".to_string(),
                    Value::Array(
                        meta.drilldown_detail_fields
                            .iter()
                            .map(|field| Value::String(field.clone()))
                            .collect(),
                    ),
                );
            }
            if !meta.drilldown_recommended_dimensions.is_empty() {
                obj.insert(
                    "explain_recommended_dimensions".to_string(),
                    Value::Array(
                        meta.drilldown_recommended_dimensions
                            .iter()
                            .map(|dimension| Value::String(dimension.clone()))
                            .collect(),
                    ),
                );
            }
            if let Some(numerator) = meta
                .drilldown_ratio_numerator
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "explain_ratio_numerator".to_string(),
                    Value::String(numerator.to_string()),
                );
            }
            if let Some(denominator) = meta
                .drilldown_ratio_denominator
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "explain_ratio_denominator".to_string(),
                    Value::String(denominator.to_string()),
                );
            }
            if let Some(formula) = meta
                .drilldown_ratio_formula
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "explain_ratio_formula".to_string(),
                    Value::String(formula.to_string()),
                );
            }
            if !meta.explain_composition_by.is_empty() {
                obj.insert(
                    "explain_composition_by".to_string(),
                    Value::Array(
                        meta.explain_composition_by
                            .iter()
                            .map(|field| Value::String(field.clone()))
                            .collect(),
                    ),
                );
            }
            if let Some(field) = meta
                .explain_trend_field
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "explain_trend_field".to_string(),
                    Value::String(field.to_string()),
                );
            }
            if let Some(grain) = meta
                .explain_trend_grain
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "explain_trend_grain".to_string(),
                    Value::String(grain.to_string()),
                );
            }
            if let Some(dataset) = meta
                .explain_detail_dataset
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                obj.insert(
                    "explain_detail_dataset".to_string(),
                    Value::String(dataset.to_string()),
                );
            }
            obj.insert(
                "analysis_enabled".to_string(),
                Value::Bool(meta.drilldown_enabled.unwrap_or(true)),
            );
            if let Some(kind_value) = meta.explain_kind.as_deref().filter(|s| !s.is_empty()) {
                obj.insert(
                    "analysis_kind".to_string(),
                    Value::String(kind_value.to_string()),
                );
            }
            if let Some(note) = meta.drilldown_note.as_deref().filter(|s| !s.is_empty()) {
                obj.insert("analysis_note".to_string(), Value::String(note.to_string()));
            }
            if !meta.drilldown_tabs.is_empty() {
                obj.insert(
                    "analysis_tabs".to_string(),
                    Value::Array(
                        meta.drilldown_tabs
                            .iter()
                            .map(|tab| Value::String(tab.clone()))
                            .collect(),
                    ),
                );
            }
            if !meta.drilldown_tab_metrics.is_empty() {
                obj.insert(
                    "analysis_tab_metrics".to_string(),
                    Value::Object(meta.drilldown_tab_metrics.clone()),
                );
            }
            let mut contract = serde_json::Map::new();
            contract.insert(
                "enabled".to_string(),
                Value::Bool(meta.drilldown_enabled.unwrap_or(true)),
            );
            if let Some(kind_value) = meta.explain_kind.as_deref().filter(|s| !s.is_empty()) {
                contract.insert("kind".to_string(), Value::String(kind_value.to_string()));
            }
            if !meta.drilldown_tabs.is_empty() {
                contract.insert(
                    "tabs".to_string(),
                    Value::Array(
                        meta.drilldown_tabs
                            .iter()
                            .map(|tab| Value::String(tab.clone()))
                            .collect(),
                    ),
                );
            }
            if !meta.drilldown_tab_metrics.is_empty() {
                contract.insert(
                    "tab_metrics".to_string(),
                    Value::Object(meta.drilldown_tab_metrics.clone()),
                );
            }
            if !meta.explain_metrics.is_empty() {
                contract.insert(
                    "explain_metrics".to_string(),
                    Value::Object(meta.explain_metrics.clone()),
                );
            }
            if let Some(title) = meta.drilldown_title.as_deref().filter(|s| !s.is_empty()) {
                contract.insert("title".to_string(), Value::String(title.to_string()));
            }
            if let Some(note) = meta.drilldown_note.as_deref().filter(|s| !s.is_empty()) {
                contract.insert("note".to_string(), Value::String(note.to_string()));
            }
            if let Some(scene_id) = meta
                .drilldown_target_scene_id
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                contract.insert(
                    "target_scene_id".to_string(),
                    Value::String(scene_id.to_string()),
                );
            }
            if let Some(dataset_id) = meta
                .drilldown_dataset_id
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                contract.insert(
                    "dataset_id".to_string(),
                    Value::String(dataset_id.to_string()),
                );
            }
            if let Some(metric_id) = meta
                .drilldown_table_metric_id
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                contract.insert(
                    "table_metric_id".to_string(),
                    Value::String(metric_id.to_string()),
                );
            }
            obj.insert("analysis_contract".to_string(), Value::Object(contract));
        }
        Value::Object(obj)
    }
}

pub(super) fn attach_host_meta(
    mut props: Value,
    compiled: &CompiledApp,
    app_path: &str,
    theme_components: &serde_json::Value,
    shared_context: &serde_json::Value,
    preview_scene_path: Option<&str>,
) -> Value {
    let mut anchor = RuntimeSceneAnchor::from_compiled(compiled);
    if let Some(path) = preview_scene_path.map(str::trim).filter(|s| !s.is_empty()) {
        anchor.scene_path = Some(path.to_string());
    }
    let active_target_file = anchor
        .scene_path
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(compiled.active_target_file.as_str());
    if let Some(map) = props.as_object_mut() {
        map.insert(
            "_mei".to_string(),
            json!({
                "app_id": compiled.app_id,
                "app_path": app_path,
                "active_scene_id": anchor.scene_id,
                "active_target_file": active_target_file,
                "entry_target": active_target_file,
                "compile_epoch": format!(
                    "{}|{}|{}",
                    scene_payload_cache_epoch(),
                    dataset_materialize_cache_epoch(),
                    active_target_file
                ),
                "step_api": format!("/api/sim/step/{}", app_path),
                "dataset_query_api": format!("/api/datasets/query/{}", app_path),
                "metric_query_api": format!("/api/datasets/metrics/{}", app_path),
                "components": theme_components.clone(),
                "shared": shared_context.clone(),
            }),
        );
    }
    props
}

pub(super) fn resolve_value(
    value: &Value,
    shared_context: &Value,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
    scene_anchor: &RuntimeSceneAnchor,
    resource_index: &RuntimeResourceIndex,
    compiled: &CompiledApp,
) -> Value {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("shared") {
                return resolve_shared_refs(value, shared_context);
            }
            if matches!(
                map.get("__ref").and_then(Value::as_str),
                Some("dataset") | Some("resource") | Some("entity")
            ) {
                if let Some(canonical_id) =
                    resolve_dataset_selector_value(compiled, value, resource_index)
                {
                    if let Some(resource) = resources.get(&canonical_id) {
                        if let Some(dataset) = resource.dataset.as_ref() {
                            return with_runtime_ref(
                                serde_json::to_value(dataset).unwrap_or(Value::Null),
                                scene_anchor.runtime_ref_extra("data", &canonical_id, None, None),
                            );
                        }
                        return serde_json::to_value(resource).unwrap_or(Value::Null);
                    }
                }
            }
            if map.get("__ref").and_then(Value::as_str) == Some("scene") {
                return serde_json::to_value(scene_contract).unwrap_or(Value::Null);
            }
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some((dataset, dataset_id)) =
                    resolve_data_ref(map, resources, compiled, resource_index)
                {
                    return with_runtime_ref(
                        serde_json::to_value(dataset).unwrap_or(Value::Null),
                        scene_anchor.runtime_ref_extra("data", &dataset_id, None, None),
                    );
                }
                return Value::Null;
            }
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                if let Some((metric, dataset_id)) =
                    resolve_metric_ref(map, resources, compiled, resource_index)
                {
                    let metric_id = map.get("id").and_then(Value::as_str).unwrap_or("");
                    let drilldown =
                        resolve_metric_drilldown_meta(resources, &dataset_id, metric_id, compiled);
                    return with_runtime_ref(
                        serde_json::to_value(metric).unwrap_or(Value::Null),
                        scene_anchor.runtime_ref_extra(
                            "metric",
                            &dataset_id,
                            Some(metric_id),
                            drilldown.as_ref(),
                        ),
                    );
                }
                return Value::Null;
            }
            if map.get("metric").and_then(Value::as_str).is_some() {
                let mut compat = serde_json::Map::new();
                compat.insert("__ref".to_string(), Value::String("metric".to_string()));
                if let Some(id) = map.get("metric").cloned() {
                    compat.insert("id".to_string(), id);
                }
                if let Some(from) = map
                    .get("from_dataset")
                    .cloned()
                    .or_else(|| map.get("from").cloned())
                {
                    compat.insert("from_dataset".to_string(), from);
                }
                if let Some((metric, dataset_id)) =
                    resolve_metric_ref(&compat, resources, compiled, resource_index)
                {
                    let metric_id = compat.get("id").and_then(Value::as_str).unwrap_or("");
                    let drilldown =
                        resolve_metric_drilldown_meta(resources, &dataset_id, metric_id, compiled);
                    return with_runtime_ref(
                        serde_json::to_value(metric).unwrap_or(Value::Null),
                        scene_anchor.runtime_ref_extra(
                            "metric",
                            &dataset_id,
                            Some(metric_id),
                            drilldown.as_ref(),
                        ),
                    );
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("rows")
            {
                if let Some((dataset, dataset_id)) =
                    resolve_rows_expr(map, resources, compiled, resource_index)
                {
                    return with_runtime_ref(
                        serde_json::to_value(dataset).unwrap_or(Value::Null),
                        scene_anchor.runtime_ref_extra("data", &dataset_id, None, None),
                    );
                }
                return Value::Null;
            }
            let mut out = serde_json::Map::new();
            for (key, entry) in map {
                out.insert(
                    key.clone(),
                    resolve_value(
                        entry,
                        shared_context,
                        scene_contract,
                        resources,
                        scene_anchor,
                        resource_index,
                        compiled,
                    ),
                );
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| {
                    resolve_value(
                        item,
                        shared_context,
                        scene_contract,
                        resources,
                        scene_anchor,
                        resource_index,
                        compiled,
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn resolve_data_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<(mei_lang_kernel::DatasetView, String)> {
    let id = map.get("id").and_then(Value::as_str)?;
    let from_dataset = map.get("from_dataset").and_then(Value::as_str);
    let selector = from_dataset.unwrap_or(id);
    let dataset_id = resolve_dataset_resource_id(compiled, selector, Some(resource_index)).ok()?;
    Some((resources.get(&dataset_id)?.dataset.clone()?, dataset_id))
}

fn resolve_metric_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<(mei_lang_kernel::MetricContract, String)> {
    let metric_id = map.get("id").and_then(Value::as_str)?;
    if let Some(entry) = compiled.world_metrics.get(metric_id) {
        if let Some(from_dataset) = map.get("from_dataset").and_then(Value::as_str) {
            let dataset_id =
                resolve_dataset_resource_id(compiled, from_dataset, Some(resource_index)).ok()?;
            if dataset_id != entry.owner_resource_id {
                return None;
            }
        }
        return Some((entry.metric.clone(), entry.owner_resource_id.clone()));
    }
    if let Some(from_dataset) = map.get("from_dataset").and_then(Value::as_str) {
        let dataset_id =
            resolve_dataset_resource_id(compiled, from_dataset, Some(resource_index)).ok()?;
        let resource = resources.get(&dataset_id)?;
        let metric = resource.dataset.as_ref()?.metrics.get(metric_id).cloned()?;
        return Some((metric, dataset_id));
    }
    resources
        .iter()
        .filter_map(|(dataset_id, resource)| {
            resource
                .dataset
                .as_ref()
                .and_then(|dataset| dataset.metrics.get(metric_id).cloned())
                .map(|metric| (metric, dataset_id.clone()))
        })
        .next()
}

fn resolve_rows_expr(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<(mei_lang_kernel::DatasetView, String)> {
    let dataset = map
        .get("dataset")
        .and_then(Value::as_str)
        .map(|value| value.strip_prefix("dataset.").unwrap_or(value).to_string())?;
    let dataset_id = resolve_dataset_resource_id(compiled, &dataset, Some(resource_index)).ok()?;
    Some((resources.get(&dataset_id)?.dataset.clone()?, dataset_id))
}

fn with_runtime_ref(mut value: Value, runtime_ref: Value) -> Value {
    if let Some(map) = value.as_object_mut() {
        map.insert("__mei_runtime_ref".to_string(), runtime_ref);
    }
    value
}

fn resolve_metric_drilldown_meta(
    resources: &BTreeMap<String, LoadedResource>,
    dataset_id: &str,
    metric_id: &str,
    compiled: &CompiledApp,
) -> Option<MetricDrilldownMeta> {
    let primary = resources
        .get(dataset_id)
        .and_then(|resource| resource.dataset.as_ref())
        .and_then(|dataset| dataset.runtime_metric_defs.get(metric_id))
        .map(|definition| metric_drilldown_from_definition(definition, compiled));

    if let Some(meta) = primary.as_ref().filter(|meta| !meta.is_empty()) {
        return Some(meta.clone());
    }

    let fallback = resources
        .iter()
        .filter(|(id, _)| id.as_str() != dataset_id)
        .filter_map(|(_, resource)| resource.dataset.as_ref())
        .filter_map(|dataset| dataset.runtime_metric_defs.get(metric_id))
        .map(|definition| metric_drilldown_from_definition(definition, compiled))
        .find(|meta| !meta.is_empty());

    fallback.or(primary)
}

fn metric_drilldown_from_definition(
    definition: &Value,
    compiled: &CompiledApp,
) -> MetricDrilldownMeta {
    let mut meta = MetricDrilldownMeta::default();
    let Some(map) = definition.as_object() else {
        return meta;
    };
    for key in ["drilldown_dataset", "drilldown"] {
        let Some(value) = map.get(key) else {
            continue;
        };
        if let Some(scene) = value.as_str().map(str::trim).filter(|v| !v.is_empty()) {
            if meta.drilldown_scene.is_none() {
                meta.drilldown_scene = Some(scene.to_string());
            }
            continue;
        }
        if let Some(obj) = value.as_object() {
            apply_drilldown_object(obj, &mut meta, compiled);
        }
    }
    if let Some(explain) = map.get("explain").and_then(Value::as_object) {
        apply_explain_object(explain, &mut meta);
    }
    if let Some(analyses) = map.get("analyses") {
        apply_analyses_value(analyses, &mut meta);
    }

    if let Some(enabled) = map.get("drilldown_enabled").and_then(Value::as_bool) {
        meta.drilldown_enabled = Some(enabled);
    }
    if meta.explain_kind.is_none() {
        if let Some(kind) = map
            .get("explain_kind")
            .and_then(Value::as_str)
            .map(str::trim)
        {
            if !kind.is_empty() {
                meta.explain_kind = Some(kind.to_string());
            }
        }
    }
    if meta.drilldown_tabs.is_empty() {
        if let Some(tabs) = map.get("drilldown_tabs").or_else(|| map.get("tabs")) {
            meta.drilldown_tabs = tabs_from_value(tabs);
        }
    }
    if meta.drilldown_basis_refs.is_empty() {
        for key in ["basis_refs", "basisRefs"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let basis = string_array_from_value(value);
            if basis.is_empty() {
                continue;
            }
            meta.drilldown_basis_refs = basis;
            break;
        }
    }
    if meta.drilldown_detail_fields.is_empty() {
        for key in ["detail_fields", "detailFields"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let detail_fields = string_array_from_value(value);
            if detail_fields.is_empty() {
                continue;
            }
            meta.drilldown_detail_fields = detail_fields;
            break;
        }
    }
    if meta.drilldown_recommended_dimensions.is_empty() {
        for key in ["recommended_dimensions", "recommendedDimensions"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let dimensions = string_array_from_value(value);
            if dimensions.is_empty() {
                continue;
            }
            meta.drilldown_recommended_dimensions = dimensions;
            break;
        }
    }
    if meta.drilldown_ratio_numerator.is_none() {
        meta.drilldown_ratio_numerator = first_non_empty_string(
            map,
            &[
                "ratio_numerator",
                "ratioNumerator",
                "drilldown_ratio_numerator",
                "numerator",
            ],
        );
    }
    if meta.drilldown_ratio_denominator.is_none() {
        meta.drilldown_ratio_denominator = first_non_empty_string(
            map,
            &[
                "ratio_denominator",
                "ratioDenominator",
                "drilldown_ratio_denominator",
                "denominator",
            ],
        );
    }
    if meta.drilldown_ratio_formula.is_none() {
        meta.drilldown_ratio_formula = first_non_empty_string(
            map,
            &[
                "ratio_formula",
                "ratioFormula",
                "drilldown_ratio_formula",
                "formula",
            ],
        );
    }
    if meta.drilldown_ratio_numerator.is_none()
        || meta.drilldown_ratio_denominator.is_none()
        || meta.drilldown_ratio_formula.is_none()
    {
        for key in ["ratio_parts", "ratioParts"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            apply_ratio_parts(value, &mut meta);
            if meta.drilldown_ratio_numerator.is_some()
                || meta.drilldown_ratio_denominator.is_some()
                || meta.drilldown_ratio_formula.is_some()
            {
                break;
            }
        }
    }
    if meta.drilldown_tab_metrics.is_empty() {
        for key in ["tab_metrics", "tabMetrics", "drilldown_tab_metrics"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let tab_metrics = object_map_from_value(value);
            if tab_metrics.is_empty() {
                continue;
            }
            meta.drilldown_tab_metrics = tab_metrics;
            break;
        }
    }
    if meta.drilldown_target_scene_id.is_none() {
        if let Some(scene) = meta.drilldown_scene.as_deref() {
            meta.drilldown_target_scene_id = resolve_drilldown_target_scene_id(compiled, scene);
        }
    }
    if meta.drilldown_enabled.is_none()
        && (meta.drilldown_scene.is_some() || meta.drilldown_target_scene_id.is_some())
    {
        meta.drilldown_enabled = Some(true);
    }
    meta
}

fn apply_drilldown_object(
    map: &serde_json::Map<String, Value>,
    meta: &mut MetricDrilldownMeta,
    compiled: &CompiledApp,
) {
    if meta.drilldown_enabled.is_none() {
        meta.drilldown_enabled = map.get("enabled").and_then(Value::as_bool);
    }
    if meta.explain_kind.is_none() {
        for key in ["kind", "explain_kind", "metric_kind"] {
            let Some(value) = map.get(key).and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            meta.explain_kind = Some(value.to_string());
            break;
        }
    }
    if meta.drilldown_tabs.is_empty() {
        for key in ["tabs", "drilldown_tabs", "default_tabs"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let tabs = tabs_from_value(value);
            if tabs.is_empty() {
                continue;
            }
            meta.drilldown_tabs = tabs;
            break;
        }
    }
    if meta.drilldown_target_scene_id.is_none() {
        for key in [
            "target_scene_id",
            "target_scene",
            "drilldown_target_scene_id",
            "drilldown_scene_id",
            "scene_id",
        ] {
            let Some(value) = map.get(key).and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            meta.drilldown_target_scene_id = resolve_drilldown_target_scene_id(compiled, value)
                .or_else(|| Some(value.to_string()));
            break;
        }
    }
    if meta.drilldown_scene.is_none() {
        for key in [
            "scene_file",
            "scene",
            "scene_path",
            "drilldown_scene",
            "path",
            "file",
        ] {
            let Some(value) = map.get(key).and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            meta.drilldown_scene = Some(value.to_string());
            break;
        }
    }
    if meta.drilldown_target_scene_id.is_none() {
        if let Some(scene) = meta.drilldown_scene.as_deref() {
            meta.drilldown_target_scene_id = resolve_drilldown_target_scene_id(compiled, scene);
        }
    }
    if meta.drilldown_title.is_none() {
        meta.drilldown_title = first_non_empty_string(map, &["title", "drilldown_title", "label"]);
    }
    if meta.drilldown_note.is_none() {
        meta.drilldown_note = first_non_empty_string(map, &["note", "desc", "description"]);
    }
    if meta.drilldown_table_metric_id.is_none() {
        meta.drilldown_table_metric_id = first_non_empty_string(
            map,
            &[
                "table_metric_id",
                "tableMetricId",
                "drilldown_table_metric_id",
            ],
        );
    }
    if meta.drilldown_dataset_id.is_none() {
        meta.drilldown_dataset_id =
            first_non_empty_string(map, &["dataset_id", "datasetId", "drilldown_dataset_id"]);
    }
    if meta.drilldown_layout_preset.is_none() {
        meta.drilldown_layout_preset =
            first_non_empty_string(map, &["layout_preset", "layoutPreset"]);
    }
    if meta.drilldown_columns.is_empty() {
        for key in ["columns", "drilldown_columns"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let columns = string_array_from_value(value);
            if columns.is_empty() {
                continue;
            }
            meta.drilldown_columns = columns;
            break;
        }
    }
    if meta.drilldown_headers.is_empty() {
        for key in ["headers", "drilldown_headers"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let headers = string_array_from_value(value);
            if headers.is_empty() {
                continue;
            }
            meta.drilldown_headers = headers;
            break;
        }
    }
    if meta.drilldown_basis_refs.is_empty() {
        for key in ["basis_refs", "basisRefs"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let basis = string_array_from_value(value);
            if basis.is_empty() {
                continue;
            }
            meta.drilldown_basis_refs = basis;
            break;
        }
    }
    if meta.drilldown_detail_fields.is_empty() {
        for key in ["detail_fields", "detailFields"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let detail_fields = string_array_from_value(value);
            if detail_fields.is_empty() {
                continue;
            }
            meta.drilldown_detail_fields = detail_fields;
            break;
        }
    }
    if meta.drilldown_recommended_dimensions.is_empty() {
        for key in ["recommended_dimensions", "recommendedDimensions"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let dimensions = string_array_from_value(value);
            if dimensions.is_empty() {
                continue;
            }
            meta.drilldown_recommended_dimensions = dimensions;
            break;
        }
    }
    if meta.drilldown_ratio_numerator.is_none() {
        meta.drilldown_ratio_numerator = first_non_empty_string(
            map,
            &[
                "ratio_numerator",
                "ratioNumerator",
                "drilldown_ratio_numerator",
                "numerator",
            ],
        );
    }
    if meta.drilldown_ratio_denominator.is_none() {
        meta.drilldown_ratio_denominator = first_non_empty_string(
            map,
            &[
                "ratio_denominator",
                "ratioDenominator",
                "drilldown_ratio_denominator",
                "denominator",
            ],
        );
    }
    if meta.drilldown_ratio_formula.is_none() {
        meta.drilldown_ratio_formula = first_non_empty_string(
            map,
            &[
                "ratio_formula",
                "ratioFormula",
                "drilldown_ratio_formula",
                "formula",
            ],
        );
    }
    if meta.drilldown_ratio_numerator.is_none()
        || meta.drilldown_ratio_denominator.is_none()
        || meta.drilldown_ratio_formula.is_none()
    {
        for key in ["ratio_parts", "ratioParts"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            apply_ratio_parts(value, meta);
            if meta.drilldown_ratio_numerator.is_some()
                || meta.drilldown_ratio_denominator.is_some()
                || meta.drilldown_ratio_formula.is_some()
            {
                break;
            }
        }
    }
    if meta.drilldown_tab_metrics.is_empty() {
        for key in ["tab_metrics", "tabMetrics", "drilldown_tab_metrics"] {
            let Some(value) = map.get(key) else {
                continue;
            };
            let tab_metrics = object_map_from_value(value);
            if tab_metrics.is_empty() {
                continue;
            }
            meta.drilldown_tab_metrics = tab_metrics;
            break;
        }
    }
}

fn apply_ratio_parts(value: &Value, meta: &mut MetricDrilldownMeta) {
    let Some(parts) = value.as_object() else {
        return;
    };
    if meta.drilldown_ratio_numerator.is_none() {
        meta.drilldown_ratio_numerator = first_non_empty_string(
            parts,
            &["numerator", "numerator_label", "numeratorLabel", "top"],
        );
    }
    if meta.drilldown_ratio_denominator.is_none() {
        meta.drilldown_ratio_denominator = first_non_empty_string(
            parts,
            &[
                "denominator",
                "denominator_label",
                "denominatorLabel",
                "bottom",
            ],
        );
    }
    if meta.drilldown_ratio_formula.is_none() {
        meta.drilldown_ratio_formula =
            first_non_empty_string(parts, &["formula", "expr", "expression"]);
    }
}

fn first_non_empty_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(value) = map.get(*key).and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        return Some(value.to_string());
    }
    None
}

fn string_array_from_value(value: &Value) -> Vec<String> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str().map(str::trim))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn object_map_from_value(value: &Value) -> serde_json::Map<String, Value> {
    let Some(map) = value.as_object() else {
        return serde_json::Map::new();
    };
    map.clone()
}

fn normalize_analysis_tab_id(value: &str) -> Option<String> {
    let raw = value.trim().to_lowercase();
    if raw.is_empty() {
        return None;
    }
    let tab = match raw.as_str() {
        "definition" | "def" | "metric_definition" | "metric-definition" => "definition",
        "composition" | "breakdown" | "group" | "group_by" | "groupby" => "composition",
        "trend" | "timeseries" | "time_series" | "time-series" | "trend_compare" => "trend",
        "numerator_denominator" | "numerator-denominator" | "numerator" | "ratio" => {
            "numerator_denominator"
        }
        "attribution" | "reason" => "attribution",
        "detail" | "details" => "detail",
        _ => raw.as_str(),
    };
    Some(tab.to_string())
}

fn apply_explain_object(map: &serde_json::Map<String, Value>, meta: &mut MetricDrilldownMeta) {
    if meta.drilldown_enabled.is_none() {
        meta.drilldown_enabled = map
            .get("analyzable")
            .or_else(|| map.get("enabled"))
            .and_then(Value::as_bool);
    }
    if meta.explain_kind.is_none() {
        meta.explain_kind = first_non_empty_string(map, &["kind", "explain_kind", "metric_kind"]);
    }
    if meta.drilldown_note.is_none() {
        meta.drilldown_note = first_non_empty_string(map, &["note", "desc", "description"]);
    }
    if meta.drilldown_basis_refs.is_empty() {
        if let Some(value) = map.get("basis_refs").or_else(|| map.get("basisRefs")) {
            meta.drilldown_basis_refs = string_array_from_value(value);
        }
    }
    if meta.drilldown_detail_fields.is_empty() {
        if let Some(value) = map.get("detail_fields").or_else(|| map.get("detailFields")) {
            meta.drilldown_detail_fields = string_array_from_value(value);
        }
    }
    if meta.drilldown_recommended_dimensions.is_empty() {
        if let Some(value) = map
            .get("recommended_dimensions")
            .or_else(|| map.get("recommendedDimensions"))
        {
            meta.drilldown_recommended_dimensions = string_array_from_value(value);
        }
    }
    if meta.explain_metrics.is_empty() {
        if let Some(value) = map.get("metrics") {
            let metrics = object_map_from_value(value);
            if !metrics.is_empty() {
                meta.explain_metrics = metrics.clone();
                if meta.drilldown_tabs.is_empty() {
                    meta.drilldown_tabs = metrics.keys().cloned().collect();
                }
            }
        }
    }
    if meta.explain_composition_by.is_empty() {
        if let Some(value) = map
            .get("composition_by")
            .or_else(|| map.get("compositionBy"))
        {
            meta.explain_composition_by = string_array_from_value(value);
        }
    }
    if meta.explain_trend_field.is_none() {
        meta.explain_trend_field = first_non_empty_string(
            map,
            &["trend_field", "trendField", "date_field", "dateField"],
        );
    }
    if meta.explain_trend_grain.is_none() {
        meta.explain_trend_grain =
            first_non_empty_string(map, &["trend_grain", "trendGrain", "trend_by", "trendBy"]);
    }
    if meta.explain_detail_dataset.is_none() {
        meta.explain_detail_dataset = first_non_empty_string(
            map,
            &["detail_dataset", "detailDataset", "dataset_id", "datasetId"],
        );
    }
    if meta.drilldown_ratio_numerator.is_none()
        || meta.drilldown_ratio_denominator.is_none()
        || meta.drilldown_ratio_formula.is_none()
    {
        if let Some(value) = map.get("ratio_parts").or_else(|| map.get("ratioParts")) {
            apply_ratio_parts(value, meta);
        }
    }
}

fn apply_analyses_value(value: &Value, meta: &mut MetricDrilldownMeta) {
    let Some(items) = value.as_array() else {
        return;
    };
    for item in items {
        let Some(entry) = item.as_object() else {
            continue;
        };
        let kind = first_non_empty_string(entry, &["kind", "type", "id"])
            .and_then(|value| normalize_analysis_tab_id(&value));
        let Some(tab_id) = kind else {
            continue;
        };
        if !meta.drilldown_tabs.contains(&tab_id) {
            meta.drilldown_tabs.push(tab_id.clone());
        }
        let override_obj = build_analysis_override_object(entry);
        if !override_obj.is_empty() {
            meta.drilldown_tab_metrics
                .insert(tab_id, Value::Object(override_obj));
        }
    }
}

fn build_analysis_override_object(
    entry: &serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    let mut obj = serde_json::Map::new();
    if let Some(table_metric_id) = first_non_empty_string(
        entry,
        &["table_metric_id", "tableMetricId", "metric_id", "metricId"],
    ) {
        obj.insert(
            "table_metric_id".to_string(),
            Value::String(table_metric_id),
        );
    }
    if let Some(dataset_id) = first_non_empty_string(entry, &["dataset_id", "datasetId"]) {
        obj.insert("dataset_id".to_string(), Value::String(dataset_id));
    }
    if let Some(columns) = entry
        .get("columns")
        .or_else(|| entry.get("detail_fields"))
        .or_else(|| entry.get("detailFields"))
    {
        let values = string_array_from_value(columns);
        if !values.is_empty() {
            obj.insert(
                "columns".to_string(),
                Value::Array(values.into_iter().map(Value::String).collect()),
            );
        }
    }
    if let Some(headers) = entry.get("headers") {
        let values = string_array_from_value(headers);
        if !values.is_empty() {
            obj.insert(
                "headers".to_string(),
                Value::Array(values.into_iter().map(Value::String).collect()),
            );
        }
    }
    if let Some(mapping) = entry.get("mapping").and_then(Value::as_object) {
        obj.insert("mapping".to_string(), Value::Object(mapping.clone()));
    }
    if let Some(chart_kind) = first_non_empty_string(entry, &["chart_kind", "chartKind", "chart"]) {
        obj.insert("chart_kind".to_string(), Value::String(chart_kind));
    }
    obj
}

fn tabs_from_value(value: &Value) -> Vec<String> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if let Some(value) = item.as_str().map(str::trim).filter(|v| !v.is_empty()) {
                return Some(value.to_string());
            }
            let map = item.as_object()?;
            for key in ["id", "tab", "key", "name"] {
                let Some(value) = map.get(key).and_then(Value::as_str).map(str::trim) else {
                    continue;
                };
                if value.is_empty() {
                    continue;
                }
                return Some(value.to_string());
            }
            None
        })
        .collect()
}

fn normalize_scene_selector(raw: &str) -> String {
    raw.trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn resolve_drilldown_target_scene_id(compiled: &CompiledApp, selector: &str) -> Option<String> {
    let normalized = normalize_scene_selector(selector);
    if normalized.is_empty() {
        return None;
    }
    if let Some(route) = compiled
        .scene_routes
        .iter()
        .find(|route| route.scene_id.trim() == normalized)
    {
        return Some(route.scene_id.clone());
    }
    compiled
        .scene_routes
        .iter()
        .find(|route| normalize_scene_selector(&route.target_file) == normalized)
        .map(|route| route.scene_id.clone())
}
