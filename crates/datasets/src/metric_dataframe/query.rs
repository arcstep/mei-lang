pub fn query_metric_dataframe(
    compiled: &CompiledApp,
    app_root: &Path,
    dataset_id: &str,
    metric_id: &str,
    scene_id: Option<&str>,
    target: Option<&str>,
    compile_revision: &str,
    options: DatasetQueryOptions,
    query_state: Option<QueryState>,
    filter_intents: Vec<FilterIntent>,
) -> Result<DatasetQueryResult> {
    let effective_query_state = query_state_from_request(
        &options.filters,
        options.search.as_deref(),
        query_state.as_ref(),
    );
    let options = DatasetQueryOptions {
        search: effective_query_state.search.clone(),
        filters: effective_query_state.filters.clone(),
        group: effective_query_state.group.clone(),
        time_range: effective_query_state.time_range.clone(),
        ..options
    };
    let (owner_resource, resolved_metric_id) =
        locate_runtime_metric_resource(compiled, dataset_id, metric_id)?;
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let primary_resource =
        locate_dataset_resource(compiled, dataset_id).map_err(|error| anyhow!("{error}"))?;
    let primary_dataset = primary_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{}` is not a dataset", primary_resource.id))?;
    let synthetic_parent =
        synthetic_scalar_rowset_parent(owner_resource, resolved_metric_id.as_str());
    let workset_metric_id = synthetic_parent
        .clone()
        .unwrap_or_else(|| resolved_metric_id.clone());
    let (workset, workset_artifact_load_ms, workset_artifact_hit) =
        load_or_build_runtime_metric_workset_artifact(
            app_root,
            &owner_resource.id,
            std::slice::from_ref(&workset_metric_id),
            owner_dataset,
        )?;
    let effective_metric_ids = workset
        .eval_metric_ids
        .clone()
        .unwrap_or_else(|| vec![workset_metric_id.clone()]);
    let defs_for_hydrate = workset.defs_for_hydrate.clone();
    let referenced_dataset_ids = eval_artifact_hydrate_dataset_ids(&defs_for_hydrate);
    let lookup_cache_keys = metric_dataframe_artifact_lookup_cache_keys(
        app_root,
        compiled,
        scene_id,
        target,
        dataset_id,
        owner_resource.id.as_str(),
        owner_dataset,
        resolved_metric_id.as_str(),
        &effective_metric_ids,
        &options,
        compile_revision,
        &filter_intents,
        &defs_for_hydrate,
    );
    let response_cache_key = lookup_cache_keys.first().cloned().unwrap_or_else(|| {
        metric_dataframe_result_cache_key(
            app_root,
            scene_id,
            target,
            owner_resource.id.as_str(),
            &metric_scope_cache_key(&effective_metric_ids),
            &options,
            compile_revision,
            "",
            &filter_intents,
        )
    });
    let response_cache_lookup_started = Instant::now();
    let result_artifact_candidate =
        default_result_artifact_scope(&effective_query_state, &filter_intents);
    let mut cached_hit = None;
    for cache_key in &lookup_cache_keys {
        if let Some(cached) = take_cached_metric_dataframe_result(cache_key) {
            if cached.rows.is_empty() && cached.total == 0 {
                continue;
            }
            cached_hit = Some((cache_key.clone(), cached));
            break;
        }
    }
    if let Some((hit_cache_key, mut cached)) = cached_hit {
        cached.perf = BTreeMap::from([
            ("response_cache_hit".to_string(), 1),
            ("result_artifact_hit".to_string(), 0),
            (
                "response_cache_key_hash".to_string(),
                hash_fingerprint(&hit_cache_key),
            ),
            ("request_dag_observed".to_string(), 0),
            ("eval_memo_hits".to_string(), 0),
            ("eval_memo_eval_node_cache_hits".to_string(), 0),
            ("eval_memo_eval_node_cache_misses".to_string(), 0),
            (
                "response_cache_lookup_ms".to_string(),
                elapsed_ms(response_cache_lookup_started),
            ),
        ]);
        return Ok(cached);
    }
    if result_artifact_candidate {
        let mut loaded_artifact = None;
        for cache_key in &lookup_cache_keys {
            if let Some((artifact, artifact_load_ms)) =
                load_metric_dataframe_result_artifact(app_root, cache_key)?
            {
                if artifact.rows.is_empty() && artifact.total == 0 {
                    continue;
                }
                loaded_artifact = Some((cache_key.clone(), artifact, artifact_load_ms));
                break;
            }
        }
        if let Some((hit_cache_key, mut artifact, artifact_load_ms)) = loaded_artifact {
            artifact.perf = BTreeMap::from([
                ("response_cache_hit".to_string(), 0),
                ("result_artifact_hit".to_string(), 1),
                ("result_artifact_load_ms".to_string(), artifact_load_ms),
                (
                    "response_cache_key_hash".to_string(),
                    hash_fingerprint(&hit_cache_key),
                ),
                ("request_dag_observed".to_string(), 0),
                ("eval_memo_hits".to_string(), 0),
                ("eval_memo_eval_node_cache_hits".to_string(), 0),
                ("eval_memo_eval_node_cache_misses".to_string(), 0),
                (
                    "response_cache_lookup_ms".to_string(),
                    elapsed_ms(response_cache_lookup_started),
                ),
            ]);
            store_cached_metric_dataframe_result(hit_cache_key.clone(), &artifact);
            return Ok(artifact);
        }
    }

    let meta = parse_source_meta(primary_dataset.source.content.as_deref());
    let response_cache_lookup_ms = elapsed_ms(response_cache_lookup_started);
    let metric_defs_for_sql = if owner_dataset.runtime_metric_defs.is_empty() {
        &defs_for_hydrate
    } else {
        &owner_dataset.runtime_metric_defs
    };
    let eval_started = Instant::now();

    // DataFusion pipeline SQL only — never silent whole-table JSON hydrate (RSS P0).
    let sql_datasets = build_compiled_datasets_map(
        compiled,
        &primary_resource.id,
        primary_dataset.clone(),
        &referenced_dataset_ids,
    );
    let sql_ids = vec![workset_metric_id.clone()];
    match crate::query_engine::try_eval_dataframe_metrics_via_sql(
        app_root,
        &sql_datasets,
        metric_defs_for_sql,
        &sql_ids,
    ) {
        Ok(Some(metrics_map)) => {
            if let Some(metric) = metrics_map.get(workset_metric_id.as_str()) {
                let (mut columns, rows) = if synthetic_parent.is_some() {
                    if metric.shape == MetricShape::Dataframe {
                        let columns = metric
                            .schema
                            .iter()
                            .map(|column| column.name.clone())
                            .collect::<Vec<_>>();
                        (columns, extract_dataframe_rows(&metric.value))
                    } else {
                        scalar_metric_to_rowset(metric)
                    }
                } else if metric.shape == MetricShape::Scalar {
                    scalar_metric_to_rowset(metric)
                } else if metric.shape != MetricShape::Dataframe
                    && metric.shape != MetricShape::Series
                    && metric.shape != MetricShape::Table
                {
                    return Err(anyhow!(
                        "metric `{metric_id}` shape is {:?}, expected dataframe",
                        metric.shape
                    ));
                } else {
                    let columns = metric
                        .schema
                        .iter()
                        .map(|column| column.name.clone())
                        .collect::<Vec<_>>();
                    (columns, extract_dataframe_rows(&metric.value))
                };
                if columns.is_empty() && !rows.is_empty() {
                    columns = infer_columns(&rows);
                }
                let (row_schema, rows) =
                    format_rows_with_dataset_schema(&columns, rows, &sql_datasets);
                if !row_schema.is_empty()
                    && columns.iter().any(|name| {
                        row_schema.iter().any(|col| {
                            col.source
                                .as_deref()
                                .map(str::trim)
                                .is_some_and(|source| source == name.as_str())
                        })
                    })
                {
                    columns = row_schema.iter().map(|col| col.name.clone()).collect();
                }
                let metric_eval_ms = elapsed_ms(eval_started);
                crate::dataset_rows_cache::record_fallback_materialization_rows(&rows);
                let materialized = MaterializedMetricDataframe {
                    columns,
                    rows,
                    row_schema,
                    normalize: meta.normalize.clone(),
                    base_perf: BTreeMap::from([
                        ("base_query_ms".to_string(), 0),
                        ("base_rowset_materialize_ms".to_string(), 0),
                        ("metric_eval_ms".to_string(), metric_eval_ms),
                        ("query_engine_pipeline_sql".to_string(), 1),
                        (
                            "workset_artifact_load_ms".to_string(),
                            workset_artifact_load_ms,
                        ),
                        (
                            "workset_artifact_hit".to_string(),
                            u64::from(workset_artifact_hit),
                        ),
                    ]),
                };
                let result = paginate_materialized_metric_dataframe(
                    &materialized,
                    &meta,
                    &metric_output_pagination_options(&options),
                    &response_cache_key,
                    response_cache_lookup_ms,
                    false,
                    Some(elapsed_ms(eval_started)),
                );
                store_cached_metric_dataframe_result(response_cache_key.clone(), &result);
                if result_artifact_candidate {
                    store_metric_dataframe_result_artifact(
                        app_root,
                        &response_cache_key,
                        &result,
                    )?;
                }
                return Ok(result);
            }
        }
        Ok(None) => {}
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "pipeline_sql_fallback: metric_id={} dataset={} reason=sql_exec_or_row_limit",
                    resolved_metric_id, owner_resource.id
                )
            });
        }
    }

    Err(anyhow!(
        "pipeline_sql_fallback: metric_id={} dataset={} reason=uncovered_pipeline — whole-table JSON hydrate is disabled",
        resolved_metric_id,
        owner_resource.id
    ))
}

