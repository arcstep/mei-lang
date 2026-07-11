use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::DatasetView;

use super::super::build::eval_rowset_with_ctx;
use crate::compile::analysis::{
    eval_context::EvalContext,
    transforms::{pivot_long_rows, unpivot_columns_rows},
};

pub(super) fn eval_rowset_unpivot_columns(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let rowset_expr = map
        .get("rowset")
        .ok_or_else(|| anyhow!("unpivot_columns expression missing rowset"))?;
    let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
    let id_field = map
        .get("id_field")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("unpivot_columns expression missing id_field"))?;
    let year_field = map
        .get("year_field")
        .and_then(Value::as_str)
        .unwrap_or("year");
    let value_field = map
        .get("value_field")
        .and_then(Value::as_str)
        .unwrap_or("value");
    let columns = map
        .get("columns")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("unpivot_columns expression missing columns"))?
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let year = object.get("year")?.as_str()?.to_string();
            let field = object.get("field")?.as_str()?.to_string();
            Some((year, field))
        })
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(anyhow!(
            "unpivot_columns expression missing column mappings"
        ));
    }
    Ok(unpivot_columns_rows(
        &rows,
        id_field,
        &columns,
        year_field,
        value_field,
    ))
}

pub(super) fn eval_rowset_pivot_long(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let rowset_expr = map
        .get("rowset")
        .ok_or_else(|| anyhow!("pivot_long expression missing rowset"))?;
    let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
    let row_field = map
        .get("row_field")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("pivot_long expression missing row_field"))?;
    let column_field = map
        .get("column_field")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("pivot_long expression missing column_field"))?;
    let value_field = map
        .get("value_field")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("pivot_long expression missing value_field"))?;
    let columns = map
        .get("columns")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .or_else(|| item.as_i64().map(|value| value.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .ok_or_else(|| anyhow!("pivot_long expression missing columns"))?;
    let row_universe = map
        .get("row_universe")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty());
    Ok(pivot_long_rows(
        &rows,
        row_field,
        column_field,
        value_field,
        &columns,
        row_universe.as_deref(),
    ))
}
