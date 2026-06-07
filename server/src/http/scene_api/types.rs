use mei_lang_kernel::{RuntimeIntent, RuntimeSceneView, RuntimeState, RuntimeTraceItem};
use mei_lang_toolchain as toolchain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SimStepRequest {
    #[serde(default)]
    pub state: Option<RuntimeState>,
    pub intent: RuntimeIntent,
}

#[derive(Debug, Serialize)]
pub struct SimStepResponse {
    pub state: RuntimeState,
    pub scene_view: RuntimeSceneView,
    #[serde(default)]
    pub trace_delta: Vec<RuntimeTraceItem>,
    pub html: String,
}

pub(crate) type WorldScope = toolchain::WorldScope;
pub type ResourceQueryToolSpec = toolchain::ResourceQueryToolSpec;
pub type ResourceInventoryItem = toolchain::ResourceInventoryItem;
pub type ResourceInventorySnapshot = toolchain::ResourceInventorySnapshot;
pub type WorldContextSnapshot = toolchain::WorldContextSnapshot;
pub type WorldAssetListResponse = toolchain::WorldAssetListResponse;
pub type WorldAssetGetResponse = toolchain::WorldAssetGetResponse;
pub type WorldRuntimePeekResponse = toolchain::WorldRuntimePeekResponse;

#[derive(Debug, Deserialize)]
pub struct WorldAssetListQuery {
    #[serde(flatten)]
    pub scope: WorldScopeQuery,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct WorldAssetGetQuery {
    #[serde(flatten)]
    pub scope: WorldScopeQuery,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct WorldRuntimePeekQuery {
    #[serde(flatten)]
    pub scope: WorldScopeQuery,
    #[serde(default)]
    pub trace_limit: Option<usize>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WorldScopeQuery {
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
}

impl WorldScopeQuery {
    pub(crate) fn to_scope(&self) -> WorldScope {
        WorldScope {
            scene_id: self.scene_id.clone(),
            target_file: self.target_file.clone(),
        }
    }
}
