use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;
use anyhow::Result;
use super::cache::{hash_fingerprint, store_cached_metric_dataframe_result};
use super::result_artifact::load_metric_dataframe_result_artifact;
use super::types::DatasetQueryResult;
use super::util::elapsed_ms;

pub(super) fn try_load_dataframe_result_artifact(app_root: &Path, lookup_cache_keys: &[String], response_cache_lookup_started: Instant) -> Result<Option<DatasetQueryResult>> {
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
            return Ok(Some(artifact));
        }
    Ok(None)
}
