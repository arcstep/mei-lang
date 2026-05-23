
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentAsset {
    pub key: String,
    pub tag: String,
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceAppMeta {
    pub id: String,
    pub title: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceNode {
    pub name: String,
    pub path: String,
    pub kind: String,
    /// `.mei` 语义：`main` / `scene` / `mei`（普通脚本）；非 mei 文件为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mei_kind: Option<String>,
    #[serde(default)]
    pub children: Vec<WorkspaceNode>,
}
