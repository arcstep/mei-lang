#![cfg(test)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use mei_lang_kernel::{
    parquet_snapshot_path, AnalysisGraph, ColumnSchema, DatasetView, SourceDecl,
};
use serde_json::json;

use super::super::connection::with_app_connection;
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

fn write_source_and_parquet(
    app_root: &Path,
    source_rel: &str,
    create_sql: &str,
) -> DatasetView {
    prepare_app_root(app_root);
    let source_abs = app_root.join(source_rel);
    fs::create_dir_all(source_abs.parent().expect("parent")).expect("mkdir source");
    fs::write(&source_abs, b"fixture-source").expect("write source");
    let parquet = parquet_snapshot_path(app_root, source_rel, None, 1).expect("parquet path");
    fs::create_dir_all(parquet.parent().expect("parquet parent")).expect("mkdir store");
    let path_sql = format!("'{}'", parquet.to_string_lossy().replace('\'', "''"));
    with_app_connection(app_root, |conn| {
        conn.execute_batch(&format!("COPY ({create_sql}) TO {path_sql} (FORMAT PARQUET);"))?;
        Ok(())
    })
    .expect("write parquet");

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
    let view = write_source_and_parquet(
        app_root,
        "upload/data/inspections.xlsx",
        "SELECT * FROM (VALUES
            ('2025-03-10', '甲', 0.0),
            ('2025-03-12', '乙', 0.0)
         ) t(\"检查日期\", \"当事人\", \"罚款金额\")",
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
    let view = write_source_and_parquet(
        app_root,
        "upload/data/inspections.xlsx",
        "SELECT * FROM (VALUES
            ('2024-03-10', '甲', 0.0),
            ('2024-03-12', '乙', 0.0),
            ('2025-03-15', '丙', 0.0),
            ('2025-06-01', '丁', 0.0)
         ) t(\"检查日期\", \"当事人\", \"罚款金额\")",
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
    let view = write_source_and_parquet(
        app_root,
        "upload/data/penalties.xlsx",
        "SELECT * FROM (VALUES
            ('甲公司', '2024-01-10', 10000.0),
            ('甲公司', '2024-06-10', 15000.0),
            ('甲公司', '2025-02-01', 30000.0),
            ('乙公司', '2024-03-01', 5000.0)
         ) t(\"当事人\", \"检查日期\", \"罚款金额\")",
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
    let view = write_source_and_parquet(
        app_root,
        "upload/data/penalties.xlsx",
        "SELECT * FROM (VALUES
            ('事项A', '2025-01-10', 100.0),
            ('事项A', '2025-02-10', 100.0),
            ('事项A', '2025-03-10', 100.0),
            ('事项B', '2025-01-10', 100.0),
            ('事项B', '2024-01-10', 100.0)
         ) t(\"当事人\", \"检查日期\", \"罚款金额\")",
    );
    // Reuse 当事人 as 处罚事项 for this fixture by aliasing columns in schema/view id.
    let mut datasets = BTreeMap::new();
    let mut v = view;
    v.id = "penalties".into();
    // Treat 当事人 column as 处罚事项 for the aggregate party_field.
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
    assert!(!rows.is_empty());
    assert_eq!(rows[0].get("label").and_then(|v| v.as_str()), Some("事项A"));
    assert_eq!(rows[0].get("value").and_then(|v| v.as_f64()), Some(3.0));
}

#[test]
fn lookup_value_left_join_and_safe_sql_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    prepare_app_root(app_root);
    let fact = write_source_and_parquet(
        app_root,
        "upload/data/fact.xlsx",
        "SELECT * FROM (VALUES
            ('2024-01-01', 'P1', 10.0),
            ('2024-01-02', 'P2', 20.0)
         ) t(\"检查日期\", \"当事人\", \"罚款金额\")",
    );
    let dim_source = "upload/data/parties.xlsx";
    let dim_abs = app_root.join(dim_source);
    fs::create_dir_all(dim_abs.parent().unwrap()).unwrap();
    fs::write(&dim_abs, b"dim").unwrap();
    let dim_parquet = parquet_snapshot_path(app_root, dim_source, None, 1).unwrap();
    fs::create_dir_all(dim_parquet.parent().unwrap()).unwrap();
    let path_sql = format!("'{}'", dim_parquet.to_string_lossy().replace('\'', "''"));
    with_app_connection(app_root, |conn| {
        conn.execute_batch(&format!(
            "COPY (SELECT * FROM (VALUES ('P1', '甲公司'), ('P2', '乙公司')) t(id, name)) \
             TO {path_sql} (FORMAT PARQUET);"
        ))?;
        Ok(())
    })
    .unwrap();

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
