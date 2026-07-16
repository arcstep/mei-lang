use serde::{Deserialize, Serialize};

use crate::graph::types::MaterialState;
use crate::readiness::types::{ScopeCoords, UiMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationEntry {
    pub key: String,
    pub url: String,
    /// Access Stage id (wire: `stageId`; still accepts legacy `sceneId`).
    #[serde(rename = "stageId", alias = "sceneId")]
    pub scene_id: String,
    pub target_file: String,
    pub state: MaterialState,
}

impl NavigationEntry {
    pub fn to_scope_coords(&self, app_id: &str, mode: UiMode) -> ScopeCoords {
        ScopeCoords::new(
            app_id,
            mode,
            self.scene_id.as_str(),
            self.target_file.as_str(),
        )
    }

    pub fn is_ready(&self) -> bool {
        self.state == MaterialState::Ready
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationMatch {
    pub entry: Option<NavigationEntry>,
    pub scope: ScopeCoords,
    pub legacy_fallback: bool,
}

impl NavigationMatch {
    pub fn navigation_ready(&self) -> bool {
        if self.legacy_fallback {
            return false;
        }
        self.entry.as_ref().is_some_and(NavigationEntry::is_ready)
    }

    pub fn navigation_key(&self) -> Option<String> {
        self.entry.as_ref().map(|entry| entry.key.clone())
    }
}
