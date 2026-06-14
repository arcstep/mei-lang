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
    /// build 资源树：`scene_export` 子节点对应的 export id（文件路径见 `path`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_export_id: Option<String>,
    /// `world_dataset` 子节点对应的 dataset id（文件路径见 `path`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_dataset_id: Option<String>,
    /// `world_metric` / `explain_block` 父 metric id。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_metric_id: Option<String>,
    /// `explain_block` 子节点 id。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain_block_id: Option<String>,
    /// 语义检视展示 label；缺省时回退 `name`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_label: Option<String>,
    #[serde(default)]
    pub children: Vec<WorkspaceNode>,
}
