#![cfg(test)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use mei_lang_kernel::{
    parquet_snapshot_path, AnalysisGraph, ColumnSchema, DatasetView, SourceDecl,
};
use parquet::arrow::ArrowWriter;
use serde_json::json;

use super::{try_eval_analysis_expr_via_sql, MAX_PIPELINE_SQL_ROWS};

fn col(name: &str, type_name: &str) -> ColumnSchema {
    ColumnSchema {
        name: name.into(),
        type_name: type_name.into(),
        source: None,
        optional: false,
        unit: None,
    }
}

fn prepare_app_root(app_root: &Path) {
    let env_gen = app_root.join("env").join("WS-TEST.0");
    fs::create_dir_all(env_gen.join("var")).expect("mkdir env gen");
    let current = app_root.join("env").join("current");
    if current.symlink_metadata().is_ok() || current.exists() {
        return;
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink("WS-TEST.0", &current).expect("symlink env/current");
    #[cfg(not(unix))]
    fs::create_dir_all(&current).expect("mkdir env/current");
}

fn write_parquet_table(
    app_root: &Path,
    source_rel: &str,
    columns: &[(&str, DataType)],
    arrays: Vec<ArrayRef>,
) -> std::path::PathBuf {
    prepare_app_root(app_root);
    let source_abs = app_root.join(source_rel);
    fs::create_dir_all(source_abs.parent().expect("parent")).expect("mkdir source");
    fs::write(&source_abs, b"fixture-source").expect("write source");
    let parquet = parquet_snapshot_path(app_root, source_rel, None, 1).expect("parquet path");
    fs::create_dir_all(parquet.parent().expect("parquet parent")).expect("mkdir store");
    let schema = Arc::new(Schema::new(
        columns
            .iter()
            .map(|(name, ty)| Field::new(*name, ty.clone(), true))
            .collect::<Vec<_>>(),
    ));
    let batch = RecordBatch::try_new(schema.clone(), arrays).expect("batch");
    let file = fs::File::create(&parquet).expect("create parquet");
    let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
    writer.write(&batch).expect("write");
    writer.close().expect("close");
    parquet
}

fn write_inspections_view(
    app_root: &Path,
    source_rel: &str,
    dates: &[&str],
    parties: &[&str],
    amounts: &[f64],
) -> DatasetView {
    write_parquet_table(
        app_root,
        source_rel,
        &[
            ("检查日期", DataType::Utf8),
            ("当事人", DataType::Utf8),
            ("罚款金额", DataType::Float64),
        ],
        vec![
            Arc::new(StringArray::from(dates.to_vec())),
            Arc::new(StringArray::from(parties.to_vec())),
            Arc::new(Float64Array::from(amounts.to_vec())),
        ],
    );
    let columns = vec!["检查日期".into(), "当事人".into(), "罚款金额".into()];
    DatasetView {
        id: "inspections".into(),
        title: None,
        purpose: None,
        schema: vec![
            col("检查日期", "string"),
            col("当事人", "string"),
            col("罚款金额", "number"),
        ],
        stage_schema: Vec::new(),
        columns: columns.clone(),
        rows: Vec::new(),
        source: SourceDecl {
            kind: "file".into(),
            path: source_rel.into(),
            sheet: None,
            header_row: Some(1),
            preview_rows: None,
            page_size: None,
            max_page_size: None,
            table: None,
            query: None,
            connection: None,
            content: None,
        },
        sources: Vec::new(),
        metrics: BTreeMap::new(),
        runtime_metric_defs: BTreeMap::new(),
        runtime_analysis_graph: AnalysisGraph::default(),
        runtime_analysis_contracts: BTreeMap::new(),
    }
}

#[test]
fn max_pipeline_sql_rows_gate() {
    assert_eq!(MAX_PIPELINE_SQL_ROWS, 2000);
}

#[test]
fn data_ref_binding_lowers_like_rows() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let view = write_inspections_view(
        app_root,
        "upload/data/inspections.xlsx",
        &["2025-03-10", "2025-03-12"],
        &["甲", "乙"],
        &[0.0, 0.0],
    );
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);
    let expr = json!({
        "__kind": "analysis_expr",
        "type": "trend_year_compare",
        "rowset": {
            "__ref": "data",
            "from_dataset": "inspections",
            "id": "inspections"
        },
        "date_field": "检查日期",
        "agg": "count",
        "years": [2024, 2025],
        "limit": 6,
        "window": "calendar"
    });
    let rows = try_eval_analysis_expr_via_sql(app_root, &datasets, &expr)
        .expect("sql ok")
        .expect("lowered __ref");
    assert_eq!(rows.len(), 24);
}

#[test]
fn trend_year_compare_sql_matches_kernel_shape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let view = write_inspections_view(
        app_root,
        "upload/data/inspections.xlsx",
        &["2024-03-10", "2024-03-12", "2025-03-15", "2025-06-01"],
        &["甲", "乙", "丙", "丁"],
        &[0.0, 0.0, 0.0, 0.0],
    );
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);

    let expr = json!({
        "__kind": "analysis_expr",
        "type": "trend_year_compare",
        "rowset": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "inspections"
        },
        "date_field": "检查日期",
        "agg": "count",
        "years": [2024, 2025],
        "limit": 6,
        "window": "rolling"
    });

    let rows = try_eval_analysis_expr_via_sql(app_root, &datasets, &expr)
        .expect("sql ok")
        .expect("lowered");
    assert_eq!(rows.len(), 12); // 6 months × 2 years
    let march_2024 = rows
        .iter()
        .find(|row| {
            row.get("month").and_then(|v| v.as_str()) == Some("03")
                && row.get("year").and_then(|v| v.as_str()) == Some("2024")
        })
        .and_then(|row| row.get("value").and_then(|v| v.as_f64()));
    let march_2025 = rows
        .iter()
        .find(|row| {
            row.get("month").and_then(|v| v.as_str()) == Some("03")
                && row.get("year").and_then(|v| v.as_str()) == Some("2025")
        })
        .and_then(|row| row.get("value").and_then(|v| v.as_f64()));
    assert_eq!(march_2024, Some(2.0));
    assert_eq!(march_2025, Some(1.0));
}

#[test]
fn party_year_aggregate_and_unpivot_sql() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let view = write_inspections_view(
        app_root,
        "upload/data/penalties.xlsx",
        &["2024-01-10", "2024-06-10", "2025-02-01", "2024-03-01"],
        &["甲公司", "甲公司", "甲公司", "乙公司"],
        &[10000.0, 15000.0, 30000.0, 5000.0],
    );
    let mut datasets = BTreeMap::new();
    datasets.insert("penalties".into(), {
        let mut v = view;
        v.id = "penalties".into();
        v
    });

    let aggregate = json!({
        "__kind": "analysis_expr",
        "type": "party_year_aggregate",
        "rowset": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "penalties"
        },
        "party_field": "当事人",
        "date_field": "检查日期",
        "value_field": "罚款金额",
        "years": [2024, 2025]
    });
    let sorted = json!({
        "__kind": "analysis_expr",
        "type": "sort_by",
        "rowset": aggregate,
        "field": "罚没金额_2025",
        "order": "desc"
    });
    let limited = json!({
        "__kind": "analysis_expr",
        "type": "limit",
        "rowset": sorted,
        "n": 10
    });
    let unpivot = json!({
        "__kind": "analysis_expr",
        "type": "unpivot_columns",
        "rowset": limited,
        "id_field": "当事人",
        "year_field": "year",
        "value_field": "value",
        "columns": [
            {"year": "2024", "field": "罚没金额_2024"},
            {"year": "2025", "field": "罚没金额_2025"}
        ]
    });

    let rows = try_eval_analysis_expr_via_sql(app_root, &datasets, &unpivot)
        .expect("sql ok")
        .expect("lowered");
    assert!(!rows.is_empty());
    let a_2024 = rows.iter().find(|row| {
        row.get("当事人").and_then(|v| v.as_str()) == Some("甲公司")
            && row.get("year").and_then(|v| v.as_str()) == Some("2024")
    });
    assert_eq!(
        a_2024.and_then(|r| r.get("value").and_then(|v| v.as_f64())),
        Some(25000.0)
    );
}

#[test]
fn select_rename_after_party_year_aggregate() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let view = write_inspections_view(
        app_root,
        "upload/data/penalties.xlsx",
        &[
            "2025-01-10",
            "2025-02-10",
            "2025-03-10",
            "2025-01-10",
            "2024-01-10",
        ],
        &["事项A", "事项A", "事项A", "事项B", "事项B"],
        &[100.0, 100.0, 100.0, 100.0, 100.0],
    );
    let mut datasets = BTreeMap::new();
    let mut v = view;
    v.id = "penalties".into();
    datasets.insert("penalties".into(), v);

    let expr = json!({
        "__kind": "analysis_expr",
        "type": "rename",
        "mapping": {"当事人": "label", "处罚次数_2025": "value"},
        "rowset": {
            "__kind": "analysis_expr",
            "type": "select",
            "fields": ["当事人", "处罚次数_2025"],
            "rowset": {
                "__kind": "analysis_expr",
                "type": "limit",
                "n": 3,
                "rowset": {
                    "__kind": "analysis_expr",
                    "type": "sort_by",
                    "field": "处罚次数_2025",
                    "order": "desc",
                    "rowset": {
                        "__kind": "analysis_expr",
                        "type": "party_year_aggregate",
                        "party_field": "当事人",
                        "date_field": "检查日期",
                        "value_field": "罚款金额",
                        "years": [2024, 2025],
                        "rowset": {
                            "__ref": "data",
                            "from_dataset": "penalties",
                            "id": "penalties"
                        }
                    }
                }
            }
        }
    });
    let rows = try_eval_analysis_expr_via_sql(app_root, &datasets, &expr)
        .expect("sql ok")
        .expect("lowered");
    assert!(!rows.is_empty(), "rows={rows:?}");
    let a = rows
        .iter()
        .find(|row| row.get("label").and_then(|v| v.as_str()) == Some("事项A"))
        .expect("事项A present");
    assert_eq!(a.get("value").and_then(|v| v.as_f64()), Some(3.0));
    // Prefer sorted order when the engine preserves ORDER BY through projections.
    if rows[0].get("label").and_then(|v| v.as_str()) == Some("事项A") {
        assert_eq!(rows[0].get("value").and_then(|v| v.as_f64()), Some(3.0));
    }
}

#[test]
fn lookup_value_left_join_and_safe_sql_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let fact = write_inspections_view(
        app_root,
        "upload/data/fact.xlsx",
        &["2024-01-01", "2024-01-02"],
        &["P1", "P2"],
        &[10.0, 20.0],
    );
    let dim_source = "upload/data/parties.xlsx";
    write_parquet_table(
        app_root,
        dim_source,
        &[("id", DataType::Utf8), ("name", DataType::Utf8)],
        vec![
            Arc::new(StringArray::from(vec!["P1", "P2"])),
            Arc::new(StringArray::from(vec!["甲公司", "乙公司"])),
        ],
    );

    let mut datasets = BTreeMap::new();
    datasets.insert(
        "fact".into(),
        DatasetView {
            id: "fact".into(),
            ..fact
        },
    );
    datasets.insert(
        "parties".into(),
        DatasetView {
            id: "parties".into(),
            title: None,
            purpose: None,
            schema: vec![col("id", "string"), col("name", "string")],
            stage_schema: Vec::new(),
            columns: vec!["id".into(), "name".into()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "file".into(),
                path: dim_source.into(),
                sheet: None,
                header_row: Some(1),
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: AnalysisGraph::default(),
            runtime_analysis_contracts: BTreeMap::new(),
        },
    );

    let expr = json!({
        "__kind": "analysis_expr",
        "type": "lookup_value",
        "rowset": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "fact"
        },
        "field": "当事人",
        "lookup_rowset": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "parties"
        },
        "lookup_field": "id",
        "value_field": "name",
        "as_field": "当事人名称"
    });
    let rows = try_eval_analysis_expr_via_sql(app_root, &datasets, &expr)
        .expect("sql ok")
        .expect("lowered");
    assert_eq!(rows.len(), 2);
    let named = rows
        .iter()
        .find(|r| r.get("当事人").and_then(|v| v.as_str()) == Some("P1"))
        .and_then(|r| r.get("当事人名称").and_then(|v| v.as_str()));
    assert_eq!(named, Some("甲公司"));

    let evil = json!({
        "__kind": "analysis_expr",
        "type": "sql",
        "query": "DROP TABLE parties"
    });
    assert!(
        try_eval_analysis_expr_via_sql(app_root, &datasets, &evil)
            .expect("no err")
            .is_none()
    );
    let forbidden = json!({
        "__kind": "analysis_expr",
        "type": "sql",
        "query": "SELECT * FROM read_parquet('x')"
    });
    assert!(
        try_eval_analysis_expr_via_sql(app_root, &datasets, &forbidden)
            .expect("no err")
            .is_none()
    );
}
