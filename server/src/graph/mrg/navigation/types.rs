use serde::{Deserialize, Serialize};

use crate::graph::types::MaterialState;
use crate::readiness::types::{ScopeCoords, UiMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationEntry {
    pub key: String,
    pub url: String,
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
        self.entry
            .as_ref()
            .is_some_and(NavigationEntry::is_ready)
    }

    pub fn navigation_key(&self) -> Option<String> {
        self.entry.as_ref().map(|entry| entry.key.clone())
    }
}

pub fn parse_navigation_node(value: &serde_json::Value) -> Option<NavigationEntry> {
    use crate::graph::types::GraphNodeKind;
    let id = value.get("id")?;
    let kind = id.get("kind")?.as_str()?;
    if kind != GraphNodeKind::Navigation.slug() {
        return None;
    }
    let key = id.get("key")?.as_str()?.to_string();
    let url = value.get("url")?.as_str()?.to_string();
    let scene_id = value
        .get("sceneId")
        .or_else(|| value.get("scene_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let target_file = value
        .get("targetFile")
        .or_else(|| value.get("target_file"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let state = value
        .get("state")
        .and_then(|v| v.as_str())
        .map(|text| match text {
            "ready" => MaterialState::Ready,
            "stale" => MaterialState::Stale,
            "warming" => MaterialState::Warming,
            "failed" => MaterialState::Failed,
            _ => MaterialState::Missing,
        })
        .unwrap_or(MaterialState::Ready);
    Some(NavigationEntry {
        key,
        url,
        scene_id,
        target_file,
        state,
    })
}
