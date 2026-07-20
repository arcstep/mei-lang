use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_kernel::{FilterIntent, MetricContract, QueryState};
use serde::{Deserialize, Serialize};

use crate::eval_cache_io_stats::{
    record_artifact_write, record_response_store_atomic, record_response_store_skipped,
};
use crate::util::read_json_artifact_lenient;
use crate::DatasetQueryResult;

const METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-response-result-artifact-v1";
const METRIC_RESPONSE_LITE_ARTIFACT_SCHEMA_VERSION: &str = "mei-metric-response-lite-v1";
const METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-dataframe-result-artifact-v1";
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricResponseIndexStats {
    pub load_ms: u64,
    pub entry_count: usize,
    pub rebuilt: bool,
}

include!("core.rs");
include!("store.rs");

#[cfg(test)]
mod lite_artifact_tests {
    use super::*;
    use mei_lang_kernel::MetricShape;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn sample_contract(id: &str, value: serde_json::Value) -> MetricContract {
        MetricContract {
            id: id.to_string(),
            label: None,
            unit: None,
            purpose: None,
            shape: MetricShape::Scalar,
            schema: Vec::new(),
            value,
            value_format: None,
            dataset: None,
            transforms: Vec::new(),
        }
    }

    fn temp_app_root() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let app_root = temp.path().to_path_buf();
        let env_dir = app_root.join("env").join("WS-20260720.0");
        fs::create_dir_all(env_dir.join("build")).expect("mkdir build");
        fs::create_dir_all(env_dir.join("var")).expect("mkdir var");
        let current = app_root.join("env").join("current");
        #[cfg(unix)]
        std::os::unix::fs::symlink("WS-20260720.0", &current).expect("symlink env/current");
        #[cfg(not(unix))]
        fs::create_dir_all(&current).expect("mkdir env/current");
        (temp, app_root)
    }

    #[test]
    fn store_dual_writes_lite_and_hydrate_skips_full() {
        let (_temp, app_root) = temp_app_root();
        let app_root = app_root.as_path();
        let mut metrics_map = BTreeMap::new();
        metrics_map.insert(
            "kpi_count".to_string(),
            sample_contract("kpi_count", serde_json::json!({"value": 1})),
        );
        metrics_map.insert(
            "kpi_count::__scalar_rowset__".to_string(),
            sample_contract(
                "kpi_count::__scalar_rowset__",
                serde_json::json!({"rows": (0..2000).collect::<Vec<_>>()}),
            ),
        );
        let covered = BTreeSet::from([
            "kpi_count".to_string(),
            "kpi_count::__scalar_rowset__".to_string(),
        ]);
        store_metric_response_result_artifact(
            app_root,
            "cache-key-1",
            2000,
            &metrics_map,
            &covered,
            true,
        )
        .expect("store");
        assert!(metric_response_result_artifact_exists(
            app_root,
            "cache-key-1"
        ));
        let lite_path = metric_response_lite_artifact_path(app_root, "cache-key-1");
        assert!(lite_path.is_file());

        let before = take_lite_artifact_io_stats();
        let _ = before;
        let loaded = load_metric_response_lite_artifact(app_root, "cache-key-1")
            .expect("load lite")
            .expect("lite present");
        assert!(loaded.0.metrics_map.contains_key("kpi_count"));
        assert!(!loaded
            .0
            .metrics_map
            .contains_key("kpi_count::__scalar_rowset__"));
        let stats = take_lite_artifact_io_stats();
        assert_eq!(stats.lite_hydrated, 1);
        assert_eq!(stats.full_artifact_loads, 0);
        assert_eq!(stats.lite_backfill, 0);
    }

    #[test]
    fn missing_lite_backfills_from_full_once() {
        let (_temp, app_root) = temp_app_root();
        let app_root = app_root.as_path();
        let mut metrics_map = BTreeMap::new();
        metrics_map.insert(
            "kpi_only".to_string(),
            sample_contract("kpi_only", serde_json::json!({"value": 9})),
        );
        let covered = BTreeSet::from(["kpi_only".to_string()]);
        // Write full only (simulate legacy cache).
        let path = metric_response_result_artifact_path(app_root, "legacy-key");
        let persisted = PersistedMetricResponseResultArtifact {
            schema_version: METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION.to_string(),
            response_cache_key: "legacy-key".to_string(),
            total_rows: 1,
            metrics_map: metrics_map.clone(),
            covered_metric_ids: covered.clone(),
            complete: true,
            generated_at_ms: 1,
            slot_revision: None,
        };
        write_json_artifact(&path, &persisted).expect("write full");
        let _ = take_lite_artifact_io_stats();
        let loaded = load_metric_response_lite_artifact(app_root, "legacy-key")
            .expect("backfill")
            .expect("lite");
        assert!(loaded.0.metrics_map.contains_key("kpi_only"));
        assert!(metric_response_lite_artifact_path(app_root, "legacy-key").is_file());
        let stats = take_lite_artifact_io_stats();
        assert_eq!(stats.lite_backfill, 1);
        assert_eq!(stats.lite_hydrated, 1);
        assert_eq!(stats.full_artifact_loads, 0);
    }
}
