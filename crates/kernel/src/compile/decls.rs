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
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct FrameFileRefDecl {
    pub kind: String,
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
pub(super) struct DatasetViewDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub rowset: Option<Value>,
    #[serde(default)]
    pub schema: Vec<ColumnSchema>,
    #[serde(default)]
    pub metrics: Vec<MetricDecl>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct MetricDecl {
    pub kind: String,
    pub metric_type: String,
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub schema: Vec<ColumnSchema>,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
    #[serde(default)]
    pub value: Option<Value>,
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
