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
    PanelContract,
    CatalogResource,
    MetricDefBundle,
    SemanticGraph,
    AssemblyView,
    DataSource,
    EvalPlan,
    Workset,
    MaterialSlot,
    Navigation,
}

impl GraphNodeKind {
    pub fn slug(self) -> &'static str {
        match self {
            Self::AppSkeleton => "app_skeleton",
            Self::ScenePayload => "scene_payload",
            Self::PanelContract => "panel_contract",
            Self::CatalogResource => "catalog_resource",
            Self::MetricDefBundle => "metric_def_bundle",
            Self::SemanticGraph => "semantic_graph",
            Self::AssemblyView => "assembly_view",
            Self::DataSource => "data_source",
            Self::EvalPlan => "eval_plan",
            Self::Workset => "workset",
            Self::MaterialSlot => "material_slot",
            Self::Navigation => "navigation",
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
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "contentHash")]
    pub content_hash: Option<String>,
}

pub fn stable_hash(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
