use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_data_snapshot_import_entry,
    resolve_versioned_source_identifier, source_file_content_signature, CompiledApp, DatasetView,
    FilterIntent, QueryState, RuntimeMetricEvalScope,
};
use serde::Serialize;
use serde_json::Value;

use crate::metric_hydrate::collect_dataset_ids_from_metric_defs;
use crate::metric_locate::locate_runtime_metric_resource;
use crate::metric_response_cache::{
    metric_response_cache_scope_key, metric_response_prebuild_dataset_key,
    metric_response_prebuild_shared_key,
};
use crate::types::DatasetQueryOptions;

use super::query_normalize::{
    dimension_bindings_from_query_state, dimension_bindings_from_query_state_for_datasets,
    filter_intents_from_request, normalize_query_filters, query_state_from_request,
};

const REVISION_FINGERPRINT_CACHE_TTL: Duration = Duration::from_millis(4000);
const REVISION_FINGERPRINT_CACHE_MAX: usize = 512;

#[derive(Clone)]
struct RevisionFingerprintCacheEntry {
    value: String,
    cached_at: Instant,
}

fn revision_fingerprint_cache() -> &'static Mutex<BTreeMap<String, RevisionFingerprintCacheEntry>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, RevisionFingerprintCacheEntry>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
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

fn filter_intents_fingerprint(scope: &RuntimeMetricEvalScope) -> String {
    serialize_cache_value(&scope.filter_intents)
}

fn dimension_bindings_fingerprint(scope: &RuntimeMetricEvalScope) -> String {
    serialize_cache_value(&scope.dimension_bindings)
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
    let cache_key = revision_fingerprint_cache_key(app_root, datasets, &dataset_ids);
    if let Ok(cache) = revision_fingerprint_cache().lock() {
        if let Some(entry) = cache.get(&cache_key) {
            if entry.cached_at.elapsed() <= REVISION_FINGERPRINT_CACHE_TTL {
                return entry.value.clone();
            }
        }
    }
    let mut fingerprints = dataset_ids
        .into_iter()
        .filter_map(|dataset_id| lookup_dataset_view(datasets, dataset_id.as_str()))
        .map(|dataset| dataset_source_fingerprint(app_root, dataset))
        .collect::<Vec<_>>();
    fingerprints.sort();
    let value = format!(
        "materialize={}|deps={}",
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&fingerprints)
    );
    if let Ok(mut cache) = revision_fingerprint_cache().lock() {
        cache.insert(
            cache_key,
            RevisionFingerprintCacheEntry {
                value: value.clone(),
                cached_at: Instant::now(),
            },
        );
        if cache.len() > REVISION_FINGERPRINT_CACHE_MAX {
            let overflow = cache.len().saturating_sub(REVISION_FINGERPRINT_CACHE_MAX);
            if overflow > 0 {
                let keys = cache.keys().take(overflow).cloned().collect::<Vec<_>>();
                for key in keys {
                    cache.remove(&key);
                }
            }
        }
    }
    value
}

pub(crate) fn metric_request_revision_fingerprint_for_compiled(
    app_root: &Path,
    compiled: &CompiledApp,
    base_dataset_id: &str,
    metric_defs: &BTreeMap<String, Value>,
) -> String {
    let mut dataset_ids = collect_dataset_ids_from_metric_defs(metric_defs);
    let base_dataset_id = base_dataset_id.trim();
    if !base_dataset_id.is_empty() {
        dataset_ids.insert(base_dataset_id.to_string());
    }
    let cache_key = revision_fingerprint_cache_key_for_compiled(app_root, compiled, &dataset_ids);
    if let Ok(cache) = revision_fingerprint_cache().lock() {
        if let Some(entry) = cache.get(&cache_key) {
            if entry.cached_at.elapsed() <= REVISION_FINGERPRINT_CACHE_TTL {
                return entry.value.clone();
            }
        }
    }
    let mut fingerprints = dataset_ids
        .into_iter()
        .filter_map(|dataset_id| lookup_compiled_dataset_view(compiled, dataset_id.as_str()))
        .map(|dataset| dataset_source_fingerprint(app_root, dataset))
        .collect::<Vec<_>>();
    fingerprints.sort();
    let value = format!(
        "materialize={}|deps={}",
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&fingerprints)
    );
    if let Ok(mut cache) = revision_fingerprint_cache().lock() {
        cache.insert(
            cache_key,
            RevisionFingerprintCacheEntry {
                value: value.clone(),
                cached_at: Instant::now(),
            },
        );
        if cache.len() > REVISION_FINGERPRINT_CACHE_MAX {
            let overflow = cache.len().saturating_sub(REVISION_FINGERPRINT_CACHE_MAX);
            if overflow > 0 {
                let keys = cache.keys().take(overflow).cloned().collect::<Vec<_>>();
                for key in keys {
                    cache.remove(&key);
                }
            }
        }
    }
    value
}

fn revision_fingerprint_cache_key(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    dataset_ids: &BTreeSet<String>,
) -> String {
    let mut items = dataset_ids
        .iter()
        .filter_map(|dataset_id| lookup_dataset_view(datasets, dataset_id.as_str()))
        .map(dataset_source_cache_fingerprint)
        .collect::<Vec<_>>();
    items.sort();
    format!(
        "{}|materialize={}|deps={}",
        app_root.display(),
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&items)
    )
}

fn revision_fingerprint_cache_key_for_compiled(
    app_root: &Path,
    compiled: &CompiledApp,
    dataset_ids: &BTreeSet<String>,
) -> String {
    let mut items = dataset_ids
        .iter()
        .filter_map(|dataset_id| lookup_compiled_dataset_view(compiled, dataset_id.as_str()))
        .map(dataset_source_cache_fingerprint)
        .collect::<Vec<_>>();
    items.sort();
    format!(
        "{}|materialize={}|deps={}",
        app_root.display(),
        dataset_materialize_cache_epoch(),
        serialize_cache_value(&items)
    )
}

fn dataset_source_cache_fingerprint(dataset: &DatasetView) -> String {
    let kind = dataset.source.kind.trim();
    let path = dataset.source.path.trim();
    format!(
        "{}|kind={}|path={}|sheet={}|header_row={}",
        dataset.id,
        kind,
        path,
        dataset.source.sheet.as_deref().unwrap_or(""),
        dataset.source.header_row.unwrap_or(1).max(1)
    )
}

pub(crate) fn runtime_metric_eval_scope(
    binding_datasets: &[&DatasetView],
    base_dataset_id: &str,
    scene_id: &str,
    target: Option<&str>,
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    query_state_override: Option<&QueryState>,
    filter_intents_override: &[FilterIntent],
    dependency_revision_key: &str,
) -> Result<RuntimeMetricEvalScope> {
    let normalized_filters = normalize_query_filters(filters);
    let query_state = query_state_from_request(&normalized_filters, search, query_state_override);
    let normalized_search = query_state.search.clone().unwrap_or_default();
    let filter_intents = filter_intents_from_request(&query_state, filter_intents_override);
    let dimension_bindings = if binding_datasets.is_empty() {
        dimension_bindings_from_query_state(&query_state)
    } else {
        validate_runtime_scope_bindings(&query_state, binding_datasets)?;
        dimension_bindings_from_query_state_for_datasets(&query_state, binding_datasets)
    };
    Ok(RuntimeMetricEvalScope {
        base_dataset_id: base_dataset_id.trim().to_string(),
        scene_id: scene_id.trim().to_string(),
        target: target.unwrap_or("").trim().to_string(),
        search: normalized_search,
        query_state,
        filter_intents,
        dimension_bindings,
        filters_fingerprint: serialize_cache_value(&normalized_filters),
        dependency_revision_key: dependency_revision_key.to_string(),
    })
}

fn validate_runtime_scope_bindings(state: &QueryState, datasets: &[&DatasetView]) -> Result<()> {
    use crate::metric_hydrate::{
        resolve_dataset_query_bindings_from_state, unresolved_filter_dimensions_for_datasets,
    };
    let unresolved = unresolved_filter_dimensions_for_datasets(state, datasets);
    if !unresolved.is_empty() {
        let dataset_ids = datasets
            .iter()
            .map(|dataset| dataset.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(anyhow!(
            "runtime metric query requires resolvable filter bindings across datasets [{}]: {}",
            dataset_ids,
            unresolved.join(", ")
        ));
    }
    for dataset in datasets {
        let resolution = resolve_dataset_query_bindings_from_state(state, dataset);
        if let Some(dimension) = resolution.unresolved_time_range_dimension {
            return Err(anyhow!(
                "runtime metric query requires resolvable time_range.dimension binding for dataset `{}`: {}",
                dataset.id,
                dimension
            ));
        }
    }
    Ok(())
}

pub(crate) fn eval_node_cache_key(
    expr_fingerprint: &str,
    scope: &RuntimeMetricEvalScope,
) -> String {
    format!(
        "expr={}|dataset={}|scene={}|target={}|search={}|filters={}|filter_intents={}|dimension_bindings={}|group={}|time_range={}|deps={}",
        expr_fingerprint.trim(),
        scope.base_dataset_id.trim(),
        scope.scene_id.trim(),
        scope.target.trim(),
        scope.search.trim(),
        scope.filters_fingerprint.trim(),
        filter_intents_fingerprint(scope),
        dimension_bindings_fingerprint(scope),
        scope.query_state.group_identity_key(),
        scope.query_state.time_range_identity_key(),
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
    let content_signature = resolve_data_snapshot_import_entry(
        app_root,
        path,
        dataset.source.sheet.as_deref(),
        dataset.source.header_row.unwrap_or(1).max(1) as usize,
    )
    .map(|entry| format!("import:{}", entry.content_signature))
    .unwrap_or_else(|| {
        format!(
            "source:{}",
            source_file_content_signature(absolute_path.as_path(), resolved_identifier.as_str())
        )
    });
    format!(
        "{}|kind={}|path={}|content_sig={}|sheet={}|header_row={}",
        dataset.id,
        kind,
        resolved_identifier,
        content_signature,
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

pub(crate) fn dataset_metric_identity_key(dataset: &DatasetView) -> String {
    let mut metric_keys = dataset
        .runtime_metric_defs
        .keys()
        .map(|metric_id| metric_id.as_str())
        .collect::<Vec<_>>();
    metric_keys.sort_unstable();
    let source_path = dataset.source.path.trim().replace('\\', "/");
    format!("{source_path}|{}", metric_keys.join(","))
}

pub(crate) fn dataset_resource_lookup_aliases(dataset_id: &str) -> Vec<String> {
    let trimmed = dataset_id.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut aliases = vec![trimmed.to_string()];
    if let Some(_capsule_path) =
        mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(trimmed)
    {
        if !aliases.iter().any(|id| id == "__world_metrics__") {
            aliases.push("__world_metrics__".to_string());
        }
    }
    if let Some((_, bare)) = trimmed.rsplit_once("::") {
        let bare = bare.trim();
        if !bare.is_empty() && !aliases.iter().any(|id| id == bare) {
            aliases.push(bare.to_string());
        }
    }
    aliases
}

pub(crate) fn equivalent_dataset_resource_ids(
    compiled: &CompiledApp,
    owner_dataset: &DatasetView,
) -> Vec<String> {
    let identity = dataset_metric_identity_key(owner_dataset);
    let mut ids = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            (dataset_metric_identity_key(dataset) == identity).then(|| resource.id.clone())
        })
        .collect::<Vec<_>>();
    for alias in dataset_resource_lookup_aliases(owner_dataset.id.as_str()) {
        if !ids.iter().any(|id| id == &alias) {
            ids.push(alias);
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

pub(crate) fn metric_response_artifact_lookup_cache_keys(
    app_id: &str,
    app_root: &Path,
    compiled: &CompiledApp,
    scene_id: &str,
    scene_path: Option<&str>,
    primary_dataset_id: &str,
    owner_dataset: &DatasetView,
    query: &DatasetQueryOptions,
    compile_revision: &str,
    filter_intents: &[FilterIntent],
    prefer_prebuild_keys: bool,
) -> Vec<String> {
    let mut dataset_ids = equivalent_dataset_resource_ids(compiled, owner_dataset);
    for alias in dataset_resource_lookup_aliases(primary_dataset_id) {
        if !dataset_ids.iter().any(|id| id == &alias) {
            dataset_ids.push(alias);
        }
    }
    if let Some(index) = dataset_ids.iter().position(|id| id == primary_dataset_id) {
        if index > 0 {
            let primary = dataset_ids.remove(index);
            dataset_ids.insert(0, primary);
        }
    } else {
        dataset_ids.insert(0, primary_dataset_id.to_string());
    }
    let mut scene_paths = Vec::new();
    if let Some(path) = scene_path.map(str::trim).filter(|value| !value.is_empty()) {
        scene_paths.push(path.to_string());
    }
    if let Some(capsule_path) =
        mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(primary_dataset_id)
    {
        if !scene_paths.iter().any(|path| path == &capsule_path) {
            scene_paths.push(capsule_path);
        }
    }
    if scene_paths.is_empty() {
        scene_paths.push(String::new());
    }
    let mut keys = Vec::new();
    let mut seen = BTreeSet::new();
    for dataset_id in dataset_ids {
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            compiled,
            dataset_id.as_str(),
            &owner_dataset.runtime_metric_defs,
        );
        for scene_path in &scene_paths {
            let scoped_scene_path = if scene_path.is_empty() {
                None
            } else {
                Some(scene_path.as_str())
            };
            let scoped_key = metric_response_cache_scope_key(
                app_id,
                scene_id,
                scoped_scene_path,
                dataset_id.as_str(),
                query,
                compile_revision,
                &dependency_revision_key,
                filter_intents,
            );
            let shared_key = metric_response_prebuild_shared_key(
                app_id,
                dataset_id.as_str(),
                query,
                &dependency_revision_key,
            );
            let dataset_key = metric_response_prebuild_dataset_key(app_id, dataset_id.as_str(), query);
            let ordered_keys = if prefer_prebuild_keys {
                vec![dataset_key, shared_key, scoped_key]
            } else {
                vec![scoped_key, shared_key, dataset_key]
            };
            for key in ordered_keys {
                if seen.insert(key.clone()) {
                    keys.push(key);
                }
            }
        }
    }
    keys
}

fn dataframe_result_cache_key(
    app_root: &Path,
    scene_id: Option<&str>,
    target: Option<&str>,
    dataset_id: &str,
    metric_id: &str,
    options: &DatasetQueryOptions,
    compile_revision: &str,
    dependency_revision_key: &str,
    filter_intents: &[FilterIntent],
) -> String {
    let group = serialize_cache_value(&options.group);
    let time_range = serialize_cache_value(&options.time_range);
    let sort = serialize_cache_value(&options.sort);
    let column_state = serialize_cache_value(&options.column_state);
    let scope = format!(
        "{}|compile={}|{}|scene={}|target={}|{}|{}|search={}|filters={}|group={}|time_range={}|filter_intents={}",
        app_root.display(),
        compile_revision,
        dependency_revision_key,
        scene_id.unwrap_or("").trim(),
        target.unwrap_or("").trim(),
        dataset_id,
        metric_id,
        options.search.as_deref().unwrap_or(""),
        serialize_cache_value(&options.filters),
        group,
        time_range,
        serde_json::to_string(filter_intents).unwrap_or_else(|_| "[]".to_string()),
    );
    format!(
        "{}|page={}|page_size={}|full={}|sort={}|column_state={}|summary={}",
        scope,
        options.page,
        options.page_size,
        options.collect_all,
        sort,
        column_state,
        options.summary
    )
}

fn dataframe_query_option_variants(options: &DatasetQueryOptions) -> Vec<DatasetQueryOptions> {
    let mut variants = vec![options.clone()];
    if options.summary {
        let mut without_summary = options.clone();
        without_summary.summary = false;
        variants.push(without_summary);
    } else {
        let mut with_summary = options.clone();
        with_summary.summary = true;
        variants.push(with_summary);
    }
    variants
}

pub(crate) fn equivalent_dataframe_metric_scope_tokens(
    compiled: &CompiledApp,
    dataset_id: &str,
    resolved_metric_id: &str,
    effective_metric_ids: &[String],
) -> Vec<String> {
    let mut tokens = BTreeSet::new();
    if !effective_metric_ids.is_empty() {
        tokens.insert(metric_scope_cache_key(effective_metric_ids));
    }
    tokens.insert(metric_scope_cache_key(std::slice::from_ref(
        &resolved_metric_id.to_string(),
    )));
    if let Some(short) = resolved_metric_id.rsplit("::").next() {
        tokens.insert(metric_scope_cache_key(std::slice::from_ref(
            &short.to_string(),
        )));
        if !short.contains("__scalar_rowset__") {
            let scalar = format!("{short}::__scalar_rowset__");
            tokens.insert(metric_scope_cache_key(std::slice::from_ref(&scalar)));
        }
    }
    if let Ok((owner, canonical)) =
        locate_runtime_metric_resource(compiled, dataset_id, resolved_metric_id)
    {
        if let Some(dataset) = owner.dataset.as_ref() {
            for def_key in dataset.runtime_metric_defs.keys() {
                if let Ok((_, candidate)) =
                    locate_runtime_metric_resource(compiled, dataset_id, def_key)
                {
                    if candidate == canonical {
                        tokens.insert(metric_scope_cache_key(std::slice::from_ref(&candidate)));
                    }
                }
            }
        }
    }
    tokens.into_iter().collect()
}

fn world_metrics_resource_ids(compiled: &CompiledApp) -> Vec<String> {
    compiled
        .resources
        .iter()
        .filter(|resource| {
            resource.id == "__world_metrics__" || resource.id.starts_with("__world_metrics__::")
        })
        .map(|resource| resource.id.clone())
        .collect()
}

pub(crate) fn metric_dataframe_artifact_lookup_cache_keys(
    app_root: &Path,
    compiled: &CompiledApp,
    scene_id: Option<&str>,
    target: Option<&str>,
    primary_dataset_id: &str,
    owner_resource_id: &str,
    owner_dataset: &DatasetView,
    resolved_metric_id: &str,
    effective_metric_ids: &[String],
    options: &DatasetQueryOptions,
    compile_revision: &str,
    filter_intents: &[FilterIntent],
    defs_for_dependency: &BTreeMap<String, Value>,
) -> Vec<String> {
    let mut dataset_ids = equivalent_dataset_resource_ids(compiled, owner_dataset);
    for world_metrics_id in world_metrics_resource_ids(compiled) {
        if !dataset_ids.iter().any(|id| id == &world_metrics_id) {
            dataset_ids.push(world_metrics_id);
        }
    }
    if let Some(index) = dataset_ids.iter().position(|id| id == owner_resource_id) {
        if index > 0 {
            let owner = dataset_ids.remove(index);
            dataset_ids.insert(0, owner);
        }
    } else {
        dataset_ids.insert(0, owner_resource_id.to_string());
    }
    if !primary_dataset_id.is_empty() && !dataset_ids.iter().any(|id| id == primary_dataset_id) {
        dataset_ids.push(primary_dataset_id.to_string());
    }
    let metric_tokens = equivalent_dataframe_metric_scope_tokens(
        compiled,
        primary_dataset_id,
        resolved_metric_id,
        effective_metric_ids,
    );
    let dependency_defs = if owner_dataset.runtime_metric_defs.is_empty() {
        defs_for_dependency
    } else {
        &owner_dataset.runtime_metric_defs
    };
    let mut keys = Vec::new();
    let mut seen = BTreeSet::new();
    for dataset_id in dataset_ids {
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            compiled,
            dataset_id.as_str(),
            dependency_defs,
        );
        for metric_token in &metric_tokens {
            for query_options in dataframe_query_option_variants(options) {
                let key = dataframe_result_cache_key(
                    app_root,
                    scene_id,
                    target,
                    dataset_id.as_str(),
                    metric_token,
                    &query_options,
                    compile_revision,
                    &dependency_revision_key,
                    filter_intents,
                );
                if seen.insert(key.clone()) {
                    keys.push(key);
                }
            }
        }
    }
    keys
}

fn lookup_compiled_dataset_view<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    compiled.resources.iter().find_map(|resource| {
        let dataset = resource.dataset.as_ref()?;
        (resource.id == normalized
            || dataset.id == normalized
            || resource.id.ends_with(&format!("::{normalized}"))
            || resource.id.ends_with(&format!("/{normalized}")))
        .then_some(dataset)
    })
}
