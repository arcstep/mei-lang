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
    match value {
        Data::Empty => Value::Null,
        Data::String(text) => Value::String(text.clone()),
        Data::Float(number) => json_number_f64(*number),
        Data::Int(integer) => json!(*integer),
        Data::Bool(flag) => Value::Bool(*flag),
        Data::DateTime(date) => Value::String(date.to_string()),
        Data::DateTimeIso(text) | Data::DurationIso(text) => Value::String(text.clone()),
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
    use std::path::Path;

    use super::materialize_xlsx_column_headers;

    #[test]
    fn load_spbjw_warning_xlsx_preserves_leading_empty_header_columns() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../workspaces/spbjw/upload/11.预警清单、问题跟踪清单.20260527.xlsx");
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
            vec![
                "__EMPTY",
                "__EMPTY_1",
                "__EMPTY_2",
                "预警ID",
                "预警条数"
            ]
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
    let snapshot = load_xlsx_table_snapshot(path, path.to_string_lossy().as_ref(), sheet, header_row, max_rows)?;
    Ok(snapshot.rows)
}
