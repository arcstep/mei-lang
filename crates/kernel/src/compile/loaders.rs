//! 遗留数据集文件加载（与旧版 mei-lang `data/loaders.rs` 行为对齐）。
use std::{
    fs,
    io::{Read, Seek},
    path::Path,
};

use anyhow::{Context, Result};
use calamine::{open_workbook, Data, Reader, Xls, Xlsx};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct XlsxTableSnapshot {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

fn value_as_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn json_number_f64(number: f64) -> Value {
    serde_json::Number::from_f64(number)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn xlsx_cell(value: &Data) -> Value {
    use crate::compile::analysis::dates::format_calendar_date_value;

    match value {
        Data::Empty => Value::Null,
        Data::String(text) => Value::String(text.clone()),
        Data::Float(number) => json_number_f64(*number),
        Data::Int(integer) => json!(*integer),
        Data::Bool(flag) => Value::Bool(*flag),
        Data::DateTime(date) => format_calendar_date_value(&Value::String(date.to_string())),
        Data::DateTimeIso(text) | Data::DurationIso(text) => {
            format_calendar_date_value(&Value::String(text.clone()))
        }
        Data::Error(error) => Value::String(error.to_string()),
    }
}

fn xlsx_header(value: &Data) -> String {
    match xlsx_cell(value) {
        Value::Null => String::new(),
        Value::String(text) => text,
        other => value_as_text(&other),
    }
}

/// 空表头列赋 `__EMPTY` / `__EMPTY_N`，与 `ds.column(name, source = "__EMPTY")` 的 normalize 映射一致。
pub fn materialize_xlsx_column_headers(raw_headers: &[String]) -> Vec<String> {
    let mut empty_idx = 0usize;
    raw_headers
        .iter()
        .map(|raw| {
            let text = raw.trim().to_string();
            if !text.is_empty() {
                return text;
            }
            let name = if empty_idx == 0 {
                "__EMPTY".to_string()
            } else {
                format!("__EMPTY_{empty_idx}")
            };
            empty_idx += 1;
            name
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::materialize_xlsx_column_headers;

    fn optional_external_workspace() -> Option<PathBuf> {
        let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
        let path = PathBuf::from(raw.trim());
        if path.as_os_str().is_empty() || !path.is_dir() {
            return None;
        }
        Some(path.canonicalize().unwrap_or(path))
    }

    fn zhifa_upload(rel: &str) -> Option<PathBuf> {
        let ws = optional_external_workspace()?;
        let candidates = [
            ws.join("zhifa").join(rel),
            ws.join(rel),
            ws.join("apps/zhifa").join(rel),
        ];
        candidates.into_iter().find(|p: &PathBuf| p.is_file())
    }

    #[test]
    fn load_spbjw_penalty_legacy_xls_coerces_date_columns_in_schema() {
        use crate::compile::analysis::dates::coerce_rows_to_schema;
        use crate::model::ColumnSchema;

        let Some(path) = zhifa_upload("upload/8.行政处罚结果清单.xlsx") else {
            eprintln!("skip: set MEI_TEST_WORKSPACE with zhifa upload xlsx");
            return;
        };
        let rows = super::load_legacy_xlsx_rows(&path, None, 1, Some(20))
            .expect("load spbjw penalty result list");
        assert!(!rows.is_empty(), "penalty rows should not be empty");
        let schema = vec![
            ColumnSchema {
                name: "立案日期".to_string(),
                type_name: "date".to_string(),
                source: Some("立案日期".to_string()),
                optional: true,
                unit: None,
            },
            ColumnSchema {
                name: "做出处罚日期".to_string(),
                type_name: "date".to_string(),
                source: Some("做出处罚日期".to_string()),
                optional: true,
                unit: None,
            },
            ColumnSchema {
                name: "执行日期".to_string(),
                type_name: "date".to_string(),
                source: Some("执行日期".to_string()),
                optional: true,
                unit: None,
            },
        ];
        let coerced = coerce_rows_to_schema(rows, &schema);
        let with_decision_date = coerced
            .iter()
            .find(|row| {
                row.get("做出处罚日期")
                    .and_then(|value| value.as_str())
                    .map(|text| !text.trim().is_empty())
                    .unwrap_or(false)
            })
            .expect("sample row with 做出处罚日期");
        let decision_date = with_decision_date
            .get("做出处罚日期")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        assert!(
            decision_date.len() >= 10 && decision_date.as_bytes().get(4) == Some(&b'-'),
            "做出处罚日期 should coerce to YYYY-MM-DD, got {decision_date:?}"
        );
    }

    #[test]
    fn load_spbjw_inspection_xlsx_coerces_check_date_column() {
        use crate::compile::analysis::dates::coerce_rows_to_schema;
        use crate::model::ColumnSchema;

        let Some(path) = zhifa_upload("upload/5.行政检查结果清单.xlsx") else {
            eprintln!("skip: set MEI_TEST_WORKSPACE with zhifa upload xlsx");
            return;
        };
        let rows = super::load_legacy_xlsx_rows(&path, Some("总表"), 1, Some(20))
            .expect("load spbjw inspection list");
        let schema = vec![ColumnSchema {
            name: "检查日期".to_string(),
            type_name: "date".to_string(),
            source: Some("检查日期".to_string()),
            optional: false,
            unit: None,
        }];
        let coerced = coerce_rows_to_schema(rows, &schema);
        let with_date = coerced
            .iter()
            .find(|row| row.get("检查日期").is_some())
            .expect("sample row with 检查日期");
        let check_date = with_date
            .get("检查日期")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        assert!(
            check_date.len() >= 10 && check_date.as_bytes().get(4) == Some(&b'-'),
            "检查日期 should coerce to YYYY-MM-DD, got {check_date:?}"
        );
    }

    #[test]
    fn load_spbjw_warning_xlsx_preserves_leading_empty_header_columns() {
        let Some(path) = zhifa_upload("upload/11.预警清单、问题跟踪清单20260606.xlsx")
        else {
            eprintln!("skip: set MEI_TEST_WORKSPACE with zhifa upload xlsx");
            return;
        };
        let rows = super::load_legacy_xlsx_rows(&path, None, 4, Some(20))
            .expect("load spbjw warning list xlsx");
        let row = rows
            .iter()
            .find(|row| {
                row.get("预警ID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.contains("YJ2025001"))
                    .unwrap_or(false)
            })
            .expect("sample row with 预警ID");
        assert!(
            row.get("__EMPTY").is_some(),
            "raw row keys: {:?}",
            row.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
    }

    #[test]
    fn materialize_xlsx_column_headers_names_empty_leading_columns() {
        let headers = materialize_xlsx_column_headers(&[
            String::new(),
            String::new(),
            String::new(),
            "预警ID".to_string(),
            "预警条数".to_string(),
        ]);
        assert_eq!(
            headers,
            vec!["__EMPTY", "__EMPTY_1", "__EMPTY_2", "预警ID", "预警条数"]
        );
    }
}

fn is_ole_compound_document(path: &Path) -> bool {
    fs::read(path)
        .map(|bytes| bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]))
        .unwrap_or(false)
}

fn xlsx_table_from_reader<R, RS>(
    workbook: &mut R,
    sheet: Option<&str>,
    header_row: usize,
    max_rows: Option<usize>,
) -> Result<XlsxTableSnapshot>
where
    R: Reader<RS>,
    RS: Read + Seek,
    <R as Reader<RS>>::Error: std::fmt::Display,
{
    let sheet_name = if let Some(sheet) = sheet.filter(|value| !value.is_empty()) {
        sheet.to_string()
    } else {
        workbook.sheet_names().first().cloned().unwrap_or_default()
    };
    if sheet_name.is_empty() {
        return Ok(XlsxTableSnapshot {
            columns: Vec::new(),
            rows: Vec::new(),
        });
    }
    let range = workbook.worksheet_range(&sheet_name).map_err(|error| {
        anyhow::anyhow!("failed to read Excel worksheet `{sheet_name}`: {error}")
    })?;
    let mut rows = range.rows();
    for _ in 0..header_row.saturating_sub(1) {
        rows.next();
    }
    let Some(header_row_cells) = rows.next() else {
        return Ok(XlsxTableSnapshot {
            columns: Vec::new(),
            rows: Vec::new(),
        });
    };
    let raw_headers: Vec<String> = header_row_cells
        .iter()
        .map(|cell| xlsx_header(cell))
        .collect();
    let headers = materialize_xlsx_column_headers(&raw_headers);
    let mut out = Vec::new();
    for row in rows {
        if max_rows.is_some_and(|cap| out.len() >= cap) {
            break;
        }
        let mut obj = Map::new();
        for (index, header) in headers.iter().enumerate() {
            let cell = row.get(index).map(xlsx_cell).unwrap_or(Value::Null);
            obj.insert(header.clone(), cell);
        }
        if obj.values().any(|value| !value.is_null()) {
            out.push(Value::Object(obj));
        }
    }
    Ok(XlsxTableSnapshot {
        columns: headers,
        rows: out,
    })
}

/// 从 `.csv` 读取完整表快照（`header_row` 从 1 计数：该行作为表头，之前行跳过）。
pub fn load_csv_table_snapshot(
    path: &Path,
    header_row: usize,
    max_rows: Option<usize>,
) -> Result<XlsxTableSnapshot> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("failed to open csv {}", path.display()))?;
    let header_row = header_row.max(1);
    let mut records = reader.records();
    for _ in 1..header_row {
        let _ = records
            .next()
            .transpose()
            .with_context(|| format!("failed to skip csv rows before header in {}", path.display()))?;
    }
    let header_record = records
        .next()
        .transpose()
        .with_context(|| format!("failed to read csv header in {}", path.display()))?
        .with_context(|| format!("csv missing header row in {}", path.display()))?;
    let raw_headers = header_record
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let headers = materialize_xlsx_column_headers(&raw_headers);
    let mut out = Vec::new();
    for record in records {
        if max_rows.is_some_and(|cap| out.len() >= cap) {
            break;
        }
        let record = record.with_context(|| format!("failed to read csv row in {}", path.display()))?;
        let mut obj = Map::new();
        for (index, header) in headers.iter().enumerate() {
            let cell = record
                .get(index)
                .map(|value| Value::String(value.to_string()))
                .unwrap_or(Value::Null);
            obj.insert(header.clone(), cell);
        }
        if obj.values().any(|value| match value {
            Value::Null => false,
            Value::String(s) => !s.is_empty(),
            _ => true,
        }) {
            out.push(Value::Object(obj));
        }
    }
    Ok(XlsxTableSnapshot {
        columns: headers,
        rows: out,
    })
}

/// 从 JSON 数组表读取快照（顶层必须是 object 数组；嵌套值序列化为字符串）。
pub fn load_json_table_snapshot(
    path: &Path,
    max_rows: Option<usize>,
) -> Result<XlsxTableSnapshot> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read json {}", path.display()))?;
    let json: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid json {}", path.display()))?;
    let rows_in = json
        .as_array()
        .cloned()
        .with_context(|| format!("json root must be an array: {}", path.display()))?;
    let mut columns: Vec<String> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for row in &rows_in {
        let Some(obj) = row.as_object() else {
            continue;
        };
        for key in obj.keys() {
            if seen.insert(key.clone()) {
                columns.push(key.clone());
            }
        }
    }
    let columns = materialize_xlsx_column_headers(&columns);
    let mut out = Vec::new();
    for row in rows_in {
        if max_rows.is_some_and(|cap| out.len() >= cap) {
            break;
        }
        let Some(obj) = row.as_object() else {
            continue;
        };
        let mut mapped = Map::new();
        for col in &columns {
            let cell = obj.get(col).cloned().unwrap_or(Value::Null);
            mapped.insert(col.clone(), cell);
        }
        if mapped.values().any(|value| !value.is_null()) {
            out.push(Value::Object(mapped));
        }
    }
    Ok(XlsxTableSnapshot {
        columns,
        rows: out,
    })
}

/// 从 `.xlsx` 或 OLE 容器内的 `.xls` 读取完整表快照（表头行号从 1 计数，与 `ds.xlsx(..., header_row = n)` 一致）。
pub fn load_xlsx_table_snapshot(
    path: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
    max_rows: Option<usize>,
) -> Result<XlsxTableSnapshot> {
    let ext_xls = source_path.to_ascii_lowercase().ends_with(".xls");
    if ext_xls || is_ole_compound_document(path) {
        let mut workbook: Xls<_> = open_workbook(path)
            .with_context(|| format!("failed to open legacy xls {}", path.display()))?;
        return xlsx_table_from_reader(&mut workbook, sheet, header_row, max_rows);
    }
    match open_workbook::<Xlsx<_>, &Path>(path) {
        Ok(mut workbook) => xlsx_table_from_reader(&mut workbook, sheet, header_row, max_rows),
        Err(xlsx_err) => {
            let mut workbook: Xls<_> = open_workbook(path).with_context(|| {
                format!(
                    "failed to open as Office Open XML ({xlsx_err}); legacy xls fallback also failed for {}",
                    path.display()
                )
            })?;
            xlsx_table_from_reader(&mut workbook, sheet, header_row, max_rows)
        }
    }
}

/// 从 `.xlsx` 或 OLE 容器内的 `.xls` 读取行（表头行号从 1 计数，与 `ds.xlsx(..., header_row = n)` 一致）。
#[cfg(test)]
pub(super) fn load_legacy_xlsx_rows(
    path: &Path,
    sheet: Option<&str>,
    header_row: usize,
    max_rows: Option<usize>,
) -> Result<Vec<Value>> {
    let snapshot = load_xlsx_table_snapshot(
        path,
        path.to_string_lossy().as_ref(),
        sheet,
        header_row,
        max_rows,
    )?;
    Ok(snapshot.rows)
}
