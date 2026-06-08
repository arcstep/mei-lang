use mei_lang_kernel::{CompiledApp};
use serde_json::Value;

use super::drilldown::MetricDrilldownMeta;
use super::explain::{object_map_from_value, string_array_from_value};

pub(super) fn apply_drilldown_object(
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

pub(crate) fn apply_ratio_parts(value: &Value, meta: &mut MetricDrilldownMeta) {
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

pub(crate) fn first_non_empty_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
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

pub(crate) fn metric_note_text(value: &Value) -> Option<String> {
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return Some(text.to_string());
    }
    value
        .as_object()
        .and_then(|map| first_non_empty_string(map, &["content", "text", "note", "markdown", "md"]))
}

pub(super) fn apply_metric_narrative(map: &serde_json::Map<String, Value>, meta: &mut MetricDrilldownMeta) {
    if meta.drilldown_note.is_none() {
        if let Some(note_value) = map.get("note") {
            meta.drilldown_note = metric_note_text(note_value);
        }
        if meta.drilldown_note.is_none() {
            meta.drilldown_note = first_non_empty_string(map, &["desc", "description"]);
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
}
pub(crate) fn tabs_from_value(value: &Value) -> Vec<String> {
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

pub(super) fn resolve_drilldown_target_scene_id(compiled: &CompiledApp, selector: &str) -> Option<String> {
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
