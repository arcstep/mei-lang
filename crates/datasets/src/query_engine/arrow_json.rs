use anyhow::{bail, Result};
use datafusion::arrow::array::{
    Array, BooleanArray, Date32Array, Float64Array, Int32Array, Int64Array, StringArray,
    TimestampMicrosecondArray, TimestampMillisecondArray, TimestampNanosecondArray,
    TimestampSecondArray,
};
use datafusion::arrow::datatypes::{DataType, TimeUnit};
use datafusion::arrow::record_batch::RecordBatch;
use serde_json::{json, Map, Value};

pub fn batches_to_json_rows(batches: &[RecordBatch]) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for batch in batches {
        let names: Vec<String> = batch
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect();
        for row_idx in 0..batch.num_rows() {
            let mut map = Map::new();
            for (col_idx, name) in names.iter().enumerate() {
                map.insert(
                    name.clone(),
                    array_value_to_json(batch.column(col_idx), row_idx)?,
                );
            }
            rows.push(Value::Object(map));
        }
    }
    Ok(rows)
}

pub fn first_scalar_i64(batches: &[RecordBatch]) -> Result<i64> {
    let Some(batch) = batches.first() else {
        bail!("empty scalar result");
    };
    if batch.num_rows() == 0 || batch.num_columns() == 0 {
        bail!("empty scalar result");
    }
    match array_value_to_json(batch.column(0), 0)? {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .ok_or_else(|| anyhow::anyhow!("scalar is not i64")),
        Value::Null => Ok(0),
        other => bail!("scalar is not i64: {other}"),
    }
}

pub fn first_scalar_f64(batches: &[RecordBatch]) -> Result<f64> {
    let Some(batch) = batches.first() else {
        bail!("empty scalar result");
    };
    if batch.num_rows() == 0 || batch.num_columns() == 0 {
        bail!("empty scalar result");
    }
    match array_value_to_json(batch.column(0), 0)? {
        Value::Number(n) => n
            .as_f64()
            .or_else(|| n.as_i64().map(|i| i as f64))
            .ok_or_else(|| anyhow::anyhow!("scalar is not f64")),
        Value::Null => Ok(0.0),
        other => bail!("scalar is not f64: {other}"),
    }
}

fn array_value_to_json(array: &dyn Array, row: usize) -> Result<Value> {
    if array.is_null(row) {
        return Ok(Value::Null);
    }
    Ok(match array.data_type() {
        DataType::Utf8 => {
            let a = array
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| anyhow::anyhow!("utf8 downcast"))?;
            Value::String(a.value(row).to_string())
        }
        DataType::Utf8View => {
            use datafusion::arrow::array::StringViewArray;
            let a = array
                .as_any()
                .downcast_ref::<StringViewArray>()
                .ok_or_else(|| anyhow::anyhow!("utf8view downcast"))?;
            Value::String(a.value(row).to_string())
        }
        DataType::LargeUtf8 => {
            use datafusion::arrow::array::LargeStringArray;
            let a = array
                .as_any()
                .downcast_ref::<LargeStringArray>()
                .ok_or_else(|| anyhow::anyhow!("largeutf8 downcast"))?;
            Value::String(a.value(row).to_string())
        }
        DataType::Int64 => {
            let a = array
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| anyhow::anyhow!("i64 downcast"))?;
            json!(a.value(row))
        }
        DataType::Int32 => {
            let a = array
                .as_any()
                .downcast_ref::<Int32Array>()
                .ok_or_else(|| anyhow::anyhow!("i32 downcast"))?;
            json!(a.value(row))
        }
        DataType::Float64 => {
            let a = array
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| anyhow::anyhow!("f64 downcast"))?;
            json!(a.value(row))
        }
        DataType::Float32 => {
            use datafusion::arrow::array::Float32Array;
            let a = array
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| anyhow::anyhow!("f32 downcast"))?;
            json!(a.value(row) as f64)
        }
        DataType::Boolean => {
            let a = array
                .as_any()
                .downcast_ref::<BooleanArray>()
                .ok_or_else(|| anyhow::anyhow!("bool downcast"))?;
            Value::Bool(a.value(row))
        }
        DataType::Date32 => {
            let a = array
                .as_any()
                .downcast_ref::<Date32Array>()
                .ok_or_else(|| anyhow::anyhow!("date32 downcast"))?;
            let days = i64::from(a.value(row));
            let date = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
                .and_then(|epoch| epoch.checked_add_signed(chrono::Duration::days(days)))
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| format!("date32:{days}"));
            Value::String(date)
        }
        DataType::Timestamp(unit, _) => timestamp_to_json(array, row, *unit)?,
        other => Value::String(format!("{other:?}")),
    })
}

fn timestamp_to_json(array: &dyn Array, row: usize, unit: TimeUnit) -> Result<Value> {
    let micros = match unit {
        TimeUnit::Second => {
            let a = array
                .as_any()
                .downcast_ref::<TimestampSecondArray>()
                .ok_or_else(|| anyhow::anyhow!("ts sec"))?;
            a.value(row) * 1_000_000
        }
        TimeUnit::Millisecond => {
            let a = array
                .as_any()
                .downcast_ref::<TimestampMillisecondArray>()
                .ok_or_else(|| anyhow::anyhow!("ts ms"))?;
            a.value(row) * 1_000
        }
        TimeUnit::Microsecond => {
            let a = array
                .as_any()
                .downcast_ref::<TimestampMicrosecondArray>()
                .ok_or_else(|| anyhow::anyhow!("ts us"))?;
            a.value(row)
        }
        TimeUnit::Nanosecond => {
            let a = array
                .as_any()
                .downcast_ref::<TimestampNanosecondArray>()
                .ok_or_else(|| anyhow::anyhow!("ts ns"))?;
            a.value(row) / 1_000
        }
    };
    Ok(json!(micros))
}
