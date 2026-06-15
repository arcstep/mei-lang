use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::super::dates::parse_row_date;
use super::super::schema::{row_number, row_string};

fn row_group_component(row: &Value, field: &str) -> Option<String> {
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

fn row_group_key(row: &Value, group_fields: &[String]) -> Option<Vec<String>> {
    let mut key = Vec::with_capacity(group_fields.len());
    for field in group_fields {
        key.push(row_group_component(row, field)?);
    }
    Some(key)
}

fn year_values_from_rows(rows: &[Value], year_field: &str) -> BTreeSet<i32> {
    let mut years = BTreeSet::new();
    for row in rows {
        if let Some(component) = row_group_component(row, year_field) {
            if let Ok(year) = component.parse::<i32>() {
                years.insert(year);
            }
        }
    }
    years
}

/// `group_by` 在指定 `pivot_field` / `pivot_columns` 时输出宽表（每个 pivot 列值为计数）。
pub fn aggregate_group_rows_pivot(
    rows: &[Value],
    group_fields: &[String],
    pivot_field: &str,
    pivot_columns: &[String],
    universe_first: Option<&[String]>,
) -> Vec<Value> {
    if group_fields.is_empty() {
        return Vec::new();
    }
    let mut counts: BTreeMap<Vec<String>, BTreeMap<String, usize>> = BTreeMap::new();
    for row in rows {
        let Some(key) = row_group_key(row, group_fields) else {
            continue;
        };
        let category = row_string(row, pivot_field);
        if !pivot_columns.iter().any(|item| item == &category) {
            continue;
        }
        *counts.entry(key).or_default().entry(category).or_insert(0) += 1;
    }

    let year_field = group_fields.get(1).filter(|field| field.as_str() == "年份");
    let mut years = year_field
        .map(|field| year_values_from_rows(rows, field))
        .unwrap_or_default();
    for key in counts.keys() {
        if key.len() >= 2 {
            if let Ok(year) = key[1].parse::<i32>() {
                years.insert(year);
            }
        }
    }
    let years_to_use: Vec<i32> = years.into_iter().collect();
    if year_field.is_some() && years_to_use.is_empty() {
        return Vec::new();
    }

    let first_field = &group_fields[0];
    let dimensions: Vec<String> = if let Some(labels) = universe_first {
        labels.to_vec()
    } else {
        counts
            .keys()
            .map(|key| key[0].clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };

    let mut out = Vec::new();
    if group_fields.len() == 2 && year_field.is_some() {
        let year_field_name = year_field.unwrap();
        for dimension in dimensions {
            for year in &years_to_use {
                let key = vec![dimension.clone(), year.to_string()];
                let mut object = serde_json::Map::new();
                object.insert(first_field.clone(), Value::String(dimension.clone()));
                object.insert(year_field_name.clone(), json!(*year));
                let bucket = counts.get(&key);
                for column in pivot_columns {
                    let count = bucket
                        .and_then(|stats| stats.get(column))
                        .copied()
                        .unwrap_or(0);
                    object.insert(column.clone(), json!(count));
                }
                out.push(Value::Object(object));
            }
        }
        return out;
    }

    for (key, bucket) in counts {
        let mut object = serde_json::Map::new();
        for (index, field) in group_fields.iter().enumerate() {
            let component = &key[index];
            if field == "年份" {
                if let Ok(year) = component.parse::<i32>() {
                    object.insert(field.clone(), json!(year));
                } else {
                    object.insert(field.clone(), Value::String(component.clone()));
                }
            } else {
                object.insert(field.clone(), Value::String(component.clone()));
            }
        }
        for column in pivot_columns {
            let count = bucket.get(column).copied().unwrap_or(0);
            object.insert(column.clone(), json!(count));
        }
        out.push(Value::Object(object));
    }
    out
}

pub fn party_year_aggregate_rows(
    rows: &[Value],
    party_field: &str,
    date_field: &str,
    value_field: &str,
    years: &[i32],
) -> Vec<Value> {
    let mut parties: BTreeMap<String, BTreeMap<i32, (f64, usize)>> = BTreeMap::new();
    for row in rows {
        let party = row_string(row, party_field);
        if party.is_empty() {
            continue;
        }
        let Some((year, _, _)) = parse_row_date(row, date_field) else {
            continue;
        };
        if !years.contains(&year) {
            continue;
        }
        let amount = row_number(row, value_field).unwrap_or(0.0);
        let entry = parties
            .entry(party)
            .or_default()
            .entry(year)
            .or_insert((0.0, 0));
        entry.0 += amount;
        entry.1 += 1;
    }
    let mut out = Vec::new();
    for (party, year_stats) in parties {
        let mut object = serde_json::Map::new();
        object.insert(party_field.to_string(), Value::String(party));
        for year in years {
            let (sum, count) = year_stats.get(year).copied().unwrap_or((0.0, 0));
            object.insert(format!("罚没金额_{year}"), json!(sum));
            object.insert(format!("处罚次数_{year}"), json!(count as f64));
        }
        if years.len() >= 2 {
            let prev = years[years.len() - 2];
            let curr = years[years.len() - 1];
            let prev_sum = year_stats.get(&prev).map(|(sum, _)| *sum).unwrap_or(0.0);
            let curr_sum = year_stats.get(&curr).map(|(sum, _)| *sum).unwrap_or(0.0);
            object.insert(
                format!("同比降低额_{curr}"),
                json!((prev_sum - curr_sum).max(0.0)),
            );
        }
        out.push(Value::Object(object));
    }
    out
}

pub fn unpivot_columns_rows(
    rows: &[Value],
    id_field: &str,
    columns: &[(String, String)],
    year_field: &str,
    value_field: &str,
) -> Vec<Value> {
    let mut out = Vec::new();
    for row in rows {
        let id = row_string(row, id_field);
        if id.is_empty() {
            continue;
        }
        for (year_label, source_field) in columns {
            let value = row_number(row, source_field).unwrap_or(0.0);
            let mut object = serde_json::Map::new();
            object.insert(id_field.to_string(), Value::String(id.clone()));
            object.insert(year_field.to_string(), Value::String(year_label.clone()));
            object.insert(value_field.to_string(), json!(value));
            out.push(Value::Object(object));
        }
    }
    out
}

pub fn aggregate_group_rows(
    rows: &[Value],
    group_field: &str,
    value_field: Option<&str>,
    agg: &str,
    limit: Option<usize>,
) -> Vec<Value> {
    let mut grouped = std::collections::BTreeMap::<String, Vec<f64>>::new();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for row in rows {
        let label = row_string(row, group_field);
        if label.is_empty() {
            continue;
        }
        *counts.entry(label.clone()).or_insert(0) += 1;
        if let Some(field) = value_field {
            if let Some(number) = row_number(row, field) {
                grouped.entry(label).or_default().push(number);
            } else {
                grouped.entry(label).or_default();
            }
        } else {
            grouped.entry(label).or_default();
        }
    }
    let mut out = Vec::new();
    for (label, numbers) in grouped {
        let value = match agg {
            "sum" => numbers.iter().sum::<f64>(),
            "avg" => {
                if numbers.is_empty() {
                    0.0
                } else {
                    numbers.iter().sum::<f64>() / numbers.len() as f64
                }
            }
            "min" => numbers.into_iter().reduce(f64::min).unwrap_or(0.0),
            "max" => numbers.into_iter().reduce(f64::max).unwrap_or(0.0),
            _ => counts.get(&label).copied().unwrap_or(0) as f64,
        };
        let mut row = serde_json::Map::new();
        row.insert(group_field.to_string(), Value::String(label));
        row.insert("value".to_string(), json!(value));
        out.push(Value::Object(row));
    }
    if let Some(limit) = limit {
        out.truncate(limit);
    }
    out
}

pub fn summarize_rows(rows: &[Value], agg: &str, value_field: &str) -> Vec<Value> {
    if agg == "count" {
        return vec![json!({ "value": rows.len() })];
    }
    let values = rows
        .iter()
        .filter_map(|row| row_number(row, value_field))
        .collect::<Vec<_>>();
    let value = match agg {
        "sum" => values.iter().sum::<f64>(),
        "avg" => {
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            }
        }
        "min" => values.into_iter().reduce(f64::min).unwrap_or(0.0),
        "max" => values.into_iter().reduce(f64::max).unwrap_or(0.0),
        _ => 0.0,
    };
    vec![json!({ "value": value })]
}
