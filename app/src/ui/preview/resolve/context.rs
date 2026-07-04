use std::collections::BTreeMap;

use mei_lang_kernel::{
    catalog_scene_routes_from_app_root, dataset_materialize_cache_epoch,
    host_runtime_capabilities_catalog, host_runtime_contract_descriptor, is_stock_catalog_app,
    load_cache_generation, load_mei_config_for_app, resolve_dataset_selector_value,
    scene_payload_cache_epoch, CompiledApp, CompiledSceneRoute, LoadedResource,
    RuntimeResourceIndex, SceneContract,
};
use serde_json::{json, Value};

use super::super::theme::resolve_shared_refs;

use super::drilldown::resolve_metric_drilldown_meta;
use super::drilldown::MetricDrilldownMeta;
use super::host_ssr_payload::{dataset_for_host_ssr, metric_for_host_ssr};
use super::refs::{normalize_v2_metric_ref, resolve_data_ref, resolve_metric_ref, resolve_rows_expr, with_runtime_ref};

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
pub(crate) struct RuntimeSceneAnchor {
    pub scene_id: String,
    pub scene_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HostMetaOptions {
    pub include_scene_drilldown_context: bool,
    /// Build/App/Access SSR：`_mei.runtime_capabilities` 改由 `#mei-host-runtime-capabilities` 全局注入。
    pub host_ssr_slim_payload: bool,
    pub data_mode: Option<String>,
}

pub(crate) fn host_runtime_capabilities_value(app_path: &str, data_mode: Option<&str>) -> Value {
    let mode = data_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("eval");
    let eval_enabled = mode == "eval";
    let fixture_enabled = mode == "fixture";
    json!({
        "data_mode": mode,
        "rows_query": {
            "enabled": eval_enabled,
            "api": format!("/api/datasets/query/{}", app_path),
            "scene_qualified": true,
        },
        "fixture_query": {
            "enabled": fixture_enabled,
            "api": format!("/api/datasets/fixture/{}", app_path),
            "scene_qualified": true,
        },
        "metric_query": {
            "enabled": eval_enabled || fixture_enabled,
            "api": if fixture_enabled {
                format!("/api/datasets/fixture/{}", app_path)
            } else {
                format!("/api/datasets/metrics/{}", app_path)
            },
            "scene_qualified": true,
        },
        "metric_batch_query": {
            "enabled": eval_enabled || fixture_enabled,
            "api": if fixture_enabled {
                format!("/api/datasets/fixture/{}", app_path)
            } else {
                format!("/api/datasets/metrics/{}", app_path)
            },
            "scene_qualified": true,
        },
        "catalog": host_runtime_capabilities_catalog(),
        "host_contract": host_runtime_contract_descriptor(),
    })
}

impl RuntimeSceneAnchor {
    /// Prefer catalog `scene_routes` entry matching the preview target over inner scene ids
    /// (pack previews declare `scene(id = "home")` but catalog routes use `chart.pie`, etc.).
    pub fn for_preview(
        compiled: &CompiledApp,
        preview_scene_path: Option<&str>,
        fallback_scene_id: Option<&str>,
    ) -> Self {
        let path = preview_scene_path
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(compiled.active_target_file.as_str());
        let route_scene_id = |routes: &[CompiledSceneRoute]| {
            routes
                .iter()
                .find(|route| route.target_file == path)
                .map(|route| route.scene_id.clone())
        };
        let mut anchor = Self::from_compiled(compiled);
        if let Some(scene_id) = route_scene_id(&compiled.scene_routes) {
            anchor.scene_id = scene_id;
        } else if is_stock_catalog_app(compiled.app_id.as_str()) {
            let app_root = std::path::Path::new(compiled.app_root.as_str());
            if let Some(scene_id) = route_scene_id(&catalog_scene_routes_from_app_root(app_root)) {
                anchor.scene_id = scene_id;
            }
        } else if let Some(fallback) =
            fallback_scene_id.map(str::trim).filter(|value| !value.is_empty())
        {
            if anchor.scene_id == "default" {
                anchor.scene_id = fallback.to_string();
            }
        }
        anchor.scene_path = Some(path.to_string());
        anchor
    }

    pub fn from_compiled(compiled: &CompiledApp) -> Self {
        let target = compiled.active_target_file.trim();
        let route_scene = compiled
            .scene_routes
            .iter()
            .find(|route| route.target_file == target)
            .map(|route| route.scene_id.clone());
        let scene_id = route_scene
            .or_else(|| {
                compiled
                    .active_scene
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "default".to_string());
        let scene_path = if target.is_empty() {
            None
        } else {
            Some(target.to_string())
        };
        Self {
            scene_id,
            scene_path,
        }
    }

    pub(crate) fn runtime_ref_extra(
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

pub(crate) fn attach_host_meta(
    mut props: Value,
    compiled: &CompiledApp,
    app_path: &str,
    theme_components: &serde_json::Value,
    preview_scene_path: Option<&str>,
    options: HostMetaOptions,
) -> Value {
    let anchor = RuntimeSceneAnchor::for_preview(compiled, preview_scene_path, None);
    let active_target_file = anchor
        .scene_path
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(compiled.active_target_file.as_str());
    if let Some(map) = props.as_object_mut() {
        let mut host_meta = serde_json::Map::new();
        host_meta.insert("app_id".to_string(), Value::String(compiled.app_id.clone()));
        host_meta.insert("app_path".to_string(), Value::String(app_path.to_string()));
        host_meta.insert(
            "active_scene_id".to_string(),
            Value::String(anchor.scene_id.clone()),
        );
        host_meta.insert(
            "active_target_file".to_string(),
            Value::String(active_target_file.to_string()),
        );
        host_meta.insert(
            "entry_target".to_string(),
            Value::String(active_target_file.to_string()),
        );
        host_meta.insert(
            "compile_epoch".to_string(),
            Value::String(format!(
                "{}|{}|{}",
                scene_payload_cache_epoch(),
                dataset_materialize_cache_epoch(),
                active_target_file
            )),
        );
        host_meta.insert(
            "data_generation".to_string(),
            Value::String(
                load_cache_generation(
                    std::path::Path::new(compiled.app_root.as_str()),
                    compiled.app_id.as_str(),
                )
                .data_generation,
            ),
        );
        let config = load_mei_config_for_app(
            std::path::Path::new(compiled.app_root.as_str()),
            None,
        );
        host_meta.insert(
            "client_query_cache".to_string(),
            json!({
                "persist": config.runtime.client_query_cache.persist,
                "ttl_ms": config.runtime.client_query_cache.ttl_ms,
                "max_entries": config.runtime.client_query_cache.max_entries,
            }),
        );
        host_meta.insert(
            "step_api".to_string(),
            Value::String(format!("/api/sim/step/{}", app_path)),
        );
        if !options.host_ssr_slim_payload {
            host_meta.insert(
                "runtime_capabilities".to_string(),
                host_runtime_capabilities_value(
                    app_path,
                    options.data_mode.as_deref(),
                ),
            );
        }
        if let Some(mode) = options.data_mode.as_deref() {
            host_meta.insert("data_mode".to_string(), Value::String(mode.to_string()));
        }
        host_meta.insert("components".to_string(), theme_components.clone());
        if options.include_scene_drilldown_context {
            host_meta.insert(
                "scene_local_nav_by_target".to_string(),
                json!(compiled.scene_local_nav_by_target),
            );
            host_meta.insert(
                "scene_bindings_by_id".to_string(),
                json!(compiled.scene_bindings_by_id),
            );
            host_meta.insert(
                "scene_examples_by_id".to_string(),
                json!(compiled.scene_examples_by_id),
            );
            host_meta.insert(
                "scene_projection_assembly_by_id".to_string(),
                json!(compiled.scene_projection_assembly_by_id),
            );
        }
        map.insert("_mei".to_string(), Value::Object(host_meta));
    }
    props
}

pub(crate) fn resolve_value(
    value: &Value,
    shared_context: &Value,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
    scene_anchor: &RuntimeSceneAnchor,
    resource_index: &RuntimeResourceIndex,
    compiled: &CompiledApp,
    host_ssr_slim_payload: bool,
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
        host_ssr_slim_payload,
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
    host_ssr_slim_payload: bool,
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
                            let payload = if host_ssr_slim_payload {
                                dataset_for_host_ssr(dataset)
                            } else {
                                serde_json::to_value(dataset).unwrap_or(Value::Null)
                            };
                            return with_runtime_ref(
                                payload,
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
                    let payload = if host_ssr_slim_payload {
                        dataset_for_host_ssr(&dataset)
                    } else {
                        serde_json::to_value(&dataset).unwrap_or(Value::Null)
                    };
                    return with_runtime_ref(
                        payload,
                        scene_anchor.runtime_ref_extra("data", &dataset_id, None, None),
                    );
                }
                return Value::Null;
            }
            if map.get("__ref").and_then(Value::as_str) == Some("metric_ref") {
                if let Some(v1_ref) = normalize_v2_metric_ref(value) {
                    return resolve_value(
                        &v1_ref,
                        shared_context,
                        scene_contract,
                        resources,
                        scene_anchor,
                        resource_index,
                        compiled,
                        host_ssr_slim_payload,
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
                        if host_ssr_slim_payload {
                            metric_for_host_ssr(&metric)
                        } else {
                            serde_json::to_value(&metric).unwrap_or(Value::Null)
                        },
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
                        if host_ssr_slim_payload {
                            metric_for_host_ssr(&metric)
                        } else {
                            serde_json::to_value(&metric).unwrap_or(Value::Null)
                        },
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
                    let payload = if host_ssr_slim_payload {
                        dataset_for_host_ssr(&dataset)
                    } else {
                        serde_json::to_value(&dataset).unwrap_or(Value::Null)
                    };
                    return with_runtime_ref(
                        payload,
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
                        host_ssr_slim_payload,
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
                        host_ssr_slim_payload,
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}
