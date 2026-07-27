#![cfg(test)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::{Date32Array, Float64Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use mei_lang_kernel::ColumnSchema;
use parquet::arrow::ArrowWriter;
use serde_json::json;

use super::query::{query_parquet_page, ParquetPageQuery};
use crate::types::DatasetQueryOptions;

fn write_sample_parquet(app_root: &std::path::Path) -> PathBuf {
    let data_dir = app_root.join("data");
    fs::create_dir_all(&data_dir).expect("mkdir");
    let parquet = data_dir.join("sample.parquet");
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("name", DataType::Utf8, false),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec!["a", "b", "c"])),
        ],
    )
    .expect("batch");
    let file = fs::File::create(&parquet).expect("create parquet");
    let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
    writer.write(&batch).expect("write");
    writer.close().expect("close");
    parquet
}

#[test]
fn query_parquet_page_pushdown_limits_rows() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let parquet = write_sample_parquet(app_root);
    let schema = vec![
        ColumnSchema {
            name: "id".into(),
            type_name: "integer".into(),
            source: None,
            optional: false,
            unit: None,
        },
        ColumnSchema {
            name: "name".into(),
            type_name: "string".into(),
            source: None,
            optional: false,
            unit: None,
        },
    ];
    let options = DatasetQueryOptions {
        page: 1,
        page_size: 2,
        collect_all: false,
        ..DatasetQueryOptions::default()
    };
    let page = query_parquet_page(
        app_root,
        ParquetPageQuery {
            parquet_path: parquet.as_path(),
            schema: &schema,
            physical_columns: None,
            normalize: &BTreeMap::new(),
            options: &options,
        },
    )
    .expect("page query");
    assert_eq!(page.total, 3);
    assert_eq!(page.rows.len(), 2);
    assert!(page.has_more);
    assert_eq!(page.rows_materialized, 2);
    assert_eq!(page.rows[0].get("id"), Some(&json!(1)));
}

#[test]
fn query_parquet_page_filter_eq() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let parquet = write_sample_parquet(app_root);
    let schema = vec![ColumnSchema {
        name: "name".into(),
        type_name: "string".into(),
        source: None,
        optional: false,
        unit: None,
    }];
    let mut filters = BTreeMap::new();
    filters.insert("name".into(), "b".into());
    let options = DatasetQueryOptions {
        page: 1,
        page_size: 10,
        filters,
        collect_all: false,
        ..DatasetQueryOptions::default()
    };
    let page = query_parquet_page(
        app_root,
        ParquetPageQuery {
            parquet_path: parquet.as_path(),
            schema: &schema,
            physical_columns: None,
            normalize: &BTreeMap::new(),
            options: &options,
        },
    )
    .expect("filtered page");
    assert_eq!(page.total, 1);
    assert_eq!(page.rows.len(), 1);
}

fn unix_days(year: i32, month: u32, day: u32) -> i32 {
    use chrono::NaiveDate;
    let d = NaiveDate::from_ymd_opt(year, month, day).expect("date");
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch");
    (d - epoch).num_days() as i32
}

fn write_date_mixed_parquet(app_root: &std::path::Path) -> PathBuf {
    let data_dir = app_root.join("data");
    fs::create_dir_all(&data_dir).expect("mkdir");
    let parquet = data_dir.join("dates.parquet");
    let schema = Arc::new(Schema::new(vec![
        Field::new("检查日期", DataType::Date32, true),
        Field::new("excel_serial", DataType::Float64, true),
        Field::new("id", DataType::Int64, false),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Date32Array::from(vec![
                Some(unix_days(2025, 1, 15)),
                Some(unix_days(2024, 6, 1)),
                Some(unix_days(2025, 6, 15)),
            ])),
            // unused by the Date32 between test; kept for schema realism
            Arc::new(Float64Array::from(vec![
                Some(45_717.0),
                Some(45_444.0),
                None,
            ])),
            Arc::new(Int64Array::from(vec![1, 2, 3])),
        ],
    )
    .expect("batch");
    let file = fs::File::create(&parquet).expect("create parquet");
    let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
    writer.write(&batch).expect("write");
    writer.close().expect("close");
    parquet
}

#[test]
fn query_parquet_page_between_on_date32_column() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let parquet = write_date_mixed_parquet(app_root);
    let schema = vec![
        ColumnSchema {
            name: "检查日期".into(),
            type_name: "date".into(),
            source: None,
            optional: true,
            unit: None,
        },
        ColumnSchema {
            name: "id".into(),
            type_name: "integer".into(),
            source: None,
            optional: false,
            unit: None,
        },
    ];
    let mut filters = BTreeMap::new();
    filters.insert("检查日期".into(), "between:2025-01-01..2025-12-31".into());
    let options = DatasetQueryOptions {
        page: 1,
        page_size: 10,
        filters,
        collect_all: false,
        ..DatasetQueryOptions::default()
    };
    let page = query_parquet_page(
        app_root,
        ParquetPageQuery {
            parquet_path: parquet.as_path(),
            schema: &schema,
            physical_columns: None,
            normalize: &BTreeMap::new(),
            options: &options,
        },
    )
    .expect("date32 between must not fail with Date32→Float64 try_cast");
    // 2025-01-15 and ~2025-06-15 in range; 2024-06-01 out
    assert_eq!(page.total, 2);
    assert_eq!(page.rows.len(), 2);
}

#[test]
fn query_parquet_page_drange_matches_filter_bar_encoding() {
    // filter-bar writes drange:; SQL path must not fall back to equality on the
    // encoded string (that yields total=0 for every real date column).
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let parquet = write_date_mixed_parquet(app_root);
    let schema = vec![
        ColumnSchema {
            name: "检查日期".into(),
            type_name: "date".into(),
            source: None,
            optional: true,
            unit: None,
        },
        ColumnSchema {
            name: "id".into(),
            type_name: "integer".into(),
            source: None,
            optional: false,
            unit: None,
        },
    ];
    let mut filters = BTreeMap::new();
    filters.insert("检查日期".into(), "drange:2025-01-01..2025-12-31".into());
    let options = DatasetQueryOptions {
        page: 1,
        page_size: 10,
        filters,
        collect_all: false,
        ..DatasetQueryOptions::default()
    };
    let page = query_parquet_page(
        app_root,
        ParquetPageQuery {
            parquet_path: parquet.as_path(),
            schema: &schema,
            physical_columns: None,
            normalize: &BTreeMap::new(),
            options: &options,
        },
    )
    .expect("drange must lower to DATE BETWEEN");
    assert_eq!(page.total, 2);
    assert_eq!(page.rows.len(), 2);
}

#[test]
fn ensure_parquet_view_missing_optional_schema_source_becomes_null() {
    use super::register::ensure_parquet_view;

    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let parquet = write_sample_parquet(app_root);
    let schema = vec![
        ColumnSchema {
            name: "id".into(),
            type_name: "integer".into(),
            source: None,
            optional: false,
            unit: None,
        },
        ColumnSchema {
            name: "missing_col".into(),
            type_name: "string".into(),
            source: Some("视频路径".into()),
            optional: true,
            unit: None,
        },
    ];
    let (view, cols) =
        ensure_parquet_view(app_root, parquet.as_path(), &schema, None).expect("view");
    assert!(view.starts_with("mei_pq_"));
    assert!(cols.contains(&"id".to_string()));
    assert!(cols.contains(&"missing_col".to_string()));
}

#[test]
fn ensure_parquet_view_missing_required_schema_source_fails() {
    use super::register::ensure_parquet_view;

    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let parquet = write_sample_parquet(app_root);
    let schema = vec![ColumnSchema {
        name: "name".into(),
        type_name: "string".into(),
        source: Some("旧表头名".into()),
        optional: false,
        unit: None,
    }];
    let err = ensure_parquet_view(app_root, parquet.as_path(), &schema, None)
        .expect_err("required missing source must fail");
    let message = format!("{err:#}");
    assert!(
        message.contains("schema.source") || message.contains("旧表头名"),
        "unexpected error: {message}"
    );
}
