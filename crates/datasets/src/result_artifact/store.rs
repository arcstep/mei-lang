pub fn load_prebuild_metric_response_artifact_dataset_fallback(
    app_root: &Path,
    app_id: &str,
    dataset_id: &str,
    query: &DatasetQueryOptions,
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> Result<Option<(String, LoadedMetricResponseArtifact, u64)>> {
    ensure_prebuild_metric_response_index(app_root)?;
    let Ok(guard) = prebuild_metric_response_index().lock() else {
        return Ok(None);
    };
    let Some(index) = guard.as_ref() else {
        return Ok(None);
    };
    let dataset_candidates = crate::metric_cache_key::dataset_resource_lookup_aliases(dataset_id);
    let mut best: Option<(String, u64, bool, usize)> = None;
    for entry in &index.entries {
        let dataset_matches = dataset_candidates.iter().any(|candidate| {
            prebuild_metric_response_key_matches_dataset_query(
                entry.response_cache_key.as_str(),
                app_id,
                candidate.as_str(),
                query,
            )
        });
        if !dataset_matches {
            continue;
        }
        let covers = if request_all_metrics {
            entry.complete
        } else {
            requested_metric_ids
                .iter()
                .all(|metric_id| entry.covered_metric_ids.contains(metric_id))
        };
        if !covers {
            continue;
        }
        let covered_count = if request_all_metrics {
            entry.covered_metric_ids.len()
        } else {
            requested_metric_ids.len()
        };
        let replace = best.as_ref().is_none_or(|(_, best_at, complete, count)| {
            entry.complete && !*complete
                || (entry.complete == *complete
                    && (entry.generated_at_ms > *best_at
                        || (entry.generated_at_ms == *best_at && covered_count > *count)))
        });
        if replace {
            best = Some((
                entry.response_cache_key.clone(),
                entry.generated_at_ms,
                entry.complete,
                covered_count,
            ));
        }
    }
    let Some((cache_key, _, _, _)) = best else {
        return Ok(None);
    };
    load_metric_response_result_artifact(app_root, cache_key.as_str())
        .map(|loaded| loaded.map(|(artifact, ms)| (cache_key, artifact, ms)))
}

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

#[cfg(test)]
mod fallback_tests {
    use super::*;
    use crate::types::DatasetQueryOptions;
    use std::path::PathBuf;

    #[test]
    fn sidecar_roundtrip_preloads_memory_index() {
        let app_root =
            std::env::temp_dir().join(format!("mei-metric-index-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&app_root);
        fs::create_dir_all(&app_root).expect("create temp app root");
        let cache_key = "prebuild|response|app=demo|dataset=sample|dependency=dep|search=|filters={}|group=[]|time_range=null";
        store_metric_response_result_artifact(
            app_root.as_path(),
            cache_key,
            1,
            &BTreeMap::new(),
            &BTreeSet::from(["m1".to_string()]),
            true,
        )
        .expect("store artifact");
        invalidate_prebuild_metric_response_index(Some(app_root.as_path()));
        let first = preload_prebuild_metric_response_index(app_root.as_path()).expect("preload");
        assert!(first.entry_count >= 1);
        assert!(metric_response_index_path(app_root.as_path()).is_file());
        let second =
            preload_prebuild_metric_response_index(app_root.as_path()).expect("preload again");
        assert_eq!(second.load_ms, 0);
        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn lenient_sidecar_load_skips_rebuild_on_fingerprint_mismatch() {
        let app_root =
            std::env::temp_dir().join(format!("mei-metric-index-mismatch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&app_root);
        fs::create_dir_all(&app_root).expect("create temp app root");
        let cache_key = "prebuild|response|app=demo|dataset=sample|dependency=dep|search=|filters={}|group=[]|time_range=null";
        store_metric_response_result_artifact(
            app_root.as_path(),
            cache_key,
            1,
            &BTreeMap::new(),
            &BTreeSet::from(["m1".to_string()]),
            true,
        )
        .expect("store artifact");
        let _ = rebuild_and_install_prebuild_metric_response_index(app_root.as_path())
            .expect("initial rebuild");
        invalidate_prebuild_metric_response_index(Some(app_root.as_path()));
        fs::write(
            metric_response_artifact_dir(app_root.as_path()).join("orphan.json"),
            "{}",
        )
        .expect("add orphan artifact without updating sidecar");
        let stats =
            preload_prebuild_metric_response_index(app_root.as_path()).expect("lenient preload");
        assert!(
            stats.load_ms < 200,
            "fingerprint mismatch must not trigger full rebuild on preload, got {}ms",
            stats.load_ms
        );
        assert!(stats.entry_count >= 1);
        assert!(!stats.rebuilt);
        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn zhifa_sidecar_preload_is_fast_after_warm_sidecar() {
        let app_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspaces/ws-spbjw/zhifa");
        if !app_root.is_dir() {
            return;
        }
        let _ =
            preload_prebuild_metric_response_index(app_root.as_path()).expect("initial preload");
        invalidate_prebuild_metric_response_index(Some(app_root.as_path()));
        let stats =
            preload_prebuild_metric_response_index(app_root.as_path()).expect("sidecar preload");
        assert!(
            stats.load_ms < 500,
            "expected fast sidecar preload for zhifa, got {}ms for {} entries",
            stats.load_ms,
            stats.entry_count
        );
    }

    #[test]
    fn dataset_fallback_finds_zhifa_supervision_world_metrics() {
        let app_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workspaces/ws-spbjw/zhifa");
        if !app_root.is_dir() {
            return;
        }
        let query = DatasetQueryOptions::default();
        let requested = BTreeSet::from([
            "scenes/08-监督成效.mei::effectiveness_transfer_clue_count".to_string(),
        ]);
        let loaded = load_prebuild_metric_response_artifact_dataset_fallback(
            app_root.as_path(),
            "zhifa",
            "__world_metrics__::scenes/08-监督成效.mei::metrics",
            &query,
            &requested,
            false,
        )
        .expect("fallback load");
        assert!(
            loaded.is_some(),
            "expected prebuild artifact for 08-监督成效 world_metrics"
        );
    }
}
