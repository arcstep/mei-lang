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
    let materialized_cache_key = metric_dataframe_scope_cache_key(
        app_root,
        scene_id,
        target,
        owner_resource.id.as_str(),
        &metric_scope_cache_key(&effective_metric_ids),
        &options,
        compile_revision,
        &metric_request_revision_fingerprint_for_compiled(
            app_root,
            compiled,
            owner_resource.id.as_str(),
            if owner_dataset.runtime_metric_defs.is_empty() {
                &defs_for_hydrate
            } else {
                &owner_dataset.runtime_metric_defs
            },
        ),
        &filter_intents,
    );
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
    if let Some(materialized) = take_cached_metric_dataframe_materialized(&materialized_cache_key) {
        if materialized.rows.len() >= MIN_MATERIALIZED_METRIC_ROWS_TO_CACHE {
            let response_cache_lookup_ms = elapsed_ms(response_cache_lookup_started);
            let result = paginate_materialized_metric_dataframe(
                &materialized,
                &meta,
                &metric_output_pagination_options(&options),
                &response_cache_key,
                response_cache_lookup_ms,
                true,
                Some(0),
            );
            store_cached_metric_dataframe_result(response_cache_key.clone(), &result);
            return Ok(result);
        }
    }

    let response_cache_lookup_ms = elapsed_ms(response_cache_lookup_started);
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        compiled,
        owner_resource.id.as_str(),
        if owner_dataset.runtime_metric_defs.is_empty() {
            &defs_for_hydrate
        } else {
            &owner_dataset.runtime_metric_defs
        },
    );
    let eval_started = Instant::now();
    let primary_filters =
        resolve_dataset_query_bindings_from_state(&effective_query_state, primary_dataset)
            .mapped_filters;
    let base_query = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: options.search.clone(),
        filters: primary_filters,
        group: options.group.clone(),
        time_range: options.time_range.clone(),
        collect_all: true,
        sort: Vec::new(),
        column_state: None,
        summary: false,
    };
    let base_started = Instant::now();
    let filtered_rows = query_dataset_rows(app_root, primary_dataset, base_query.clone())?;
    let base_query_ms = elapsed_ms(base_started);
    let base_rowset_materialize_ms = base_query_ms;

    let mut runtime_dataset = primary_dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }

    let mut datasets = build_compiled_datasets_map(
        compiled,
        &primary_resource.id,
        runtime_dataset.clone(),
        &referenced_dataset_ids,
    );

    hydrate_file_backed_datasets_for_metric_defs(
        app_root,
        &mut datasets,
        &defs_for_hydrate,
        &base_query,
    )
    .with_context(|| {
        format!(
            "metric_hydrate_binding_failed(dataframe): dataset={} metric={}",
            owner_resource.id, resolved_metric_id
        )
    })?;

    let binding_datasets = unique_dataset_views(primary_dataset, datasets.values());
    let supplementary_binding_datasets: Vec<&DatasetView> = binding_datasets
        .into_iter()
        .filter(|view| view.id != primary_dataset.id)
        .collect();
    let metric_started = Instant::now();
    let eval_scope = runtime_metric_eval_scope(
        Some(primary_dataset),
        &primary_resource.id,
        scene_id.unwrap_or(""),
        target,
        effective_query_state.search.as_deref(),
        &effective_query_state.filters,
        Some(&effective_query_state),
        &filter_intents,
        &dependency_revision_key,
        &supplementary_binding_datasets,
    )
    .with_context(|| {
        format!(
            "metric_scope_binding_failed(dataframe): dataset={} metric={}",
            owner_resource.id, resolved_metric_id
        )
    })?;
    let eval_execution = execute_runtime_eval_plan_artifacts(
        app_root,
        &owner_resource.id,
        &effective_metric_ids,
        &owner_dataset.runtime_metric_defs,
        &datasets,
        &runtime_dataset.rows,
        &eval_scope,
        default_result_artifact_scope(&effective_query_state, &filter_intents),
    )
    .with_context(|| {
        format!(
            "metric_eval_recursion_guard_tripped(dataframe): dataset={} metric={}",
            owner_resource.id, resolved_metric_id
        )
    })?;
    let metrics_map = eval_execution.metrics_map;
    let eval_report = eval_execution.eval_report;
    let eval_artifact_load_ms = eval_execution.eval_artifact_load_ms;
    let eval_artifact_hit = eval_execution.eval_artifact_hit;
    let metric_eval_ms = elapsed_ms(metric_started);
    let dag_metrics = &eval_report.request_dag_metrics;
    let eval_plan = &eval_report.eval_plan;
    let eval_scope_key = format!(
        "{}|{}|{}",
        eval_scope.base_dataset_id,
        eval_scope.query_state.group_identity_key(),
        eval_scope.query_state.time_range_identity_key()
    );

    let metric_source_id = synthetic_parent
        .as_deref()
        .unwrap_or(resolved_metric_id.as_str());
    let metric = metrics_map
        .get(metric_source_id)
        .ok_or_else(|| anyhow!("metric `{metric_id}` evaluation returned nothing"))?;
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
        // Tables/charts may request scalar metrics via /query without `::__scalar_rowset__`.
        scalar_metric_to_rowset(metric)
    } else if metric.shape != MetricShape::Dataframe {
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
    let (row_schema, rows) = format_rows_with_dataset_schema(&columns, rows, &datasets);

    let closure_set = effective_metric_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let closure_edges = owner_dataset
        .runtime_analysis_graph
        .edges
        .iter()
        .filter(|edge| closure_set.contains(&edge.from) && closure_set.contains(&edge.to))
        .count() as u64;

    let materialized = MaterializedMetricDataframe {
        expires_at: Instant::now() + metric_dataframe_materialized_cache_ttl(),
        columns,
        rows,
        row_schema,
        normalize: meta.normalize.clone(),
        base_perf: BTreeMap::from([
            ("base_query_ms".to_string(), base_query_ms),
            (
                "base_rowset_materialize_ms".to_string(),
                base_rowset_materialize_ms,
            ),
            ("metric_eval_ms".to_string(), metric_eval_ms),
            ("eval_artifact_load_ms".to_string(), eval_artifact_load_ms),
            (
                "eval_artifact_hit".to_string(),
                u64::from(eval_artifact_hit),
            ),
            (
                "eval_node_artifact_load_ms".to_string(),
                eval_execution.eval_node_artifact_load_ms,
            ),
            (
                "eval_node_artifact_hits".to_string(),
                eval_execution.eval_node_artifact_hits,
            ),
            (
                "eval_node_artifact_stores".to_string(),
                eval_execution.eval_node_artifact_stores,
            ),
            (
                "workset_artifact_load_ms".to_string(),
                workset_artifact_load_ms,
            ),
            (
                "workset_artifact_hit".to_string(),
                u64::from(workset_artifact_hit),
            ),
            (
                "eval_plan_targets".to_string(),
                eval_plan.targets.len() as u64,
            ),
            ("eval_plan_nodes".to_string(), eval_plan.nodes.len() as u64),
            ("eval_plan_edges".to_string(), eval_plan.edges.len() as u64),
            (
                "eval_plan_metric_nodes".to_string(),
                eval_plan.node_count_by_kind(EvalPlanNodeKind::MetricEval) as u64,
            ),
            (
                "eval_plan_rowset_nodes".to_string(),
                eval_plan.node_count_by_kind(EvalPlanNodeKind::Rowset) as u64,
            ),
            (
                "eval_plan_scalar_nodes".to_string(),
                eval_plan.node_count_by_kind(EvalPlanNodeKind::ScalarExpr) as u64,
            ),
            (
                "eval_plan_hydrate_nodes".to_string(),
                eval_plan.node_count_by_kind(EvalPlanNodeKind::Hydrate) as u64,
            ),
            (
                "eval_scope_key_hash".to_string(),
                hash_fingerprint(&eval_scope_key),
            ),
            (
                "eval_scope_group_key_hash".to_string(),
                hash_fingerprint(&eval_scope.query_state.group_identity_key()),
            ),
            (
                "eval_scope_time_range_key_hash".to_string(),
                hash_fingerprint(&eval_scope.query_state.time_range_identity_key()),
            ),
            (
                "eval_scope_group_dimensions".to_string(),
                eval_scope.query_state.group.len() as u64,
            ),
            ("request_dag_nodes".to_string(), dag_metrics.nodes as u64),
            ("request_dag_edges".to_string(), dag_metrics.edges as u64),
            ("request_dag_hits".to_string(), dag_metrics.hits),
            ("request_dag_misses".to_string(), dag_metrics.misses),
            ("request_dag_observed".to_string(), 1),
            (
                "request_dag_request_cache_hits".to_string(),
                dag_metrics.request_cache_hits,
            ),
            (
                "request_dag_eval_node_cache_hits".to_string(),
                dag_metrics.eval_node_cache_hits,
            ),
            (
                "request_dag_eval_node_cache_misses".to_string(),
                dag_metrics.eval_node_cache_misses,
            ),
            ("eval_memo_hits".to_string(), dag_metrics.request_cache_hits),
            (
                "eval_memo_eval_node_cache_hits".to_string(),
                dag_metrics.eval_node_cache_hits,
            ),
            (
                "eval_memo_eval_node_cache_misses".to_string(),
                dag_metrics.eval_node_cache_misses,
            ),
            (
                "analysis_closure_nodes".to_string(),
                effective_metric_ids.len() as u64,
            ),
            ("analysis_closure_edges".to_string(), closure_edges),
            (
                "eval_node_cache_enabled".to_string(),
                u64::from(runtime_eval_node_cache_enabled()),
            ),
        ]),
    };
    store_cached_metric_dataframe_materialized(materialized_cache_key, materialized.clone());

    let metric_dataframe_eval_ms = elapsed_ms(eval_started);
    let mut result = paginate_materialized_metric_dataframe(
        &materialized,
        &meta,
        &metric_output_pagination_options(&options),
        &response_cache_key,
        response_cache_lookup_ms,
        false,
        Some(metric_dataframe_eval_ms),
    );
    result.perf.extend(filtered_rows.perf);
    store_cached_metric_dataframe_result(response_cache_key.clone(), &result);
    if result_artifact_candidate {
        store_metric_dataframe_result_artifact(app_root, &response_cache_key, &result)?;
    }
    Ok(result)
}

