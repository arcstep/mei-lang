
use serde::{Deserialize, Serialize};


/// One node in the build-view reachability tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReachabilityTreeNode {
    pub id: String,
    pub node_id: String,
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub badges: Vec<String>,
    /// Compile scene anchor (`home`, board export id, …) for fast client navigation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compile_scene: String,
    /// Compile preview target file (`scenes/home.mei`, `*.board.mei`, …).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compile_target: String,
    /// Board slot layout zone (`filter`, `chart`, `detail`, …) for build preview inspect.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub board_layout_zone: String,
    /// UI structure role (`scene`, `plane`, `region`, `section`, `content`, …).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ui_role: String,
    /// Scene-relative preview scope path for client dim/highlight.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preview_scope: String,
    /// Plane tier (`T0`, `T1`, `T2`, `P`, `C`, `H`) for plane-level dim.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub plane_tier: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ReachabilityTreeNode>,
}

impl Default for ReachabilityTreeNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            node_id: String::new(),
            kind: String::new(),
            label: String::new(),
            badges: Vec::new(),
            compile_scene: String::new(),
            compile_target: String::new(),
            board_layout_zone: String::new(),
            ui_role: String::new(),
            preview_scope: String::new(),
            plane_tier: String::new(),
            children: Vec::new(),
        }
    }
}

/// Top-level grouping root in the build-view sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReachabilityTreeRoot {
    pub group: String,
    pub label: String,
    #[serde(default)]
    pub default_open: bool,
    #[serde(default)]
    pub children: Vec<ReachabilityTreeNode>,
}

