//! Multi-value / hyphen-range object identity keys (Excel association cells).

/// Canonical cell form for `column(..., normalize = "object_keys")`:
/// blank sentinels → empty; ranges / 顿号 → `、`-joined unique keys.
pub fn normalize_object_keys_cell(raw: &str) -> String {
    split_multi_object_keys(raw).join("、")
}

/// Blank / sentinel identities that must never participate in object links or lookups.
pub fn is_blank_object_identity(raw: &str) -> bool {
    let text = raw.trim();
    if text.is_empty() {
        return true;
    }
    const SENTINELS: &[&str] = &[
        "—",
        "-",
        "/",
        "无",
        "暂无",
        "待定",
        "未知",
        "n/a",
        "na",
        "null",
        "none",
        "无承办部门",
        "无部门",
        "－",
        "―",
    ];
    if SENTINELS
        .iter()
        .any(|sentinel| sentinel.eq_ignore_ascii_case(text))
    {
        return true;
    }
    text.chars()
        .all(|ch| matches!(ch, '-' | '—' | '－' | '―' | '=' | ' ' | '\t' | '\n' | '\r'))
}

fn is_id_prefix_char(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

fn split_prefix_digits(raw: &str) -> Option<(&str, &str)> {
    let mut digit_start = None;
    for (idx, ch) in raw.char_indices() {
        if ch.is_ascii_digit() {
            digit_start = Some(idx);
            break;
        }
        if !is_id_prefix_char(ch) {
            return None;
        }
    }
    let digit_start = digit_start?;
    let prefix = &raw[..digit_start];
    let digits = &raw[digit_start..];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((prefix, digits))
}

/// Expand `XH2025010-XH2025011` / `XH2025003-XH2025009` into inclusive ID lists.
fn expand_object_id_range_token(raw: &str) -> Option<Vec<String>> {
    let text = raw.trim();
    if text.is_empty() || is_blank_object_identity(text) {
        return None;
    }
    let hyphen = text.char_indices().find_map(|(idx, ch)| {
        if matches!(ch, '-' | '–' | '—' | '－') {
            Some((idx, ch.len_utf8()))
        } else {
            None
        }
    })?;
    let left = text[..hyphen.0].trim();
    let right = text[hyphen.0 + hyphen.1..].trim();
    let (prefix_left, digits_left) = split_prefix_digits(left)?;
    let (prefix_right, digits_right) = split_prefix_digits(right)?;
    if !prefix_right.is_empty() && prefix_right != prefix_left {
        return None;
    }
    let start: u128 = digits_left.parse().ok()?;
    let end: u128 = digits_right.parse().ok()?;
    if end < start || end - start > 200 {
        return None;
    }
    let width = digits_left.len();
    let mut out = Vec::with_capacity((end - start + 1) as usize);
    for n in start..=end {
        out.push(format!("{prefix_left}{n:0width$}"));
    }
    Some(out)
}

fn strip_numbered_prefix(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(dot) = trimmed.find('.') {
        let head = &trimmed[..dot];
        if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) {
            return trimmed[dot + 1..].trim_start();
        }
    }
    trimmed
}

/// Split Excel multi-value association IDs (顿号 / comma / whitespace / hyphen ranges).
pub fn split_multi_object_keys(raw: &str) -> Vec<String> {
    let text = raw.trim();
    if text.is_empty() || is_blank_object_identity(text) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for part in text.split(|ch| {
        matches!(
            ch,
            '\n' | '\r' | ' ' | '\t' | '、' | '，' | ',' | ';' | '；'
        )
    }) {
        let cleaned = strip_numbered_prefix(part)
            .trim_matches(|ch| ch == '《' || ch == '》')
            .trim();
        if cleaned.is_empty() || is_blank_object_identity(cleaned) {
            continue;
        }
        let keys = expand_object_id_range_token(cleaned)
            .unwrap_or_else(|| vec![cleaned.to_string()]);
        for key in keys {
            if key.is_empty() || is_blank_object_identity(&key) {
                continue;
            }
            if seen.insert(key.clone()) {
                out.push(key);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_sentinels_and_ranges() {
        assert!(is_blank_object_identity("——"));
        assert!(is_blank_object_identity("-"));
        assert!(split_multi_object_keys("——").is_empty());
        assert_eq!(normalize_object_keys_cell("——"), "");
        assert_eq!(
            split_multi_object_keys("XH2025010-XH2025011"),
            vec!["XH2025010".to_string(), "XH2025011".to_string()]
        );
        assert_eq!(
            normalize_object_keys_cell("XH2025010-XH2025011"),
            "XH2025010、XH2025011"
        );
        assert_eq!(
            normalize_object_keys_cell("XH2025001、XH2025002"),
            "XH2025001、XH2025002"
        );
        assert_eq!(split_multi_object_keys("XH2025003-XH2025009").len(), 7);
        assert_eq!(
            split_multi_object_keys("XH2025001、XH2025002"),
            vec!["XH2025001".to_string(), "XH2025002".to_string()]
        );
        assert_eq!(
            split_multi_object_keys("1. XH2025012\n——"),
            vec!["XH2025012".to_string()]
        );
    }
}
