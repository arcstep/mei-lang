#![cfg(test)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use mei_lang_kernel::ColumnSchema;
use serde_json::json;

use super::connection::with_app_connection;
use super::query::{query_parquet_page, DuckdbPageQuery};
use crate::types::DatasetQueryOptions;

fn write_sample_parquet(app_root: &std::path::Path) -> PathBuf {
    let data_dir = app_root.join("data");
    fs::create_dir_all(&data_dir).expect("mkdir");
    let parquet = data_dir.join("sample.parquet");
    let path_sql = format!("'{}'", parquet.to_string_lossy().replace('\'', "''"));
    with_app_connection(app_root, |conn| {
        conn.execute_batch(&format!(
            "COPY (SELECT * FROM (VALUES (1, 'a'), (2, 'b'), (3, 'c')) t(id, name)) TO {path_sql} (FORMAT PARQUET);"
        ))?;
        Ok(())
    })
    .expect("write parquet");
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
        DuckdbPageQuery {
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
        DuckdbPageQuery {
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

#[test]
fn ensure_parquet_view_missing_schema_source_becomes_null() {
    use super::register::ensure_parquet_view;

    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let parquet = write_sample_parquet(app_root);
    let schema = vec![
        ColumnSchema {
            name: "id".into(),
            type_name: "integer".into(),
            source: Some("id".into()),
            optional: false,
            unit: None,
        },
        ColumnSchema {
            name: "missing_col".into(),
            type_name: "string".into(),
            source: Some("missing_col".into()),
            optional: true,
            unit: None,
        },
    ];
    let (view, cols) =
        ensure_parquet_view(app_root, parquet.as_path(), &schema, None).expect("view");
    assert!(view.starts_with("mei_pq_"));
    assert_eq!(cols, vec!["id".to_string(), "missing_col".to_string()]);
    let n: i64 = with_app_connection(app_root, |conn| {
        let sql = format!("SELECT COUNT(*) FROM \"{view}\"");
        Ok(conn.query_row(&sql, [], |row| row.get(0))?)
    })
    .expect("count");
    assert_eq!(n, 3);
    let missing: Option<String> = with_app_connection(app_root, |conn| {
        let sql = format!("SELECT \"missing_col\" FROM \"{view}\" LIMIT 1");
        Ok(conn.query_row(&sql, [], |row| row.get(0))?)
    })
    .expect("missing");
    assert!(missing.is_none());
}
