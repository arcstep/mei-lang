use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_runtime_metric_def_key, resolve_versioned_source_identifier,
    DatasetView, RuntimeMetricEvalScope,
};
use serde::Serialize;
use serde_json::Value;

use super::metric_hydrate::collect_dataset_ids_from_metric_defs;

pub(crate) fn normalize_query_search(search: Option<&str>) -> Option<String> {
    let value = search.unwrap_or("").trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub(crate) fn normalize_query_filters(
    filters: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut normalized = BTreeMap::new();
    for (key, value) in filters {
        let normalized_key = key.trim();
        let normalized_value = value.trim();
        if normalized_key.is_empty() || normalized_value.is_empty() {
            continue;
        }
        normalized.insert(normalized_key.to_string(), normalized_value.to_string());
    }
    normalized
}

pub(crate) fn resolve_runtime_metric_ids(
    resource_id: &str,
    requested_metric_ids: &[String],
    defs: &BTreeMap<String, Value>,
) -> Vec<String> {
    requested_metric_ids
        .iter()
        .filter_map(|metric_id| resolve_runtime_metric_def_key(resource_id, metric_id, defs))
        .collect()
}

pub(crate) fn select_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    resolved_metric_ids: &[String],
) -> BTreeMap<String, Value> {
    if resolved_metric_ids.is_empty() {
        return metric_defs.clone();
    }
    resolved_metric_ids
        .iter()
        .filter_map(|metric_id| {
            metric_defs
                .get(metric_id)
                .cloned()
                .map(|value| (metric_id.clone(), value))
        })
        .collect()
}

pub(crate) fn metric_scope_cache_key(resolved_metric_ids: &[String]) -> String {
    if resolved_metric_ids.is_empty() {
        return "*".to_string();
    }
    let mut ids = resolved_metric_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    serialize_cache_value(&ids)
}

pub(crate) fn serialize_cache_value<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

pub(crate) fn metric_request_revision_fingerprint(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    base_dataset_id: &str,
    metric_defs: &BTreeMap<String, Value>,
) -> String {
    let mut dataset_ids = collect_dataset_ids_from_metric_defs(metric_defs);
    let base_dataset_id = base_dataset_id.trim();
    if !base_dataset_id.is_empty() {
        dataset_ids.insert(base_dataset_id.to_string());
    }
    let mut fingerprints = dataset_ids
        .into_iter()
        .filter_map(|dataset_id| lookup_dataset_view(datasets, dataset_id.as_str()))
        .map(|dataset| dataset_source_fingerprint(app_root, dataset))
        .collect::<Vec<_>>();
    fingerprints.sort();
    format!(
        "materialize={}|deps={}",
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&fingerprints)
    )
}

pub(crate) fn runtime_metric_eval_scope(
    base_dataset_id: &str,
    scene_id: &str,
    target: Option<&str>,
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    dependency_revision_key: &str,
) -> RuntimeMetricEvalScope {
    let normalized_filters = normalize_query_filters(filters);
    let normalized_search = normalize_query_search(search).unwrap_or_default();
    RuntimeMetricEvalScope {
        base_dataset_id: base_dataset_id.trim().to_string(),
        scene_id: scene_id.trim().to_string(),
        target: target.unwrap_or("").trim().to_string(),
        search: normalized_search,
        filters_fingerprint: serialize_cache_value(&normalized_filters),
        dependency_revision_key: dependency_revision_key.to_string(),
    }
}

pub(crate) fn eval_node_cache_key(expr_fingerprint: &str, scope: &RuntimeMetricEvalScope) -> String {
    format!(
        "expr={}|dataset={}|scene={}|target={}|search={}|filters={}|deps={}",
        expr_fingerprint.trim(),
        scope.base_dataset_id.trim(),
        scope.scene_id.trim(),
        scope.target.trim(),
        scope.search.trim(),
        scope.filters_fingerprint.trim(),
        scope.dependency_revision_key.trim()
    )
}

fn dataset_source_fingerprint(app_root: &Path, dataset: &DatasetView) -> String {
    let kind = dataset.source.kind.trim();
    let path = dataset.source.path.trim();
    if path.is_empty() || path.starts_with("dataset_view:") {
        return format!(
            "{}|kind={}|path={}|sheet={}|header_row={}",
            dataset.id,
            kind,
            path,
            dataset.source.sheet.as_deref().unwrap_or(""),
            dataset.source.header_row.unwrap_or(1).max(1)
        );
    }
    let resolved_identifier = resolve_versioned_source_identifier(app_root, path);
    let absolute_path = app_root.join(&resolved_identifier);
    let modified_ms = std::fs::metadata(&absolute_path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!(
        "{}|kind={}|path={}|mtime={}|sheet={}|header_row={}",
        dataset.id,
        kind,
        resolved_identifier,
        modified_ms,
        dataset.source.sheet.as_deref().unwrap_or(""),
        dataset.source.header_row.unwrap_or(1).max(1)
    )
}

fn lookup_dataset_view<'a>(
    datasets: &'a BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    datasets
        .get(normalized)
        .or_else(|| datasets.get(dataset_id))
        .or_else(|| {
            datasets.iter().find_map(|(key, dataset)| {
                (dataset.id == normalized
                    || key.ends_with(&format!("::{normalized}"))
                    || key.ends_with(&format!("/{normalized}")))
                .then_some(dataset)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::SourceDecl;

    #[test]
    fn metric_scope_cache_key_sorts_and_dedups() {
        let value = metric_scope_cache_key(&[
            "b".to_string(),
            "a".to_string(),
            "b".to_string(),
        ]);
        assert_eq!(value, "[\"a\",\"b\"]");
    }

    #[test]
    fn metric_request_revision_fingerprint_includes_base_dataset() {
        let mut datasets = BTreeMap::new();
        datasets.insert(
            "sample".to_string(),
            DatasetView {
                id: "sample".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: Vec::new(),
                rows: Vec::new(),
                source: SourceDecl {
                    kind: "derived".to_string(),
                    path: "legacy.metric_pack:sample".to_string(),
                    sheet: None,
                    header_row: None,
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: BTreeMap::new(),
                runtime_metric_defs: BTreeMap::new(),
            },
        );
        let fingerprint = metric_request_revision_fingerprint(
            Path::new("/tmp"),
            &datasets,
            "sample",
            &BTreeMap::new(),
        );
        assert!(fingerprint.contains("sample"));
    }

    #[test]
    fn eval_node_cache_key_contains_scope_dimensions() {
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: "warning_list".to_string(),
            scene_id: "home".to_string(),
            target: "scenes/home.mei".to_string(),
            search: "abc".to_string(),
            filters_fingerprint: "{\"status\":\"待办\"}".to_string(),
            dependency_revision_key: "deps=v1".to_string(),
        };
        let key = eval_node_cache_key("expr:count(rowset)", &scope);
        assert!(key.contains("expr=expr:count(rowset)"));
        assert!(key.contains("dataset=warning_list"));
        assert!(key.contains("scene=home"));
        assert!(key.contains("target=scenes/home.mei"));
        assert!(key.contains("search=abc"));
        assert!(key.contains("filters={\"status\":\"待办\"}"));
        assert!(key.contains("deps=deps=v1"));
    }

    #[test]
    fn normalize_query_filters_drops_empty_and_trims() {
        let raw = BTreeMap::from([
            (" status ".to_string(), " 待办 ".to_string()),
            ("empty".to_string(), "".to_string()),
            ("  ".to_string(), "x".to_string()),
        ]);
        let normalized = normalize_query_filters(&raw);
        assert_eq!(normalized.get("status"), Some(&"待办".to_string()));
        assert!(!normalized.contains_key("empty"));
        assert_eq!(normalized.len(), 1);
    }

    #[test]
    fn normalize_query_search_trims_blank() {
        assert_eq!(normalize_query_search(Some("  abc ")), Some("abc".to_string()));
        assert_eq!(normalize_query_search(Some("   ")), None);
        assert_eq!(normalize_query_search(None), None);
    }
}
