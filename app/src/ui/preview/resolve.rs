use std::collections::{BTreeMap, BTreeSet};

use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_dataset_resource_id, resolve_dataset_selector_value,
    resolve_runtime_metric_def_key, scene_payload_cache_epoch, CompiledApp, LoadedResource,
    RuntimeResourceIndex, SceneContract,
};
use serde_json::{json, Value};

use super::theme::resolve_shared_refs;

/// Controls whether nested popup/board_link bindings stay as authored refs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BindingResolveContext {
    #[default]
    Normal,
    PopupPayload,
}

fn external_scene_locator(map: &serde_json::Map<String, Value>) -> bool {
    map.get("__ref").and_then(Value::as_str) == Some("scene")
        && (map.contains_key("scene_file") || map.contains_key("scene_id"))
}

fn preserve_popup_binding(value: &Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    match map.get("__ref").and_then(Value::as_str) {
        Some("scene") => external_scene_locator(map),
        Some("metric") | Some("data") | Some("explain_metric") => true,
        _ => false,
    }
}

/// Scene anchor injected into `__mei_runtime_ref` for scene-qualified runtime APIs.
#[derive(Debug, Clone)]
pub(super) struct RuntimeSceneAnchor {
    pub scene_id: String,
    pub scene_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct MetricDrilldownMeta {
    analysis_contract: Option<Value>,
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
    explain_metrics: Vec<Value>,
    analysis_nodes: Vec<Value>,
    analysis_blocks: Vec<Value>,
    analysis_objects: serde_json::Map<String, Value>,
    explain_composition_by: Vec<String>,
    explain_trend_field: Option<String>,
    explain_trend_grain: Option<String>,
    explain_detail_dataset: Option<String>,
    legacy_drilldown_fallback: bool,
}

impl MetricDrilldownMeta {
    fn is_empty(&self) -> bool {
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

    fn has_explain_semantics(&self) -> bool {
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
            // Host consumers only receive the derived analysis contract. Legacy drilldown
            // compatibility stays folded inside preview resolution and is never re-exposed.
            if let Some(contract) = meta.analysis_contract.as_ref() {
                obj.insert("analysis_contract".to_string(), contract.clone());
            }
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
                "runtime_capabilities": {
                    "rows_query": {
                        "enabled": true,
                        "api": format!("/api/datasets/query/{}", app_path),
                        "scene_qualified": true,
                    },
                    "metric_query": {
                        "enabled": true,
                        "api": format!("/api/datasets/metrics/{}", app_path),
                        "scene_qualified": true,
                    },
                },
                "components": theme_components.clone(),
                "shared": shared_context.clone(),
                "scene_local_nav_by_target": compiled.scene_local_nav_by_target.clone(),
                "scene_bindings_by_id": compiled.scene_bindings_by_id.clone(),
                "scene_examples_by_id": compiled.scene_examples_by_id.clone(),
                "scene_projection_assembly_by_id": compiled.scene_projection_assembly_by_id.clone(),
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
    resolve_value_in_context(
        value,
        shared_context,
        scene_contract,
        resources,
        scene_anchor,
        resource_index,
        compiled,
        BindingResolveContext::Normal,
    )
}

fn resolve_value_in_context(
    value: &Value,
    shared_context: &Value,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
    scene_anchor: &RuntimeSceneAnchor,
    resource_index: &RuntimeResourceIndex,
    compiled: &CompiledApp,
    binding_context: BindingResolveContext,
) -> Value {
    if binding_context == BindingResolveContext::PopupPayload && preserve_popup_binding(value) {
        return value.clone();
    }
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
                if external_scene_locator(map) {
                    return value.clone();
                }
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
                    let drilldown = resolve_metric_drilldown_meta(
                        resources,
                        &dataset_id,
                        metric_id,
                        compiled,
                        resource_index,
                    );
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
                    let drilldown = resolve_metric_drilldown_meta(
                        resources,
                        &dataset_id,
                        metric_id,
                        compiled,
                        resource_index,
                    );
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
                let child_context = match binding_context {
                    BindingResolveContext::PopupPayload => BindingResolveContext::PopupPayload,
                    BindingResolveContext::Normal
                        if matches!(key.as_str(), "popup" | "analysis") =>
                    {
                        BindingResolveContext::PopupPayload
                    }
                    _ => BindingResolveContext::Normal,
                };
                out.insert(
                    key.clone(),
                    resolve_value_in_context(
                        entry,
                        shared_context,
                        scene_contract,
                        resources,
                        scene_anchor,
                        resource_index,
                        compiled,
                        child_context,
                    ),
                );
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| {
                    resolve_value_in_context(
                        item,
                        shared_context,
                        scene_contract,
                        resources,
                        scene_anchor,
                        resource_index,
                        compiled,
                        binding_context,
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

/// `world.add_metric` / `world(metrics=...)` 物化进 ledger 时 owner 为 `__world_metrics__`（或带路径后缀），
/// 与 `metric_ref(..., from_dataset = "<源数据集>")` 中的 lineage 提示 id 不同，不应因此拒绝解析。
fn is_scene_direct_world_metric_owner(owner_resource_id: &str) -> bool {
    owner_resource_id == "__world_metrics__"
        || owner_resource_id.starts_with("__world_metrics__::")
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
            match resolve_dataset_resource_id(compiled, from_dataset, Some(resource_index)) {
                Ok(dataset_id) => {
                    if dataset_id != entry.owner_resource_id
                        && !is_scene_direct_world_metric_owner(&entry.owner_resource_id)
                    {
                        return None;
                    }
                }
                Err(_) if !is_scene_direct_world_metric_owner(&entry.owner_resource_id) => {
                    return None;
                }
                Err(_) => {}
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
    let canonical_id = resolve_runtime_metric_def_key(&resource.id, metric_id, &dataset.runtime_metric_defs)
        .unwrap_or_else(|| metric_id.to_string());
    dataset.runtime_analysis_contracts.get(&canonical_id).cloned()
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

fn normalize_explain_source(value: &Value) -> Option<Value> {
    let map = value.as_object()?;
    match map.get("__ref").and_then(Value::as_str) {
        Some("metric") => {
            let metric_id = first_non_empty_string(map, &["id"])?;
            let mut source = serde_json::Map::new();
            source.insert("kind".to_string(), Value::String("metric_ref".to_string()));
            source.insert("metric_id".to_string(), Value::String(metric_id));
            if let Some(dataset_id) = first_non_empty_string(map, &["from_dataset"]) {
                source.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
            if let Some(scene_id) = first_non_empty_string(map, &["scene_id"]) {
                source.insert("scene_id".to_string(), Value::String(scene_id));
            }
            if let Some(scene_file) = first_non_empty_string(map, &["scene_file"]) {
                source.insert("scene_file".to_string(), Value::String(scene_file));
            }
            Some(Value::Object(source))
        }
        Some("dataset") | Some("data") => {
            let dataset_id = first_non_empty_string(map, &["id"])?;
            let mut source = serde_json::Map::new();
            source.insert("kind".to_string(), Value::String("dataset_ref".to_string()));
            source.insert("dataset_id".to_string(), Value::String(dataset_id));
            if let Some(scene_id) = first_non_empty_string(map, &["scene_id"]) {
                source.insert("scene_id".to_string(), Value::String(scene_id));
            }
            if let Some(scene_file) = first_non_empty_string(map, &["scene_file"]) {
                source.insert("scene_file".to_string(), Value::String(scene_file));
            }
            Some(Value::Object(source))
        }
        _ => Some(Value::Object(map.clone())),
    }
}

fn table_metric_id_from_source(value: &Value) -> Option<String> {
    let map = value.as_object()?;
    first_non_empty_string(map, &["table_metric_id", "metric_id"])
}

fn dataset_id_from_source(value: &Value) -> Option<String> {
    let map = value.as_object()?;
    first_non_empty_string(map, &["dataset_id"])
}

fn normalize_explain_entry_object(obj: &serde_json::Map<String, Value>) -> Option<Value> {
    let raw_kind = first_non_empty_string(obj, &["kind", "type", "id"])?;
    let kind = normalize_analysis_tab_id(&raw_kind)?;
    let id = first_non_empty_string(obj, &["id", "key", "name"])
        .and_then(|raw| normalize_analysis_tab_id(&raw))
        .unwrap_or_else(|| kind.clone());
    let mut entry = obj.clone();
    entry.insert("id".to_string(), Value::String(id));
    entry.insert("kind".to_string(), Value::String(kind.clone()));
    if let Some(source) = obj.get("source").and_then(normalize_explain_source) {
        entry.insert("source".to_string(), source.clone());
        if !entry.contains_key("table_metric_id") {
            if let Some(metric_id) = table_metric_id_from_source(&source) {
                entry.insert("table_metric_id".to_string(), Value::String(metric_id));
            }
        }
        if !entry.contains_key("dataset_id") {
            if let Some(dataset_id) = dataset_id_from_source(&source) {
                entry.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
        }
    }
    entry.insert("support_role".to_string(), Value::String(kind));
    Some(Value::Object(entry))
}

fn normalize_analysis_node_object(obj: &serde_json::Map<String, Value>) -> Option<Value> {
    let local_id = first_non_empty_string(obj, &["analysis_local_id", "key", "id"])?;
    let scoped_metric_id =
        first_non_empty_string(obj, &["analysis_scoped_id", "analysis_node_id", "key", "id"])?;
    let mut node = serde_json::Map::new();
    node.insert("id".to_string(), Value::String(local_id));
    node.insert("metric_id".to_string(), Value::String(scoped_metric_id));
    node.insert("node_kind".to_string(), Value::String("metric".to_string()));
    if let Some(shape) = first_non_empty_string(obj, &["shape"]) {
        node.insert("shape".to_string(), Value::String(shape));
    }
    if let Some(label) = first_non_empty_string(obj, &["label"]) {
        node.insert("label".to_string(), Value::String(label));
    }
    if let Some(parent_metric_id) = first_non_empty_string(obj, &["analysis_parent_metric_id"]) {
        node.insert(
            "parent_metric_id".to_string(),
            Value::String(parent_metric_id),
        );
    }
    node.insert(
        "can_explain".to_string(),
        Value::Bool(
            obj.get("explain")
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty()),
        ),
    );
    Some(Value::Object(node))
}

fn explain_metric_entries_from_value(value: &Value) -> Vec<Value> {
    let mut entries: Vec<Value> = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items {
            let Some(map) = item.as_object() else {
                continue;
            };
            let id = first_non_empty_string(map, &["id", "key", "name"])
                .and_then(|raw| normalize_analysis_tab_id(&raw));
            let kind = first_non_empty_string(map, &["kind", "type"])
                .and_then(|raw| normalize_analysis_tab_id(&raw));
            let Some(metric_id) = id.or(kind) else {
                continue;
            };
            let mut entry = map.clone();
            entry.insert("id".to_string(), Value::String(metric_id));
            entries.push(Value::Object(entry));
        }
        return entries;
    }
    if let Some(map) = value.as_object() {
        for (key, item) in map {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let mut entry = obj.clone();
            let id = first_non_empty_string(obj, &["id"])
                .and_then(|raw| normalize_analysis_tab_id(&raw))
                .or_else(|| normalize_analysis_tab_id(key))
                .unwrap_or_else(|| key.trim().to_string());
            entry.insert("id".to_string(), Value::String(id));
            entries.push(Value::Object(entry));
        }
    }
    entries
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

fn apply_explain_items(items: &[Value], meta: &mut MetricDrilldownMeta) {
    for item in items {
        let Some(map) = item.as_object() else {
            continue;
        };
        if map.get("__kind").and_then(Value::as_str) == Some("data_product") {
            if let Some(node) = normalize_analysis_node_object(map) {
                if let Some(node_id) = node
                    .as_object()
                    .and_then(|value| value.get("metric_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                {
                    meta.analysis_objects.insert(node_id, node.clone());
                }
                meta.analysis_nodes.push(node);
            }
            continue;
        }
        let raw_kind = first_non_empty_string(map, &["kind", "type", "id"]).unwrap_or_default();
        let normalized_kind =
            normalize_analysis_tab_id(&raw_kind).unwrap_or_else(|| raw_kind.trim().to_string());
        if normalized_kind == "note" {
            if meta.drilldown_note.is_none() {
                meta.drilldown_note = first_non_empty_string(
                    map,
                    &["note", "content", "text", "markdown", "md", "desc", "description"],
                );
            }
            let mut block = map.clone();
            block.insert("id".to_string(), Value::String("note".to_string()));
            block.insert("kind".to_string(), Value::String("note".to_string()));
            meta.analysis_blocks.push(Value::Object(block));
            continue;
        }
        let Some(entry) = normalize_explain_entry_object(map) else {
            continue;
        };
        let entry_obj = entry.as_object().expect("normalized explain entry");
        let tab_id = entry_obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_default();
        let kind = entry_obj
            .get("kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if !tab_id.is_empty() && !meta.drilldown_tabs.contains(&tab_id) {
            meta.drilldown_tabs.push(tab_id);
        }
        if kind == "definition" {
            if meta.drilldown_note.is_none() {
                meta.drilldown_note = first_non_empty_string(
                    entry_obj,
                    &["note", "content", "text", "markdown", "md", "desc", "description"],
                );
            }
            if meta.drilldown_basis_refs.is_empty() {
                if let Some(value) = entry_obj.get("basis_refs").or_else(|| entry_obj.get("basisRefs")) {
                    meta.drilldown_basis_refs = string_array_from_value(value);
                }
            }
            if meta.drilldown_recommended_dimensions.is_empty() {
                if let Some(value) = entry_obj
                    .get("recommended_dimensions")
                    .or_else(|| entry_obj.get("recommendedDimensions"))
                {
                    meta.drilldown_recommended_dimensions = string_array_from_value(value);
                }
            }
        } else if kind == "detail" {
            if meta.drilldown_detail_fields.is_empty() {
                if let Some(value) = entry_obj.get("fields") {
                    meta.drilldown_detail_fields = string_array_from_value(value);
                }
            }
            if meta.drilldown_headers.is_empty() {
                if let Some(value) = entry_obj.get("headers") {
                    meta.drilldown_headers = string_array_from_value(value);
                }
            }
            if meta.drilldown_table_metric_id.is_none() {
                meta.drilldown_table_metric_id = first_non_empty_string(
                    entry_obj,
                    &[
                        "table_metric_id",
                        "tableMetricId",
                        "metric_id",
                        "metricId",
                        "detail_table_metric_id",
                        "detailTableMetricId",
                    ],
                );
            }
            if meta.explain_detail_dataset.is_none() {
                meta.explain_detail_dataset = first_non_empty_string(
                    entry_obj,
                    &["detail_dataset", "detailDataset", "dataset_id", "datasetId"],
                );
            }
        } else if kind == "composition" {
            if meta.explain_composition_by.is_empty() {
                if let Some(by) = first_non_empty_string(entry_obj, &["by"]) {
                    meta.explain_composition_by = vec![by];
                }
            }
        } else if kind == "trend" {
            if meta.explain_trend_field.is_none() {
                meta.explain_trend_field =
                    first_non_empty_string(entry_obj, &["date_field", "dateField"]);
            }
            if meta.explain_trend_grain.is_none() {
                meta.explain_trend_grain = first_non_empty_string(entry_obj, &["grain"]);
            }
        } else if kind == "numerator_denominator" {
            if meta.drilldown_ratio_numerator.is_none() {
                meta.drilldown_ratio_numerator =
                    first_non_empty_string(entry_obj, &["numerator"]);
            }
            if meta.drilldown_ratio_denominator.is_none() {
                meta.drilldown_ratio_denominator =
                    first_non_empty_string(entry_obj, &["denominator"]);
            }
            if meta.drilldown_ratio_formula.is_none() {
                meta.drilldown_ratio_formula = first_non_empty_string(entry_obj, &["formula"]);
            }
        }
        meta.explain_metrics.push(entry.clone());
        meta.analysis_blocks.push(entry);
    }
    sync_explain_metric_tab_overrides(meta);
}

fn apply_explain_object(map: &serde_json::Map<String, Value>, meta: &mut MetricDrilldownMeta) {
    if meta.drilldown_note.is_none() {
        meta.drilldown_note = first_non_empty_string(map, &["note", "desc", "description"]);
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
    if meta.drilldown_basis_refs.is_empty() {
        if let Some(value) = map.get("basis_refs").or_else(|| map.get("basisRefs")) {
            meta.drilldown_basis_refs = string_array_from_value(value);
        }
    }
    if meta.drilldown_table_metric_id.is_none() {
        meta.drilldown_table_metric_id = first_non_empty_string(
            map,
            &[
                "detail_table_metric_id",
                "detailTableMetricId",
                "detail_metric_id",
                "detailMetricId",
                "table_metric_id",
                "tableMetricId",
            ],
        );
    }
    let uses_table_metric = meta
        .drilldown_table_metric_id
        .as_deref()
        .is_some_and(|value| !value.is_empty());
    if meta.explain_detail_dataset.is_none() {
        meta.explain_detail_dataset = first_non_empty_string(
            map,
            &["detail_dataset", "detailDataset", "dataset_id", "datasetId"],
        );
    }
    if meta.drilldown_dataset_id.is_none() && !uses_table_metric {
        meta.drilldown_dataset_id = meta
            .explain_detail_dataset
            .clone()
            .or_else(|| {
                first_non_empty_string(
                    map,
                    &["dataset_id", "datasetId", "drilldown_dataset_id"],
                )
            });
    }
    if meta.explain_metrics.is_empty() {
        if let Some(value) = map.get("metrics") {
            let metrics = explain_metric_entries_from_value(value);
            if !metrics.is_empty() {
                meta.explain_metrics = metrics.clone();
                if meta.drilldown_tabs.is_empty() {
                    meta.drilldown_tabs = metrics
                        .iter()
                        .filter_map(|entry| {
                            entry
                                .as_object()
                                .and_then(|obj| obj.get("id"))
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|id| !id.is_empty())
                                .map(str::to_string)
                        })
                        .collect();
                }
            }
        }
    }
    sync_explain_metric_tab_overrides(meta);
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
    if meta.drilldown_ratio_numerator.is_none()
        || meta.drilldown_ratio_denominator.is_none()
        || meta.drilldown_ratio_formula.is_none()
    {
        if let Some(value) = map.get("ratio_parts").or_else(|| map.get("ratioParts")) {
            apply_ratio_parts(value, meta);
        }
    }
    if meta.analysis_blocks.is_empty() {
        if let Some(note) = meta.drilldown_note.as_deref().filter(|value| !value.is_empty()) {
            meta.analysis_blocks.push(json!({
                "__kind": "explain_item",
                "id": "note",
                "kind": "note",
                "note": note,
                "content": note,
                "format": "text",
            }));
        }
        for entry in &meta.explain_metrics {
            meta.analysis_blocks.push(entry.clone());
        }
    }
}

/// 将 `explain_metric` 上的 `by` / `date_field` 同步到 `drilldown_tab_metrics`，供 popup 派生查询使用。
fn sync_explain_metric_tab_overrides(meta: &mut MetricDrilldownMeta) {
    for entry in &meta.explain_metrics {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        let Some(tab_id) = obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let normalized_source = obj.get("source").and_then(normalize_explain_source);
        let mut override_obj = serde_json::Map::new();
        if kind == "detail" {
            let detail_table_metric_id = first_non_empty_string(
                obj,
                &[
                    "table_metric_id",
                    "tableMetricId",
                    "metric_id",
                    "metricId",
                    "detail_table_metric_id",
                    "detailTableMetricId",
                    "detail_metric_id",
                    "detailMetricId",
                ],
            )
            .or_else(|| {
                meta.drilldown_table_metric_id
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            });
            let detail_table_metric_id = detail_table_metric_id
                .or_else(|| normalized_source.as_ref().and_then(table_metric_id_from_source));
            if let Some(table_metric_id) = detail_table_metric_id {
                override_obj.insert(
                    "table_metric_id".to_string(),
                    Value::String(table_metric_id),
                );
            }
            let detail_dataset_id = first_non_empty_string(
                obj,
                &[
                    "detail_dataset",
                    "detailDataset",
                    "dataset_id",
                    "datasetId",
                ],
            )
            .or_else(|| {
                meta.drilldown_dataset_id
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            });
            let detail_dataset_id = detail_dataset_id
                .or_else(|| normalized_source.as_ref().and_then(dataset_id_from_source));
            if let Some(dataset_id) = detail_dataset_id {
                override_obj.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
        } else if kind == "composition" {
            if let Some(by) = obj.get("by").and_then(Value::as_str).map(str::trim) {
                if !by.is_empty() {
                    override_obj.insert(
                        "composition_by".to_string(),
                        Value::Array(vec![Value::String(by.to_string())]),
                    );
                }
            }
            if let Some(table_metric_id) = first_non_empty_string(
                obj,
                &[
                    "table_metric_id",
                    "tableMetricId",
                    "metric_id",
                    "metricId",
                ],
            )
            .or_else(|| normalized_source.as_ref().and_then(table_metric_id_from_source))
            {
                override_obj.insert(
                    "table_metric_id".to_string(),
                    Value::String(table_metric_id),
                );
            }
            if let Some(dataset_id) =
                first_non_empty_string(obj, &["dataset_id", "datasetId"])
                    .or_else(|| normalized_source.as_ref().and_then(dataset_id_from_source))
            {
                override_obj.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
        } else if kind == "trend" {
            if let Some(field) = first_non_empty_string(obj, &["date_field", "dateField"]) {
                override_obj.insert("trend_field".to_string(), Value::String(field));
            }
            if let Some(grain) = first_non_empty_string(obj, &["grain"]) {
                override_obj.insert("trend_grain".to_string(), Value::String(grain));
            }
            if let Some(table_metric_id) = first_non_empty_string(
                obj,
                &[
                    "table_metric_id",
                    "tableMetricId",
                    "metric_id",
                    "metricId",
                ],
            )
            .or_else(|| normalized_source.as_ref().and_then(table_metric_id_from_source))
            {
                override_obj.insert(
                    "table_metric_id".to_string(),
                    Value::String(table_metric_id),
                );
            }
            if let Some(dataset_id) =
                first_non_empty_string(obj, &["dataset_id", "datasetId"])
                    .or_else(|| normalized_source.as_ref().and_then(dataset_id_from_source))
            {
                override_obj.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
        }
        if let Some(source) = normalized_source {
            override_obj.insert("source".to_string(), source);
        }
        if override_obj.is_empty() {
            continue;
        }
        match meta.drilldown_tab_metrics.get_mut(tab_id) {
            Some(Value::Object(existing)) => {
                for (key, value) in override_obj {
                    existing.insert(key, value);
                }
            }
            _ => {
                meta.drilldown_tab_metrics
                    .insert(tab_id.to_string(), Value::Object(override_obj));
            }
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
