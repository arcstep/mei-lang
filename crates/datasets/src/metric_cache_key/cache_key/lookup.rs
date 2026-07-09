fn expand_scene_path_lookup_variants(
    scene_path: Option<&str>,
    primary_dataset_id: &str,
) -> Vec<String> {
    let mut scene_paths = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_path = |path: &str| {
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        for key in mei_lang_kernel::app_source_rel_path_lookup_keys(path) {
            if seen.insert(key.clone()) {
                scene_paths.push(key);
            }
        }
    };
    if let Some(path) = scene_path {
        push_path(path);
    }
    if let Some(capsule_path) =
        mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(primary_dataset_id)
    {
        push_path(capsule_path.as_str());
    }
    if scene_paths.is_empty() {
        scene_paths.push(String::new());
    }
    scene_paths
}

fn append_metric_response_lookup_keys(
    app_id: &str,
    app_root: &Path,
    compiled: &CompiledApp,
    scene_id: &str,
    scene_paths: &[String],
    dataset_ids: &[String],
    dependency_metric_defs: &BTreeMap<String, Value>,
    query: &DatasetQueryOptions,
    compile_revision: &str,
    filter_intents: &[FilterIntent],
    prefer_prebuild_keys: bool,
    slot_revision: Option<&str>,
    seen: &mut BTreeSet<String>,
    keys: &mut Vec<String>,
) {
    for dataset_id in dataset_ids {
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            compiled,
            dataset_id.as_str(),
            dependency_metric_defs,
        );
        let bundle_revision = format!(
            "mdb:{}",
            stable_slot_hash(&serde_json::to_string(dependency_metric_defs).unwrap_or_default())
        );
        for scene_path in scene_paths {
            let scoped_scene_path = if scene_path.is_empty() {
                None
            } else {
                Some(scene_path.as_str())
            };
            let scope_key =
                crate::metric_response_cache::metric_eval_scope_key(scene_id, scoped_scene_path);
            let effective_compile_revision = effective_compile_revision_for_slot(
                compile_revision,
                bundle_revision.as_str(),
                dependency_revision_key.as_str(),
                scope_key.as_str(),
            );
            let resolved_slot_revision = slot_revision
                .map(str::to_string)
                .unwrap_or_else(|| {
                    crate::metric_response_cache::compute_metric_slot_revision(
                        bundle_revision.as_str(),
                        dependency_revision_key.as_str(),
                        scope_key.as_str(),
                    )
                });
            let scoped_key = metric_response_cache_scope_key(
                app_id,
                scene_id,
                scoped_scene_path,
                dataset_id.as_str(),
                query,
                effective_compile_revision.as_str(),
                &dependency_revision_key,
                filter_intents,
                Some(resolved_slot_revision.as_str()),
            );
            let shared_key = metric_response_prebuild_shared_key(
                app_id,
                dataset_id.as_str(),
                query,
                &dependency_revision_key,
            );
            let dataset_key =
                metric_response_prebuild_dataset_key(app_id, dataset_id.as_str(), query);
            let data_generation = resolve_metric_data_generation(
                app_root,
                app_id,
                compiled,
                dataset_id.as_str(),
                dependency_metric_defs,
            );
            let idempotent_key = metric_shared_cache_key_with_data_generation(
                app_id,
                data_generation.as_str(),
                dataset_id.as_str(),
                query,
                dependency_revision_key.as_str(),
            );
            let ordered_keys = if prefer_prebuild_keys {
                vec![idempotent_key, dataset_key, shared_key, scoped_key]
            } else {
                vec![scoped_key, idempotent_key, shared_key, dataset_key]
            };
            for key in ordered_keys {
                if seen.insert(key.clone()) {
                    keys.push(key);
                }
            }
        }
    }
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
    slot_revision: Option<&str>,
    dependency_metric_defs: Option<&BTreeMap<String, Value>>,
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
    let scene_paths = expand_scene_path_lookup_variants(scene_path, primary_dataset_id);
    let mut keys = Vec::new();
    let mut seen = BTreeSet::new();
    append_metric_response_lookup_keys(
        app_id,
        app_root,
        compiled,
        scene_id,
        scene_paths.as_slice(),
        dataset_ids.as_slice(),
        &owner_dataset.runtime_metric_defs,
        query,
        compile_revision,
        filter_intents,
        prefer_prebuild_keys,
        slot_revision,
        &mut seen,
        &mut keys,
    );
    if let Some(subset_defs) = dependency_metric_defs {
        if subset_defs != &owner_dataset.runtime_metric_defs {
            append_metric_response_lookup_keys(
                app_id,
                app_root,
                compiled,
                scene_id,
                scene_paths.as_slice(),
                dataset_ids.as_slice(),
                subset_defs,
                query,
                compile_revision,
                filter_intents,
                prefer_prebuild_keys,
                slot_revision,
                &mut seen,
                &mut keys,
            );
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
    // Prebuild writes one collect_all rowset; runtime queries with smaller page_size must still hit it.
    let mut prebuild_canonical = options.clone();
    prebuild_canonical.page = 1;
    prebuild_canonical.page_size = crate::metric_dataframe::MAX_PAGE_SIZE;
    prebuild_canonical.collect_all = true;
    if !variants.iter().any(|variant| {
        variant.page_size == prebuild_canonical.page_size && variant.collect_all
    }) {
        variants.push(prebuild_canonical);
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
    if let Some(short) = resolved_metric_id.rsplit("::").next().filter(|part| {
        !part.is_empty() && *part != "__scalar_rowset__"
    }) {
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
    let mut push_dataset_keys = |dataset_id: &str| {
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            compiled,
            dataset_id,
            dependency_defs,
        );
        for metric_token in &metric_tokens {
            for query_options in dataframe_query_option_variants(options) {
                let key = dataframe_result_cache_key(
                    app_root,
                    scene_id,
                    target,
                    dataset_id,
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
    };
    if !primary_dataset_id.is_empty() {
        push_dataset_keys(primary_dataset_id);
    }
    for dataset_id in dataset_ids {
        if dataset_id == primary_dataset_id {
            continue;
        }
        push_dataset_keys(dataset_id.as_str());
    }
    keys
}

pub(crate) fn lookup_compiled_dataset_view<'a>(
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
