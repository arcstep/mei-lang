//! Polars 后端门控：默认关闭；启用 `mei-lang-kernel/polars` feature 后再评估接入。

use serde_json::Value;

pub fn polars_engine_available() -> bool {
    cfg!(feature = "polars")
}

/// 预留 Polars 后端；当前始终回退 JSON/columnar 路径。
pub fn try_polars_group_by(_rows: &[Value], _group_field: &str, _agg: &str) -> Option<Vec<Value>> {
    if !polars_engine_available() {
        return None;
    }
    None
}
