use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_kernel::{FilterIntent, MetricContract, QueryState};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::metric_response_cache::{
    metric_response_prebuild_dataset_key, prebuild_metric_response_key_matches_dataset_query,
};
use crate::types::DatasetQueryOptions;
use crate::util::read_json_artifact_lenient;
use crate::DatasetQueryResult;

const METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-response-result-artifact-v1";
const METRIC_DATAFRAME_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-dataframe-result-artifact-v1";
const METRIC_RESPONSE_INDEX_SCHEMA_VERSION: &str = "mei-metric-response-index-v1";

#[derive(Debug, Clone, Copy, Default)]
pub struct MetricResponseIndexStats {
    pub load_ms: u64,
    pub entry_count: usize,
    pub rebuilt: bool,
}

thread_local! {
    static LAST_METRIC_RESPONSE_INDEX_STATS: Cell<MetricResponseIndexStats> =
        Cell::new(MetricResponseIndexStats::default());
}

include!("core.rs");
include!("index_a.rs");
include!("index_b.rs");
include!("store.rs");
