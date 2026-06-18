//! Rowset 热算子路由：`MEI_ROWSET_ENGINE=columnar` 时走列式快路径，否则回退 JSON。

mod columnar;
mod polars_gate;

pub use columnar::{try_group_by_count_columnar, try_where_eq_columnar};
pub(crate) use polars_gate::try_polars_group_by;
