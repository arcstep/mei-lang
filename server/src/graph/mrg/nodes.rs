use serde::{Deserialize, Serialize};

use crate::graph::types::{GraphNodeId, MaterialState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MrgNodeRecord {
    Navigation {
        id: GraphNodeId,
        url: String,
        #[serde(rename = "sceneId")]
        scene_id: String,
        #[serde(rename = "targetFile")]
        target_file: String,
        state: MaterialState,
    },
    EvalPlan {
        id: GraphNodeId,
        #[serde(rename = "ownerResourceId")]
        owner_resource_id: String,
        #[serde(rename = "bundleRevision")]
        bundle_revision: String,
        #[serde(rename = "planContentHash")]
        plan_content_hash: String,
        state: MaterialState,
    },
    Workset {
        id: GraphNodeId,
        #[serde(rename = "worksetId")]
        workset_id: String,
        #[serde(rename = "metricIds")]
        metric_ids: Vec<String>,
        #[serde(rename = "ownerResourceId")]
        owner_resource_id: String,
        state: MaterialState,
    },
    DataSource {
        id: GraphNodeId,
        #[serde(rename = "sourceKey")]
        source_key: String,
        revision: String,
        #[serde(rename = "parquetContentHash")]
        parquet_content_hash: String,
        state: MaterialState,
    },
}

impl MrgNodeRecord {
    pub fn navigation(
        key: &str,
        url: &str,
        scene_id: &str,
        target_file: &str,
        state: MaterialState,
    ) -> Self {
        Self::Navigation {
            id: GraphNodeId::new(crate::graph::types::GraphNodeKind::Navigation, key.to_string()),
            url: url.to_string(),
            scene_id: scene_id.to_string(),
            target_file: target_file.to_string(),
            state,
        }
    }

    pub fn from_legacy_json(value: &serde_json::Value) -> Option<Self> {
        if let Some(entry) = crate::graph::mrg::navigation::types::parse_navigation_node(value) {
            return Some(Self::navigation(
                entry.key.as_str(),
                entry.url.as_str(),
                entry.scene_id.as_str(),
                entry.target_file.as_str(),
                entry.state,
            ));
        }
        let id = value.get("id")?;
        let kind = id.get("kind")?.as_str()?;
        let key = id.get("key")?.as_str()?.to_string();
        let state = parse_material_state(value.get("state"));
        match kind {
            "eval_plan" => Some(Self::EvalPlan {
                id: GraphNodeId::new(crate::graph::types::GraphNodeKind::EvalPlan, key),
                owner_resource_id: text_field(value, "ownerResourceId")?,
                bundle_revision: text_field(value, "bundleRevision")?,
                plan_content_hash: text_field(value, "planContentHash")?,
                state,
            }),
            "workset" => Some(Self::Workset {
                id: GraphNodeId::new(crate::graph::types::GraphNodeKind::Workset, key),
                workset_id: text_field(value, "worksetId")?,
                metric_ids: value
                    .get("metricIds")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                owner_resource_id: text_field(value, "ownerResourceId")?,
                state,
            }),
            "data_source" => Some(Self::DataSource {
                id: GraphNodeId::new(crate::graph::types::GraphNodeKind::DataSource, key),
                source_key: text_field(value, "sourceKey")?,
                revision: text_field(value, "revision")?,
                parquet_content_hash: text_field(value, "parquetContentHash")?,
                state,
            }),
            _ => None,
        }
    }

    pub fn to_legacy_json(&self) -> serde_json::Value {
        match self {
            Self::Navigation {
                id,
                url,
                scene_id,
                target_file,
                state,
            } => serde_json::json!({
                "id": id,
                "url": url,
                "sceneId": scene_id,
                "targetFile": target_file,
                "state": material_state_slug(state.clone()),
            }),
            Self::EvalPlan {
                id,
                owner_resource_id,
                bundle_revision,
                plan_content_hash,
                state,
            } => serde_json::json!({
                "id": id,
                "ownerResourceId": owner_resource_id,
                "bundleRevision": bundle_revision,
                "planContentHash": plan_content_hash,
                "state": material_state_slug(state.clone()),
            }),
            Self::Workset {
                id,
                workset_id,
                metric_ids,
                owner_resource_id,
                state,
            } => serde_json::json!({
                "id": id,
                "worksetId": workset_id,
                "metricIds": metric_ids,
                "ownerResourceId": owner_resource_id,
                "state": material_state_slug(state.clone()),
            }),
            Self::DataSource {
                id,
                source_key,
                revision,
                parquet_content_hash,
                state,
            } => serde_json::json!({
                "id": id,
                "sourceKey": source_key,
                "revision": revision,
                "parquetContentHash": parquet_content_hash,
                "state": material_state_slug(state.clone()),
            }),
        }
    }
}

fn text_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|text| !text.is_empty())
}

fn parse_material_state(value: Option<&serde_json::Value>) -> MaterialState {
    value
        .and_then(|v| v.as_str())
        .map(|text| match text {
            "ready" => MaterialState::Ready,
            "stale" => MaterialState::Stale,
            "warming" => MaterialState::Warming,
            "failed" => MaterialState::Failed,
            _ => MaterialState::Missing,
        })
        .unwrap_or(MaterialState::Ready)
}

fn material_state_slug(state: MaterialState) -> &'static str {
    match state {
        MaterialState::Ready => "ready",
        MaterialState::Stale => "stale",
        MaterialState::Warming => "warming",
        MaterialState::Failed => "failed",
        MaterialState::Missing => "missing",
    }
}

pub fn deserialize_mrg_nodes<'de, D>(deserializer: D) -> Result<Vec<MrgNodeRecord>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(values
        .iter()
        .filter_map(MrgNodeRecord::from_legacy_json)
        .collect())
}

pub fn serialize_mrg_nodes<S>(
    nodes: &[MrgNodeRecord],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(nodes.len()))?;
    for node in nodes {
        seq.serialize_element(&node.to_legacy_json())?;
    }
    seq.end()
}
