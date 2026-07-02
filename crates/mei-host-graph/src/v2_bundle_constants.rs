//! Resolve top-level bundle constants (`YEAR_CURR = 2025`) and substitute `__var` in metric IR.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

/// Parse `NAME = expr` lines before the first `metric_def_bundle(` call.
pub fn parse_bundle_constants_from_source(content: &str) -> BTreeMap<String, Value> {
    let mut constants = BTreeMap::new();
    for line in content.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with("use ") {
            continue;
        }
        if line.starts_with("metric_def_bundle") {
            break;
        }
        let Some((name, rhs)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let rhs = rhs.trim().trim_end_matches(',');
        if let Some(value) = eval_bundle_const_expr(rhs, &constants) {
            constants.insert(name.to_string(), value);
        }
    }
    constants
}

fn eval_bundle_const_expr(expr: &str, constants: &BTreeMap<String, Value>) -> Option<Value> {
    let expr = expr.trim();
    if expr.is_empty() {
        return None;
    }
    if (expr.starts_with('"') && expr.ends_with('"'))
        || (expr.starts_with('\'') && expr.ends_with('\''))
    {
        return Some(json!(expr[1..expr.len() - 1]));
    }
    if let Ok(n) = expr.parse::<i64>() {
        return Some(json!(n));
    }
    if let Ok(n) = expr.parse::<f64>() {
        return serde_json::Number::from_f64(n).map(Value::Number);
    }
    if let Some(inner) = expr.strip_prefix("str(").and_then(|s| s.strip_suffix(')')) {
        let inner = inner.trim();
        if let Some(value) = constants.get(inner) {
            return Some(json!(scalar_to_string(value)));
        }
        return Some(json!(inner));
    }
    if constants.contains_key(expr) {
        return constants.get(expr).cloned();
    }
    if expr.contains('+') {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quote = false;
        let mut quote = '\0';
        for ch in expr.chars() {
            if in_quote {
                current.push(ch);
                if ch == quote {
                    in_quote = false;
                }
                continue;
            }
            if ch == '"' || ch == '\'' {
                in_quote = true;
                quote = ch;
                current.push(ch);
                continue;
            }
            if ch == '+' {
                parts.push(current.trim().to_string());
                current.clear();
                continue;
            }
            current.push(ch);
        }
        if !current.trim().is_empty() {
            parts.push(current.trim().to_string());
        }
        let mut out = String::new();
        for part in parts {
            let piece = eval_bundle_const_expr(part.as_str(), constants)?;
            out.push_str(&scalar_to_string(&piece));
        }
        return Some(json!(out));
    }
    if expr.starts_with('[') && expr.ends_with(']') {
        let inner = expr[1..expr.len() - 1].trim();
        if inner.is_empty() {
            return Some(json!([]));
        }
        let items = inner
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|item| eval_bundle_const_expr(item, constants).unwrap_or_else(|| json!(item)))
            .collect::<Vec<_>>();
        return Some(json!(items));
    }
    None
}

fn scalar_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn merge_v2_json_values(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Object(left_map), Value::Object(right_map)) => {
            let mut merged = left_map.clone();
            for (key, value) in right_map {
                if let Some(existing) = merged.get_mut(key) {
                    *existing = merge_v2_json_values(existing, value);
                } else {
                    merged.insert(key.clone(), value.clone());
                }
            }
            Value::Object(merged)
        }
        (Value::Null, Value::Object(map)) | (Value::Object(map), Value::Null) => {
            Value::Object(map.clone())
        }
        (Value::Object(map), _) => Value::Object(map.clone()),
        (_, right) => right.clone(),
    }
}

fn resolve_v2_binop(
    op: &str,
    left: &Value,
    right: &Value,
    constants: &BTreeMap<String, Value>,
) -> Value {
    let left = resolve_v2_constants(left, constants);
    let right = resolve_v2_constants(right, constants);
    if op.contains("Merge") {
        return merge_v2_json_values(&left, &right);
    }
    json!(scalar_to_string(&left) + &scalar_to_string(&right))
}

/// Walk lowered / compiled metric JSON and replace `{ "__var": "NAME" }` nodes.
pub fn resolve_v2_constants(value: &Value, constants: &BTreeMap<String, Value>) -> Value {
    if let Some(map) = value.as_object() {
        if let Some(name) = map.get("__var").and_then(Value::as_str) {
            if let Some(resolved) = constants.get(name) {
                return resolved.clone();
            }
            return value.clone();
        }
        if let Some(call) = map.get("__call").and_then(Value::as_str) {
            if call == "str" {
                let args = map.get("__args").and_then(Value::as_object);
                let arg0 = args
                    .and_then(|m| m.get("arg0"))
                    .map(|v| resolve_v2_constants(v, constants))
                    .unwrap_or(Value::Null);
                return json!(scalar_to_string(&arg0));
            }
        }
        if map.contains_key("__binop") {
            let op = map
                .get("__binop")
                .and_then(Value::as_str)
                .unwrap_or("Add");
            let left = map.get("left").cloned().unwrap_or(Value::Null);
            let right = map.get("right").cloned().unwrap_or(Value::Null);
            return resolve_v2_binop(op, &left, &right, constants);
        }
        let mut out = Map::new();
        for (key, child) in map {
            out.insert(key.clone(), resolve_v2_constants(child, constants));
        }
        return Value::Object(out);
    }
    if let Some(array) = value.as_array() {
        return Value::Array(
            array
                .iter()
                .map(|item| resolve_v2_constants(item, constants))
                .collect(),
        );
    }
    value.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_penalty_bundle_constants() {
        let src = r#"
YEAR_PREV = 2024
YEAR_CURR = 2025
PENALTY_PARTY_YEAR_COUNT_CURR = "处罚次数_" + str(YEAR_CURR)
metric_def_bundle(
"#;
        let constants = parse_bundle_constants_from_source(src);
        assert_eq!(constants.get("YEAR_CURR"), Some(&json!(2025)));
        assert_eq!(
            constants.get("PENALTY_PARTY_YEAR_COUNT_CURR"),
            Some(&json!("处罚次数_2025"))
        );
    }

    #[test]
    fn resolve_var_in_metric_json() {
        let constants = BTreeMap::from([
            ("YEAR_CURR".to_string(), json!(2025)),
            (
                "PENALTY_PARTY_YEAR_COUNT_CURR".to_string(),
                json!("处罚次数_2025"),
            ),
        ]);
        let raw = json!({
            "years": [{"__var": "YEAR_PREV"}, {"__var": "YEAR_CURR"}],
            "field": {"__var": "PENALTY_PARTY_YEAR_COUNT_CURR"}
        });
        let resolved = resolve_v2_constants(&raw, &constants);
        assert_eq!(resolved["years"], json!([{"__var": "YEAR_PREV"}, 2025]));
        assert_eq!(resolved["field"], json!("处罚次数_2025"));
    }

    #[test]
    fn resolve_merge_binop_into_object() {
        let raw = json!({
            "__binop": "Merge",
            "left": {"padding": "0", "background": "transparent"},
            "right": {"border": "none", "background": {"image": "url(/bg.svg)"}}
        });
        let resolved = resolve_v2_constants(&raw, &BTreeMap::new());
        assert_eq!(
            resolved,
            json!({
                "padding": "0",
                "background": {"image": "url(/bg.svg)"},
                "border": "none",
            })
        );
    }
}
