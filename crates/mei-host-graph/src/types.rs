use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphNodeId {
    pub kind: GraphNodeKind,
    pub key: String,
}

impl GraphNodeId {
    pub fn new(kind: GraphNodeKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into(),
        }
    }

    pub fn stable_key(&self) -> String {
        format!("{}:{}", self.kind.slug(), self.key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    AppSkeleton,
    ScenePayload,
    ContentPanel,
    CatalogResource,
    MetricDefBundle,
    SemanticGraph,
    PageInstance,
    DataSource,
    EvalPlan,
    Workset,
    MaterialSlot,
    Navigation,
    WarmupPolicy,
    WorldModel,
}

impl GraphNodeKind {
    pub fn slug(self) -> &'static str {
        match self {
            Self::AppSkeleton => "app_skeleton",
            Self::ScenePayload => "scene_payload",
            Self::ContentPanel => "content_panel",
            Self::CatalogResource => "catalog_resource",
            Self::MetricDefBundle => "metric_def_bundle",
            Self::SemanticGraph => "semantic_graph",
            Self::PageInstance => "page_instance",
            Self::DataSource => "data_source",
            Self::EvalPlan => "eval_plan",
            Self::Workset => "workset",
            Self::MaterialSlot => "material_slot",
            Self::Navigation => "navigation",
            Self::WarmupPolicy => "warmup_policy",
            Self::WorldModel => "world_model",
        }
    }

    pub fn from_block_kind(kind: &str) -> Self {
        match kind {
            "app_skeleton" => Self::AppSkeleton,
            "content_panel" => Self::ContentPanel,
            "metric_def_bundle" => Self::MetricDefBundle,
            "page_instance" => Self::PageInstance,
            "navigation" | "link_decl" => Self::Navigation,
            "warmup_policy" => Self::WarmupPolicy,
            "world" => Self::WorldModel,
            _ => Self::SemanticGraph,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialState {
    Missing,
    Warming,
    Ready,
    Stale,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadRef {
    pub kind: String,
    #[serde(rename = "contentHash")]
    pub content_hash: String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
}

impl PayloadRef {
    pub fn new(
        kind: impl Into<String>,
        content_hash: impl Into<String>,
        schema_version: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            content_hash: content_hash.into(),
            schema_version: schema_version.into(),
        }
    }
}

pub fn stable_hash(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
