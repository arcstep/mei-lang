//! Lenient JSON deserializers for browser-originated dataset query payloads.

use std::collections::BTreeMap;

use serde::{de::Error, Deserialize, Deserializer};

fn json_value_to_string(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => {
            let trimmed = text.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        }
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        serde_json::Value::Array(items) => {
            let values = items
                .into_iter()
                .filter_map(json_value_to_string)
                .collect::<Vec<_>>();
            (!values.is_empty()).then(|| values.join(","))
        }
        serde_json::Value::Object(object) => serde_json::to_string(&object).ok(),
    }
}

fn parse_usize_text(text: &str) -> Option<usize> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<usize>().ok()
}

pub fn string_map<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(serde_json::Value::Object(object)) = value else {
        return Ok(BTreeMap::new());
    };
    Ok(object
        .into_iter()
        .filter_map(|(key, value)| {
            let normalized_key = key.trim().to_string();
            if normalized_key.is_empty() {
                return None;
            }
            json_value_to_string(value).map(|value| (normalized_key, value))
        })
        .collect())
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

    #[test]
    fn deserializes_scalar_filter_map_values() {
        #[derive(Debug, Deserialize)]
        struct FilterProbe {
            #[serde(default, deserialize_with = "string_map")]
            filters: BTreeMap<String, String>,
        }

        let parsed: FilterProbe = serde_json::from_value(serde_json::json!({
            "filters": {
                "year": 2025,
                "active": true,
                "district": ["沙坪坝区", 500106]
            }
        }))
        .expect("filters");

        assert_eq!(parsed.filters.get("year"), Some(&"2025".to_string()));
        assert_eq!(parsed.filters.get("active"), Some(&"true".to_string()));
        assert_eq!(
            parsed.filters.get("district"),
            Some(&"沙坪坝区,500106".to_string())
        );
    }
}
