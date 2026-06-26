use std::collections::BTreeMap;

use super::{
    imported_capsule_path_from_world_metrics_resource_id, resolve_runtime_metric_def_key,
};
use serde_json::json;

#[test]
fn imported_capsule_path_from_world_metrics_resource_id_parses_scoped_owner() {
    assert_eq!(
        imported_capsule_path_from_world_metrics_resource_id(
            "__world_metrics__::scenes/5_问题办理/问题办理.mei::metrics"
        ),
        Some("scenes/5_问题办理/问题办理.mei".to_string())
    );
    assert!(
        imported_capsule_path_from_world_metrics_resource_id("__world_metrics__").is_none()
    );
}

#[test]
fn resolve_runtime_metric_def_key_namespaces_rowset_suffix_keys() {
    let resource_id = "__world_metrics__::scenes/5_问题办理/问题办理.mei::metrics";
    let mut defs = BTreeMap::new();
    defs.insert(
        "scenes/5_问题办理/问题办理.mei::warnings_pending_count::__scalar_rowset__"
            .to_string(),
        json!({"id": "scenes/5_问题办理/问题办理.mei::warnings_pending_count::__scalar_rowset__"}),
    );
    assert_eq!(
        resolve_runtime_metric_def_key(
            resource_id,
            "warnings_pending_count::__scalar_rowset__",
            &defs,
        ),
        Some(
            "scenes/5_问题办理/问题办理.mei::warnings_pending_count::__scalar_rowset__"
                .to_string()
        )
    );
}

#[test]
fn resolve_runtime_metric_def_key_resolves_namespaced_dataset_resource_defs() {
    let resource_id = "scenes/2_行政检查/行政检查.mei::administrative_inspection_results";
    let mut defs = BTreeMap::new();
    defs.insert(
        "scenes/2_行政检查/行政检查.mei::inspections_total_count".to_string(),
        json!({"id": "scenes/2_行政检查/行政检查.mei::inspections_total_count"}),
    );
    assert_eq!(
        resolve_runtime_metric_def_key(resource_id, "inspections_total_count", &defs),
        Some("scenes/2_行政检查/行政检查.mei::inspections_total_count".to_string())
    );
}

#[test]
fn resolve_runtime_metric_def_key_falls_back_to_parent_scalar_rowset() {
    let resource_id = "__world_metrics__::scenes/03-指标体系.mei::metrics";
    let parent = "scenes/03-指标体系.mei::inspection_frequency_reduction_rate";
    let mut defs = BTreeMap::new();
    defs.insert(
        parent.to_string(),
        json!({"id": parent, "shape": "scalar_map"}),
    );
    assert_eq!(
        resolve_runtime_metric_def_key(
            resource_id,
            "scenes/03-指标体系.mei::inspection_frequency_reduction_rate::__scalar_rowset__",
            &defs,
        ),
        Some(format!("{parent}::__scalar_rowset__"))
    );
}

#[test]
fn resolve_runtime_metric_def_key_falls_back_to_namespaced_import_key() {
    let resource_id = "__world_metrics__::scenes/5_问题办理/问题办理.mei::metrics";
    let mut defs = BTreeMap::new();
    defs.insert(
        "scenes/5_问题办理/问题办理.mei::warnings_pending_table".to_string(),
        json!({"id": "scenes/5_问题办理/问题办理.mei::warnings_pending_table"}),
    );
    assert_eq!(
        resolve_runtime_metric_def_key(resource_id, "warnings_pending_table", &defs),
        Some("scenes/5_问题办理/问题办理.mei::warnings_pending_table".to_string())
    );
    assert_eq!(
        resolve_runtime_metric_def_key(resource_id, "missing", &defs),
        None
    );
}
