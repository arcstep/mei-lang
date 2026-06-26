mod basic;
mod aggregate;
mod trend;
mod pivot;

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::DatasetView;
use crate::compile::analysis::eval_context::EvalContext;

use basic::*;
use aggregate::*;
use trend::*;
use pivot::*;

pub(super) fn eval_analysis_rowset(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let analysis_type = map
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("analysis expression missing type"))?;
    match analysis_type {
        "rows" => eval_rowset_rows(map, datasets, ctx),
        "where" => eval_rowset_where(map, datasets, ctx),
        "select" => eval_rowset_select(map, datasets, ctx),
        "rename" => eval_rowset_rename(map, datasets, ctx),
        "mutate" => eval_rowset_mutate(map, datasets, ctx),
        "sort_by" => eval_rowset_sort_by(map, datasets, ctx),
        "reorder" => eval_rowset_reorder(map, datasets, ctx),
        "stage" => eval_rowset_stage(map, datasets, ctx),
        "first_by" => eval_rowset_first_by(map, datasets, ctx),
        "distinct_by" => eval_rowset_distinct_by(map, datasets, ctx),
        "table_rows" => eval_rowset_table_rows(map, datasets, ctx),
        "split_text" => eval_rowset_split_text(map, datasets, ctx),
        "lookup_value" => eval_rowset_lookup_value(map, datasets, ctx),
        "latest_days" | "latest_months" => eval_rowset_latest_window(map, datasets, ctx),
        "bucket_date" => eval_rowset_bucket_date(map, datasets, ctx),
        "limit" => eval_rowset_limit(map, datasets, ctx),
        "concat_rowsets" => eval_rowset_concat_rowsets(map, datasets, ctx),
        "group_by" => eval_rowset_group_by(map, datasets, ctx),
        "agg" => eval_rowset_agg(map, datasets, ctx),
        "party_year_aggregate" => eval_rowset_party_year_aggregate(map, datasets, ctx),
        "trend" => eval_rowset_trend(map, datasets, ctx),
        "trend_year_compare" => eval_rowset_trend_year_compare(map, datasets, ctx),
        "unpivot_columns" => eval_rowset_unpivot_columns(map, datasets, ctx),
        "pivot_long" => eval_rowset_pivot_long(map, datasets, ctx),
        other => Err(anyhow!("unsupported rowset analysis `{other}`")),
    }
}
