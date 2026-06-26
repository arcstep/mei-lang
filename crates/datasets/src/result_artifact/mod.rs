use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_kernel::{FilterIntent, MetricContract, QueryState};
use serde::{Deserialize, Serialize};

use crate::util::read_json_artifact_lenient;
use crate::DatasetQueryResult;

const METRIC_RESPONSE_RESULT_ARTIFACT_SCHEMA_VERSION: &str =
    "mei-metric-response-result-artifact-v1";
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
