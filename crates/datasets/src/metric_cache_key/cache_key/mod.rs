use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_data_snapshot_import_entry,
    resolve_versioned_source_identifier, resolve_versioned_source_path,
    source_file_content_signature, CompiledApp, DatasetView, FilterIntent, QueryState,
    RuntimeMetricEvalScope,
};
use serde::Serialize;
use serde_json::Value;

use crate::idempotency_key::{
    metric_shared_cache_key_with_data_generation, resolve_metric_data_generation,
};
use crate::metric_hydrate::collect_dataset_ids_from_metric_defs;
use crate::metric_locate::locate_runtime_metric_resource;
use crate::metric_response_cache::{
    metric_response_cache_scope_key, metric_response_prebuild_dataset_key,
    metric_response_prebuild_shared_key,
};
use crate::types::DatasetQueryOptions;

use super::query_normalize::{
    dimension_bindings_from_query_state, dimension_bindings_from_query_state_for_datasets,
    filter_intents_from_request, normalize_query_filters, query_state_from_request,
};

const REVISION_FINGERPRINT_CACHE_TTL: Duration = Duration::from_millis(4000);
const REVISION_FINGERPRINT_CACHE_MAX: usize = 512;

#[derive(Clone)]
struct RevisionFingerprintCacheEntry {
    value: String,
    cached_at: Instant,
}

fn revision_fingerprint_cache() -> &'static Mutex<BTreeMap<String, RevisionFingerprintCacheEntry>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, RevisionFingerprintCacheEntry>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn graph_slot_revision_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MEI_GRAPH_REGISTRY")
            .map(|value| {
                let trimmed = value.trim();
                trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
            })
            .unwrap_or(false)
    })
}

include!("identity.rs");
include!("scope.rs");
include!("lookup.rs");
