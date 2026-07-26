fn parse_filter_spec(expected: &str) -> FilterSpec {
    let trimmed = expected.trim();
    if trimmed.is_empty() {
        return FilterSpec::Contains(String::new());
    }
    if let Some(rest) = trimmed.strip_prefix("not:") {
        return FilterSpec::Not(Box::new(parse_filter_spec(rest)));
    }
    if let Some(rest) = trimmed.strip_prefix("eq:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Eq,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("gte:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Gte,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("gt:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Gt,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("lte:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Lte,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("lt:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Lt,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("mrange:") {
        if let Some((start, end)) = rest.split_once("..") {
            let start = start.trim();
            let end = end.trim();
            if !start.is_empty() && !end.is_empty() {
                return FilterSpec::MonthRange {
                    start: start.to_string(),
                    end: end.to_string(),
                };
            }
        }
    }
    if let Some(rest) = trimmed.strip_prefix("drange:") {
        if let Some((start, end)) = rest.split_once("..") {
            let start = start.trim();
            let end = end.trim();
            if !start.is_empty() && !end.is_empty() {
                return FilterSpec::DateRange {
                    start: start.to_string(),
                    end: end.to_string(),
                };
            }
        }
    }
    if let Some(rest) = trimmed.strip_prefix("in:") {
        return FilterSpec::InValues(
            split_filter_values(rest)
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
    }
    if let Some(rest) = trimmed.strip_prefix("contains_any:") {
        return FilterSpec::ContainsAny(
            split_filter_values(rest)
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
    }
    if let Some(rest) = trimmed.strip_prefix("m:") {
        return FilterSpec::Month(
            split_filter_values(rest)
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
    }
    if let Some(rest) = trimmed.strip_prefix("contains:") {
        return FilterSpec::Contains(rest.to_string());
    }
    FilterSpec::Contains(trimmed.to_string())
}

fn eval_num_compare(actual: f64, op: NumCompareOp, expected: f64) -> bool {
    const EPS: f64 = 1e-9;
    match op {
        NumCompareOp::Eq => (actual - expected).abs() <= EPS,
        NumCompareOp::Gt => actual > expected,
        NumCompareOp::Gte => actual >= expected - EPS,
        NumCompareOp::Lt => actual < expected,
        NumCompareOp::Lte => actual <= expected + EPS,
    }
}

fn eval_filter_spec(actual: &str, spec: &FilterSpec) -> bool {
    match spec {
        FilterSpec::Not(inner) => !eval_filter_spec(actual, inner),
        FilterSpec::Contains(needle) => {
            if needle.is_empty() {
                return true;
            }
            actual.contains(needle.as_str())
        }
        FilterSpec::ContainsAny(values) => {
            if values.is_empty() {
                return true;
            }
            values
                .iter()
                .any(|needle| !needle.is_empty() && actual.contains(needle.as_str()))
        }
        FilterSpec::InValues(values) => values.iter().any(|part| actual == part.as_str()),
        FilterSpec::Month(values) => {
            let Some(actual_month) = extract_year_month(actual) else {
                return false;
            };
            values.iter().any(|part| actual_month == *part)
        }
        FilterSpec::MonthRange { start, end } => {
            let Some(actual_month) = extract_year_month(actual) else {
                return false;
            };
            actual_month.as_str() >= start.as_str() && actual_month.as_str() <= end.as_str()
        }
        FilterSpec::DateRange { start, end } => {
            let Some(actual_ord) = sort_datetime(actual) else {
                return false;
            };
            let Some(start_ord) = sort_datetime(start) else {
                return false;
            };
            let Some(end_ord) = sort_datetime(end) else {
                return false;
            };
            actual_ord >= start_ord && actual_ord <= end_ord
        }
        FilterSpec::NumCompare { op, value } => parse_filter_number(actual)
            .is_some_and(|actual_value| eval_num_compare(actual_value, *op, *value)),
    }
}

fn split_filter_values(raw: &str) -> Vec<&str> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn extract_year_month(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.len() >= 7 && trimmed.as_bytes().get(4) == Some(&b'-') {
        let prefix = &trimmed[..7];
        if prefix.chars().take(4).all(|ch| ch.is_ascii_digit())
            && prefix.as_bytes().get(4) == Some(&b'-')
            && prefix[5..].chars().all(|ch| ch.is_ascii_digit())
        {
            return Some(prefix.to_string());
        }
    }
    if let Some(ms) = sort_datetime(trimmed) {
        let datetime = DateTime::<Utc>::from_timestamp_millis(ms)?;
        return Some(format!("{:04}-{:02}", datetime.year(), datetime.month()));
    }
    None
}

fn row_matches_filter_value(actual: &str, expected: &str) -> bool {
    eval_filter_spec(actual, &parse_filter_spec(expected))
}

#[cfg(test)]
mod filter_spec_tests {
    use super::{eval_filter_spec, parse_filter_spec, FilterSpec};

    #[test]
    fn parse_not_in_values() {
        let spec = parse_filter_spec("not:in:红,黄");
        match &spec {
            FilterSpec::Not(inner) => match inner.as_ref() {
                FilterSpec::InValues(values) => assert_eq!(values, &vec!["红", "黄"]),
                _ => panic!("expected in values"),
            },
            _ => panic!("expected not"),
        }
        assert!(!eval_filter_spec("红", &spec));
        assert!(eval_filter_spec("蓝", &spec));
    }

    #[test]
    fn parse_numeric_gte() {
        let spec = parse_filter_spec("gte:10");
        assert!(eval_filter_spec("12", &spec));
        assert!(eval_filter_spec("10", &spec));
        assert!(!eval_filter_spec("9", &spec));
    }

    #[test]
    fn parse_contains_any_membership() {
        let spec = parse_filter_spec("contains_any:红,黄");
        match &spec {
            FilterSpec::ContainsAny(values) => assert_eq!(values, &vec!["红", "黄"]),
            _ => panic!("expected contains_any"),
        }
        assert!(eval_filter_spec("蓝/黄/红", &spec));
        assert!(eval_filter_spec("黄", &spec));
        assert!(!eval_filter_spec("蓝", &spec));
        assert!(!eval_filter_spec("/", &spec));
    }

    #[test]
    fn parse_month_range() {
        let spec = parse_filter_spec("mrange:2024-01..2024-06");
        assert!(eval_filter_spec("2024-03-15", &spec));
        assert!(!eval_filter_spec("2023-12-01", &spec));
    }

    #[test]
    fn parse_date_range() {
        let spec = parse_filter_spec("drange:2024-01-15..2024-06-30");
        assert!(eval_filter_spec("2024-03-15", &spec));
        assert!(eval_filter_spec("2024-01-15", &spec));
        assert!(eval_filter_spec("2024-06-30", &spec));
        assert!(!eval_filter_spec("2024-01-14", &spec));
        assert!(!eval_filter_spec("2024-07-01", &spec));
    }
}

