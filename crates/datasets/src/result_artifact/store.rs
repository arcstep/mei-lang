pub fn load_metric_dataframe_result_artifact(
    app_root: &Path,
    response_cache_key: &str,
) -> Result<Option<(DatasetQueryResult, u64)>> {
    let started = Instant::now();
    let Some(artifact) = crate::load_small_artifact::<PersistedMetricDataframeResultArtifact>(
        app_root,
        METRIC_DATAFRAME_KIND,
        response_cache_key,
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
    crate::load_small_artifact::<PersistedMetricDataframeResultArtifact>(
        app_root,
        METRIC_DATAFRAME_KIND,
        response_cache_key,
    )
    .ok()
    .flatten()
    .is_some()
}

pub fn store_metric_dataframe_result_artifact(
    app_root: &Path,
    response_cache_key: &str,
    result: &DatasetQueryResult,
) -> Result<()> {
    let bytes = crate::store_small_artifact(
        app_root,
        METRIC_DATAFRAME_KIND,
        response_cache_key,
        &PersistedMetricDataframeResultArtifact {
            schema_version: METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION.to_string(),
            response_cache_key: response_cache_key.to_string(),
            result: result.clone(),
            generated_at_ms: now_epoch_ms(),
        },
    )?;
    record_artifact_write(bytes as u64);
    Ok(())
}
