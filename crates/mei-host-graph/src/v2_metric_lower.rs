//! Lower v2 `__call` metric bundle IR into v1 runtime metric defs (analysis_expr / data_product).

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

const WARNING_LIST_DETAIL_FIELDS: &[&str] = &[
    "序号",
    "监督领域",
    "监督类别",
    "预警ID",
    "预警条数",
    "主责单位",
    "问题分类名称",
    "问题描述",
    "预警类型",
    "预警等级",
    "预警时间",
    "问题跟踪ID",
    "承办部门",
    "分办时间",
    "办结时间",
    "是否查实",
    "查实条数",
    "查实率",
    "是否转问题线索",
    "问题线索数量",
    "核查情况",
    "处理结果",
];

const ISSUE_RESULT_DETAIL_FIELDS: &[&str] = &[
    "序号",
    "处理结果ID",
    "监督领域名称",
    "监督类别",
    "问题线索编号",
    "是否立案",
    "姓名/单位",
    "工作单位",
    "职务",
    "职级",
    "政治面貌",
    "处理处分",
    "挽回资金",
    "健全机制",
    "预警ID",
    "主责单位",
    "问题分类名称",
    "问题描述",
    "预警类型",
    "预警等级",
    "预警时间",
    "问题跟踪ID",
    "承办部门",
    "分办时间",
    "办结时间",
    "是否查实",
    "是否转问题线索",
    "核查情况",
    "处理结果",
];

#[derive(Debug, Default, Clone)]
pub struct V2MetricLowerContext {
    dataset_rowsets: BTreeMap<String, Value>,
}

impl V2MetricLowerContext {
    pub fn from_bundle_datasets(datasets: &[Value]) -> Self {
        let mut dataset_rowsets = BTreeMap::new();
        for item in datasets {
            let Some(name) = v2_call_name(item) else {
                continue;
            };
            if name != "dataset" {
                continue;
            }
            let Some(args) = item.get("__args").and_then(Value::as_object) else {
                continue;
            };
            let Some(id) = args
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            dataset_rowsets.insert(id.to_string(), data_ref(id));
        }
        let ctx = Self { dataset_rowsets };
        let mut resolved = ctx.dataset_rowsets.clone();
        for item in datasets {
            let Some(name) = v2_call_name(item) else {
                continue;
            };
            if name != "dataset_view" {
                continue;
            }
            let Some(args) = item.get("__args").and_then(Value::as_object) else {
                continue;
            };
            let Some(id) = args
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let rowset = args
                .get("rowset")
                .map(|value| {
                    lower_rowset(
                        value,
                        &V2MetricLowerContext {
                            dataset_rowsets: resolved.clone(),
                        },
                    )
                })
                .or_else(|| {
                    args.get("from")
                        .and_then(Value::as_str)
                        .map(|from| resolve_data_ref(from, &resolved))
                });
            if let Some(rowset) = rowset {
                resolved.insert(id.to_string(), rowset);
            }
        }
        Self {
            dataset_rowsets: resolved,
        }
    }
}

pub fn lower_v2_runtime_metric_defs(
    raw: BTreeMap<String, Value>,
    ctx: &V2MetricLowerContext,
) -> BTreeMap<String, Value> {
    raw.into_iter()
        .filter_map(|(id, metric)| lower_v2_metric(&id, &metric, ctx).map(|lowered| (id, lowered)))
        .collect()
}

fn lower_v2_metric(id: &str, value: &Value, ctx: &V2MetricLowerContext) -> Option<Value> {
    if value.get("__call").is_some() {
        let name = v2_call_name(value)?;
        let args = value.get("__args")?;
        return lower_v2_metric_call(id, name.as_str(), args, ctx);
    }
    Some(value.clone())
}

fn lower_v2_metric_call(
    id: &str,
    name: &str,
    args: &Value,
    ctx: &V2MetricLowerContext,
) -> Option<Value> {
    match name {
        "metric_scalar" => Some(lower_metric_scalar(id, args, ctx)),
        "metric_dataframe" => Some(lower_metric_dataframe(id, args, ctx)),
        _ => None,
    }
}

fn lower_metric_scalar(id: &str, args: &Value, ctx: &V2MetricLowerContext) -> Value {
    let map = args.as_object().cloned().unwrap_or_default();
    let mut base_rowset = base_rowset_from_scalar_args(&map, ctx);
    if let Some(filters) = map.get("filters").and_then(Value::as_object) {
        base_rowset = apply_scalar_filters(base_rowset, filters);
    }
    let value_expr = if let Some(agg) = map.get("agg") {
        lower_agg_on_rowset(agg, base_rowset.clone(), ctx)
    } else {
        json!(null)
    };
    let mut out = Map::new();
    out.insert("key".to_string(), json!(id));
    out.insert("id".to_string(), json!(id));
    if let Some(label) = map.get("label") {
        out.insert("label".to_string(), label.clone());
    }
    if let Some(unit) = map.get("unit") {
        out.insert("unit".to_string(), unit.clone());
    }
    if let Some(note) = map.get("note") {
        out.insert("note".to_string(), note.clone());
    }
    if let Some(value_format) = map.get("value_format") {
        out.insert("value_format".to_string(), value_format.clone());
    }
    out.insert("shape".to_string(), json!("scalar_map"));
    out.insert("values".to_string(), json!({"value": value_expr}));
    out.insert(
        "schema".to_string(),
        json!([{"name": "value", "type": "number"}]),
    );
    if let Some(explain) = map.get("explain").and_then(Value::as_array) {
        out.insert("explain".to_string(), lower_explain_items(explain, ctx));
    }
    Value::Object(out)
}

fn lower_metric_dataframe(id: &str, args: &Value, ctx: &V2MetricLowerContext) -> Value {
    let map = args.as_object().cloned().unwrap_or_default();
    let value_expr = if let Some(pipeline) = map.get("pipeline").and_then(Value::as_array) {
        lower_pipeline(pipeline, ctx)
    } else {
        json!(null)
    };
    let mut out = Map::new();
    out.insert("key".to_string(), json!(id));
    out.insert("id".to_string(), json!(id));
    if let Some(label) = map.get("label") {
        out.insert("label".to_string(), label.clone());
    }
    out.insert("shape".to_string(), json!("dataframe"));
    out.insert("value".to_string(), value_expr);
    if let Some(schema) = map.get("schema") {
        out.insert("schema".to_string(), schema.clone());
    }
    if let Some(explain) = map.get("explain").and_then(Value::as_array) {
        out.insert("explain".to_string(), lower_explain_items(explain, ctx));
    }
    Value::Object(out)
}

fn base_rowset_from_scalar_args(map: &Map<String, Value>, ctx: &V2MetricLowerContext) -> Value {
    if let Some(rowset) = map.get("rowset") {
        lower_rowset(rowset, ctx)
    } else if let Some(dataset) = map.get("dataset").and_then(Value::as_str) {
        resolve_data_ref(dataset, &ctx.dataset_rowsets)
    } else {
        json!(null)
    }
}

fn apply_scalar_filters(rowset: Value, filters: &Map<String, Value>) -> Value {
    if filters.is_empty() {
        return rowset;
    }
    let predicates: Vec<Value> = filters
        .iter()
        .map(|(field, value)| aek("eq", &[("field", json!(field)), ("value", value.clone())]))
        .collect();
    let predicate = if predicates.len() == 1 {
        predicates
            .into_iter()
            .next()
            .expect("single scalar filter predicate")
    } else {
        aek("and", &[("predicates", json!(predicates))])
    };
    aek("where", &[("rowset", rowset), ("predicate", predicate)])
}

fn lower_pipeline(steps: &[Value], ctx: &V2MetricLowerContext) -> Value {
    let mut rowset = json!(null);
    for step in steps {
        rowset = lower_pipeline_step(&rowset, step, ctx);
    }
    rowset
}

fn lower_pipeline_step(input: &Value, step: &Value, ctx: &V2MetricLowerContext) -> Value {
    let Some(name) = v2_call_name(step) else {
        return input.clone();
    };
    let args = step.get("__args").cloned().unwrap_or(json!({}));
    match name.as_str() {
        "data_ref" => lower_rowset(step, ctx),
        "where" => aek(
            "where",
            &[
                ("rowset", input.clone()),
                ("predicate", lower_predicate(arg0(&args))),
            ],
        ),
        "first_by" => aek(
            "first_by",
            &[
                ("rowset", input.clone()),
                ("field", json!(arg0_string(&args).unwrap_or_default())),
            ],
        ),
        "select" => aek(
            "select",
            &[("rowset", input.clone()), ("fields", arg0(&args).clone())],
        ),
        "sort_by" => aek(
            "sort_by",
            &[
                ("rowset", input.clone()),
                ("field", json!(arg0_string(&args).unwrap_or_default())),
                (
                    "order",
                    json!(args.get("order").and_then(Value::as_str).unwrap_or("asc")),
                ),
            ],
        ),
        "rename" => aek(
            "rename",
            &[("rowset", input.clone()), ("mapping", arg0(&args).clone())],
        ),
        "mutate" => {
            // Pipeline form: mutate({updates}); two-arg form keeps updates in arg1.
            let updates = if args.get("arg1").is_some() {
                arg1(&args)
            } else {
                arg0(&args)
            };
            aek(
                "mutate",
                &[
                    ("rowset", input.clone()),
                    ("updates", lower_mutate_updates(updates)),
                ],
            )
        }
        "limit" => aek(
            "limit",
            &[("rowset", input.clone()), ("n", arg0(&args).clone())],
        ),
        "label_status_pending" => lower_label_status_pending(input, &args),
        "concat_rowsets" => lower_concat_rowsets(&args, ctx),
        "group_by" => lower_group_by(Some(input), &args, ctx),
        "lookup_value" => lower_lookup_value(Some(input), &args, ctx),
        "party_year_aggregate" => lower_party_year_aggregate(Some(input), &args, ctx),
        "trend_year_compare" => lower_trend_year_compare(&args, ctx, Some(input)),
        "unpivot_columns" => lower_unpivot_columns(Some(input), &args, ctx),
        _ => input.clone(),
    }
}

fn lower_concat_rowsets(args: &Value, ctx: &V2MetricLowerContext) -> Value {
    let mut rowsets = Vec::new();
    if let Some(arr) = args.get("rowsets").and_then(Value::as_array) {
        for expr in arr {
            rowsets.push(lower_rowset(expr, ctx));
        }
    } else {
        for index in 0..32 {
            let key = format!("arg{index}");
            let Some(expr) = args.get(&key) else {
                break;
            };
            rowsets.push(lower_rowset(expr, ctx));
        }
    }
    aek("concat_rowsets", &[("rowsets", json!(rowsets))])
}

fn lower_label_status_pending(input: &Value, args: &Value) -> Value {
    let in_progress = args
        .get("in_progress")
        .and_then(Value::as_str)
        .unwrap_or("在办");
    let default = args
        .get("default")
        .and_then(Value::as_str)
        .unwrap_or("待办");
    let completed = args
        .get("completed")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let field = args
        .get("field")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("当前状态")
        .to_string();
    let status_update = |label: &str| {
        let mut updates = Map::new();
        updates.insert(
            field.clone(),
            aek("lit", &[("value", json!(label))]),
        );
        Value::Object(updates)
    };
    // 办理三态（不含「问题跟踪ID」门槛）：
    // - 待办：承办部门为空（含 — 等占位）
    // - 在办：承办部门有值且办结时间为空
    // - 办结：办结时间有值
    // 互斥切分：办结优先，其次在办，其余待办。
    let pending = aek(
        "mutate",
        &[
            (
                "rowset",
                aek(
                    "where",
                    &[
                        ("rowset", input.clone()),
                        (
                            "predicate",
                            aek(
                                "and",
                                &[(
                                    "predicates",
                                    json!([
                                        aek("blank", &[("field", json!("承办部门"))]),
                                        aek("blank", &[("field", json!("办结时间"))]),
                                    ]),
                                )],
                            ),
                        ),
                    ],
                ),
            ),
            ("updates", status_update(default)),
        ],
    );
    let in_progress_rows = aek(
        "mutate",
        &[
            (
                "rowset",
                aek(
                    "where",
                    &[
                        ("rowset", input.clone()),
                        (
                            "predicate",
                            aek(
                                "and",
                                &[(
                                    "predicates",
                                    json!([
                                        aek("present", &[("field", json!("承办部门"))]),
                                        aek("blank", &[("field", json!("办结时间"))]),
                                    ]),
                                )],
                            ),
                        ),
                    ],
                ),
            ),
            ("updates", status_update(in_progress)),
        ],
    );
    if let Some(completed_label) = completed {
        let completed_rows = aek(
            "mutate",
            &[
                (
                    "rowset",
                    aek(
                        "where",
                        &[
                            ("rowset", input.clone()),
                            (
                                "predicate",
                                aek("present", &[("field", json!("办结时间"))]),
                            ),
                        ],
                    ),
                ),
                ("updates", status_update(completed_label)),
            ],
        );
        return aek(
            "concat_rowsets",
            &[("rowsets", json!([pending, in_progress_rows, completed_rows]))],
        );
    }
    let other = aek(
        "mutate",
        &[
            (
                "rowset",
                aek(
                    "where",
                    &[
                        ("rowset", input.clone()),
                        (
                            "predicate",
                            aek(
                                "not",
                                &[(
                                    "predicate",
                                    aek(
                                        "or",
                                        &[(
                                            "predicates",
                                            json!([
                                                aek(
                                                    "and",
                                                    &[(
                                                        "predicates",
                                                        json!([
                                                            aek(
                                                                "blank",
                                                                &[("field", json!("承办部门"))]
                                                            ),
                                                            aek(
                                                                "blank",
                                                                &[("field", json!("办结时间"))]
                                                            ),
                                                        ]),
                                                    )],
                                                ),
                                                aek(
                                                    "and",
                                                    &[(
                                                        "predicates",
                                                        json!([
                                                            aek(
                                                                "present",
                                                                &[("field", json!("承办部门"))]
                                                            ),
                                                            aek(
                                                                "blank",
                                                                &[("field", json!("办结时间"))]
                                                            ),
                                                        ]),
                                                    )],
                                                ),
                                            ]),
                                        )],
                                    ),
                                )],
                            ),
                        ),
                    ],
                ),
            ),
            ("updates", status_update(default)),
        ],
    );
    aek(
        "concat_rowsets",
        &[("rowsets", json!([pending, in_progress_rows, other]))],
    )
}

fn lower_agg_on_rowset(agg: &Value, base_rowset: Value, ctx: &V2MetricLowerContext) -> Value {
    if let Some(name) = v2_call_name(agg) {
        return match name.as_str() {
            "count" => ae("count", vec![("rowset".to_string(), base_rowset)]),
            "sum" => lower_sum_agg(agg, base_rowset),
            "max" | "min" | "avg" | "median" => lower_field_agg(name.as_str(), agg, base_rowset),
            "ratio" => {
                let num = agg
                    .get("__args")
                    .and_then(|a| a.get("arg0"))
                    .map(|v| lower_nested_agg(v, base_rowset.clone(), ctx))
                    .unwrap_or(json!(0));
                let den = agg
                    .get("__args")
                    .and_then(|a| a.get("arg1"))
                    .map(|v| lower_nested_agg(v, base_rowset.clone(), ctx))
                    .unwrap_or(json!(0));
                aek("ratio", &[("numerator", num), ("denominator", den)])
            }
            "change_rate" => {
                let args = agg.get("__args").and_then(Value::as_object);
                let current = args
                    .and_then(|m| m.get("current"))
                    .map(|v| lower_scalar_expr(v, ctx))
                    .unwrap_or(json!(0));
                let base = args
                    .and_then(|m| m.get("base"))
                    .map(|v| lower_scalar_expr(v, ctx))
                    .unwrap_or(json!(0));
                let mode = args
                    .and_then(|m| m.get("mode"))
                    .and_then(Value::as_str)
                    .unwrap_or("growth");
                aek(
                    "change_rate",
                    &[("current", current), ("base", base), ("mode", json!(mode))],
                )
            }
            "transfer_clue_count" => expand_transfer_clue_count(agg, ctx),
            "mechanism_item_count" => expand_mechanism_item_count(agg, ctx),
            other => expand_known_agg_macro(other, agg, base_rowset.clone())
                .unwrap_or_else(|| ae("count", vec![("rowset".to_string(), base_rowset)])),
        };
    }
    json!(null)
}

fn lower_field_agg(name: &str, agg: &Value, base_rowset: Value) -> Value {
    let args = agg.get("__args").and_then(Value::as_object);
    let field = args
        .and_then(|m| m.get("field").or_else(|| m.get("arg0")))
        .and_then(Value::as_str)
        .unwrap_or("value")
        .to_string();
    ae(
        name,
        vec![(
            "value".to_string(),
            ae(
                "number",
                vec![
                    ("source".to_string(), base_rowset),
                    ("field".to_string(), json!(field)),
                ],
            ),
        )],
    )
}

fn lower_sum_agg(agg: &Value, base_rowset: Value) -> Value {
    let args = agg.get("__args").and_then(Value::as_object);
    let field = args
        .and_then(|m| m.get("field").or_else(|| m.get("arg0")))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let group_by = args.and_then(|m| m.get("group_by")).and_then(Value::as_str);
    let rowset = if let Some(group_field) = group_by {
        ae(
            "first_by",
            vec![
                ("rowset".to_string(), base_rowset),
                ("field".to_string(), json!(group_field)),
            ],
        )
    } else {
        base_rowset
    };
    ae(
        "sum",
        vec![(
            "value".to_string(),
            ae(
                "number",
                vec![
                    ("source".to_string(), rowset),
                    ("field".to_string(), json!(field)),
                ],
            ),
        )],
    )
}

fn lower_sum_on_rowset(agg: &Value, base_rowset: Value) -> Value {
    if v2_call_name(agg).as_deref() == Some("sum") {
        let field = agg
            .get("__args")
            .and_then(|args| {
                args.get("field")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| arg0_string_from_value(agg))
            })
            .or_else(|| arg0_string_from_value(agg));
        if let Some(field) = field {
            return ae(
                "sum",
                vec![(
                    "value".to_string(),
                    ae(
                        "number",
                        vec![
                            ("source".to_string(), base_rowset),
                            ("field".to_string(), json!(field)),
                        ],
                    ),
                )],
            );
        }
    }
    if v2_call_name(agg).as_deref() == Some("count") {
        return ae("count", vec![("rowset".to_string(), base_rowset)]);
    }
    json!(0)
}

fn lower_nested_agg(agg: &Value, base_rowset: Value, ctx: &V2MetricLowerContext) -> Value {
    if let Some(name) = v2_call_name(agg) {
        let args = agg.get("__args").cloned().unwrap_or(json!({}));
        return match name.as_str() {
            "count" => {
                let inner = arg0(&args);
                if let Some(inner_name) = v2_call_name(inner) {
                    // Authoring sugar: count(where(pred)) binds metric base_rowset.
                    // Two-arg where(rowset, pred) continues to lower via lower_rowset.
                    if inner_name == "where" {
                        if let Some(rowset) =
                            lower_predicate_only_where(inner, base_rowset.clone(), ctx)
                        {
                            return ae("count", vec![("rowset".to_string(), rowset)]);
                        }
                    }
                    ae(
                        "count",
                        vec![("rowset".to_string(), lower_rowset(inner, ctx))],
                    )
                } else {
                    ae("count", vec![("rowset".to_string(), base_rowset)])
                }
            }
            "sum" => lower_sum_on_rowset(agg, base_rowset),
            _ => lower_scalar_expr(agg, ctx),
        };
    }
    lower_sum_on_rowset(agg, base_rowset)
}

/// `where(pred)` inside nested agg — arg0 is predicate, rowset inherited from metric.
fn lower_predicate_only_where(
    where_call: &Value,
    base_rowset: Value,
    _ctx: &V2MetricLowerContext,
) -> Option<Value> {
    let args = where_call.get("__args")?;
    let a0 = arg0(args);
    let a1 = arg1(args);
    if !a1.is_null() {
        return None;
    }
    let pred_name = v2_call_name(a0)?;
    if is_rowset_call_name(pred_name.as_str()) {
        return None;
    }
    Some(aek(
        "where",
        &[("rowset", base_rowset), ("predicate", lower_predicate(a0))],
    ))
}

fn is_rowset_call_name(name: &str) -> bool {
    matches!(
        name,
        "data_ref"
            | "where"
            | "first_by"
            | "select"
            | "group_by"
            | "lookup_value"
            | "party_year_aggregate"
            | "trend_year_compare"
            | "pivot_long"
            | "unpivot_columns"
            | "mutate"
            | "concat_rowsets"
            | "party_gov_sanction_rows"
            | "handled_person_rows"
    )
}

fn lower_scalar_expr(value: &Value, ctx: &V2MetricLowerContext) -> Value {
    if let Some(name) = v2_call_name(value) {
        let args = value.get("__args").cloned().unwrap_or(json!({}));
        return match name.as_str() {
            "year_count" => ae(
                "count",
                vec![(
                    "rowset".to_string(),
                    year_between_rowset(
                        lower_rowset(arg0(&args), ctx),
                        arg1_string(&args),
                        arg2_from_args(&args),
                    ),
                )],
            ),
            "year_sum" => {
                let rowset = year_between_rowset(
                    lower_rowset(arg0(&args), ctx),
                    arg1_string(&args),
                    arg3_from_args(&args),
                );
                let value_field = args
                    .get("arg2")
                    .or_else(|| args.get("value_field"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                ae(
                    "sum",
                    vec![(
                        "value".to_string(),
                        ae(
                            "number",
                            vec![
                                ("source".to_string(), rowset),
                                ("field".to_string(), json!(value_field)),
                            ],
                        ),
                    )],
                )
            }
            "count" => lower_nested_agg(value, json!(null), ctx),
            "lit" => {
                let lit_value = args
                    .get("arg0")
                    .or_else(|| args.get("value"))
                    .cloned()
                    .unwrap_or(Value::Null);
                aek("lit", &[("value", lit_value)])
            }
            _ => json!(0),
        };
    }
    json!(0)
}

fn lower_mutate_updates(updates: &Value) -> Value {
    let Some(object) = updates.as_object() else {
        return updates.clone();
    };
    let mut out = serde_json::Map::new();
    for (key, value) in object {
        out.insert(key.clone(), lower_row_value_expr(value));
    }
    Value::Object(out)
}

fn lower_row_value_expr(value: &Value) -> Value {
    if let Some(name) = v2_call_name(value) {
        let args = value.get("__args").cloned().unwrap_or(json!({}));
        return match name.as_str() {
            "lit" => {
                let lit_value = args
                    .get("arg0")
                    .or_else(|| args.get("value"))
                    .cloned()
                    .unwrap_or(Value::Null);
                aek("lit", &[("value", lit_value)])
            }
            "col" | "number" | "text" => {
                let field = args
                    .get("field")
                    .or_else(|| args.get("arg0"))
                    .cloned()
                    .unwrap_or(json!(""));
                aek(name.as_str(), &[("field", field)])
            }
            "extract_match" | "extract_number" => {
                let field = args
                    .get("field")
                    .or_else(|| args.get("arg0"))
                    .cloned()
                    .unwrap_or(json!(""));
                let pattern = args
                    .get("pattern")
                    .or_else(|| args.get("arg1"))
                    .cloned()
                    .unwrap_or(json!(""));
                aek(name.as_str(), &[("field", field), ("pattern", pattern)])
            }
            "div" => {
                let field = args
                    .get("field")
                    .or_else(|| args.get("arg0"))
                    .cloned()
                    .unwrap_or(json!(""));
                let by = args
                    .get("by")
                    .or_else(|| args.get("arg1"))
                    .cloned()
                    .unwrap_or(json!(1));
                aek("div", &[("field", field), ("by", by)])
            }
            "coalesce" => {
                let fields = args
                    .get("fields")
                    .or_else(|| args.get("arg0"))
                    .cloned()
                    .unwrap_or(json!([]));
                aek("coalesce", &[("fields", fields)])
            }
            "sub" => {
                let left_field = args
                    .get("left_field")
                    .or_else(|| args.get("arg0"))
                    .cloned()
                    .unwrap_or(json!(""));
                let right_field = args
                    .get("right_field")
                    .or_else(|| args.get("arg1"))
                    .cloned()
                    .unwrap_or(json!(""));
                aek(
                    "sub",
                    &[("left_field", left_field), ("right_field", right_field)],
                )
            }
            _ => value.clone(),
        };
    }
    if value.is_object() {
        return value.clone();
    }
    aek("lit", &[("value", value.clone())])
}

fn arg2_from_args(args: &Value) -> Value {
    args.get("arg2")
        .or_else(|| args.get("year"))
        .cloned()
        .unwrap_or(json!(2025))
}

fn arg3_from_args(args: &Value) -> Value {
    args.get("arg3")
        .or_else(|| args.get("year"))
        .cloned()
        .unwrap_or(json!(2025))
}

fn year_between_rowset(rowset: Value, field: Option<String>, year: Value) -> Value {
    let field = field.unwrap_or_default();
    let year_text = year
        .as_i64()
        .or_else(|| year.as_u64().map(|v| v as i64))
        .or_else(|| year.as_str().and_then(|s| s.parse::<i64>().ok()))
        .unwrap_or(2025);
    let year_str = year_text.to_string();
    ae(
        "where",
        vec![
            ("rowset".to_string(), rowset),
            (
                "predicate".to_string(),
                ae(
                    "between",
                    vec![
                        ("field".to_string(), json!(field)),
                        ("lower".to_string(), json!(format!("{year_str}-01-01"))),
                        ("upper".to_string(), json!(format!("{year_str}-12-31"))),
                    ],
                ),
            ),
        ],
    )
}

fn lower_trend_year_compare(
    args: &Value,
    ctx: &V2MetricLowerContext,
    input: Option<&Value>,
) -> Value {
    let map = args.as_object().cloned().unwrap_or_default();
    let rowset = if let Some(inp) = input {
        inp.clone()
    } else {
        map.get("arg0")
            .map(|v| lower_rowset(v, ctx))
            .unwrap_or(json!(null))
    };
    let date_field = map
        .get("date_field")
        .or_else(|| map.get("arg1"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let value_field = map.get("value").cloned().unwrap_or(Value::Null);
    let agg = map.get("agg").and_then(Value::as_str).unwrap_or("count");
    let years = map.get("years").cloned().unwrap_or(json!([2024, 2025]));
    let limit = map.get("limit").cloned().unwrap_or(json!(6));
    let window = map
        .get("window")
        .and_then(Value::as_str)
        .unwrap_or("rolling");
    aek(
        "trend_year_compare",
        &[
            ("rowset", rowset),
            ("date_field", json!(date_field)),
            ("value", value_field),
            ("agg", json!(agg)),
            ("years", years),
            ("limit", limit),
            ("window", json!(window)),
        ],
    )
}

fn lower_pivot_long(args: &Value, ctx: &V2MetricLowerContext) -> Value {
    let map = args.as_object().cloned().unwrap_or_default();
    let rowset = map
        .get("arg0")
        .map(|v| lower_rowset(v, ctx))
        .unwrap_or(json!(null));
    let row_field = map
        .get("row_field")
        .and_then(Value::as_str)
        .unwrap_or("month");
    let column_field = map
        .get("column_field")
        .and_then(Value::as_str)
        .unwrap_or("year");
    let value_field = map
        .get("value_field")
        .and_then(Value::as_str)
        .unwrap_or("value");
    let columns = map.get("columns").cloned().unwrap_or(json!([]));
    aek(
        "pivot_long",
        &[
            ("rowset", rowset),
            ("row_field", json!(row_field)),
            ("column_field", json!(column_field)),
            ("value_field", json!(value_field)),
            ("columns", columns),
        ],
    )
}

fn expand_known_agg_macro(name: &str, agg: &Value, base_rowset: Value) -> Option<Value> {
    match name {
        "count_distinct" => {
            let prefix_field = agg
                .get("__args")
                .and_then(|a| a.get("prefix_field"))
                .and_then(Value::as_str)
                .unwrap_or("序号");
            let filtered = ae(
                "where",
                vec![
                    ("rowset".to_string(), base_rowset),
                    (
                        "predicate".to_string(),
                        ae(
                            "matches",
                            vec![
                                ("field".to_string(), json!(prefix_field)),
                                ("pattern".to_string(), json!("^\\s*\\d+(?:-.*)?\\s*$")),
                            ],
                        ),
                    ),
                ],
            );
            let mutated = ae(
                "mutate",
                vec![
                    ("rowset".to_string(), filtered.clone()),
                    (
                        "updates".to_string(),
                        json!({
                            "序号前缀": ae(
                                "extract_number",
                                vec![
                                    ("source".to_string(), filtered),
                                    ("field".to_string(), json!(prefix_field)),
                                    ("pattern".to_string(), json!("^\\s*(\\d+)")),
                                ],
                            )
                        }),
                    ),
                ],
            );
            Some(ae(
                "count",
                vec![(
                    "rowset".to_string(),
                    ae(
                        "first_by",
                        vec![
                            ("rowset".to_string(), mutated),
                            // Distinct by extracted numeric prefix, not the raw serial cell.
                            ("field".to_string(), json!("序号前缀")),
                        ],
                    ),
                )],
            ))
        }
        _ => None,
    }
}

fn expand_person_rowset(name: &str, value: &Value, ctx: &V2MetricLowerContext) -> Option<Value> {
    let rowset = value
        .get("__args")
        .and_then(|a| a.get("arg0"))
        .map(|value| lower_rowset(value, ctx))?;
    let filtered = if name == "party_gov_sanction_rows" {
        ae(
            "where",
            vec![
                ("rowset".to_string(), rowset),
                (
                    "predicate".to_string(),
                    ae(
                        "contains",
                        vec![
                            ("field".to_string(), json!("处理处分")),
                            ("value".to_string(), json!("第二种")),
                        ],
                    ),
                ),
            ],
        )
    } else {
        ae(
            "where",
            vec![
                ("rowset".to_string(), rowset),
                (
                    "predicate".to_string(),
                    ae(
                        "and",
                        vec![(
                            "predicates".to_string(),
                            json!([
                                ae("present", vec![("field".to_string(), json!("职务"))]),
                                ae(
                                    "not",
                                    vec![(
                                        "predicate".to_string(),
                                        ae(
                                            "placeholder_only",
                                            vec![("field".to_string(), json!("职务"))],
                                        ),
                                    )],
                                ),
                            ]),
                        )],
                    ),
                ),
            ],
        )
    };
    Some(ae(
        "first_by",
        vec![
            ("rowset".to_string(), filtered),
            ("field".to_string(), json!("处理结果ID")),
        ],
    ))
}

fn expand_transfer_clue_count(agg: &Value, ctx: &V2MetricLowerContext) -> Value {
    let rows = agg
        .get("__args")
        .and_then(|a| a.get("arg0"))
        .map(|value| lower_rowset(value, ctx))
        .unwrap_or_else(|| resolve_data_ref("warning_list", &ctx.dataset_rowsets));
    let transfer_rows = aek(
        "where",
        &[
            ("rowset", rows.clone()),
            (
                "predicate",
                aek(
                    "and",
                    &[(
                        "predicates",
                        json!([
                            aek("present", &[("field", json!("问题跟踪ID"))]),
                            aek(
                                "contains",
                                &[("field", json!("是否转问题线索")), ("value", json!("是"))]
                            ),
                        ]),
                    )],
                ),
            ),
        ],
    );
    let paren_rows = aek(
        "where",
        &[
            ("rowset", transfer_rows.clone()),
            (
                "predicate",
                aek(
                    "matches",
                    &[
                        ("field", json!("是否转问题线索")),
                        ("pattern", json!("[（(]\\s*\\d+\\s*[）)]")),
                    ],
                ),
            ),
        ],
    );
    aek(
        "sum_rowset_counts",
        &[("rowsets", json!([transfer_rows, paren_rows]))],
    )
}

fn expand_mechanism_item_count(agg: &Value, ctx: &V2MetricLowerContext) -> Value {
    let rows = agg
        .get("__args")
        .and_then(|a| a.get("arg0"))
        .map(|value| lower_rowset(value, ctx))
        .unwrap_or_else(|| resolve_data_ref("issue_result_list", &ctx.dataset_rowsets));
    let source_rows = aek(
        "where",
        &[
            ("rowset", rows),
            (
                "predicate",
                aek("not_empty", &[("field", json!("健全机制"))]),
            ),
        ],
    );
    let split_comma = aek(
        "split_text",
        &[
            ("rowset", source_rows),
            ("field", json!("健全机制")),
            ("delimiter", json!("、")),
        ],
    );
    let split_items = aek(
        "split_text",
        &[
            ("rowset", split_comma),
            ("field", json!("健全机制")),
            ("delimiter", json!("》《")),
        ],
    );
    aek("count", &[("rowset", split_items)])
}

fn is_rowset_expr(value: &Value) -> bool {
    v2_call_name(value).is_some() || value.get("__ref").is_some()
}

fn kw_or_arg(args: &Value, name: &str, index: usize) -> Option<Value> {
    args.get(name)
        .cloned()
        .or_else(|| args.get(&format!("arg{index}")).cloned())
}

fn lower_group_by(input: Option<&Value>, args: &Value, ctx: &V2MetricLowerContext) -> Value {
    let rowset = if let Some(inp) = input {
        inp.clone()
    } else if is_rowset_expr(arg0(args)) {
        lower_rowset(arg0(args), ctx)
    } else {
        json!(null)
    };
    let by = if input.is_some() || !is_rowset_expr(arg0(args)) {
        kw_or_arg(args, "by", 0)
    } else {
        kw_or_arg(args, "by", 1)
    };
    let mut pairs = vec![("rowset", rowset)];
    if let Some(by) = by {
        pairs.push(("by", by));
    }
    if let Some(fields) = args.get("fields") {
        pairs.push(("fields", fields.clone()));
    }
    if let Some(value) = args.get("value") {
        pairs.push(("value", value.clone()));
    }
    if let Some(agg) = args.get("agg") {
        pairs.push(("agg", agg.clone()));
    }
    if let Some(pivot_field) = args.get("pivot_field") {
        pairs.push(("pivot_field", pivot_field.clone()));
    }
    if let Some(pivot_columns) = args.get("pivot_columns") {
        pairs.push(("pivot_columns", pivot_columns.clone()));
    }
    if let Some(universe) = args.get("universe") {
        pairs.push(("universe", lower_rowset(universe, ctx)));
    }
    aek("group_by", &pairs)
}

fn lower_lookup_value(input: Option<&Value>, args: &Value, ctx: &V2MetricLowerContext) -> Value {
    let base_idx = if input.is_some() {
        0usize
    } else if is_rowset_expr(arg0(args)) {
        1usize
    } else {
        0usize
    };
    let rowset = if let Some(inp) = input {
        inp.clone()
    } else if is_rowset_expr(arg0(args)) {
        lower_rowset(arg0(args), ctx)
    } else {
        json!(null)
    };
    let field = kw_or_arg(args, "field", base_idx).unwrap_or(json!(""));
    let lookup_rowset = kw_or_arg(args, "lookup_rowset", base_idx + 1)
        .map(|value| lower_rowset(&value, ctx))
        .unwrap_or(json!(null));
    let lookup_field = kw_or_arg(args, "lookup_field", base_idx + 2).unwrap_or(json!(""));
    let value_field = kw_or_arg(args, "value_field", base_idx + 3).unwrap_or(json!(""));
    let as_field = args
        .get("as_field")
        .cloned()
        .or_else(|| kw_or_arg(args, "as_field", base_idx + 4))
        .unwrap_or_else(|| value_field.clone());
    aek(
        "lookup_value",
        &[
            ("rowset", rowset),
            ("field", field),
            ("lookup_rowset", lookup_rowset),
            ("lookup_field", lookup_field),
            ("value_field", value_field),
            ("as_field", as_field),
        ],
    )
}

fn lower_party_year_aggregate(
    input: Option<&Value>,
    args: &Value,
    ctx: &V2MetricLowerContext,
) -> Value {
    let rowset = if let Some(inp) = input {
        inp.clone()
    } else {
        kw_or_arg(args, "rowset", 0)
            .map(|value| lower_rowset(&value, ctx))
            .unwrap_or(json!(null))
    };
    let party_field = kw_or_arg(args, "party_field", 0).unwrap_or(json!(""));
    let date_field = kw_or_arg(args, "date_field", 1).unwrap_or(json!(""));
    let value_field = kw_or_arg(args, "value_field", 2).unwrap_or(json!(""));
    let years = args.get("years").cloned().unwrap_or(json!([]));
    aek(
        "party_year_aggregate",
        &[
            ("rowset", rowset),
            ("party_field", party_field),
            ("date_field", date_field),
            ("value_field", value_field),
            ("years", years),
        ],
    )
}

fn lower_unpivot_columns(input: Option<&Value>, args: &Value, ctx: &V2MetricLowerContext) -> Value {
    let rowset = if let Some(inp) = input {
        inp.clone()
    } else {
        kw_or_arg(args, "rowset", 0)
            .map(|value| lower_rowset(&value, ctx))
            .unwrap_or(json!(null))
    };
    let id_field = kw_or_arg(args, "id_field", 0).unwrap_or(json!(""));
    let columns = args.get("columns").cloned().unwrap_or(json!([]));
    let year_field = kw_or_arg(args, "year_field", 1).unwrap_or(json!("year"));
    let value_field = kw_or_arg(args, "value_field", 2).unwrap_or(json!("value"));
    aek(
        "unpivot_columns",
        &[
            ("rowset", rowset),
            ("id_field", id_field),
            ("columns", columns),
            ("year_field", year_field),
            ("value_field", value_field),
        ],
    )
}

fn lower_rowset(value: &Value, ctx: &V2MetricLowerContext) -> Value {
    if let Some(name) = v2_call_name(value) {
        let args = value.get("__args").cloned().unwrap_or(json!({}));
        return match name.as_str() {
            "data_ref" => resolve_data_ref(
                arg0_string(&args).unwrap_or_default().as_str(),
                &ctx.dataset_rowsets,
            ),
            "where" => aek(
                "where",
                &[
                    ("rowset", lower_rowset(arg0(&args), ctx)),
                    ("predicate", lower_predicate(arg1(&args))),
                ],
            ),
            "latest_days" => aek(
                "latest_days",
                &[
                    ("rowset", lower_rowset(arg0(&args), ctx)),
                    (
                        "field",
                        json!(kw_or_arg(&args, "field", 1)
                            .and_then(|v| v.as_str().map(str::to_string))
                            .unwrap_or_default()),
                    ),
                    (
                        "days",
                        kw_or_arg(&args, "days", 2).unwrap_or(json!(7)),
                    ),
                ],
            ),
            "latest_months" => aek(
                "latest_months",
                &[
                    ("rowset", lower_rowset(arg0(&args), ctx)),
                    (
                        "field",
                        json!(kw_or_arg(&args, "field", 1)
                            .and_then(|v| v.as_str().map(str::to_string))
                            .unwrap_or_default()),
                    ),
                    (
                        "months",
                        kw_or_arg(&args, "months", 2).unwrap_or(json!(6)),
                    ),
                ],
            ),
            "first_by" => aek(
                "first_by",
                &[
                    ("rowset", lower_rowset(arg0(&args), ctx)),
                    ("field", json!(arg1_string(&args).unwrap_or_default())),
                ],
            ),
            "select" => aek(
                "select",
                &[
                    ("rowset", lower_rowset(arg0(&args), ctx)),
                    (
                        "fields",
                        args.get("fields")
                            .cloned()
                            .or_else(|| kw_or_arg(&args, "fields", 1))
                            .unwrap_or(json!([])),
                    ),
                ],
            ),
            "group_by" => lower_group_by(None, &args, ctx),
            "lookup_value" => lower_lookup_value(None, &args, ctx),
            "party_year_aggregate" => lower_party_year_aggregate(None, &args, ctx),
            "trend_year_compare" => lower_trend_year_compare(&args, ctx, None),
            "pivot_long" => lower_pivot_long(&args, ctx),
            "unpivot_columns" => lower_unpivot_columns(None, &args, ctx),
            "mutate" => {
                let rowset = lower_rowset(arg0(&args), ctx);
                let updates = arg1(&args);
                aek(
                    "mutate",
                    &[
                        ("rowset", rowset),
                        ("updates", lower_mutate_updates(updates)),
                    ],
                )
            }
            "label_status_pending" => {
                // Expression form: label_status_pending(rowset, …kwargs).
                // Pipeline form is handled in lower_pipeline_step.
                let input = lower_rowset(arg0(&args), ctx);
                lower_label_status_pending(&input, &args)
            }
            "concat_rowsets" => lower_concat_rowsets(&args, ctx),
            "party_gov_sanction_rows" | "handled_person_rows" => {
                expand_person_rowset(name.as_str(), value, ctx).unwrap_or(json!(null))
            }
            _ => json!(null),
        };
    }
    value.clone()
}

fn resolve_data_ref(dataset_id: &str, dataset_rowsets: &BTreeMap<String, Value>) -> Value {
    let dataset_id = dataset_id.trim();
    if let Some(rowset) = dataset_rowsets.get(dataset_id) {
        return rowset.clone();
    }
    data_ref(dataset_id)
}

fn lower_predicate(value: &Value) -> Value {
    if let Some(name) = v2_call_name(value) {
        let args = value.get("__args").cloned().unwrap_or(json!({}));
        return match name.as_str() {
            "and_" | "and" => aek(
                "and",
                &[(
                    "predicates",
                    json!(positional_args(&args)
                        .iter()
                        .map(lower_predicate)
                        .collect::<Vec<_>>()),
                )],
            ),
            "or_" | "or" => aek(
                "or",
                &[(
                    "predicates",
                    json!(positional_args(&args)
                        .iter()
                        .map(lower_predicate)
                        .collect::<Vec<_>>()),
                )],
            ),
            "not_" | "not" => aek("not", &[("predicate", lower_predicate(arg0(&args)))]),
            "not_empty" => aek(
                "not_empty",
                &[("field", json!(arg0_string(&args).unwrap_or_default()))],
            ),
            "present" => aek(
                "present",
                &[("field", json!(arg0_string(&args).unwrap_or_default()))],
            ),
            "blank" => aek(
                "blank",
                &[("field", json!(arg0_string(&args).unwrap_or_default()))],
            ),
            "contains" => aek(
                "contains",
                &[
                    ("field", json!(arg0_string(&args).unwrap_or_default())),
                    (
                        "value",
                        args.get("value")
                            .or_else(|| args.get("arg1"))
                            .cloned()
                            .unwrap_or(json!("")),
                    ),
                ],
            ),
            "matches" => aek(
                "matches",
                &[
                    ("field", json!(arg0_string(&args).unwrap_or_default())),
                    (
                        "pattern",
                        json!(args
                            .get("pattern")
                            .or_else(|| args.get("arg1"))
                            .and_then(Value::as_str)
                            .unwrap_or("")),
                    ),
                ],
            ),
            "in_values" => aek(
                "in_values",
                &[
                    ("field", json!(arg0_string(&args).unwrap_or_default())),
                    ("values", arg1(&args).clone()),
                ],
            ),
            "eq" => aek(
                "eq",
                &[
                    ("field", json!(arg0_string(&args).unwrap_or_default())),
                    ("value", arg1(&args).clone()),
                ],
            ),
            "gt" => aek(
                "gt",
                &[
                    ("field", json!(arg0_string(&args).unwrap_or_default())),
                    ("value", arg1(&args).clone()),
                ],
            ),
            "field_gt" => aek(
                "field_gt",
                &[
                    ("left_field", json!(arg0_string(&args).unwrap_or_default())),
                    ("right_field", json!(arg1_string(&args).unwrap_or_default())),
                ],
            ),
            "is_yes" => aek(
                "and",
                &[(
                    "predicates",
                    json!([
                        aek(
                            "not_empty",
                            &[("field", json!(arg0_string(&args).unwrap_or_default()))]
                        ),
                        aek(
                            "contains",
                            &[
                                ("field", json!(arg0_string(&args).unwrap_or_default())),
                                ("value", json!("是")),
                            ],
                        ),
                    ]),
                )],
            ),
            _ => json!(null),
        };
    }
    value.clone()
}

fn lower_explain_items(items: &[Value], ctx: &V2MetricLowerContext) -> Value {
    Value::Array(
        items
            .iter()
            .filter_map(|item| lower_explain_item(item, ctx))
            .collect(),
    )
}

fn lower_explain_item(value: &Value, ctx: &V2MetricLowerContext) -> Option<Value> {
    let name = v2_call_name(value)?;
    let args = value.get("__args")?.as_object()?;
    if name == "dataframe" {
        let id = args
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("dataframe");
        let mut product = Map::new();
        product.insert("__kind".to_string(), json!("data_product"));
        product.insert("shape".to_string(), json!("dataframe"));
        product.insert("id".to_string(), json!(id));
        if let Some(label) = args.get("label") {
            product.insert("label".to_string(), label.clone());
        }
        if let Some(value_expr) = args.get("value") {
            product.insert("value".to_string(), lower_rowset(value_expr, ctx));
        }
        return Some(Value::Object(product));
    }
    let kind = match name.as_str() {
        "detail" => "detail",
        "composition" => "composition",
        "ratio" => "ratio",
        _ => return None,
    };
    let mut out = Map::new();
    out.insert("__kind".to_string(), json!("explain_item"));
    out.insert("kind".to_string(), json!(kind));
    if let Some(id) = args.get("id").and_then(Value::as_str) {
        out.insert("id".to_string(), json!(id));
    } else {
        out.insert("id".to_string(), json!(kind));
    }
    if let Some(label) = args.get("label").and_then(Value::as_str) {
        out.insert("label".to_string(), json!(label));
    }
    if let Some(by) = args.get("by").and_then(Value::as_str) {
        out.insert("by".to_string(), json!(by));
    }
    if let Some(top_n) = args.get("top_n") {
        out.insert("top_n".to_string(), top_n.clone());
    }
    if let Some(value_field) = args.get("value_field").and_then(Value::as_str) {
        out.insert("value_field".to_string(), json!(value_field));
    }
    if let Some(agg) = args.get("agg").and_then(Value::as_str) {
        out.insert("agg".to_string(), json!(agg));
    }
    if let Some(delimiter) = args.get("delimiter").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
        out.insert("delimiter".to_string(), json!(delimiter));
    }
    if let Some(fields) = args.get("fields") {
        out.insert("fields".to_string(), lower_field_list(fields));
    }
    if let Some(numerator) = args.get("numerator").and_then(Value::as_str) {
        out.insert("numerator".to_string(), json!(numerator));
    }
    if let Some(denominator) = args.get("denominator").and_then(Value::as_str) {
        out.insert("denominator".to_string(), json!(denominator));
    }
    Some(Value::Object(out))
}

fn lower_field_list(value: &Value) -> Value {
    if let Some(name) = v2_call_name(value) {
        return match name.as_str() {
            "warning_list_detail_fields" => string_array(WARNING_LIST_DETAIL_FIELDS),
            "issue_result_detail_fields" => string_array(ISSUE_RESULT_DETAIL_FIELDS),
            _ => json!([]),
        };
    }
    value.clone()
}

fn string_array(values: &[&str]) -> Value {
    Value::Array(values.iter().map(|v| json!(v)).collect())
}

fn data_ref(dataset_id: &str) -> Value {
    json!({"__ref": "data", "from_dataset": dataset_id, "id": dataset_id})
}

fn ae(type_name: &str, fields: Vec<(String, Value)>) -> Value {
    let mut map = Map::new();
    map.insert("__kind".to_string(), json!("analysis_expr"));
    map.insert("type".to_string(), json!(type_name));
    for (key, value) in fields {
        map.insert(key, value);
    }
    Value::Object(map)
}

fn aek(type_name: &str, fields: &[(&str, Value)]) -> Value {
    ae(
        type_name,
        fields
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect(),
    )
}

fn v2_call_name(value: &Value) -> Option<String> {
    value
        .get("__call")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn arg0(args: &Value) -> &Value {
    args.get("arg0").unwrap_or(&Value::Null)
}

fn arg1(args: &Value) -> &Value {
    args.get("arg1").unwrap_or(&Value::Null)
}

fn arg0_string(args: &Value) -> Option<String> {
    arg0(args).as_str().map(str::to_string)
}

fn arg1_string(args: &Value) -> Option<String> {
    arg1(args).as_str().map(str::to_string)
}

fn arg0_string_from_value(value: &Value) -> Option<String> {
    value
        .get("__args")
        .and_then(|args| arg0_string(args))
        .or_else(|| value.as_str().map(str::to_string))
}

fn positional_args(args: &Value) -> Vec<Value> {
    let Some(map) = args.as_object() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut index = 0usize;
    loop {
        let key = format!("arg{index}");
        let Some(value) = map.get(&key) else {
            break;
        };
        out.push(value.clone());
        index += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lower_metric_scalar_max_with_filters() {
        let raw = json!({
            "__call": "metric_scalar",
            "__args": {
                "id": "enforcement_units_count",
                "label": "执法单位",
                "unit": "个",
                "dataset": "static_metrics",
                "agg": {"__call": "max", "__args": {"field": "value"}},
                "filters": {"metric_id": "enforcement_units_count"}
            }
        });
        let ctx = V2MetricLowerContext::default();
        let lowered = lower_v2_metric("enforcement_units_count", &raw, &ctx).expect("lower");
        assert_eq!(
            lowered
                .pointer("/values/value/type")
                .and_then(|v| v.as_str()),
            Some("max"),
            "max(field=value) must not fall back to count, got {lowered}"
        );
        assert_eq!(
            lowered
                .pointer("/values/value/value/type")
                .and_then(|v| v.as_str()),
            Some("number")
        );
        assert_eq!(
            lowered
                .pointer("/values/value/value/source/type")
                .and_then(|v| v.as_str()),
            Some("where"),
            "filters must lower to where(...), got {lowered}"
        );
        assert_eq!(
            lowered
                .pointer("/values/value/value/source/predicate/type")
                .and_then(|v| v.as_str()),
            Some("eq")
        );
        assert_eq!(
            lowered
                .pointer("/values/value/value/source/predicate/field")
                .and_then(|v| v.as_str()),
            Some("metric_id")
        );
    }

    #[test]
    fn lower_realtime_warning_detail_has_count_rowset() {
        let raw = json!({
            "__call": "metric_scalar",
            "__args": {
                "id": "realtime_warning_detail",
                "label": "实时预警详情",
                "unit": "件",
                "dataset": "warning_list",
                "agg": {"__call": "count", "__args": {}}
            }
        });
        let ctx = V2MetricLowerContext::default();
        let lowered = lower_v2_metric("realtime_warning_detail", &raw, &ctx).expect("lower");
        assert_eq!(
            lowered.get("shape").and_then(|v| v.as_str()),
            Some("scalar_map")
        );
        assert!(
            lowered
                .pointer("/values/value/type")
                .and_then(|v| v.as_str())
                == Some("count")
        );
    }

    #[test]
    fn lower_issue_verification_rate_inlines_warning_detail_view() {
        let bundle_datasets = json!([
            {
                "__call": "dataset",
                "__args": {"id": "warning_list", "source": {"__ref": "source_ref", "__args": {"arg0": "alert_tracking"}}}
            },
            {
                "__call": "dataset_view",
                "__args": {
                    "id": "warning_detail",
                    "from": "warning_list",
                    "rowset": {
                        "__call": "where",
                        "__args": {
                            "arg0": {
                                "__call": "first_by",
                                "__args": {
                                    "arg0": {"__call": "data_ref", "__args": {"arg0": "warning_list"}},
                                    "arg1": "预警ID"
                                }
                            },
                            "arg1": {
                                "__call": "in_values",
                                "__args": {"arg0": "是否查实", "values": ["是", "否"]}
                            }
                        }
                    }
                }
            }
        ]);
        let ctx =
            V2MetricLowerContext::from_bundle_datasets(bundle_datasets.as_array().expect("array"));
        let raw = json!({
            "__call": "metric_scalar",
            "__args": {
                "id": "effectiveness_issue_verification_rate",
                "rowset": {"__call": "data_ref", "__args": {"arg0": "warning_detail"}},
                "agg": {
                    "__call": "ratio",
                    "__args": {
                        "arg0": {"__call": "sum", "__args": {"arg0": "查实条数"}},
                        "arg1": {"__call": "sum", "__args": {"arg0": "预警条数"}}
                    }
                }
            }
        });
        let lowered =
            lower_v2_metric("effectiveness_issue_verification_rate", &raw, &ctx).expect("lower");
        let rowset = lowered
            .pointer("/values/value/numerator/value/rowset")
            .or_else(|| lowered.pointer("/values/value/numerator/value/source/rowset"));
        assert!(
            rowset.is_some(),
            "ratio numerator should retain inlined warning_detail rowset, got {lowered}"
        );
        assert!(
            !rowset
                .and_then(|value| value.as_object())
                .and_then(|map| map.get("id"))
                .and_then(Value::as_str)
                .is_some_and(|id| id == "warning_detail"),
            "warning_detail data_ref should be inlined"
        );
    }

    #[test]
    fn lower_mechanism_documents_list_explain_as_data_product() {
        let bundle_datasets = json!([
            {
                "__call": "dataset",
                "__args": {
                    "id": "mechanism_documents",
                    "source": {"__ref": "source_ref", "__args": {"arg0": "mechanism_documents"}},
                },
            },
        ]);
        let ctx =
            V2MetricLowerContext::from_bundle_datasets(bundle_datasets.as_array().expect("array"));
        let raw = json!({
            "__call": "metric_scalar",
            "__args": {
                "id": "effectiveness_mechanism_item_count",
                "agg": {"__call": "count", "__args": {}},
                "explain": [
                    {
                        "__call": "dataframe",
                        "__args": {
                            "id": "mechanism_documents_list",
                            "label": "健全机制清单",
                            "value": {"__call": "data_ref", "__args": {"arg0": "mechanism_documents"}},
                        },
                    },
                ],
            },
        });
        let lowered =
            lower_v2_metric("effectiveness_mechanism_item_count", &raw, &ctx).expect("lower");
        let explain = lowered
            .get("explain")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .expect("explain item");
        assert_eq!(
            explain.get("__kind").and_then(Value::as_str),
            Some("data_product")
        );
        assert_eq!(
            explain.get("id").and_then(Value::as_str),
            Some("mechanism_documents_list")
        );
        assert_eq!(
            explain.get("shape").and_then(Value::as_str),
            Some("dataframe")
        );
    }

    #[test]
    fn lower_property_procedure_share_from_fixture_payload() {
        let raw = serde_json::from_str::<Value>(
            r#"{
            "__call": "metric_dataframe",
            "__args": {
                "id": "property_procedure_share",
                "label": "项目程序占比",
                "pipeline": [
                    {
                        "__call": "concat_rowsets",
                        "__args": {
                            "arg0": {
                                "__call": "mutate",
                                "__args": {
                                    "arg0": {
                                        "__call": "where",
                                        "__args": {
                                            "arg0": {"__call": "data_ref", "__args": {"arg0": "maintenance_projects_ds"}},
                                            "arg1": {"__call": "in_values", "__args": {"arg0": "紧急程序", "arg1": ["是"]}}
                                        }
                                    },
                                    "arg1": {"label": {"__call": "lit", "__args": {"arg0": "紧急"}}}
                                }
                            },
                            "arg1": {
                                "__call": "mutate",
                                "__args": {
                                    "arg0": {
                                        "__call": "where",
                                        "__args": {
                                            "arg0": {"__call": "data_ref", "__args": {"arg0": "maintenance_projects_ds"}},
                                            "arg1": {"__call": "in_values", "__args": {"arg0": "普通程序", "arg1": ["是"]}}
                                        }
                                    },
                                    "arg1": {"label": {"__call": "lit", "__args": {"arg0": "普通"}}}
                                }
                            }
                        }
                    },
                    {"__call": "group_by", "__args": {"arg0": "label"}},
                    {"__call": "select", "__args": {"arg0": ["label", "value"]}}
                ]
            }
        }"#,
        )
        .expect("fixture json");
        let ctx = V2MetricLowerContext::default();
        let lowered = lower_v2_metric("property_procedure_share", &raw, &ctx).expect("lower");
        let rowset_type = lowered
            .pointer("/value/rowset/rowset/type")
            .and_then(|v| v.as_str());
        assert_eq!(rowset_type, Some("concat_rowsets"), "got {lowered}");
    }

    #[test]
    fn lower_concat_rowsets_pipeline_preserves_lit_labels() {
        let raw = json!({
            "__call": "metric_dataframe",
            "__args": {
                "id": "property_procedure_share",
                "label": "项目程序占比",
                "pipeline": [
                    {
                        "__call": "concat_rowsets",
                        "__args": {
                            "arg0": {
                                "__call": "mutate",
                                "__args": {
                                    "arg0": {
                                        "__call": "where",
                                        "__args": {
                                            "arg0": {"__call": "data_ref", "__args": {"arg0": "maintenance_projects_ds"}},
                                            "arg1": {
                                                "__call": "in_values",
                                                "__args": {"arg0": "紧急程序", "arg1": ["是"]}
                                            }
                                        }
                                    },
                                    "arg1": {
                                        "label": {"__call": "lit", "__args": {"arg0": "紧急"}}
                                    }
                                }
                            },
                            "arg1": {
                                "__call": "mutate",
                                "__args": {
                                    "arg0": {
                                        "__call": "where",
                                        "__args": {
                                            "arg0": {"__call": "data_ref", "__args": {"arg0": "maintenance_projects_ds"}},
                                            "arg1": {
                                                "__call": "in_values",
                                                "__args": {"arg0": "普通程序", "arg1": ["是"]}
                                            }
                                        }
                                    },
                                    "arg1": {
                                        "label": {"__call": "lit", "__args": {"arg0": "普通"}}
                                    }
                                }
                            }
                        }
                    },
                    {"__call": "group_by", "__args": {"arg0": "label"}},
                    {"__call": "select", "__args": {"arg0": ["label", "value"]}}
                ]
            }
        });
        let ctx = V2MetricLowerContext::default();
        let lowered = lower_v2_metric("property_procedure_share", &raw, &ctx).expect("lower");
        assert_eq!(
            lowered.pointer("/value/type").and_then(|v| v.as_str()),
            Some("select"),
            "pipeline should lower to select(...), got {lowered}"
        );
        assert_eq!(
            lowered
                .pointer("/value/rowset/type")
                .and_then(|v| v.as_str()),
            Some("group_by")
        );
        assert_eq!(
            lowered
                .pointer("/value/rowset/rowset/type")
                .and_then(|v| v.as_str()),
            Some("concat_rowsets"),
            "concat_rowsets must survive lowering, got {lowered}"
        );
        let rowsets = lowered
            .pointer("/value/rowset/rowset/rowsets")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            rowsets.len(),
            2,
            "expected urgent+normal rowsets, got {lowered}"
        );
        assert_eq!(
            rowsets[0]
                .pointer("/updates/label/type")
                .and_then(|v| v.as_str()),
            Some("lit")
        );
        assert_eq!(
            rowsets[0]
                .pointer("/updates/label/value")
                .and_then(|v| v.as_str()),
            Some("紧急")
        );
    }

    #[test]
    fn year_between_rowset_uses_lower_upper_bounds() {
        let rowset = year_between_rowset(
            json!({"__ref": "data", "id": "administrative_inspection"}),
            Some("检查日期".to_string()),
            json!(2024),
        );
        let predicate = rowset.pointer("/predicate").expect("predicate");
        assert_eq!(
            predicate.get("type").and_then(Value::as_str),
            Some("between")
        );
        assert_eq!(
            predicate.get("lower").and_then(Value::as_str),
            Some("2024-01-01")
        );
        assert_eq!(
            predicate.get("upper").and_then(Value::as_str),
            Some("2024-12-31")
        );
        assert!(predicate.get("min").is_none());
        assert!(predicate.get("max").is_none());
    }

    #[test]
    fn ratio_count_where_predicate_only_binds_metric_rowset() {
        let bundle_datasets = json!([
            {
                "__call": "dataset",
                "__args": {
                    "id": "issue_result_list",
                    "source": {"__ref": "source_ref", "__args": {"arg0": "issue_handling_results"}}
                }
            },
            {
                "__call": "dataset_view",
                "__args": {
                    "id": "verified_issue_tracking_rows",
                    "from": "issue_result_list",
                    "rowset": {
                        "__call": "where",
                        "__args": {
                            "arg0": {
                                "__call": "first_by",
                                "__args": {
                                    "arg0": {
                                        "__call": "data_ref",
                                        "__args": {"arg0": "issue_result_list"}
                                    },
                                    "arg1": "问题跟踪ID"
                                }
                            },
                            "arg1": {
                                "__call": "in_values",
                                "__args": {"arg0": "是否查实", "arg1": ["是"]}
                            }
                        }
                    }
                }
            }
        ]);
        let ctx =
            V2MetricLowerContext::from_bundle_datasets(bundle_datasets.as_array().expect("array"));
        let raw = json!({
            "__call": "metric_scalar",
            "__args": {
                "id": "effectiveness_verified_rectification_rate",
                "rowset": {
                    "__call": "data_ref",
                    "__args": {"arg0": "verified_issue_tracking_rows"}
                },
                "agg": {
                    "__call": "ratio",
                    "__args": {
                        "arg0": {
                            "__call": "count",
                            "__args": {
                                "arg0": {
                                    "__call": "where",
                                    "__args": {
                                        "arg0": {
                                            "__call": "not_empty",
                                            "__args": {"arg0": "健全机制"}
                                        }
                                    }
                                }
                            }
                        },
                        "arg1": {"__call": "count", "__args": {}}
                    }
                }
            }
        });
        let lowered = lower_v2_metric("effectiveness_verified_rectification_rate", &raw, &ctx)
            .expect("lower");
        assert_eq!(
            lowered
                .pointer("/values/value/numerator/rowset/type")
                .and_then(|v| v.as_str()),
            Some("where"),
            "numerator must be count(where(...)), got {lowered}"
        );
        assert_eq!(
            lowered
                .pointer("/values/value/numerator/rowset/predicate/type")
                .and_then(|v| v.as_str()),
            Some("not_empty"),
            "predicate-only where must keep not_empty, got {lowered}"
        );
        assert_eq!(
            lowered
                .pointer("/values/value/numerator/rowset/predicate/field")
                .and_then(|v| v.as_str()),
            Some("健全机制")
        );
        assert!(
            lowered
                .pointer("/values/value/numerator/rowset/rowset")
                .is_some_and(|v| !v.is_null()),
            "predicate-only where must bind metric base_rowset, got {lowered}"
        );
    }

    #[test]
    fn lower_latest_days_dataset_view_inlines_into_scalar_count() {
        let bundle_datasets = json!([
            {
                "__call": "dataset",
                "__args": {
                    "id": "inspections",
                    "source": {"__ref": "source_ref", "__args": {"arg0": "inspections"}},
                },
            },
            {
                "__call": "dataset_view",
                "__args": {
                    "id": "inspection_week_rows",
                    "from": "inspections",
                    "rowset": {
                        "__call": "latest_days",
                        "__args": {
                            "arg0": {"__call": "data_ref", "__args": {"arg0": "inspections"}},
                            "arg1": "检查日期",
                            "arg2": 7
                        }
                    }
                }
            }
        ]);
        let ctx =
            V2MetricLowerContext::from_bundle_datasets(bundle_datasets.as_array().expect("array"));
        let raw = json!({
            "__call": "metric_scalar",
            "__args": {
                "id": "inspections_week_count",
                "rowset": {"__call": "data_ref", "__args": {"arg0": "inspection_week_rows"}},
                "agg": {"__call": "count", "__args": {}}
            }
        });
        let lowered = lower_v2_metric("inspections_week_count", &raw, &ctx).expect("lower");
        assert_eq!(
            lowered
                .pointer("/values/value/rowset/type")
                .and_then(Value::as_str),
            Some("latest_days"),
            "week dataset_view must inline latest_days, got {lowered}"
        );
        assert_eq!(
            lowered
                .pointer("/values/value/rowset/days")
                .and_then(Value::as_u64),
            Some(7)
        );
    }

    #[test]
    fn lower_label_status_pending_dataset_view_inlines_into_scalar_count() {
        let bundle_datasets = json!([
            {
                "__call": "dataset",
                "__args": {
                    "id": "warning_list",
                    "source": {"__ref": "source_ref", "__args": {"arg0": "alert_tracking"}},
                },
            },
            {
                "__call": "dataset_view",
                "__args": {
                    "id": "issue_handling_list",
                    "from": "warning_list",
                    "rowset": {
                        "__call": "label_status_pending",
                        "__args": {
                            "arg0": {
                                "__call": "first_by",
                                "__args": {
                                    "arg0": {
                                        "__call": "data_ref",
                                        "__args": {"arg0": "warning_list"}
                                    },
                                    "arg1": "预警ID"
                                }
                            },
                            "in_progress": "在办",
                            "default": "待办",
                            "completed": "办结",
                            "field": "办理状态"
                        }
                    }
                }
            }
        ]);
        let ctx =
            V2MetricLowerContext::from_bundle_datasets(bundle_datasets.as_array().expect("array"));
        let raw = json!({
            "__call": "metric_scalar",
            "__args": {
                "id": "issue_handling_analytics",
                "rowset": {"__call": "data_ref", "__args": {"arg0": "issue_handling_list"}},
                "agg": {"__call": "count", "__args": {}}
            }
        });
        let lowered = lower_v2_metric("issue_handling_analytics", &raw, &ctx).expect("lower");
        assert_eq!(
            lowered
                .pointer("/values/value/rowset/type")
                .and_then(Value::as_str),
            Some("concat_rowsets"),
            "issue_handling_list must inline label_status_pending, got {lowered}"
        );
        assert_eq!(
            lowered
                .pointer("/values/value/rowset/rowsets/0/updates/办理状态/value")
                .and_then(Value::as_str),
            Some("待办"),
            "pending branch must mutate 办理状态=待办, got {lowered}"
        );
    }
}
