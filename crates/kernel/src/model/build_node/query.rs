use super::tab_visible_for_node;

use super::{BuildNodeId, BuildNodeKind, BuildViewTab};

use serde::{Deserialize, Serialize};

/// Legacy build-view query fields (`file`, `world_*`, old tabs) mapped to canonical node + tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBuildQuery {
    pub file: Option<String>,
    pub scene: Option<String>,
    pub world_metric: Option<String>,
    pub world_dataset: Option<String>,
    pub explain: Option<String>,
    pub tab: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBuildViewQuery {
    pub node: BuildNodeId,
    pub tab: BuildViewTab,
    pub scope: BuildExecScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildExecScope {
    #[default]
    Warmup,
    Empty,
    LastRequest,
    Custom,
}

impl BuildExecScope {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Warmup => "warmup",
            Self::Empty => "empty",
            Self::LastRequest => "last_request",
            Self::Custom => "custom",
        }
    }

    pub fn parse_slug(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "empty" => Self::Empty,
            "last_request" => Self::LastRequest,
            "custom" => Self::Custom,
            _ => Self::Warmup,
        }
    }
}

pub fn resolve_build_view_query(
    node: Option<&str>,
    scope: Option<&str>,
    tab: Option<&str>,
    legacy: &LegacyBuildQuery,
) -> Option<ResolvedBuildViewQuery> {
    let exec_scope = scope.map(BuildExecScope::parse_slug).unwrap_or_default();

    if let Some(raw_node) = node.map(str::trim).filter(|value| !value.is_empty()) {
        let parsed = BuildNodeId::parse(raw_node)?;
        let tab = tab
            .and_then(BuildViewTab::parse_slug)
            .filter(|candidate| tab_visible_for_node(&parsed, *candidate))
            .unwrap_or_else(|| parsed.default_tab());
        return Some(ResolvedBuildViewQuery {
            node: parsed,
            tab,
            scope: exec_scope,
        });
    }

    let file = legacy
        .file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let tab = legacy
        .tab
        .as_deref()
        .and_then(BuildViewTab::parse_slug)
        .unwrap_or(BuildViewTab::Preview);

    if let Some(metric) = legacy
        .world_metric
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let node = if let Some(explain) = legacy
            .explain
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            BuildNodeId::world_explain(file, metric, explain)
        } else {
            BuildNodeId::world_metric(file, metric)
        };
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    if let Some(dataset) = legacy
        .world_dataset
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let node = BuildNodeId::world_dataset(file, dataset);
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    if file.ends_with(".world.mei") {
        let node = BuildNodeId::new(BuildNodeKind::WorldFile, file);
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    if file.ends_with(".board.mei") || file.ends_with(".page.mei") {
        let node = if let Some(scene) = legacy
            .scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            BuildNodeId::board_file(format!("{file}#{scene}"))
        } else {
            BuildNodeId::board_file(file)
        };
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    if file.ends_with(".mei") && !file.ends_with(".board.mei") && !file.ends_with(".page.mei") {
        if let Some(scene_id) = scene_id_from_scene_mei_path(file) {
            let node = BuildNodeId::scene(scene_id);
            let tab = if tab_visible_for_node(&node, tab) {
                tab
            } else {
                node.default_tab()
            };
            return Some(ResolvedBuildViewQuery {
                node,
                tab,
                scope: exec_scope,
            });
        }
    }

    if let Some(scene) = legacy
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let node = BuildNodeId::scene(scene);
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    let node = BuildNodeId::new(BuildNodeKind::WorldFile, file);
    let tab = if tab_visible_for_node(&node, tab) {
        tab
    } else {
        BuildViewTab::Preview
    };
    Some(ResolvedBuildViewQuery {
        node,
        tab,
        scope: exec_scope,
    })
}

/// Scene export id for scene capsule `.mei` paths (`src/scene/home/assembly.mei` → `home`).
fn scene_id_from_scene_mei_path(file: &str) -> Option<String> {
    let normalized = file.trim().replace('\\', "/");
    if normalized.ends_with("/assembly.mei") {
        let parent = normalized.strip_suffix("/assembly.mei")?;
        if parent.is_empty() {
            return None;
        }
        if let Some(scene_id) = parent.rsplit('/').next().filter(|value| !value.is_empty()) {
            return Some(scene_id.to_string());
        }
    }
    normalized
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".mei"))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
