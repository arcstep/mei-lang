use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::model::{ColumnSchema, EntityDecl, ResourceDecl, WorldGridDecl};

#[derive(Debug, Clone, Deserialize)]
pub(super) struct SceneFileRefDecl {
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct WorldFileRefDecl {
    #[serde(rename = "kind")]
    _kind: String,
    pub path: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct FrameFileRefDecl {
    #[serde(rename = "kind")]
    _kind: String,
    pub path: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct WorldAddResourceDecl {
    pub kind: String,
    pub resource: ResourceDecl,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct WorldAddEntityDecl {
    pub kind: String,
    pub entity: EntityDecl,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct WorldAddMetricDecl {
    pub kind: String,
    pub metric: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct WorldSetTopologyDecl {
    pub kind: String,
    pub topology: WorldGridDecl,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct FrameSetLayoutDecl {
    pub kind: String,
    pub layout: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LegacyDatasetNodeDecl {
    pub key: String,
    pub kind: String,
    #[serde(default)]
    pub columns: Vec<ColumnSchema>,
    #[serde(default)]
    pub normalize: BTreeMap<String, String>,
    #[serde(default)]
    pub rowset: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(super) struct LegacySourceDecl {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    /// 工作表名称；缺省为工作簿中的第一张表。
    #[serde(default)]
    pub sheet: Option<String>,
    /// 表头所在行号（从 1 计数，与 Excel 行号一致）；缺省为 1。
    #[serde(default)]
    pub header_row: Option<i64>,
    /// 编译期预览行数上限（用于惰性加载首屏快照）。
    #[serde(default)]
    pub preview_rows: Option<i64>,
    /// 组件默认分页大小。
    #[serde(default)]
    pub page_size: Option<i64>,
    /// 组件允许的最大分页大小。
    #[serde(default)]
    pub max_page_size: Option<i64>,
    /// 数据库表名（kind=db 时可选）。
    #[serde(default)]
    pub table: Option<String>,
    /// 数据库查询 SQL（kind=db 时可选；优先于 table）。
    #[serde(default)]
    pub query: Option<String>,
    /// 数据库连接串（kind=db 时使用，如 sqlite:///abs/path.db）。
    #[serde(default)]
    pub connection: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LegacyDatasetDecl {
    #[serde(default)]
    pub data_ref: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub source: LegacySourceDecl,
    pub dataset: LegacyDatasetNodeDecl,
    #[serde(default)]
    pub metrics: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LegacyMetricPackDecl {
    pub metric_pack: LegacyMetricPackMetaDecl,
    #[serde(default)]
    pub metrics: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LegacyMetricPackMetaDecl {
    pub id: String,
    #[serde(default)]
    pub purpose: Option<String>,
}
