use std::collections::BTreeMap;

use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::types::stable_hash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefBundleRecord {
    pub owner_resource_id: String,
    pub revision: String,
    pub defs_fingerprint: String,
    pub metric_ids: Vec<String>,
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
