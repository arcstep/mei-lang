use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::content_store::{self, METRIC_DEF_BUNDLE};
use crate::graph::types::stable_hash;

pub const METRIC_DEF_BUNDLE_ARTIFACT_SCHEMA: &str = "mei-metric-def-bundle-artifact-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefBundleArtifact {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "ownerResourceId")]
    pub owner_resource_id: String,
    pub revision: String,
    #[serde(rename = "runtimeMetricDefs")]
    pub runtime_metric_defs: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefBundleRecord {
    pub owner_resource_id: String,
    pub revision: String,
    pub defs_fingerprint: String,
    pub metric_ids: Vec<String>,
    #[serde(skip)]
    pub runtime_metric_defs: BTreeMap<String, Value>,
}

pub fn extract_metric_def_bundles(
    compiled: &CompiledApp,
    dataset_runtime_payloads: &BTreeMap<String, DatasetRuntimePayloadView>,
) -> BTreeMap<String, MetricDefBundleRecord> {
    let mut bundles = BTreeMap::new();
    for resource in &compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        let defs = dataset_runtime_payloads
            .get(&resource.id)
            .map(|payload| payload.runtime_metric_defs.clone())
            .unwrap_or_else(|| dataset.runtime_metric_defs.clone());
        if defs.is_empty() {
            continue;
        }
        let mut metric_ids = defs.keys().cloned().collect::<Vec<_>>();
        metric_ids.sort();
        let fingerprint = metric_defs_fingerprint(&defs);
        let revision = format!("mdb:{fingerprint}");
        bundles.insert(
            resource.id.clone(),
            MetricDefBundleRecord {
                owner_resource_id: resource.id.clone(),
                revision,
                defs_fingerprint: fingerprint,
                metric_ids,
                runtime_metric_defs: defs,
            },
        );
    }
    bundles
}

#[derive(Debug, Clone, Default)]
pub struct DatasetRuntimePayloadView {
    pub runtime_metric_defs: BTreeMap<String, Value>,
}

pub fn metric_defs_fingerprint(defs: &BTreeMap<String, Value>) -> String {
    let serialized = serde_json::to_string(defs).unwrap_or_default();
    stable_hash(&serialized)
}

pub fn persist_metric_def_bundle(
    app_root: &Path,
    bundle: &MetricDefBundleRecord,
) -> anyhow::Result<String> {
    let artifact = MetricDefBundleArtifact {
        schema_version: METRIC_DEF_BUNDLE_ARTIFACT_SCHEMA.to_string(),
        owner_resource_id: bundle.owner_resource_id.clone(),
        revision: bundle.revision.clone(),
        runtime_metric_defs: bundle.runtime_metric_defs.clone(),
    };
    let bytes = serde_json::to_vec(&artifact)?;
    let put = content_store::put_if_absent(app_root, METRIC_DEF_BUNDLE, &bytes)?;
    Ok(put.content_hash)
}

pub fn load_metric_def_bundle(
    app_root: &Path,
    content_hash: &str,
) -> anyhow::Result<Option<MetricDefBundleArtifact>> {
    let Some(path) = content_store::get(app_root, METRIC_DEF_BUNDLE, content_hash) else {
        return Ok(None);
    };
    let raw = std::fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn metric_defs_fingerprint_changes_with_defs() {
        let mut defs = BTreeMap::new();
        defs.insert("m1".to_string(), json!({"shape": "scalar"}));
        let a = metric_defs_fingerprint(&defs);
        defs.insert("m2".to_string(), json!({"shape": "dataframe"}));
        let b = metric_defs_fingerprint(&defs);
        assert_ne!(a, b);
    }
}
