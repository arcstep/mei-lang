//! Lower v2 `__call` metric bundle IR into v1 runtime metric defs (analysis_expr / data_product).

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

const WARNING_LIST_DETAIL_FIELDS: &[&str] = &[
    "序号", "监督领域", "监督类别", "预警ID", "预警条数", "主责单位", "问题分类名称", "问题描述",
    "预警类型", "预警等级", "预警时间", "问题跟踪ID", "承办部门", "分办时间", "办结时间", "是否查实",
    "查实条数", "是否转问题线索", "核查情况", "处理结果",
];

const ISSUE_RESULT_DETAIL_FIELDS: &[&str] = &[
    "序号", "处理结果ID", "监督领域名称", "监督类别", "问题线索编号", "是否立案", "姓名/单位", "工作单位",
    "职务", "职级", "政治面貌", "处理处分", "挽回资金", "健全机制", "预警ID", "主责单位", "问题分类名称",
    "问题描述", "预警类型", "预警等级", "预警时间", "问题跟踪ID", "承办部门", "分办时间", "办结时间",
    "是否查实", "是否转问题线索", "核查情况", "处理结果",
];

pub fn lower_v2_runtime_metric_defs(raw: BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    raw.into_iter()
        .filter_map(|(id, metric)| lower_v2_metric(&id, &metric).map(|lowered| (id, lowered)))
        .collect()
}

fn lower_v2_metric(id: &str, value: &Value) -> Option<Value> {
    if value.get("__call").is_some() {
        let name = v2_call_name(value)?;
        let args = value.get("__args")?;
        return lower_v2_metric_call(id, name.as_str(), args);
    }
    Some(value.clone())
}

fn lower_v2_metric_call(id: &str, name: &str, args: &Value) -> Option<Value> {
    match name {
        "metric_scalar" => Some(lower_metric_scalar(id, args)),
        "metric_dataframe" => Some(lower_metric_dataframe(id, args)),
        _ => None,
    }
}

fn lower_metric_scalar(id: &str, args: &Value) -> Value {
    let map = args.as_object().cloned().unwrap_or_default();
    let base_rowset = base_rowset_from_scalar_args(&map);
    let value_expr = if let Some(agg) = map.get("agg") {
        lower_agg_on_rowset(agg, base_rowset.clone())
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
    out.insert(
        "values".to_string(),
        json!({"value": value_expr}),
    );
    out.insert(
        "schema".to_string(),
        json!([{"name": "value", "type": "number"}]),
    );
    if let Some(explain) = map.get("explain").and_then(Value::as_array) {
        out.insert("explain".to_string(), lower_explain_items(explain));
    }
    Value::Object(out)
}

fn lower_metric_dataframe(id: &str, args: &Value) -> Value {
    let map = args.as_object().cloned().unwrap_or_default();
    let value_expr = if let Some(pipeline) = map.get("pipeline").and_then(Value::as_array) {
        lower_pipeline(pipeline)
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
        out.insert("explain".to_string(), lower_explain_items(explain));
    }
    Value::Object(out)
}

fn base_rowset_from_scalar_args(map: &Map<String, Value>) -> Value {
    if let Some(rowset) = map.get("rowset") {
        lower_rowset(rowset)
    } else if let Some(dataset) = map.get("dataset").and_then(Value::as_str) {
        data_ref(dataset)
    } else {
        json!(null)
    }
}

fn lower_pipeline(steps: &[Value]) -> Value {
    let mut rowset = json!(null);
    for step in steps {
        rowset = lower_pipeline_step(&rowset, step);
    }
    rowset
}

fn lower_pipeline_step(input: &Value, step: &Value) -> Value {
    let Some(name) = v2_call_name(step) else {
        return input.clone();
    };
    let args = step.get("__args").cloned().unwrap_or(json!({}));
    match name.as_str() {
        "data_ref" => lower_rowset(step),
        "where" => aek("where", &[("rowset", input.clone()), ("predicate", lower_predicate(arg0(&args)))],
        ),
        "first_by" => aek("first_by", &[
                ("rowset", input.clone()),
                ("field", json!(arg0_string(&args).unwrap_or_default())),
            ],
        ),
        "select" => aek("select", &[
                ("rowset", input.clone()),
                ("fields", arg0(&args).clone()),
            ],
        ),
        "sort_by" => aek("sort_by", &[
                ("rowset", input.clone()),
                ("field", json!(arg0_string(&args).unwrap_or_default())),
                ("order", json!(args.get("order").and_then(Value::as_str).unwrap_or("asc"))),
            ],
        ),
        "rename" => aek("rename", &[
                ("rowset", input.clone()),
                ("mapping", arg0(&args).clone()),
            ],
        ),
        "mutate" => aek("mutate", &[
                ("rowset", input.clone()),
                ("updates", arg1(&args).clone()),
            ],
        ),
        "limit" => aek("limit", &[
                ("rowset", input.clone()),
                ("n", arg0(&args).clone()),
            ],
        ),
        "label_status_pending" => lower_label_status_pending(input, &args),
        _ => input.clone(),
    }
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
    let pending = aek("mutate", &[
            (
                "rowset",
                aek("where", &[
                        (
                            "rowset",
                            input.clone(),
                        ),
                        (
                            "predicate",
                            aek("and", &[(
                                    "predicates",
                                    json!([
                                        aek("present", &[("field", json!("问题跟踪ID"))]),
                                        aek("blank", &[("field", json!("承办部门"))]),
                                    ]),
                                )],
                            ),
                        ),
                    ],
                ),
            ),
            ("updates", json!({"当前状态": aek("lit", &[("value", json!(default))])})),
        ],
    );
    let in_progress_rows = aek("mutate", &[
            (
                "rowset",
                aek("where", &[
                        ("rowset", input.clone()),
                        (
                            "predicate",
                            aek("and", &[(
                                    "predicates",
                                    json!([
                                        aek("present", &[("field", json!("问题跟踪ID"))]),
                                        aek("present", &[("field", json!("承办部门"))]),
                                        aek("blank", &[("field", json!("办结时间"))]),
                                    ]),
                                )],
                            ),
                        ),
                    ],
                ),
            ),
            (
                "updates",
                json!({"当前状态": aek("lit", &[("value", json!(in_progress))])}),
            ),
        ],
    );
    let other = aek("mutate", &[
            (
                "rowset",
                aek("where", &[
                        ("rowset", input.clone()),
                        (
                            "predicate",
                            aek("not", &[(
                                    "predicate",
                                    aek("or", &[(
                                            "predicates",
                                            json!([
                                                aek("and", &[(
                                                        "predicates",
                                                        json!([
                                                            aek("present", &[("field", json!("问题跟踪ID"))]),
                                                            aek("blank", &[("field", json!("承办部门"))]),
                                                        ]),
                                                    )],
                                                ),
                                                aek("and", &[(
                                                        "predicates",
                                                        json!([
                                                            aek("present", &[("field", json!("问题跟踪ID"))]),
                                                            aek("present", &[("field", json!("承办部门"))]),
                                                            aek("blank", &[("field", json!("办结时间"))]),
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
            ("updates", json!({"当前状态": aek("lit", &[("value", json!(default))])})),
        ],
    );
    aek("concat_rowsets", &[("rowsets", json!([pending, in_progress_rows, other]))],
    )
}

fn lower_agg_on_rowset(agg: &Value, base_rowset: Value) -> Value {
    if let Some(name) = v2_call_name(agg) {
        return match name.as_str() {
            "count" => ae("count", vec![("rowset".to_string(), base_rowset)]),
            "sum" => lower_sum_agg(agg, base_rowset),
            "ratio" => {
                let num = agg
                    .get("__args")
                    .and_then(|a| a.get("arg0"))
                    .map(|v| lower_sum_on_rowset(v, base_rowset.clone()))
                    .unwrap_or(json!(0));
                let den = agg
                    .get("__args")
                    .and_then(|a| a.get("arg1"))
                    .map(|v| lower_sum_on_rowset(v, base_rowset.clone()))
                    .unwrap_or(json!(0));
                aek("ratio", &[("numerator", num), ("denominator", den)],
                )
            }
            "transfer_clue_count" => expand_transfer_clue_count(agg),
            "mechanism_item_count" => expand_mechanism_item_count(agg),
            other => expand_known_agg_macro(other, agg, base_rowset.clone()).unwrap_or_else(|| {
                ae("count", vec![("rowset".to_string(), base_rowset)])
            }),
        };
    }
    json!(null)
}

fn lower_sum_agg(agg: &Value, base_rowset: Value) -> Value {
    let args = agg.get("__args").and_then(Value::as_object);
    let field = args
        .and_then(|m| m.get("field").or_else(|| m.get("arg0")))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let group_by = args
        .and_then(|m| m.get("group_by"))
        .and_then(Value::as_str);
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
        if let Some(field) = arg0_string_from_value(agg) {
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
    json!(0)
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
                                ("field".to_string(), json!("序号")),
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
                                    ("field".to_string(), json!("序号")),
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
                            ("field".to_string(), json!(prefix_field)),
                        ],
                    ),
                )],
            ))
        }
        _ => None,
    }
}

fn expand_person_rowset(name: &str, value: &Value) -> Option<Value> {
    let rowset = value
        .get("__args")
        .and_then(|a| a.get("arg0"))
        .map(lower_rowset)?;
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

fn expand_transfer_clue_count(agg: &Value) -> Value {
    let rows = agg
        .get("__args")
        .and_then(|a| a.get("arg0"))
        .map(lower_rowset)
        .unwrap_or(data_ref("warning_list"));
    let transfer_rows = aek("where", &[
            ("rowset", rows.clone()),
            (
                "predicate",
                aek("and", &[(
                        "predicates",
                        json!([
                            aek("present", &[("field", json!("问题跟踪ID"))]),
                            aek("contains", &[("field", json!("是否转问题线索")), ("value", json!("是"))]),
                        ]),
                    )],
                ),
            ),
        ],
    );
    let paren_rows = aek("where", &[
            ("rowset", transfer_rows.clone()),
            (
                "predicate",
                aek("matches", &[("field", json!("是否转问题线索")), ("pattern", json!("[（(]\\s*\\d+\\s*[）)]"))]),
            ),
        ],
    );
    aek("sum_rowset_counts", &[("rowsets", json!([transfer_rows, paren_rows]))],
    )
}

fn expand_mechanism_item_count(agg: &Value) -> Value {
    let rows = agg
        .get("__args")
        .and_then(|a| a.get("arg0"))
        .map(lower_rowset)
        .unwrap_or(data_ref("issue_result_list"));
    let source_rows = aek("where", &[
            ("rowset", rows),
            ("predicate", aek("not_empty", &[("field", json!("健全机制"))])),
        ],
    );
    let split_comma = aek("split_text", &[
            ("rowset", source_rows),
            ("field", json!("健全机制")),
            ("delimiter", json!("、")),
        ],
    );
    let split_items = aek("split_text", &[
            ("rowset", split_comma),
            ("field", json!("健全机制")),
            ("delimiter", json!("》《")),
        ],
    );
    aek("count", &[("rowset", split_items)])
}

fn lower_rowset(value: &Value) -> Value {
    if let Some(name) = v2_call_name(value) {
        let args = value.get("__args").cloned().unwrap_or(json!({}));
        return match name.as_str() {
            "data_ref" => data_ref(
                arg0_string(&args)
                    .unwrap_or_default()
                    .as_str(),
            ),
            "where" => aek("where", &[
                    ("rowset", lower_rowset(arg0(&args))),
                    ("predicate", lower_predicate(arg1(&args))),
                ],
            ),
            "first_by" => aek("first_by", &[
                    ("rowset", lower_rowset(arg0(&args))),
                    ("field", json!(arg1_string(&args).unwrap_or_default())),
                ],
            ),
            "party_gov_sanction_rows" | "handled_person_rows" => {
                expand_person_rowset(name.as_str(), value).unwrap_or(json!(null))
            }
            _ => lower_expr(value),
        };
    }
    lower_expr(value)
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
            "not_empty" => aek("not_empty", &[("field", json!(arg0_string(&args).unwrap_or_default()))]),
            "present" => aek("present", &[("field", json!(arg0_string(&args).unwrap_or_default()))]),
            "blank" => aek("blank", &[("field", json!(arg0_string(&args).unwrap_or_default()))]),
            "contains" => aek("contains", &[
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
            "matches" => aek("matches", &[
                    ("field", json!(arg0_string(&args).unwrap_or_default())),
                    (
                        "pattern",
                        json!(args.get("pattern")
                            .or_else(|| args.get("arg1"))
                            .and_then(Value::as_str)
                            .unwrap_or("")),
                    ),
                ],
            ),
            "in_values" => aek("in_values", &[
                    ("field", json!(arg0_string(&args).unwrap_or_default())),
                    ("values", arg1(&args).clone()),
                ],
            ),
            "is_yes" => aek("and", &[(
                    "predicates",
                    json!([
                        aek("not_empty", &[("field", json!(arg0_string(&args).unwrap_or_default()))]),
                        aek("contains", &[
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

fn lower_expr(value: &Value) -> Value {
    if value.get("__call").is_some() {
        return lower_rowset(value);
    }
    value.clone()
}

fn lower_explain_items(items: &[Value]) -> Value {
    Value::Array(
        items
            .iter()
            .filter_map(|item| lower_explain_item(item))
            .collect(),
    )
}

fn lower_explain_item(value: &Value) -> Option<Value> {
    let name = v2_call_name(value)?;
    let args = value.get("__args")?.as_object()?;
    let kind = match name.as_str() {
        "detail" => "detail",
        "composition" => "composition",
        "ratio" => "ratio",
        "dataframe" => "detail",
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
    value.get("__call").and_then(Value::as_str).map(str::to_string)
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
        let lowered = lower_v2_metric("realtime_warning_detail", &raw).expect("lower");
        assert_eq!(lowered.get("shape").and_then(|v| v.as_str()), Some("scalar_map"));
        assert!(lowered
            .pointer("/values/value/type")
            .and_then(|v| v.as_str())
            == Some("count"));
    }
}
