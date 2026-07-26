pub(crate) fn normalize_search(search: Option<&str>) -> Option<String> {
    search
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

/// Columns whose values are inventory-style serials (`1`, `1-2`, `10-3`).
pub(crate) fn is_serial_number_field(field: &str) -> bool {
    let name = field.trim();
    !name.is_empty() && (name == "序号" || name.ends_with("序号"))
}

/// Zero-pad each digit run so lexical order matches human numeric order per segment.
/// Separators (e.g. `-`) are kept. Example: `1-2` → `0000000001-0000000002`.
pub(crate) fn serial_number_sort_key(text: &str) -> String {
    const PAD: usize = 10;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(trimmed.len().saturating_mul(2));
    let mut chars = trimmed.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            let mut digits = String::new();
            digits.push(ch);
            while let Some(next) = chars.peek().copied() {
                if next.is_ascii_digit() {
                    digits.push(chars.next().expect("peeked digit"));
                } else {
                    break;
                }
            }
            let significant = digits.trim_start_matches('0');
            let body = if significant.is_empty() {
                "0"
            } else {
                significant
            };
            if body.len() >= PAD {
                out.push_str(body);
            } else {
                out.extend(std::iter::repeat('0').take(PAD - body.len()));
                out.push_str(body);
            }
        } else {
            out.push(ch);
        }
    }
    out
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
        let ordering = if is_serial_number_field(field) {
            compare_serial_number_values(left_value, right_value)
        } else {
            compare_sort_values(left_value, right_value)
        };
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

fn compare_serial_number_values(left: &Value, right: &Value) -> Ordering {
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
    serial_number_sort_key(&left_text).cmp(&serial_number_sort_key(&right_text))
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

#[cfg(test)]
mod serial_number_sort_tests {
    use super::{compare_serial_number_values, is_serial_number_field, serial_number_sort_key};
    use serde_json::json;
    use std::cmp::Ordering;

    #[test]
    fn detects_serial_fields() {
        assert!(is_serial_number_field("序号"));
        assert!(is_serial_number_field("事项序号"));
        assert!(!is_serial_number_field("风险事项"));
        assert!(!is_serial_number_field(""));
    }

    #[test]
    fn pads_digit_runs_and_keeps_hyphen() {
        assert_eq!(
            serial_number_sort_key("1-2"),
            "0000000001-0000000002"
        );
        assert_eq!(serial_number_sort_key("10"), "0000000010");
        assert_eq!(serial_number_sort_key("01"), "0000000001");
    }

    #[test]
    fn hyphenated_serials_sort_numerically() {
        let mut values = ["1-10", "1-2", "2", "10-1", "1"]
            .map(|v| json!(v))
            .to_vec();
        values.sort_by(|a, b| compare_serial_number_values(a, b));
        let ordered = values
            .iter()
            .map(|v| v.as_str().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec!["1", "1-2", "1-10", "2", "10-1"]);
    }

    #[test]
    fn plain_integers_sort_numerically_as_text_serials() {
        let mut values = ["10", "2", "1"].map(|v| json!(v)).to_vec();
        values.sort_by(|a, b| compare_serial_number_values(a, b));
        let ordered = values
            .iter()
            .map(|v| v.as_str().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec!["1", "2", "10"]);
        assert_eq!(
            compare_serial_number_values(&json!(2), &json!(10)),
            Ordering::Less
        );
    }
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

