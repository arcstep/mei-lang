use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

pub fn columnar_engine_enabled() -> bool {
    std::env::var("MEI_ROWSET_ENGINE")
        .ok()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("columnar"))
}

fn value_display_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                return integer.to_string();
            }
            if let Some(float) = number.as_f64() {
                if float.is_finite() && float.fract().abs() < f64::EPSILON {
                    return (float as i64).to_string();
                }
            }
            number.to_string()
        }
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn row_string(row: &Value, field: &str) -> String {
    match row.as_object().and_then(|object| object.get(field)) {
        Some(value) => value_display_text(value),
        None => String::new(),
    }
}

fn row_key(row: &Value, field: &str) -> Option<String> {
    if let Some(value) = row.get(field) {
        if let Some(year) = value.as_i64() {
            return Some(year.to_string());
        }
        if let Some(year) = value.as_f64() {
            return Some((year as i64).to_string());
        }
    }
    let text = row_string(row, field);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn try_where_eq_columnar(rows: &[Value], field: &str, expected: &str) -> Option<Vec<Value>> {
    if !columnar_engine_enabled() {
        return None;
    }
    Some(
        rows.iter()
            .filter(|row| row_key(row, field).as_deref() == Some(expected))
            .cloned()
            .collect(),
    )
}

pub fn try_group_by_count_columnar(
    rows: &[Value],
    group_field: &str,
    limit: Option<usize>,
) -> Option<Vec<Value>> {
    if !columnar_engine_enabled() {
        return None;
    }
    let mut counts = BTreeMap::<String, usize>::new();
    for row in rows {
        if let Some(key) = row_key(row, group_field) {
            *counts.entry(key).or_default() += 1;
        }
    }
    let mut out = counts
        .into_iter()
        .map(|(key, count)| {
            let mut obj = Map::new();
            obj.insert(group_field.to_string(), Value::String(key));
            obj.insert("count".to_string(), json!(count));
            Value::Object(obj)
        })
        .collect::<Vec<_>>();
    if let Some(limit) = limit {
        out.truncate(limit);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn columnar_engine_paths() {
        std::env::remove_var("MEI_ROWSET_ENGINE");
        let rows = vec![json!({"x": 1})];
        assert!(try_where_eq_columnar(&rows, "x", "1").is_none());

        std::env::set_var("MEI_ROWSET_ENGINE", "columnar");
        let rows = vec![
            json!({"year": 2024, "name": "a"}),
            json!({"year": "2024", "name": "b"}),
            json!({"year": 2023, "name": "c"}),
        ];
        let filtered = try_where_eq_columnar(&rows, "year", "2024").expect("columnar where");
        assert_eq!(filtered.len(), 2);

        let rows = vec![
            json!({"park": "A"}),
            json!({"park": "A"}),
            json!({"park": "B"}),
        ];
        let grouped = try_group_by_count_columnar(&rows, "park", None).expect("columnar group_by");
        assert_eq!(grouped.len(), 2);
        let a_count = grouped
            .iter()
            .find(|row| row.get("park").and_then(|v| v.as_str()) == Some("A"))
            .and_then(|row| row.get("count").and_then(|v| v.as_u64()));
        assert_eq!(a_count, Some(2));
        std::env::remove_var("MEI_ROWSET_ENGINE");
    }
}
