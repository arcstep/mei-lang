use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::graph::content_store::{self, DATA_SOURCE, EVAL_PLAN, WORKSET};
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::types::{GraphNodeKind, MaterialState, PayloadRef};

pub const EVAL_PLAN_ARTIFACT_SCHEMA: &str = "mei-eval-plan-artifact-v1";
pub const WORKSET_ARTIFACT_SCHEMA: &str = "mei-workset-artifact-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalPlanArtifact {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "ownerResourceId")]
    pub owner_resource_id: String,
    #[serde(rename = "bundleRevision")]
    pub bundle_revision: String,
    #[serde(rename = "planContentHash")]
    pub plan_content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorksetArtifact {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "worksetId")]
    pub workset_id: String,
    #[serde(rename = "ownerResourceId")]
    pub owner_resource_id: String,
    #[serde(rename = "metricIds")]
    pub metric_ids: Vec<String>,
}

pub fn persist_eval_plan_node(
    source_root: &Path,
    app_id: &str,
    owner_resource_id: &str,
    bundle_revision: &str,
    plan_body: &serde_json::Value,
) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(plan_body)?;
    let put = content_store::put_if_absent(
        mei_lang_kernel::resolve_app_root(source_root, app_id).as_path(),
        EVAL_PLAN,
        &bytes,
    )?;
    let artifact = EvalPlanArtifact {
        schema_version: EVAL_PLAN_ARTIFACT_SCHEMA.to_string(),
        owner_resource_id: owner_resource_id.to_string(),
        bundle_revision: bundle_revision.to_string(),
        plan_content_hash: put.content_hash.clone(),
    };
    let artifact_bytes = serde_json::to_vec(&artifact)?;
    let _ = content_store::put_if_absent(
        mei_lang_kernel::resolve_app_root(source_root, app_id).as_path(),
        EVAL_PLAN,
        &artifact_bytes,
    )?;
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    registry.upsert_typed_node(serde_json::json!({
        "id": { "kind": GraphNodeKind::EvalPlan.slug(), "key": owner_resource_id },
        "ownerResourceId": owner_resource_id,
        "bundleRevision": bundle_revision,
        "planContentHash": put.content_hash,
        "state": MaterialState::Ready,
        "payloadRef": PayloadRef::new(EVAL_PLAN, put.content_hash.clone(), EVAL_PLAN_ARTIFACT_SCHEMA),
    }));
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)?;
    Ok(put.content_hash)
}

pub fn persist_workset_node(
    source_root: &Path,
    app_id: &str,
    workset_id: &str,
    owner_resource_id: &str,
    metric_ids: &[String],
) -> anyhow::Result<()> {
    let mut ids = metric_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    ids.sort();
    let artifact = WorksetArtifact {
        schema_version: WORKSET_ARTIFACT_SCHEMA.to_string(),
        workset_id: workset_id.to_string(),
        owner_resource_id: owner_resource_id.to_string(),
        metric_ids: ids.clone(),
    };
    let bytes = serde_json::to_vec(&artifact)?;
    let put = content_store::put_if_absent(
        mei_lang_kernel::resolve_app_root(source_root, app_id).as_path(),
        WORKSET,
        &bytes,
    )?;
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    registry.upsert_typed_node(serde_json::json!({
        "id": { "kind": GraphNodeKind::Workset.slug(), "key": workset_id },
        "worksetId": workset_id,
        "ownerResourceId": owner_resource_id,
        "metricIds": ids,
        "state": MaterialState::Ready,
        "payloadRef": PayloadRef::new(WORKSET, put.content_hash, WORKSET_ARTIFACT_SCHEMA),
    }));
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)?;
    Ok(())
}

pub fn persist_data_source_node(
    source_root: &Path,
    app_id: &str,
    source_key: &str,
    revision: &str,
    content_hash: &str,
) -> anyhow::Result<()> {
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    registry.upsert_typed_node(serde_json::json!({
        "id": { "kind": GraphNodeKind::DataSource.slug(), "key": source_key },
        "sourceKey": source_key,
        "revision": revision,
        "parquetContentHash": content_hash,
        "state": MaterialState::Ready,
        "payloadRef": PayloadRef::new(DATA_SOURCE, content_hash, "mei-data-source-artifact-v1"),
    }));
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)?;
    Ok(())
}
