fn install_prebuild_metric_response_index(index: PrebuildMetricResponseIndex) -> usize {
    let entry_count = index.entries.len();
    if let Ok(mut guard) = prebuild_metric_response_index().lock() {
        *guard = Some(index);
    }
    entry_count
}

fn try_load_prebuild_metric_response_index_from_sidecar(
    app_root: &Path,
    verify_fingerprint: bool,
) -> Result<Option<PrebuildMetricResponseIndex>> {
    load_metric_response_index_from_sidecar(app_root, verify_fingerprint)
}

fn rebuild_prebuild_metric_response_index(app_root: &Path) -> Result<PrebuildMetricResponseIndex> {
    let index = rebuild_prebuild_metric_response_index_from_artifacts(app_root)?;
    save_metric_response_index_sidecar(app_root, &index)?;
    Ok(index)
}

fn upsert_metric_response_index_entry(
    app_root: &Path,
    response_cache_key: &str,
    generated_at_ms: u64,
    complete: bool,
    covered_metric_ids: &BTreeSet<String>,
) -> Result<()> {
    if !response_cache_key.starts_with("prebuild|response|") {
        return Ok(());
    }
    let memory_entry = PrebuildMetricResponseIndexEntry {
        response_cache_key: response_cache_key.to_string(),
        generated_at_ms,
        complete,
        covered_metric_ids: covered_metric_ids.clone(),
    };
    if let Ok(mut guard) = prebuild_metric_response_index().lock() {
        if let Some(index) = guard.as_mut() {
            if index.app_root == app_root {
                if let Some(existing) = index
                    .entries
                    .iter_mut()
                    .find(|entry| entry.response_cache_key == response_cache_key)
                {
                    *existing = memory_entry.clone();
                } else {
                    index.entries.push(memory_entry.clone());
                }
            }
        }
    }

    let path = metric_response_index_path(app_root);
    let dir = metric_response_artifact_dir(app_root);
    let fingerprint = compute_metric_response_dir_fingerprint(dir.as_path())?;
    let mut sidecar = if path.is_file() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str::<PersistedMetricResponseIndexSidecar>(&raw).unwrap_or_else(|_| {
            PersistedMetricResponseIndexSidecar {
                schema_version: METRIC_RESPONSE_INDEX_SCHEMA_VERSION.to_string(),
                generated_at_ms: now_epoch_ms(),
                fingerprint: fingerprint.clone(),
                entries: Vec::new(),
            }
        })
    } else {
        PersistedMetricResponseIndexSidecar {
            schema_version: METRIC_RESPONSE_INDEX_SCHEMA_VERSION.to_string(),
            generated_at_ms: now_epoch_ms(),
            fingerprint: fingerprint.clone(),
            entries: Vec::new(),
        }
    };
    sidecar.schema_version = METRIC_RESPONSE_INDEX_SCHEMA_VERSION.to_string();
    sidecar.generated_at_ms = now_epoch_ms();
    sidecar.fingerprint = fingerprint;
    let sidecar_entry = sidecar_entry_from_memory(&memory_entry);
    if let Some(existing) = sidecar
        .entries
        .iter_mut()
        .find(|entry| entry.response_cache_key == response_cache_key)
    {
        *existing = sidecar_entry;
    } else {
        sidecar.entries.push(sidecar_entry);
    }
    write_json_artifact_atomic(path.as_path(), &sidecar)
}

pub fn invalidate_prebuild_metric_response_index(app_root: Option<&Path>) {
    let Ok(mut guard) = prebuild_metric_response_index().lock() else {
        return;
    };
    match app_root {
        Some(root) => {
            if guard.as_ref().is_some_and(|index| index.app_root == root) {
                *guard = None;
            }
        }
        None => *guard = None,
    }
}

/// Startup / post-prebuild: load sidecar when possible; rebuild only when sidecar is absent.
pub fn preload_prebuild_metric_response_index(app_root: &Path) -> Result<MetricResponseIndexStats> {
    if let Ok(guard) = prebuild_metric_response_index().lock() {
        if guard
            .as_ref()
            .is_some_and(|index| index.app_root == app_root)
        {
            let stats = MetricResponseIndexStats {
                load_ms: 0,
                entry_count: guard.as_ref().map(|index| index.entries.len()).unwrap_or(0),
                rebuilt: false,
            };
            record_metric_response_index_stats(stats);
            return Ok(stats);
        }
    }

    let started = Instant::now();
    let rebuilt = if let Some(index) =
        try_load_prebuild_metric_response_index_from_sidecar(app_root, false)?
    {
        install_prebuild_metric_response_index(index);
        false
    } else {
        rebuild_and_install_prebuild_metric_response_index(app_root)?.rebuilt
    };
    let entry_count = prebuild_metric_response_index()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|index| index.entries.len()))
        .unwrap_or(0);
    let stats = MetricResponseIndexStats {
        load_ms: started.elapsed().as_millis() as u64,
        entry_count,
        rebuilt,
    };
    record_metric_response_index_stats(stats);
    Ok(stats)
}

/// Prebuild finalize: always rescan artifacts once off the request hot path.
pub fn rebuild_and_install_prebuild_metric_response_index(
    app_root: &Path,
) -> Result<MetricResponseIndexStats> {
    let started = Instant::now();
    let index = rebuild_prebuild_metric_response_index(app_root)?;
    let entry_count = install_prebuild_metric_response_index(index);
    let stats = MetricResponseIndexStats {
        load_ms: started.elapsed().as_millis() as u64,
        entry_count,
        rebuilt: true,
    };
    record_metric_response_index_stats(stats);
    Ok(stats)
}

fn ensure_prebuild_metric_response_index(app_root: &Path) -> Result<MetricResponseIndexStats> {
    if let Ok(guard) = prebuild_metric_response_index().lock() {
        if guard
            .as_ref()
            .is_some_and(|index| index.app_root == app_root)
        {
            let stats = MetricResponseIndexStats {
                load_ms: 0,
                entry_count: guard.as_ref().map(|index| index.entries.len()).unwrap_or(0),
                rebuilt: false,
            };
            record_metric_response_index_stats(stats);
            return Ok(stats);
        }
    }

    let started = Instant::now();
    if let Some(index) = try_load_prebuild_metric_response_index_from_sidecar(app_root, false)? {
        let entry_count = install_prebuild_metric_response_index(index);
        let stats = MetricResponseIndexStats {
            load_ms: started.elapsed().as_millis() as u64,
            entry_count,
            rebuilt: false,
        };
        record_metric_response_index_stats(stats);
        return Ok(stats);
    }

    let stats = MetricResponseIndexStats {
        load_ms: started.elapsed().as_millis() as u64,
        entry_count: 0,
        rebuilt: false,
    };
    record_metric_response_index_stats(stats);
    Ok(stats)
}

pub fn prebuild_metric_response_index_covers_key(
    app_root: &Path,
    response_cache_key: &str,
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> Result<bool> {
    ensure_prebuild_metric_response_index(app_root)?;
    let Ok(guard) = prebuild_metric_response_index().lock() else {
        return Ok(false);
    };
    let Some(index) = guard.as_ref() else {
        return Ok(false);
    };
    Ok(index
        .entries
        .iter()
        .find(|entry| entry.response_cache_key == response_cache_key)
        .is_some_and(|entry| {
            if request_all_metrics {
                entry.complete
            } else {
                requested_metric_ids
                    .iter()
                    .all(|metric_id| entry.covered_metric_ids.contains(metric_id))
            }
        }))
}

