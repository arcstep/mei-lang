use super::row_ops::eval_row_value;
use super::{
    aggregate_group_rows_pivot, party_year_aggregate_rows, trend_rows_by_month,
    trend_year_compare_rows, unpivot_columns_rows,
};
use serde_json::json;

#[test]
fn trend_year_compare_aligns_months_across_years() {
    let rows = vec![
        json!({"检查日期": "2024-03-10"}),
        json!({"检查日期": "2024-03-12"}),
        json!({"检查日期": "2025-03-15"}),
        json!({"检查日期": "2025-06-01"}),
    ];
    let trend = trend_year_compare_rows(
        &rows,
        "检查日期",
        None,
        "count",
        6,
        &[2024, 2025],
        "month",
        "year",
    );
    let march_2024 = trend
        .iter()
        .find(|row| {
            row.get("month").and_then(|v| v.as_str()) == Some("03")
                && row.get("year").and_then(|v| v.as_str()) == Some("2024")
        })
        .and_then(|row| row.get("value").and_then(|v| v.as_f64()));
    let march_2025 = trend
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
fn trend_by_month_fills_missing_buckets_with_zero() {
    let rows = vec![
        json!({"做出处罚日期": "2024-05-10", "罚款金额": 100}),
        json!({"做出处罚日期": "2024-06-10", "罚款金额": 200}),
    ];
    let trend = trend_rows_by_month(&rows, "做出处罚日期", Some("罚款金额"), "sum", 6, "month");
    assert_eq!(trend.len(), 6);
    assert_eq!(
        trend[0].get("month").and_then(|v| v.as_str()),
        Some("2024-01")
    );
    assert_eq!(trend[0].get("value").and_then(|v| v.as_f64()), Some(0.0));
    assert_eq!(trend[4].get("value").and_then(|v| v.as_f64()), Some(100.0));
    assert_eq!(trend[5].get("value").and_then(|v| v.as_f64()), Some(200.0));
}

#[test]
fn aggregate_group_rows_pivot_builds_migration_wide_table() {
    let rows = vec![
        json!({"镇街/园区": "甲园", "年份": 2024, "类型": "迁入"}),
        json!({"镇街/园区": "甲园", "年份": 2024, "类型": "迁入"}),
        json!({"镇街/园区": "甲园", "年份": 2024, "类型": "迁出"}),
        json!({"镇街/园区": "乙园", "年份": 2025, "类型": "迁入"}),
        json!({"镇街/园区": "其他街道", "年份": 2025, "类型": "迁入"}),
    ];
    let stats = aggregate_group_rows_pivot(
        &rows,
        &["镇街/园区".to_string(), "年份".to_string()],
        "类型",
        &["迁入".to_string(), "迁出".to_string()],
        Some(&["甲园".to_string(), "乙园".to_string()]),
    );
    assert_eq!(stats.len(), 4);
    let a2024 = stats
        .iter()
        .find(|row| {
            row.get("镇街/园区").and_then(|v| v.as_str()) == Some("甲园")
                && row.get("年份").and_then(|v| v.as_i64()) == Some(2024)
        })
        .expect("甲园 2024");
    assert_eq!(a2024.get("迁入").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(a2024.get("迁出").and_then(|v| v.as_i64()), Some(1));
    let b2025 = stats
        .iter()
        .find(|row| {
            row.get("镇街/园区").and_then(|v| v.as_str()) == Some("乙园")
                && row.get("年份").and_then(|v| v.as_i64()) == Some(2025)
        })
        .expect("乙园 2025");
    assert_eq!(b2025.get("迁入").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(b2025.get("迁出").and_then(|v| v.as_i64()), Some(0));
}

#[test]
fn aggregate_group_rows_pivot_supports_generic_two_field_groups() {
    let rows = vec![
        json!({"年月": "2025-03-01", "镇街/园区": "甲园", "类型": "迁入"}),
        json!({"年月": "2025-03-01", "镇街/园区": "甲园", "类型": "迁入"}),
        json!({"年月": "2025-03-01", "镇街/园区": "甲园", "类型": "迁出"}),
        json!({"年月": "2025-06-01", "镇街/园区": "乙园", "类型": "迁入"}),
    ];
    let stats = aggregate_group_rows_pivot(
        &rows,
        &["年月".to_string(), "镇街/园区".to_string()],
        "类型",
        &["迁入".to_string(), "迁出".to_string()],
        None,
    );
    assert_eq!(stats.len(), 2);
    let a = stats
        .iter()
        .find(|row| {
            row.get("年月").and_then(|v| v.as_str()) == Some("2025-03-01")
                && row.get("镇街/园区").and_then(|v| v.as_str()) == Some("甲园")
        })
        .expect("甲园 2025-03");
    assert_eq!(a.get("迁入").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(a.get("迁出").and_then(|v| v.as_i64()), Some(1));
}

#[test]
fn party_year_aggregate_sums_by_execution_year_and_party() {
    let rows = vec![
        json!({"当事人": "甲公司", "执行日期": "2024-06-01", "罚款金额": 20000}),
        json!({"当事人": "甲公司", "执行日期": "2024-08-01", "罚款金额": 5000}),
        json!({"当事人": "甲公司", "执行日期": "2025-03-01", "罚款金额": 30000}),
        json!({"当事人": "乙公司", "执行日期": "2025-01-01", "罚款金额": 12000}),
    ];
    let stats = party_year_aggregate_rows(&rows, "当事人", "执行日期", "罚款金额", &[2024, 2025]);
    let a = stats
        .iter()
        .find(|row| row.get("当事人").and_then(|v| v.as_str()) == Some("甲公司"))
        .expect("甲公司");
    assert_eq!(
        a.get("罚没金额_2024").and_then(|v| v.as_f64()),
        Some(25000.0)
    );
    assert_eq!(a.get("处罚次数_2024").and_then(|v| v.as_f64()), Some(2.0));
    assert_eq!(
        a.get("罚没金额_2025").and_then(|v| v.as_f64()),
        Some(30000.0)
    );
    assert_eq!(a.get("同比降低额_2025").and_then(|v| v.as_f64()), Some(0.0));
}

#[test]
fn unpivot_columns_expands_year_metrics_for_chart() {
    let rows = vec![json!({
        "当事人": "甲公司",
        "罚没金额_2024": 25000,
        "罚没金额_2025": 30000,
    })];
    let bars = unpivot_columns_rows(
        &rows,
        "当事人",
        &[
            ("2024".to_string(), "罚没金额_2024".to_string()),
            ("2025".to_string(), "罚没金额_2025".to_string()),
        ],
        "year",
        "value",
    );
    assert_eq!(bars.len(), 2);
    assert_eq!(bars[0].get("year").and_then(|v| v.as_str()), Some("2024"));
    assert_eq!(bars[0].get("value").and_then(|v| v.as_f64()), Some(25000.0));
}

#[test]
fn extract_match_supports_regex_capture_on_row_text() {
    let expr = json!({
        "__kind": "analysis_expr",
        "type": "extract_match",
        "field": "基本情况",
        "pattern": "(\\d{4}年\\d{1,2}月)"
    });
    let row = serde_json::Map::from_iter([(
        String::from("基本情况"),
        json!("2025年1月，区文旅委对部分旅行社检查频次过高"),
    )]);
    assert_eq!(
        eval_row_value(&expr, &row).as_str(),
        Some("2025年1月")
    );
}

#[test]
fn extract_number_supports_regex_prefix_on_string_and_numeric_cells() {
    let expr = json!({
        "__kind": "analysis_expr",
        "type": "extract_number",
        "field": "序号",
        "pattern": "^\\s*(\\d+)"
    });
    let row_text = serde_json::Map::from_iter([(String::from("序号"), json!("1-2"))]);
    let row_number = serde_json::Map::from_iter([(String::from("序号"), json!(10))]);
    assert_eq!(eval_row_value(&expr, &row_text).as_f64(), Some(1.0));
    assert_eq!(eval_row_value(&expr, &row_number).as_f64(), Some(10.0));
}
