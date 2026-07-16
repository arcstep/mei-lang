use std::collections::BTreeMap;

use serde::Serialize;

use crate::graph::types::GraphNodeKind;

/// Layer gate identifier (L2–L4 per doc 86).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockLayer {
    L2,
    L3,
    L4,
}

impl BlockLayer {
    pub fn slug(self) -> &'static str {
        match self {
            Self::L2 => "L2",
            Self::L3 => "L3",
            Self::L4 => "L4",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockId {
    pub kind: GraphNodeKind,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "scopeKey")]
    pub scope_key: Option<String>,
}

impl BlockId {
    pub fn stable_key(&self) -> String {
        match self.scope_key.as_deref() {
            Some(scope) if !scope.is_empty() => {
                format!("{}:{}@{}", self.kind.slug(), self.key, scope)
            }
            _ => format!("{}:{}", self.kind.slug(), self.key),
        }
    }

    pub fn layer(&self) -> BlockLayer {
        match self.kind {
            GraphNodeKind::Navigation => BlockLayer::L2,
            GraphNodeKind::AppSkeleton
            | GraphNodeKind::ScenePayload
            | GraphNodeKind::ContentPanel
            | GraphNodeKind::CatalogResource
            | GraphNodeKind::MetricDefBundle
            | GraphNodeKind::SemanticGraph
            | GraphNodeKind::PageInstance
            | GraphNodeKind::WarmupPolicy
            | GraphNodeKind::WorldModel
            | GraphNodeKind::ObjectCatalog => BlockLayer::L3,
            GraphNodeKind::DataSource
            | GraphNodeKind::EvalPlan
            | GraphNodeKind::Workset
            | GraphNodeKind::MaterialSlot => BlockLayer::L4,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockTimingMs {
    pub compile_ms: u64,
    pub hydrate_ms: u64,
    pub query_ms: u64,
    pub eval_ms: u64,
    pub store_ms: u64,
    pub total_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockResult {
    pub ok: bool,
    pub block_id: BlockId,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "outputRevision")]
    pub output_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "contentHash")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "artifactPath")]
    pub artifact_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "slotState")]
    pub slot_state: Option<String>,
    pub timing: BlockTimingMs,
    #[serde(skip_serializing_if = "Option::is_none", rename = "errorChain")]
    pub error_chain: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl BlockResult {
    pub fn ok(block_id: BlockId, action: impl Into<String>) -> Self {
        Self {
            ok: true,
            block_id,
            action: action.into(),
            input_revision: None,
            output_revision: None,
            content_hash: None,
            artifact_path: None,
            rows: None,
            slot_state: None,
            timing: BlockTimingMs::default(),
            error_chain: None,
            details: BTreeMap::new(),
        }
    }

    pub fn err(block_id: BlockId, action: impl Into<String>, error: &anyhow::Error) -> Self {
        Self {
            ok: false,
            block_id,
            action: action.into(),
            input_revision: None,
            output_revision: None,
            content_hash: None,
            artifact_path: None,
            rows: None,
            slot_state: None,
            timing: BlockTimingMs::default(),
            error_chain: Some(format!("{error:#}")),
            details: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockEvalReport {
    pub ok: bool,
    pub app_id: String,
    pub scope_key: String,
    pub owner_resource_id: String,
    pub metric_ids: Vec<String>,
    pub results: Vec<BlockResult>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "errorChain")]
    pub error_chain: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockListReport {
    pub app_id: String,
    pub blocks: Vec<BlockListEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockListEntry {
    pub block_id: String,
    pub kind: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "scopeKey")]
    pub scope_key: Option<String>,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "lastError")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerVerifyReport {
    pub app_id: String,
    pub layer: String,
    pub ok: bool,
    pub alerts: Vec<LayerVerifyAlert>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerVerifyAlert {
    pub layer: String,
    pub block_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerStatusReport {
    pub app_id: String,
    pub mcg_nodes: usize,
    pub mrg_slots_ready: usize,
    pub mrg_slots_stale: usize,
    pub mrg_slots_failed: usize,
    pub dirty_slot_count: usize,
}
