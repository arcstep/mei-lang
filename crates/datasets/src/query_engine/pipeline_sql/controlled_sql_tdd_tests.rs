//! TDD: nested `concat_rowsets` width-copy vs shared-base category expand.
//! Production `try_lower_expr` / `lower_concat_rowsets` must factor shared bases
//! and refuse uncontrolled plans before DataFusion execute.

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

use super::exec::execute_sql_plan;
use super::lower::{
    audit_sql_plan_shape, is_controlled_sql_plan, try_lower_category_expand_shared_cte,
    try_lower_expr, CategoryExpandArm,
};

fn col(name: &str, type_name: &str) -> ColumnSchema {
    ColumnSchema {
        name: name.into(),
        type_name: type_name.into(),
        source: None,
        optional: false,
        unit: None,
        normalize: None,
                primary: false,
            hidden: false,
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

fn write_issues_view(app_root: &Path) -> DatasetView {
    prepare_app_root(app_root);
    let source_rel = "upload/data/issues.csv";
    let source_abs = app_root.join(source_rel);
    fs::create_dir_all(source_abs.parent().expect("parent")).expect("mkdir");
    fs::write(&source_abs, b"fixture").expect("source");
    let parquet = parquet_snapshot_path(app_root, source_rel, None, 1).expect("parquet path");
    fs::create_dir_all(parquet.parent().expect("parent")).expect("mkdir store");
    let schema = Arc::new(Schema::new(vec![
        Field::new("处理结果ID-问题跟踪ID", DataType::Utf8, true),
        Field::new("是否转问题线索", DataType::Utf8, true),
        Field::new("是否立案", DataType::Utf8, true),
        Field::new("是否处分", DataType::Utf8, true),
        Field::new("挽回资金", DataType::Float64, true),
        Field::new("健全机制", DataType::Utf8, true),
    ]));
    // R1 matches 线索+立案；R2 matches 处分+资金；R3 matches 机制 only.
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec!["A-1", "B-2", "C-3"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["是", "否", "否"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["是", "否", "否"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["否", "是", "否"])) as ArrayRef,
            Arc::new(Float64Array::from(vec![0.0, 12.5, 0.0])) as ArrayRef,
            Arc::new(StringArray::from(vec!["", "", "制度修订"])) as ArrayRef,
        ],
    )
    .expect("batch");
    let file = fs::File::create(&parquet).expect("create");
    let mut writer = ArrowWriter::try_new(file, schema, None).expect("writer");
    writer.write(&batch).expect("write");
    writer.close().expect("close");

    DatasetView {
        id: "issue_result_list".into(),
        title: None,
        purpose: None,
        schema: vec![
            col("处理结果ID-问题跟踪ID", "string"),
            col("是否转问题线索", "string"),
            col("是否立案", "string"),
            col("是否处分", "string"),
            col("挽回资金", "number"),
            col("健全机制", "string"),
        ],
        stage_schema: Vec::new(),
        columns: vec![
            "处理结果ID-问题跟踪ID".into(),
            "是否转问题线索".into(),
            "是否立案".into(),
            "是否处分".into(),
            "挽回资金".into(),
            "健全机制".into(),
        ],
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
                        primary_key: None,
            },
        sources: Vec::new(),
        metrics: BTreeMap::new(),
        runtime_metric_defs: BTreeMap::new(),
        runtime_analysis_graph: AnalysisGraph::default(),
        runtime_analysis_contracts: BTreeMap::new(),
    }
}

fn ae(ty: &str, fields: serde_json::Value) -> serde_json::Value {
    let mut obj = fields.as_object().cloned().unwrap_or_default();
    obj.insert("__kind".into(), json!("analysis_expr"));
    obj.insert("type".into(), json!(ty));
    serde_json::Value::Object(obj)
}

fn in_values(field: &str, values: &[&str]) -> serde_json::Value {
    ae(
        "in_values",
        json!({
            "field": field,
            "values": values,
        }),
    )
}

fn gt(field: &str, value: f64) -> serde_json::Value {
    ae("gt", json!({ "field": field, "value": value }))
}

fn not_empty(field: &str) -> serde_json::Value {
    ae("not_empty", json!({ "field": field }))
}

fn mutate_category(rowset: serde_json::Value, pred: serde_json::Value, label: &str) -> serde_json::Value {
    ae(
        "mutate",
        json!({
            "rowset": ae("where", json!({ "rowset": rowset, "predicate": pred })),
            "updates": { "监督成效类别": ae("lit", json!({ "value": label })) }
        }),
    )
}

/// Like `party_gov_sanction_rows` / `handled_person_rows`: first_by(where(...)).
fn mutate_category_first_by(
    rowset: serde_json::Value,
    pred: serde_json::Value,
    label: &str,
) -> serde_json::Value {
    ae(
        "mutate",
        json!({
            "rowset": ae("first_by", json!({
                "rowset": ae("where", json!({ "rowset": rowset, "predicate": pred })),
                "field": "处理结果ID-问题跟踪ID",
            })),
            "updates": { "监督成效类别": ae("lit", json!({ "value": label })) }
        }),
    )
}

/// Mimic effectiveness: inner status-mark concat, then 6-way category concat
/// that **re-embeds** the inner tree in every arm (legacy width copy).
fn legacy_nested_category_expand_expr() -> serde_json::Value {
    let base = ae("rows", json!({ "dataset": "issue_result_list" }));
    let first = ae(
        "first_by",
        json!({
            "rowset": base,
            "field": "处理结果ID-问题跟踪ID",
        }),
    );
    // Inner "marked" layer (2-way concat) — each outer arm will re-lower this whole tree.
    let marked = ae(
        "concat_rowsets",
        json!({
            "rowsets": [
                ae("mutate", json!({
                    "rowset": ae("where", json!({
                        "rowset": first.clone(),
                        "predicate": in_values("是否处分", &["是"]),
                    })),
                    "updates": { "处分标记": ae("lit", json!({ "value": "是" })) }
                })),
                ae("mutate", json!({
                    "rowset": ae("where", json!({
                        "rowset": first.clone(),
                        "predicate": in_values("是否处分", &["否"]),
                    })),
                    "updates": { "处分标记": ae("lit", json!({ "value": "否" })) }
                })),
            ]
        }),
    );
    ae(
        "concat_rowsets",
        json!({
            "rowsets": [
                mutate_category(marked.clone(), in_values("是否转问题线索", &["是"]), "转问题线索"),
                mutate_category(marked.clone(), in_values("是否立案", &["是"]), "立案"),
                mutate_category(marked.clone(), in_values("是否处分", &["是"]), "党纪政务处分"),
                mutate_category(marked.clone(), in_values("处分标记", &["是"]), "处理"),
                mutate_category(marked.clone(), gt("挽回资金", 0.0), "挽回资金"),
                mutate_category(marked.clone(), not_empty("健全机制"), "健全机制"),
            ]
        }),
    )
}

/// Same shape as zhifa `effectiveness_analytics_list`: two arms are
/// `mutate(first_by(where(SAME,…)))` (party_gov / handled_person).
fn effectiveness_like_mixed_first_by_expand_expr() -> serde_json::Value {
    let base = ae("rows", json!({ "dataset": "issue_result_list" }));
    let first = ae(
        "first_by",
        json!({
            "rowset": base,
            "field": "处理结果ID-问题跟踪ID",
        }),
    );
    let marked = ae(
        "concat_rowsets",
        json!({
            "rowsets": [
                ae("mutate", json!({
                    "rowset": ae("where", json!({
                        "rowset": first.clone(),
                        "predicate": in_values("是否处分", &["是"]),
                    })),
                    "updates": { "处分标记": ae("lit", json!({ "value": "是" })) }
                })),
                ae("mutate", json!({
                    "rowset": ae("where", json!({
                        "rowset": first.clone(),
                        "predicate": in_values("是否处分", &["否"]),
                    })),
                    "updates": { "处分标记": ae("lit", json!({ "value": "否" })) }
                })),
            ]
        }),
    );
    ae(
        "concat_rowsets",
        json!({
            "rowsets": [
                mutate_category(marked.clone(), in_values("是否转问题线索", &["是"]), "转问题线索"),
                mutate_category(marked.clone(), in_values("是否立案", &["是"]), "立案"),
                mutate_category_first_by(marked.clone(), in_values("是否处分", &["是"]), "党纪政务处分"),
                mutate_category_first_by(marked.clone(), in_values("处分标记", &["是"]), "处理"),
                mutate_category(marked.clone(), gt("挽回资金", 0.0), "挽回资金"),
                mutate_category(marked.clone(), not_empty("健全机制"), "健全机制"),
            ]
        }),
    )
}

fn shared_base_expr() -> serde_json::Value {
    // Shared base = first_by only (marking can be a separate step later).
    ae(
        "first_by",
        json!({
            "rowset": ae("rows", json!({ "dataset": "issue_result_list" })),
            "field": "处理结果ID-问题跟踪ID",
        }),
    )
}

fn shared_arms() -> Vec<CategoryExpandArm> {
    vec![
        CategoryExpandArm {
            predicate: in_values("是否转问题线索", &["是"]),
            label: "转问题线索".into(),
        },
        CategoryExpandArm {
            predicate: in_values("是否立案", &["是"]),
            label: "立案".into(),
        },
        CategoryExpandArm {
            predicate: in_values("是否处分", &["是"]),
            label: "党纪政务处分".into(),
        },
        CategoryExpandArm {
            predicate: in_values("是否处分", &["是"]),
            label: "处理".into(),
        },
        CategoryExpandArm {
            predicate: gt("挽回资金", 0.0),
            label: "挽回资金".into(),
        },
        CategoryExpandArm {
            predicate: not_empty("健全机制"),
            label: "健全机制".into(),
        },
    ]
}

#[test]
fn category_expand_concat_is_factored_and_controlled() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    let view = write_issues_view(app_root);
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);

    let expr = legacy_nested_category_expand_expr();
    let plan = try_lower_expr(app_root, &datasets, &expr)
        .expect("lower result")
        .expect("category-expand must lower via shared-base factoring");

    let audit = audit_sql_plan_shape(&plan);
    assert!(
        is_controlled_sql_plan(&plan).is_ok(),
        "factored plan must pass controlled gate; audit={audit:?}"
    );
    assert!(
        plan.final_sql.contains(" AS _arm "),
        "expected shared-base arms, sql={}",
        &plan.final_sql[..plan.final_sql.len().min(400)]
    );
    let rows = execute_sql_plan(app_root, &plan).expect("execute");
    assert!(rows.len() >= 4, "rows={rows:?}");
}

#[test]
fn effectiveness_like_mixed_first_by_arms_are_factored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    let view = write_issues_view(app_root);
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);

    let plan = try_lower_expr(
        app_root,
        &datasets,
        &effectiveness_like_mixed_first_by_expand_expr(),
    )
    .expect("lower")
    .expect("mixed first_by arms must still share base");

    let audit = audit_sql_plan_shape(&plan);
    assert!(
        is_controlled_sql_plan(&plan).is_ok(),
        "audit={audit:?} sql_len={}",
        plan.final_sql.len()
    );
    assert!(plan.final_sql.contains(" AS _arm "));
    assert!(
        plan.final_sql.matches("UNION ALL").count() <= 8,
        "union={}",
        plan.final_sql.matches("UNION ALL").count()
    );
    let rows = execute_sql_plan(app_root, &plan).expect("execute");
    assert!(rows.len() >= 4, "rows={rows:?}");
}

#[test]
fn category_expand_does_not_width_copy_marked_columns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    let view = write_issues_view(app_root);
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);

    let plan = try_lower_expr(app_root, &datasets, &legacy_nested_category_expand_expr())
        .expect("lower")
        .expect("plan");

    // Pre-fix width-copy for this fixture was ~9KB+ with 11+ UNION ALL.
    assert!(
        plan.final_sql.len() < 6_000,
        "factored SQL too large (width-copy?): {}",
        plan.final_sql.len()
    );
    assert!(
        plan.final_sql.matches("UNION ALL").count() <= 8,
        "too many UNION ALL: {}",
        plan.final_sql.matches("UNION ALL").count()
    );
    assert!(plan.final_sql.contains(" AS _arm "));
}

#[test]
fn uncontrolled_sql_gate_rejects_huge_union_plan() {
    let plan = super::lower::SqlPlan {
        setup_ddls: Vec::new(),
        final_sql: format!(
            "SELECT 1 AS _c {}",
            " UNION ALL SELECT 1 AS _c".repeat(40)
        ),
        result_columns: vec!["_c".into()],
    };
    let err = is_controlled_sql_plan(&plan).expect_err("must reject");
    assert!(
        err.contains("uncontrolled_sql_plan"),
        "err={err}"
    );
}

/// Stack overflow aborts the process (uncatchable). Keep ignored for manual runs
/// on deeper trees; default CI relies on the controlled-SQL gate above.
#[test]
#[ignore = "stack overflow aborts process; run manually with --ignored when reproducing crash"]
fn legacy_nested_concat_execute_may_abort_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    let view = write_issues_view(app_root);
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);

    let plan = try_lower_expr(app_root, &datasets, &legacy_nested_category_expand_expr())
        .expect("lower")
        .expect("plan");
    // On production-scale trees this aborted; small fixture may still succeed —
    // the point of --ignored is manual stress with deeper nests / tiny stack.
    let _ = execute_sql_plan(app_root, &plan);
}

#[test]
fn shared_base_category_expand_is_controlled_and_correct() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    let view = write_issues_view(app_root);
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);

    let plan = try_lower_category_expand_shared_cte(
        app_root,
        &datasets,
        &shared_base_expr(),
        "监督成效类别",
        &shared_arms(),
    )
    .expect("lower ok")
    .expect("shared expand must lower");

    let audit = audit_sql_plan_shape(&plan);
    assert!(
        is_controlled_sql_plan(&plan).is_ok(),
        "shared-CTE expand must pass controlled gate; audit={audit:?} sql={}",
        plan.final_sql
    );
    // Base should appear once as CTE; arms reference it, not re-inline marked trees.
    assert!(
        plan.final_sql.starts_with("WITH "),
        "expected shared base CTE, got: {}",
        &plan.final_sql[..plan.final_sql.len().min(200)]
    );
    assert_eq!(
        plan.final_sql.matches("处分标记").count(),
        0,
        "shared path should not carry the legacy inner marked concat"
    );

    let rows = execute_sql_plan(app_root, &plan).expect("execute shared plan");
    // R1 → 线索+立案；R2 → 处分+处理+资金；R3 → 机制  ⇒ 2+3+1 = 6
    assert_eq!(rows.len(), 6, "rows={rows:?}");

    use std::collections::BTreeSet;
    let labels: BTreeSet<String> = rows
        .iter()
        .filter_map(|r| {
            r.get("监督成效类别")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    let expected: BTreeSet<String> = [
        "转问题线索",
        "立案",
        "党纪政务处分",
        "处理",
        "挽回资金",
        "健全机制",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(labels, expected);
}

#[test]
fn shared_plan_sql_much_smaller_than_unfactored_expectation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app_root = dir.path();
    let view = write_issues_view(app_root);
    let mut datasets = BTreeMap::new();
    datasets.insert(view.id.clone(), view);

    let via_concat = try_lower_expr(app_root, &datasets, &legacy_nested_category_expand_expr())
        .expect("concat lower")
        .expect("concat plan");
    let via_helper = try_lower_category_expand_shared_cte(
        app_root,
        &datasets,
        &shared_base_expr(),
        "监督成效类别",
        &shared_arms(),
    )
    .expect("shared lower")
    .expect("shared plan");

    assert!(is_controlled_sql_plan(&via_concat).is_ok());
    assert!(is_controlled_sql_plan(&via_helper).is_ok());
    // Both paths must stay far below the pre-fix width-copy size (~9KB+ for 6 arms).
    assert!(
        via_concat.final_sql.len() < 6_000,
        "factored concat still too large: {}",
        via_concat.final_sql.len()
    );
    assert!(
        via_helper.final_sql.len() < 6_000,
        "helper plan still too large: {}",
        via_helper.final_sql.len()
    );
}

/// Parameterized width-copy probe for crash-threshold measurement.
///
/// Env:
/// - `MEI_WIDTH_PROBE_ARMS` (required): outer `concat_rowsets` arm count
/// - `MEI_WIDTH_PROBE_INNER` (default 2): inner marked-layer concat width
/// - `MEI_WIDTH_PROBE_NEST` (default 1): how many times to wrap marked in an extra
///   identity concat layer before outer expand (deepens tree without changing semantics much)
/// - `MEI_WIDTH_PROBE_PHASE`: `lower` | `exec` | `both` (default `both`)
/// - `MEI_WIDTH_PROBE_STACK_KB`: if set, run lower+exec on a dedicated thread with this
///   stack size (KiB). Useful to find crash threshold vs stack budget.
///
/// Exit codes (when run as `--ignored` child):
/// - 0 success
/// - 2 lower returned None / error
/// - 3 exec error (non-abort)
/// Abort / stack overflow → non-zero from OS (often 101 / SIGABRT).
#[test]
#[ignore = "subprocess probe only; driven by scripts/probe_width_copy_crash.sh"]
fn width_copy_crash_probe_child() {
    let arms: usize = std::env::var("MEI_WIDTH_PROBE_ARMS")
        .expect("MEI_WIDTH_PROBE_ARMS")
        .parse()
        .expect("arms usize");
    let inner: usize = std::env::var("MEI_WIDTH_PROBE_INNER")
        .unwrap_or_else(|_| "2".into())
        .parse()
        .unwrap_or(2)
        .max(1);
    let nest: usize = std::env::var("MEI_WIDTH_PROBE_NEST")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .unwrap_or(1)
        .max(1);
    let phase = std::env::var("MEI_WIDTH_PROBE_PHASE").unwrap_or_else(|_| "both".into());
    let stack_kb: Option<usize> = std::env::var("MEI_WIDTH_PROBE_STACK_KB")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|v| *v >= 64);

    let work = move || {
        let dir = tempfile::tempdir().expect("tempdir");
        let app_root = dir.path().to_path_buf();
        let view = write_issues_view(&app_root);
        let mut datasets = BTreeMap::new();
        datasets.insert(view.id.clone(), view);

        let expr = legacy_width_copy_expr(arms, inner, nest);
        let plan = match try_lower_expr(&app_root, &datasets, &expr) {
            Ok(Some(p)) => p,
            Ok(None) => {
                eprintln!("PROBE lower=None arms={arms} inner={inner} nest={nest}");
                std::process::exit(2);
            }
            Err(err) => {
                eprintln!("PROBE lower_err={err:#} arms={arms} inner={inner} nest={nest}");
                std::process::exit(2);
            }
        };
        eprintln!(
            "PROBE lower_ok arms={arms} inner={inner} nest={nest} sql_chars={} union_all={} width_alias={}",
            plan.final_sql.len(),
            plan.final_sql.matches("UNION ALL").count(),
            plan.final_sql.matches(" AS _mut").count() + plan.final_sql.matches(" AS _c").count(),
        );
        if phase == "lower" {
            return;
        }
        match execute_sql_plan(&app_root, &plan) {
            Ok(rows) => {
                eprintln!(
                    "PROBE exec_ok arms={arms} inner={inner} nest={nest} rows={}",
                    rows.len()
                );
            }
            Err(err) => {
                eprintln!("PROBE exec_err={err:#} arms={arms} inner={inner} nest={nest}");
                std::process::exit(3);
            }
        }
    };

    if let Some(kb) = stack_kb {
        let stack = kb.saturating_mul(1024);
        eprintln!("PROBE stack_kb={kb}");
        let handle = std::thread::Builder::new()
            .name("width-probe".into())
            .stack_size(stack)
            .spawn(work)
            .expect("spawn probe thread");
        handle.join().expect("probe thread join");
    } else {
        work();
    }
}

/// Outer `arms` × inner marked concat of width `inner`, optionally wrapped `nest` times.
fn legacy_width_copy_expr(arms: usize, inner: usize, nest: usize) -> serde_json::Value {
    let base = ae("rows", json!({ "dataset": "issue_result_list" }));
    let first = ae(
        "first_by",
        json!({
            "rowset": base,
            "field": "处理结果ID-问题跟踪ID",
        }),
    );
    let mut marked_arms = Vec::with_capacity(inner);
    for i in 0..inner {
        let label = if i % 2 == 0 { "是" } else { "否" };
        // Alternate predicates so arms stay non-empty syntactically.
        let pred = if i % 2 == 0 {
            in_values("是否处分", &["是"])
        } else {
            in_values("是否处分", &["否"])
        };
        marked_arms.push(ae(
            "mutate",
            json!({
                "rowset": ae("where", json!({
                    "rowset": first.clone(),
                    "predicate": pred,
                })),
                "updates": { "处分标记": ae("lit", json!({ "value": label })) }
            }),
        ));
    }
    let mut core = ae("concat_rowsets", json!({ "rowsets": marked_arms }));
    for _ in 1..nest {
        // Identity-ish wrap: single-arm concat still forces another lower_rel layer,
        // and multi-arm wrap duplicates the previous tree.
        core = ae(
            "concat_rowsets",
            json!({
                "rowsets": [
                    ae("mutate", json!({
                        "rowset": core.clone(),
                        "updates": { "nest_tag": ae("lit", json!({ "value": "x" })) }
                    })),
                    ae("mutate", json!({
                        "rowset": core.clone(),
                        "updates": { "nest_tag": ae("lit", json!({ "value": "y" })) }
                    })),
                ]
            }),
        );
    }
    let mut outer = Vec::with_capacity(arms);
    for i in 0..arms {
        let label = format!("cat_{i}");
        // Cycle a few cheap predicates so every arm is lowerable.
        let pred = match i % 4 {
            0 => in_values("是否转问题线索", &["是"]),
            1 => in_values("是否立案", &["是"]),
            2 => gt("挽回资金", 0.0),
            _ => not_empty("健全机制"),
        };
        outer.push(mutate_category(core.clone(), pred, &label));
    }
    ae("concat_rowsets", json!({ "rowsets": outer }))
}
