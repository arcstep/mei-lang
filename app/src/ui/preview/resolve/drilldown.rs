use std::collections::{BTreeMap, BTreeSet};

use mei_lang_kernel::{
    resolve_dataset_resource_id, resolve_runtime_metric_def_key, CompiledApp, LoadedResource,
    RuntimeResourceIndex,
};
use serde_json::Value;

use super::drilldown_apply::{
    apply_drilldown_object, apply_metric_narrative, resolve_drilldown_target_scene_id,
};
use super::explain::{
    apply_analyses_value, apply_explain_items, apply_explain_object, object_map_from_value,
    string_array_from_value,
};

pub(crate) use super::drilldown_apply::{
    apply_ratio_parts, first_non_empty_string, metric_note_text, tabs_from_value,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct MetricDrilldownMeta {
    pub(crate) analysis_contract: Option<Value>,
    pub(crate) drilldown_scene: Option<String>,
    pub(crate) drilldown_target_scene_id: Option<String>,
    pub(crate) drilldown_enabled: Option<bool>,
    pub(crate) explain_kind: Option<String>,
    pub(crate) drilldown_tabs: Vec<String>,
    pub(crate) drilldown_title: Option<String>,
    pub(crate) drilldown_note: Option<String>,
    pub(crate) drilldown_table_metric_id: Option<String>,
    pub(crate) drilldown_dataset_id: Option<String>,
    pub(crate) drilldown_layout_preset: Option<String>,
    pub(crate) drilldown_columns: Vec<String>,
    pub(crate) drilldown_headers: Vec<String>,
    pub(crate) drilldown_basis_refs: Vec<String>,
    pub(crate) drilldown_detail_fields: Vec<String>,
    pub(crate) drilldown_recommended_dimensions: Vec<String>,
    pub(crate) drilldown_ratio_numerator: Option<String>,
    pub(crate) drilldown_ratio_denominator: Option<String>,
    pub(crate) drilldown_ratio_formula: Option<String>,
    pub(crate) drilldown_tab_metrics: serde_json::Map<String, Value>,
    pub(crate) explain_metrics: Vec<Value>,
    pub(crate) analysis_nodes: Vec<Value>,
    pub(crate) analysis_blocks: Vec<Value>,
    pub(crate) analysis_objects: serde_json::Map<String, Value>,
    pub(crate) explain_composition_by: Vec<String>,
    pub(crate) explain_trend_field: Option<String>,
    pub(crate) explain_trend_grain: Option<String>,
    pub(crate) explain_detail_dataset: Option<String>,
    pub(crate) legacy_drilldown_fallback: bool,
}

impl MetricDrilldownMeta {
    pub(crate) fn is_empty(&self) -> bool {
        self.analysis_contract.is_none()
            && self.drilldown_scene.is_none()
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
            && self.analysis_nodes.is_empty()
            && self.analysis_blocks.is_empty()
            && self.analysis_objects.is_empty()
            && self.explain_composition_by.is_empty()
            && self.explain_trend_field.is_none()
            && self.explain_trend_grain.is_none()
            && self.explain_detail_dataset.is_none()
            && !self.legacy_drilldown_fallback
    }

    pub(crate) fn has_explain_semantics(&self) -> bool {
        self.analysis_contract.is_some()
            || !self.explain_metrics.is_empty()
            || !self.analysis_nodes.is_empty()
            || !self.analysis_blocks.is_empty()
            || !self.analysis_objects.is_empty()
            || self.explain_kind.is_some()
            || self.drilldown_note.is_some()
            || !self.drilldown_tabs.is_empty()
            || !self.explain_composition_by.is_empty()
            || self.explain_trend_field.is_some()
            || self.explain_trend_grain.is_some()
            || self.explain_detail_dataset.is_some()
    }
}
pub(crate) fn resolve_metric_drilldown_meta(
    resources: &BTreeMap<String, LoadedResource>,
    dataset_id: &str,
    metric_id: &str,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<MetricDrilldownMeta> {
    if let Some(contract) = lookup_runtime_analysis_contract(resources, dataset_id, metric_id) {
        let mut meta = MetricDrilldownMeta::default();
        meta.analysis_contract = Some(contract);
        return Some(meta);
    }
    let primary = resources
        .get(dataset_id)
        .and_then(|resource| resource.dataset.as_ref())
        .and_then(|dataset| dataset.runtime_metric_defs.get(metric_id))
        .map(|definition| {
            (
                metric_drilldown_from_definition(definition, compiled),
                infer_primary_metric_dataset_id(definition, compiled, resource_index),
            )
        });

    let primary = primary.map(|(mut meta, inferred_dataset_id)| {
        let uses_table_metric = meta
            .drilldown_table_metric_id
            .as_deref()
            .is_some_and(|value| !value.is_empty());
        let row_dataset_id = inferred_dataset_id
            .as_deref()
            .unwrap_or(dataset_id)
            .to_string();
        if meta.explain_detail_dataset.is_none() && !uses_table_metric {
            meta.explain_detail_dataset = Some(row_dataset_id.clone());
        }
        meta.drilldown_dataset_id = Some(if uses_table_metric {
            dataset_id.to_string()
        } else {
            meta.explain_detail_dataset
                .clone()
                .unwrap_or(row_dataset_id)
        });
        meta
    });

    if let Some(meta) = primary.as_ref().filter(|meta| !meta.is_empty()) {
        return Some(meta.clone());
    }

    let fallback = resources
        .iter()
        .filter(|(id, _)| id.as_str() != dataset_id)
        .filter_map(|(fallback_dataset_id, resource)| {
            resource
                .dataset
                .as_ref()
                .and_then(|dataset| dataset.runtime_metric_defs.get(metric_id))
                .map(|definition| {
                    (
                        fallback_dataset_id.clone(),
                        definition,
                        infer_primary_metric_dataset_id(definition, compiled, resource_index),
                    )
                })
        })
        .map(|(fallback_dataset_id, definition, inferred_dataset_id)| {
            let mut meta = metric_drilldown_from_definition(definition, compiled);
            let uses_table_metric = meta
                .drilldown_table_metric_id
                .as_deref()
                .is_some_and(|value| !value.is_empty());
            let row_dataset_id = inferred_dataset_id.unwrap_or_else(|| fallback_dataset_id.clone());
            if meta.explain_detail_dataset.is_none() && !uses_table_metric {
                meta.explain_detail_dataset = Some(row_dataset_id.clone());
            }
            meta.drilldown_dataset_id = Some(if uses_table_metric {
                fallback_dataset_id.clone()
            } else {
                meta.explain_detail_dataset
                    .clone()
                    .unwrap_or(row_dataset_id)
            });
            meta
        })
        .find(|meta| !meta.is_empty());

    fallback.or(primary)
}

fn lookup_runtime_analysis_contract(
    resources: &BTreeMap<String, LoadedResource>,
    dataset_id: &str,
    metric_id: &str,
) -> Option<Value> {
    let metric_id = metric_id.trim();
    if metric_id.is_empty() {
        return None;
    }
    let resource = resources.get(dataset_id)?;
    let dataset = resource.dataset.as_ref()?;
    let canonical_id =
        resolve_runtime_metric_def_key(&resource.id, metric_id, &dataset.runtime_metric_defs)
            .unwrap_or_else(|| metric_id.to_string());
    dataset
        .runtime_analysis_contracts
        .get(&canonical_id)
        .cloned()
}

fn infer_primary_metric_dataset_id(
    definition: &Value,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<String> {
    let mut selectors = BTreeSet::new();
    collect_metric_rowset_dataset_selectors(definition, &mut selectors);
    if selectors.len() != 1 {
        return None;
    }
    let selector = selectors.into_iter().next()?;
    resolve_dataset_resource_id(compiled, &selector, Some(resource_index)).ok()
}

fn collect_metric_rowset_dataset_selectors(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("rows")
            {
                if let Some(dataset) = map
                    .get("dataset")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    out.insert(dataset.to_string());
                }
            }
            for child in map.values() {
                collect_metric_rowset_dataset_selectors(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_metric_rowset_dataset_selectors(child, out);
            }
        }
        _ => {}
    }
}

fn metric_drilldown_from_definition(
    definition: &Value,
    compiled: &CompiledApp,
) -> MetricDrilldownMeta {
    let mut meta = MetricDrilldownMeta::default();
    let Some(map) = definition.as_object() else {
        return meta;
    };
    apply_metric_narrative(map, &mut meta);
    let has_drilldown = map.get("drilldown_dataset").is_some() || map.get("drilldown").is_some();
    if let Some(explain_items) = map.get("explain").and_then(Value::as_array) {
        apply_explain_items(explain_items, &mut meta);
    } else if let Some(explain) = map.get("explain").and_then(Value::as_object) {
        apply_explain_object(explain, &mut meta);
    }
    let has_explain = meta.has_explain_semantics();
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
    if let Some(analyses) = map.get("analyses") {
        apply_analyses_value(analyses, &mut meta);
    }
    if has_drilldown && !has_explain {
        meta.legacy_drilldown_fallback = true;
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
