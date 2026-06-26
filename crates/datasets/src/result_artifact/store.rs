pub fn load_metric_dataframe_result_artifact(
    app_root: &Path,
    response_cache_key: &str,
) -> Result<Option<(DatasetQueryResult, u64)>> {
    let started = Instant::now();
    let path = metric_dataframe_result_artifact_path(app_root, response_cache_key);
    let Some(artifact) = read_json_artifact_lenient::<PersistedMetricDataframeResultArtifact>(
        &path,
        "metric-dataframe",
    )?
    else {
        return Ok(None);
    };
    if artifact.schema_version != METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION
        || artifact.response_cache_key != response_cache_key
    {
        return Ok(None);
    }
    Ok(Some((
        artifact.result,
        started.elapsed().as_millis() as u64,
    )))
}

pub fn metric_dataframe_result_artifact_exists(app_root: &Path, response_cache_key: &str) -> bool {
    let path = metric_dataframe_result_artifact_path(app_root, response_cache_key);
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

pub fn store_metric_dataframe_result_artifact(
    app_root: &Path,
    response_cache_key: &str,
    result: &DatasetQueryResult,
) -> Result<()> {
    write_json_artifact(
        &metric_dataframe_result_artifact_path(app_root, response_cache_key),
        &PersistedMetricDataframeResultArtifact {
            schema_version: METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION.to_string(),
            response_cache_key: response_cache_key.to_string(),
            result: result.clone(),
            generated_at_ms: now_epoch_ms(),
        },
    )
}
