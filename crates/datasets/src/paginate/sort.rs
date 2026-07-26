pub(crate) fn normalize_search(search: Option<&str>) -> Option<String> {
    search
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

fn compare_rows(left: &Value, right: &Value, sort: &[TableSortSpec]) -> Ordering {
    let left_map = left.as_object();
    let right_map = right.as_object();
    for spec in sort {
        let field = spec.field.trim();
        if field.is_empty() {
            continue;
        }
        let left_value = left_map
            .and_then(|map| map.get(field))
            .unwrap_or(&Value::Null);
        let right_value = right_map
            .and_then(|map| map.get(field))
            .unwrap_or(&Value::Null);
        let ordering = compare_sort_values(left_value, right_value);
        if ordering != Ordering::Equal {
            return if spec.direction.eq_ignore_ascii_case("desc") {
                ordering.reverse()
            } else {
                ordering
            };
        }
    }
    Ordering::Equal
}

fn compare_sort_values(left: &Value, right: &Value) -> Ordering {
    let left_text = value_to_text(left);
    let right_text = value_to_text(right);
    if left_text.is_empty() && right_text.is_empty() {
        return Ordering::Equal;
    }
    if left_text.is_empty() {
        return Ordering::Greater;
    }
    if right_text.is_empty() {
        return Ordering::Less;
    }
    if let (Some(lhs), Some(rhs)) = (
        sort_number(left, &left_text),
        sort_number(right, &right_text),
    ) {
        if let Some(ordering) = lhs.partial_cmp(&rhs) {
            return ordering;
        }
    }
    if let (Some(lhs), Some(rhs)) = (sort_datetime(&left_text), sort_datetime(&right_text)) {
        return lhs.cmp(&rhs);
    }
    left_text.cmp(&right_text)
}

fn sort_number(value: &Value, text: &str) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.replace(',', "").parse::<f64>().ok(),
        _ => text.replace(',', "").parse::<f64>().ok(),
    }
}

fn sort_datetime(text: &str) -> Option<i64> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(text) {
        return Some(datetime.timestamp_millis());
    }
    for fmt in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(text, fmt) {
            return Some(
                DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).timestamp_millis(),
            );
        }
    }
    for fmt in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(text, fmt) {
            let datetime = date.and_hms_opt(0, 0, 0)?;
            return Some(
                DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).timestamp_millis(),
            );
        }
    }
    None
}

#[derive(Debug, Clone)]
enum FilterSpec {
    Contains(String),
    /// OR of substring matches (membership multi-select).
    ContainsAny(Vec<String>),
    InValues(Vec<String>),
    Month(Vec<String>),
    MonthRange { start: String, end: String },
    DateRange { start: String, end: String },
    NumCompare { op: NumCompareOp, value: f64 },
    Not(Box<FilterSpec>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumCompareOp {
    Eq,
    Gt,
    Gte,
    Lt,
    Lte,
}

fn parse_filter_number(raw: &str) -> Option<f64> {
    let text = raw.trim().replace(',', "");
    if text.is_empty() {
        return None;
    }
    text.parse::<f64>().ok()
}

