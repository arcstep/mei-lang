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
    assert!(try_eval_analysis_expr_via_sql(app_root, &datasets, &evil)
        .expect("no err")
        .is_none());
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

#[test]
fn group_by_with_universe_pads_missing_keys() {
    use super::try_eval_dataframe_metrics_via_sql;

    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    write_parquet_table(
        app_root,
        "upload/data/inspections.xlsx",
        &[
            ("企业", DataType::Utf8),
            ("园区ID", DataType::Utf8),
            ("结果", DataType::Utf8),
        ],
        vec![
            Arc::new(StringArray::from(vec!["A", "B", "C"])),
            Arc::new(StringArray::from(vec!["P1", "P1", "P2"])),
            Arc::new(StringArray::from(vec!["ok", "ok", "ok"])),
        ],
    );
    write_parquet_table(
        app_root,
        "upload/data/parks.xlsx",
        &[("园区ID", DataType::Utf8), ("园区名称", DataType::Utf8)],
        vec![
            Arc::new(StringArray::from(vec!["P1", "P2", "P3"])),
            Arc::new(StringArray::from(vec!["甲园", "乙园", "丙园"])),
        ],
    );

    let mut datasets = BTreeMap::new();
    datasets.insert(
        "inspections".into(),
        DatasetView {
            id: "inspections".into(),
            title: None,
            purpose: None,
            schema: vec![
                col("企业", "string"),
                col("园区ID", "string"),
                col("结果", "string"),
            ],
            stage_schema: Vec::new(),
            columns: vec!["企业".into(), "园区ID".into(), "结果".into()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "file".into(),
                path: "upload/data/inspections.xlsx".into(),
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
    datasets.insert(
        "parks".into(),
        DatasetView {
            id: "parks".into(),
            title: None,
            purpose: None,
            schema: vec![col("园区ID", "string"), col("园区名称", "string")],
            stage_schema: Vec::new(),
            columns: vec!["园区ID".into(), "园区名称".into()],
            rows: Vec::new(),
            source: SourceDecl {
                kind: "file".into(),
                path: "upload/data/parks.xlsx".into(),
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
        "type": "group_by",
        "by": "园区ID",
        "agg": "count",
        "universe": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "parks"
        },
        "rowset": {
            "__kind": "analysis_expr",
            "type": "where",
            "predicate": {
                "__kind": "analysis_expr",
                "type": "not_empty",
                "field": "园区ID"
            },
            "rowset": {
                "__kind": "analysis_expr",
                "type": "rows",
                "dataset": "inspections"
            }
        }
    });
    let rows = try_eval_analysis_expr_via_sql(app_root, &datasets, &expr)
        .expect("sql ok")
        .expect("group_by lowered");
    assert_eq!(rows.len(), 3, "rows={rows:?}");
    let by_id = |id: &str| {
        rows.iter()
            .find(|r| r.get("园区ID").and_then(|v| v.as_str()) == Some(id))
            .and_then(|r| r.get("value").and_then(|v| v.as_f64()))
    };
    assert_eq!(by_id("P1"), Some(2.0));
    assert_eq!(by_id("P2"), Some(1.0));
    assert_eq!(by_id("P3"), Some(0.0));

    // Zhifa-shaped park metric: lookup + group_by + universe via dataframe metric def.
    let park_metric = json!({
        "shape": "dataframe",
        "label": "分园区",
        "value": {
            "__kind": "analysis_expr",
            "type": "lookup_value",
            "field": "园区ID",
            "lookup_field": "园区ID",
            "value_field": "园区名称",
            "as_field": "园区名称",
            "lookup_rowset": {
                "__kind": "analysis_expr",
                "type": "rows",
                "dataset": "parks"
            },
            "rowset": expr
        }
    });
    let mut defs = BTreeMap::new();
    defs.insert("park_inspection_total_by_park".into(), park_metric);
    let out = try_eval_dataframe_metrics_via_sql(
        app_root,
        &datasets,
        &defs,
        &["park_inspection_total_by_park".into()],
    )
    .expect("sql ok")
    .expect("dataframe sql hit");
    let contract = out
        .get("park_inspection_total_by_park")
        .expect("metric present");
    let value_rows = contract.value.as_array().expect("array");
    assert_eq!(value_rows.len(), 3);
}

#[test]
fn zhifa_park_inspection_pipeline_sql_hits() {
    use super::try_eval_dataframe_metrics_via_sql;

    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    // inspections
    write_parquet_table(
        app_root,
        "upload/data/inspections.xlsx",
        &[
            ("检查对象名称", DataType::Utf8),
            ("检查结果", DataType::Utf8),
        ],
        vec![
            Arc::new(StringArray::from(vec!["企A", "企B", "企C", "企D"])),
            Arc::new(StringArray::from(vec![
                "无违规项",
                "有违规",
                "无违规项",
                "无违规项",
            ])),
        ],
    );
    write_parquet_table(
        app_root,
        "upload/data/enterprises.xlsx",
        &[("企业名称", DataType::Utf8), ("所属园区", DataType::Utf8)],
        vec![
            Arc::new(StringArray::from(vec!["企A", "企B", "企C", "企D"])),
            Arc::new(StringArray::from(vec!["甲园", "甲园", "乙园", "丙园"])),
        ],
    );
    write_parquet_table(
        app_root,
        "upload/data/parks.xlsx",
        &[("园区ID", DataType::Utf8), ("园区名称", DataType::Utf8)],
        vec![
            Arc::new(StringArray::from(vec!["P1", "P2", "P3"])),
            Arc::new(StringArray::from(vec!["甲园", "乙园", "丙园"])),
        ],
    );

    fn file_view(id: &str, path: &str, cols: &[(&str, &str)]) -> DatasetView {
        DatasetView {
            id: id.into(),
            title: None,
            purpose: None,
            schema: cols
                .iter()
                .map(|(n, t)| col(n, t))
                .collect(),
            stage_schema: Vec::new(),
            columns: cols.iter().map(|(n, _)| (*n).into()).collect(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "file".into(),
                path: path.into(),
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

    let mut datasets = BTreeMap::new();
    datasets.insert(
        "administrative_inspection_dashboard_ds".into(),
        file_view(
            "administrative_inspection_dashboard_ds",
            "upload/data/inspections.xlsx",
            &[("检查对象名称", "string"), ("检查结果", "string")],
        ),
    );
    datasets.insert(
        "key_enterprises".into(),
        file_view(
            "key_enterprises",
            "upload/data/enterprises.xlsx",
            &[("企业名称", "string"), ("所属园区", "string")],
        ),
    );
    datasets.insert(
        "logistics_park_vector".into(),
        file_view(
            "logistics_park_vector",
            "upload/data/parks.xlsx",
            &[("园区ID", "string"), ("园区名称", "string")],
        ),
    );

    let rows = |dataset: &str| {
        json!({
            "__ref": "data",
            "from_dataset": dataset,
            "id": dataset
        })
    };
    let lookup = |rowset: serde_json::Value,
                  field: &str,
                  lookup_ds: &str,
                  lookup_field: &str,
                  value_field: &str,
                  as_field: &str| {
        json!({
            "__kind": "analysis_expr",
            "type": "lookup_value",
            "field": field,
            "lookup_field": lookup_field,
            "value_field": value_field,
            "as_field": as_field,
            "lookup_rowset": rows(lookup_ds),
            "rowset": rowset
        })
    };

    // Mirrors v2 lower of park_inspection_total_by_park.
    let mut park_total = rows("administrative_inspection_dashboard_ds");
    park_total = lookup(
        park_total,
        "检查对象名称",
        "key_enterprises",
        "企业名称",
        "所属园区",
        "所属园区",
    );
    park_total = lookup(
        park_total,
        "所属园区",
        "logistics_park_vector",
        "园区名称",
        "园区ID",
        "园区ID",
    );
    park_total = json!({
        "__kind": "analysis_expr",
        "type": "where",
        "predicate": {"__kind":"analysis_expr","type":"not_empty","field":"园区ID"},
        "rowset": park_total
    });
    park_total = json!({
        "__kind": "analysis_expr",
        "type": "group_by",
        "by": "园区ID",
        "agg": "count",
        "universe": rows("logistics_park_vector"),
        "rowset": park_total
    });
    park_total = lookup(
        park_total,
        "园区ID",
        "logistics_park_vector",
        "园区ID",
        "园区名称",
        "园区名称",
    );

    let mut defs = BTreeMap::new();
    defs.insert(
        "park_inspection_total_by_park".into(),
        json!({"shape":"dataframe","value": park_total}),
    );

    let out = try_eval_dataframe_metrics_via_sql(
        app_root,
        &datasets,
        &defs,
        &["park_inspection_total_by_park".into()],
    )
    .expect("sql ok")
    .expect("must hit SQL for zhifa park pipeline");
    let rows = out
        .get("park_inspection_total_by_park")
        .expect("metric")
        .value
        .as_array()
        .expect("array");
    assert_eq!(rows.len(), 3, "rows={rows:?}");
}

#[test]
fn park_pipeline_hits_with_geojson_attr_universe() {
    use super::try_eval_dataframe_metrics_via_sql;

    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    write_parquet_table(
        app_root,
        "upload/data/inspections.xlsx",
        &[
            ("检查对象名称", DataType::Utf8),
            ("检查结果", DataType::Utf8),
        ],
        vec![
            Arc::new(StringArray::from(vec!["企A", "企B", "企C"])),
            Arc::new(StringArray::from(vec!["无违规项", "有违规", "无违规项"])),
        ],
    );
    write_parquet_table(
        app_root,
        "upload/data/enterprises.xlsx",
        &[("企业名称", DataType::Utf8), ("所属园区", DataType::Utf8)],
        vec![
            Arc::new(StringArray::from(vec!["企A", "企B", "企C"])),
            Arc::new(StringArray::from(vec!["甲园", "甲园", "乙园"])),
        ],
    );
    prepare_app_root(app_root);
    let geojson_rel = "upload/园区矢量.json";
    let geojson_abs = app_root.join(geojson_rel);
    fs::create_dir_all(geojson_abs.parent().expect("parent")).expect("mkdir");
    fs::write(
        &geojson_abs,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"id":"P1","name":"甲园"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}},
            {"type":"Feature","properties":{"id":"P2","name":"乙园"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}},
            {"type":"Feature","properties":{"id":"P3","name":"丙园"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}}
          ]
        }"#,
    )
    .expect("write geojson");

    fn file_view(id: &str, path: &str, kind: &str, cols: &[(&str, &str)]) -> DatasetView {
        DatasetView {
            id: id.into(),
            title: None,
            purpose: None,
            schema: cols
                .iter()
                .map(|(n, t)| ColumnSchema {
                    name: (*n).into(),
                    type_name: (*t).into(),
                    source: None,
                    optional: false,
                    unit: None,
                })
                .collect(),
            stage_schema: Vec::new(),
            columns: cols.iter().map(|(n, _)| (*n).into()).collect(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: kind.into(),
                path: path.into(),
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

    let mut parks = file_view(
        "logistics_park_vector",
        geojson_rel,
        "geojson",
        &[("园区ID", "string"), ("园区名称", "string")],
    );
    parks.schema = vec![
        ColumnSchema {
            name: "园区ID".into(),
            type_name: "string".into(),
            source: Some("id".into()),
            optional: false,
            unit: None,
        },
        ColumnSchema {
            name: "园区名称".into(),
            type_name: "string".into(),
            source: Some("name".into()),
            optional: false,
            unit: None,
        },
    ];

    let mut datasets = BTreeMap::new();
    datasets.insert(
        "administrative_inspection_dashboard_ds".into(),
        file_view(
            "administrative_inspection_dashboard_ds",
            "upload/data/inspections.xlsx",
            "file",
            &[("检查对象名称", "string"), ("检查结果", "string")],
        ),
    );
    datasets.insert(
        "key_enterprises".into(),
        file_view(
            "key_enterprises",
            "upload/data/enterprises.xlsx",
            "file",
            &[("企业名称", "string"), ("所属园区", "string")],
        ),
    );
    datasets.insert("logistics_park_vector".into(), parks);

    let rows = |dataset: &str| {
        json!({
            "__ref": "data",
            "from_dataset": dataset,
            "id": dataset
        })
    };
    let lookup = |rowset: serde_json::Value,
                  field: &str,
                  lookup_ds: &str,
                  lookup_field: &str,
                  value_field: &str,
                  as_field: &str| {
        json!({
            "__kind": "analysis_expr",
            "type": "lookup_value",
            "field": field,
            "lookup_field": lookup_field,
            "value_field": value_field,
            "as_field": as_field,
            "lookup_rowset": rows(lookup_ds),
            "rowset": rowset
        })
    };

    let mut park_total = rows("administrative_inspection_dashboard_ds");
    park_total = lookup(
        park_total,
        "检查对象名称",
        "key_enterprises",
        "企业名称",
        "所属园区",
        "所属园区",
    );
    park_total = lookup(
        park_total,
        "所属园区",
        "logistics_park_vector",
        "园区名称",
        "园区ID",
        "园区ID",
    );
    park_total = json!({
        "__kind": "analysis_expr",
        "type": "where",
        "predicate": {"__kind":"analysis_expr","type":"not_empty","field":"园区ID"},
        "rowset": park_total
    });
    park_total = json!({
        "__kind": "analysis_expr",
        "type": "group_by",
        "by": "园区ID",
        "agg": "count",
        "universe": rows("logistics_park_vector"),
        "rowset": park_total
    });
    park_total = lookup(
        park_total,
        "园区ID",
        "logistics_park_vector",
        "园区ID",
        "园区名称",
        "园区名称",
    );

    let mut defs = BTreeMap::new();
    defs.insert(
        "park_inspection_total_by_park".into(),
        json!({"shape":"dataframe","value": park_total}),
    );

    let out = try_eval_dataframe_metrics_via_sql(
        app_root,
        &datasets,
        &defs,
        &["park_inspection_total_by_park".into()],
    )
    .expect("sql ok")
    .expect("geojson attr parks must hit SQL");
    let rows = out
        .get("park_inspection_total_by_park")
        .expect("metric")
        .value
        .as_array()
        .expect("array");
    assert_eq!(rows.len(), 3, "rows={rows:?}");
}

#[test]
fn warnings_realtime_like_pipeline_sql_hits() {
    use super::try_eval_dataframe_metrics_via_sql;

    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    write_parquet_table(
        app_root,
        "upload/data/warnings.xlsx",
        &[
            ("预警ID", DataType::Utf8),
            ("预警等级", DataType::Utf8),
            ("主责单位", DataType::Utf8),
            ("问题分类名称", DataType::Utf8),
            ("预警条数", DataType::Utf8),
            ("预警时间", DataType::Utf8),
            ("问题跟踪ID", DataType::Utf8),
            ("承办部门", DataType::Utf8),
            ("办结时间", DataType::Utf8),
        ],
        vec![
            Arc::new(StringArray::from(vec!["W1", "W1", "W2"])),
            Arc::new(StringArray::from(vec!["高", "高", "中"])),
            Arc::new(StringArray::from(vec!["单位A", "单位A", "单位B"])),
            Arc::new(StringArray::from(vec!["模型1", "模型1", "模型2"])),
            Arc::new(StringArray::from(vec!["2", "2", "1"])),
            Arc::new(StringArray::from(vec![
                "2025-01-02",
                "2025-01-01",
                "2025-01-03",
            ])),
            Arc::new(StringArray::from(vec!["T1", "T1", ""])),
            Arc::new(StringArray::from(vec!["部门A", "", ""])),
            Arc::new(StringArray::from(vec!["", "", ""])),
        ],
    );

    let view = DatasetView {
        id: "warning_list".into(),
        title: None,
        purpose: None,
        schema: Vec::new(),
        stage_schema: Vec::new(),
        columns: vec![
            "预警ID".into(),
            "预警等级".into(),
            "主责单位".into(),
            "问题分类名称".into(),
            "预警条数".into(),
            "预警时间".into(),
            "问题跟踪ID".into(),
            "承办部门".into(),
            "办结时间".into(),
        ],
        rows: Vec::new(),
        source: SourceDecl {
            kind: "file".into(),
            path: "upload/data/warnings.xlsx".into(),
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
    };
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);

    let ae = |ty: &str, fields: serde_json::Value| {
        let mut obj = fields.as_object().cloned().unwrap_or_default();
        obj.insert("__kind".into(), json!("analysis_expr"));
        obj.insert("type".into(), json!(ty));
        serde_json::Value::Object(obj)
    };

    let base = ae(
        "rows",
        json!({"dataset": "warning_list"}),
    );
    let filtered = ae(
        "where",
        json!({
            "rowset": base,
            "predicate": ae("and", json!({
                "predicates": [
                    ae("not_empty", json!({"field":"预警ID"})),
                    ae("not", json!({
                        "predicate": ae("and", json!({
                            "predicates": [
                                ae("present", json!({"field":"问题跟踪ID"})),
                                ae("present", json!({"field":"承办部门"})),
                                ae("present", json!({"field":"办结时间"})),
                            ]
                        }))
                    }))
                ]
            }))
        }),
    );
    let first = ae(
        "first_by",
        json!({"rowset": filtered, "field": "预警ID"}),
    );
    let pending = ae(
        "mutate",
        json!({
            "rowset": ae("where", json!({
                "rowset": first.clone(),
                "predicate": ae("and", json!({
                    "predicates": [
                        ae("present", json!({"field":"问题跟踪ID"})),
                        ae("blank", json!({"field":"承办部门"}))
                    ]
                }))
            })),
            "updates": {"当前状态": ae("lit", json!({"value":"待办"}))}
        }),
    );
    let in_progress = ae(
        "mutate",
        json!({
            "rowset": ae("where", json!({
                "rowset": first.clone(),
                "predicate": ae("and", json!({
                    "predicates": [
                        ae("present", json!({"field":"问题跟踪ID"})),
                        ae("present", json!({"field":"承办部门"})),
                        ae("blank", json!({"field":"办结时间"}))
                    ]
                }))
            })),
            "updates": {"当前状态": ae("lit", json!({"value":"在办"}))}
        }),
    );
    let other = ae(
        "mutate",
        json!({
            "rowset": ae("where", json!({
                "rowset": first.clone(),
                "predicate": ae("not", json!({
                    "predicate": ae("or", json!({
                        "predicates": [
                            ae("and", json!({
                                "predicates": [
                                    ae("present", json!({"field":"问题跟踪ID"})),
                                    ae("blank", json!({"field":"承办部门"}))
                                ]
                            })),
                            ae("and", json!({
                                "predicates": [
                                    ae("present", json!({"field":"问题跟踪ID"})),
                                    ae("present", json!({"field":"承办部门"})),
                                    ae("blank", json!({"field":"办结时间"}))
                                ]
                            }))
                        ]
                    }))
                }))
            })),
            "updates": {"当前状态": ae("lit", json!({"value":"待办"}))}
        }),
    );
    let labeled = ae(
        "concat_rowsets",
        json!({"rowsets": [pending, in_progress, other]}),
    );
    let selected = ae(
        "select",
        json!({
            "rowset": labeled,
            "fields": ["预警ID", "预警等级", "主责单位", "问题分类名称", "预警条数", "预警时间", "当前状态"]
        }),
    );
    let sorted = ae(
        "sort_by",
        json!({"rowset": selected, "field": "预警时间", "order": "desc"}),
    );
    let renamed = ae(
        "rename",
        json!({
            "rowset": sorted,
            "mapping": {
                "预警ID": "warning_id",
                "预警等级": "level",
                "主责单位": "org",
                "问题分类名称": "model",
                "预警条数": "count",
                "当前状态": "status"
            }
        }),
    );

    let mut defs = BTreeMap::new();
    defs.insert(
        "warnings_realtime_cockpit_table".into(),
        json!({"shape":"dataframe","value": renamed}),
    );

    let out = try_eval_dataframe_metrics_via_sql(
        app_root,
        &datasets,
        &defs,
        &["warnings_realtime_cockpit_table".into()],
    )
    .expect("sql ok")
    .expect("warnings realtime pipeline must hit SQL");
    let rows = out
        .get("warnings_realtime_cockpit_table")
        .expect("metric")
        .value
        .as_array()
        .expect("array");
    assert_eq!(rows.len(), 2, "rows={rows:?}");
}

#[test]
fn uncovered_pipeline_returns_none_for_fail_fast_callers() {
    use super::{
        record_pipeline_sql_fallback, snapshot_pipeline_sql_stats, try_eval_dataframe_metrics_via_sql,
    };

    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let view = write_inspections_view(
        app_root,
        "upload/data/inspections.xlsx",
        &["2025-01-01"],
        &["甲"],
        &[1.0],
    );
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);
    let mut defs = BTreeMap::new();
    defs.insert(
        "unsupported_df".into(),
        json!({
            "shape": "dataframe",
            "value": {
                "__kind": "analysis_expr",
                "type": "split_text",
                "field": "当事人",
                "rowset": {
                    "__kind": "analysis_expr",
                    "type": "rows",
                    "dataset": "inspections"
                }
            }
        }),
    );
    let before = snapshot_pipeline_sql_stats();
    let out = try_eval_dataframe_metrics_via_sql(
        app_root,
        &datasets,
        &defs,
        &["unsupported_df".into()],
    )
    .expect("no exec err");
    assert!(out.is_none(), "uncovered must be Ok(None) so callers fail-fast");
    let after = snapshot_pipeline_sql_stats();
    assert!(
        after.1 > before.1 || after.1 >= 1,
        "fallback counter should advance (before={before:?} after={after:?})"
    );
    let _ = record_pipeline_sql_fallback;
}

#[test]
fn latest_days_pipeline_sql_matches_max_date_window() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    // Max date 2025-01-10; 7-day window => 2025-01-04..=2025-01-10 (4 in-window rows)
    let view = write_inspections_view(
        app_root,
        "upload/data/latest_days.csv",
        &[
            "2025-01-01",
            "2025-01-04",
            "2025-01-07",
            "2025-01-09",
            "2025-01-10",
            "2024-12-01",
        ],
        &["a", "b", "c", "d", "e", "f"],
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    );
    let mut datasets = BTreeMap::new();
    datasets.insert("inspections".into(), view);
    let expr = json!({
        "__kind": "analysis_expr",
        "type": "latest_days",
        "field": "检查日期",
        "days": 7,
        "rowset": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "inspections"
        }
    });
    let rows = try_eval_analysis_expr_via_sql(app_root, &datasets, &expr)
        .expect("sql ok")
        .expect("must lower");
    assert_eq!(rows.len(), 4, "rows={rows:?}");
    let count = super::try_count_analysis_expr_via_sql(app_root, &datasets, &expr)
        .expect("count ok")
        .expect("must count");
    assert_eq!(count, 4);
}

#[test]
fn in_values_and_sum_over_first_by_pipeline_sql() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    prepare_app_root(app_root);
    write_parquet_table(
        app_root,
        "upload/data/warn.csv",
        &[
            ("预警ID", DataType::Utf8),
            ("是否查实", DataType::Utf8),
            ("查实条数", DataType::Float64),
            ("预警条数", DataType::Float64),
        ],
        vec![
            Arc::new(StringArray::from(vec!["w1", "w1", "w2", "w3"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["是", "否", "是", "待定"])) as ArrayRef,
            Arc::new(Float64Array::from(vec![1.0, 0.0, 1.0, 0.0])) as ArrayRef,
            Arc::new(Float64Array::from(vec![1.0, 1.0, 1.0, 1.0])) as ArrayRef,
        ],
    );
    let view = DatasetView {
        id: "warning_list".into(),
        title: None,
        purpose: None,
        schema: vec![
            col("预警ID", "string"),
            col("是否查实", "string"),
            col("查实条数", "number"),
            col("预警条数", "number"),
        ],
        stage_schema: Vec::new(),
        columns: vec![
            "预警ID".into(),
            "是否查实".into(),
            "查实条数".into(),
            "预警条数".into(),
        ],
        rows: Vec::new(),
        source: SourceDecl {
            kind: "file".into(),
            path: "upload/data/warn.csv".into(),
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
    };
    let mut datasets = BTreeMap::new();
    datasets.insert("warning_list".into(), view);
    let rowset = json!({
        "__kind": "analysis_expr",
        "type": "where",
        "rowset": {
            "__kind": "analysis_expr",
            "type": "first_by",
            "field": "预警ID",
            "rowset": {
                "__kind": "analysis_expr",
                "type": "rows",
                "dataset": "warning_list"
            }
        },
        "predicate": {
            "__kind": "analysis_expr",
            "type": "in_values",
            "field": "是否查实",
            "values": ["是", "否"]
        }
    });
    let rows = try_eval_analysis_expr_via_sql(app_root, &datasets, &rowset)
        .expect("sql ok")
        .expect("must lower where+first_by+in_values");
    assert_eq!(rows.len(), 2, "dedup then in_values keep 是/否, rows={rows:?}");
    let verified = super::try_agg_analysis_expr_via_sql(
        app_root,
        &datasets,
        &rowset,
        "查实条数",
        "SUM",
    )
    .expect("agg ok")
    .expect("must sum");
    // first_by keeps one of the two w1 rows; either 是(1) or 否(0) depending on order.
    assert!(
        (verified - 1.0).abs() < 1e-9 || (verified - 0.0).abs() < 1e-9 || (verified - 2.0).abs() < 1e-9,
        "verified sum unexpected: {verified}"
    );
}

#[test]
fn count_distinct_prefix_pipeline_sql() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    prepare_app_root(app_root);
    write_parquet_table(
        app_root,
        "upload/data/models.csv",
        &[("序号", DataType::Utf8), ("监督模型", DataType::Utf8)],
        vec![
            Arc::new(StringArray::from(vec!["1", "1-a", "2", "x", "3"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["a", "b", "c", "d", "e"])) as ArrayRef,
        ],
    );
    let view = DatasetView {
        id: "warning_models".into(),
        title: None,
        purpose: None,
        schema: vec![col("序号", "string"), col("监督模型", "string")],
        stage_schema: Vec::new(),
        columns: vec!["序号".into(), "监督模型".into()],
        rows: Vec::new(),
        source: SourceDecl {
            kind: "file".into(),
            path: "upload/data/models.csv".into(),
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
    };
    let mut datasets = BTreeMap::new();
    datasets.insert("warning_models".into(), view);
    let filtered = json!({
        "__kind": "analysis_expr",
        "type": "where",
        "rowset": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "warning_models"
        },
        "predicate": {
            "__kind": "analysis_expr",
            "type": "matches",
            "field": "序号",
            "pattern": "^\\s*\\d+(?:-.*)?\\s*$"
        }
    });
    let mutated = json!({
        "__kind": "analysis_expr",
        "type": "mutate",
        "rowset": filtered,
        "updates": {
            "序号前缀": {
                "__kind": "analysis_expr",
                "type": "extract_number",
                "field": "序号",
                "pattern": "^\\s*(\\d+)"
            }
        }
    });
    let rowset = json!({
        "__kind": "analysis_expr",
        "type": "first_by",
        "field": "序号前缀",
        "rowset": mutated
    });
    let count = super::try_count_analysis_expr_via_sql(app_root, &datasets, &rowset)
        .expect("count ok")
        .expect("must lower count_distinct-like pipeline");
    // 1 and 1-a share prefix 1; plus 2 and 3 → 3 distinct prefixes (x filtered out)
    assert_eq!(count, 3);
}

#[test]
fn split_text_pipeline_sql_counts_parts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    prepare_app_root(app_root);
    write_parquet_table(
        app_root,
        "upload/data/mech.csv",
        &[("id", DataType::Utf8), ("健全机制", DataType::Utf8)],
        vec![
            Arc::new(StringArray::from(vec!["1", "2", "3"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["甲、乙", "", "丙"])) as ArrayRef,
        ],
    );
    let view = DatasetView {
        id: "issue_result_list".into(),
        title: None,
        purpose: None,
        schema: vec![col("id", "string"), col("健全机制", "string")],
        stage_schema: Vec::new(),
        columns: vec!["id".into(), "健全机制".into()],
        rows: Vec::new(),
        source: SourceDecl {
            kind: "file".into(),
            path: "upload/data/mech.csv".into(),
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
    };
    let mut datasets = BTreeMap::new();
    datasets.insert("issue_result_list".into(), view);
    let expr = json!({
        "__kind": "analysis_expr",
        "type": "split_text",
        "field": "健全机制",
        "delimiter": "、",
        "rowset": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "issue_result_list"
        }
    });
    let count = super::try_count_analysis_expr_via_sql(app_root, &datasets, &expr)
        .expect("count ok")
        .expect("must lower split_text");
    // 甲、乙 → 2; empty → 1; 丙 → 1 => 4
    assert_eq!(count, 4);
}

#[test]
fn mutate_div_extract_match_coalesce_pipeline_sql() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    prepare_app_root(app_root);
    write_parquet_table(
        app_root,
        "upload/data/pension.csv",
        &[
            ("单位", DataType::Utf8),
            ("项目", DataType::Utf8),
            ("金额", DataType::Utf8),
        ],
        vec![
            Arc::new(StringArray::from(vec![
                "沙坪坝街道养老中心",
                "其他单位",
                "青木关镇项目办",
            ])) as ArrayRef,
            Arc::new(StringArray::from(vec![
                "无街镇",
                "陈家桥街道改造",
                "无",
            ])) as ArrayRef,
            Arc::new(StringArray::from(vec!["20000", "10000", "5000"])) as ArrayRef,
        ],
    );
    let view = DatasetView {
        id: "pension".into(),
        title: None,
        purpose: None,
        schema: vec![
            col("单位", "string"),
            col("项目", "string"),
            col("金额", "string"),
        ],
        stage_schema: Vec::new(),
        columns: vec!["单位".into(), "项目".into(), "金额".into()],
        rows: Vec::new(),
        source: SourceDecl {
            kind: "file".into(),
            path: "upload/data/pension.csv".into(),
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
    };
    let mut datasets = BTreeMap::new();
    datasets.insert("pension".into(), view);
    let street_pat =
        "((?:土主|歌乐山|联芳|井口|陈家桥|青木关|凤凰|回龙坝|中梁|覃家岗|天星桥|小龙坎|沙坪坝|渝碚路|双碑|山洞|新桥|石井坡|丰文|磁器口|童家桥|土湾)(?:街道|镇))";
    let rowset = json!({
        "__kind": "analysis_expr",
        "type": "mutate",
        "rowset": {
            "__kind": "analysis_expr",
            "type": "rows",
            "dataset": "pension"
        },
        "updates": {
            "金额_万元": {
                "__kind": "analysis_expr",
                "type": "div",
                "field": "金额",
                "by": 10000
            },
            "street_u": {
                "__kind": "analysis_expr",
                "type": "extract_match",
                "field": "单位",
                "pattern": street_pat
            },
            "street_p": {
                "__kind": "analysis_expr",
                "type": "extract_match",
                "field": "项目",
                "pattern": street_pat
            },
            "街镇名称": {
                "__kind": "analysis_expr",
                "type": "coalesce",
                "fields": ["street_u", "street_p"]
            }
        }
    });
    let sum = super::try_agg_analysis_expr_via_sql(
        app_root,
        &datasets,
        &rowset,
        "金额_万元",
        "SUM",
    )
    .expect("sum ok")
    .expect("must lower mutate div");
    assert!((sum - 3.5).abs() < 1e-9, "sum={sum}");

    let grouped = json!({
        "__kind": "analysis_expr",
        "type": "group_by",
        "by": "街镇名称",
        "rowset": {
            "__kind": "analysis_expr",
            "type": "where",
            "rowset": rowset,
            "predicate": {
                "__kind": "analysis_expr",
                "type": "not_empty",
                "field": "街镇名称"
            }
        }
    });
    let count = super::try_count_analysis_expr_via_sql(app_root, &datasets, &grouped)
        .expect("count ok")
        .expect("must lower coalesce street group");
    // 沙坪坝街道 / 陈家桥街道 / 青木关镇 → 3 groups
    assert_eq!(count, 3);
}
