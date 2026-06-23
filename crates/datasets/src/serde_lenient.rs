//! Lenient JSON deserializers for browser-originated dataset query payloads.

use serde::{de::Error, Deserialize, Deserializer};

fn parse_usize_text(text: &str) -> Option<usize> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<usize>().ok()
}

fn parse_i64_text(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<i64>().ok()
}

fn parse_bool_text(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub fn opt_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_u64()
            .map(|value| Some(value as usize))
            .ok_or_else(|| Error::custom("expected non-negative integer")),
        Some(serde_json::Value::String(text)) => Ok(parse_usize_text(&text)),
        _ => Err(Error::custom("expected number, string, or null")),
    }
}

pub fn opt_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_i64()
            .map(Some)
            .ok_or_else(|| Error::custom("expected integer")),
        Some(serde_json::Value::String(text)) => Ok(parse_i64_text(&text)),
        _ => Err(Error::custom("expected number, string, or null")),
    }
}

pub fn opt_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Bool(flag)) => Ok(Some(flag)),
        Some(serde_json::Value::Number(number)) => {
            if let Some(value) = number.as_u64() {
                Ok(Some(value != 0))
            } else if let Some(value) = number.as_i64() {
                Ok(Some(value != 0))
            } else {
                Err(Error::custom("expected boolean-compatible number"))
            }
        }
        Some(serde_json::Value::String(text)) => parse_bool_text(&text)
            .map(Some)
            .ok_or_else(|| Error::custom("expected boolean string")),
        _ => Err(Error::custom("expected boolean, number, string, or null")),
    }
}

pub fn bool_default_false<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    opt_bool(deserializer).map(|value| value.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Probe {
        #[serde(default, deserialize_with = "opt_usize")]
        page: Option<usize>,
        #[serde(default, deserialize_with = "bool_default_false")]
        full: bool,
    }

    #[test]
    fn deserializes_string_page_and_bool() {
        let parsed: Probe = serde_json::from_value(serde_json::json!({
            "page": "2",
            "full": "true"
        }))
        .expect("probe");
        assert_eq!(parsed.page, Some(2));
        assert!(parsed.full);
    }
}
