//! Shared Pack-First resolution: L1 is handled by callers; this module loads
//! disk lite (non-bulk) then full metric-response artifacts before compute.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use super::result_artifact::{
    load_metric_response_lite_artifact, load_metric_response_result_artifact,
    LoadedMetricResponseArtifact,
};

#[derive(Debug, Clone)]
pub struct DiskMetricResponseHit {
    pub cache_key: String,
    pub artifact: LoadedMetricResponseArtifact,
    pub load_ms: u64,
    /// `lite` or `full`
    pub source: &'static str,
}

/// Try disk lite (when `prefer_lite`) then full packs for any of `lookup_cache_keys`.
/// Returns the first artifact that covers the request.
pub fn try_load_disk_metric_response(
    app_root: &Path,
    lookup_cache_keys: &[String],
    requested: &BTreeSet<String>,
    request_all_metrics: bool,
    prefer_lite: bool,
) -> Result<Option<DiskMetricResponseHit>> {
    if prefer_lite {
        for cache_key in lookup_cache_keys {
            let Some((artifact, load_ms, _stats)) =
                load_metric_response_lite_artifact(app_root, cache_key.as_str())?
            else {
                continue;
            };
            if artifact_covers_request(&artifact, requested, request_all_metrics) {
                return Ok(Some(DiskMetricResponseHit {
                    cache_key: cache_key.clone(),
                    artifact,
                    load_ms,
                    source: "lite",
                }));
            }
        }
    }

    for cache_key in lookup_cache_keys {
        let Some((artifact, load_ms)) =
            load_metric_response_result_artifact(app_root, cache_key.as_str())?
        else {
            continue;
        };
        if artifact_covers_request(&artifact, requested, request_all_metrics) {
            return Ok(Some(DiskMetricResponseHit {
                cache_key: cache_key.clone(),
                artifact,
                load_ms,
                source: "full",
            }));
        }
    }
    Ok(None)
}

fn artifact_covers_request(
    artifact: &LoadedMetricResponseArtifact,
    requested: &BTreeSet<String>,
    request_all_metrics: bool,
) -> bool {
    if request_all_metrics {
        artifact.complete
            && requested.iter().all(|metric_id| {
                artifact.covered_metric_ids.contains(metric_id)
                    || artifact.metrics_map.contains_key(metric_id)
            })
    } else {
        requested.iter().all(|metric_id| {
            artifact.covered_metric_ids.contains(metric_id)
                || artifact.metrics_map.contains_key(metric_id)
        })
    }
}
