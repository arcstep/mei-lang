use std::collections::BTreeMap;

use serde_json::Value as JsonValue;

pub type ObjectMap = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(ObjectMap),
}

impl Value {
    pub fn null() -> Self {
        Self::Null
    }

    pub fn string(text: impl Into<String>) -> Self {
        Self::String(text.into())
    }

    pub fn object(pairs: Vec<(&str, Value)>) -> Self {
        let mut map = ObjectMap::new();
        for (key, value) in pairs {
            map.insert(key.to_string(), value);
        }
        Self::Object(map)
    }

    pub fn into_json(self) -> JsonValue {
        match self {
            Value::Null => JsonValue::Null,
            Value::Bool(value) => JsonValue::Bool(value),
            Value::Number(value) => {
                if (value - value.round()).abs() < f64::EPSILON {
                    JsonValue::Number((value as i64).into())
                } else {
                    serde_json::Number::from_f64(value)
                        .map(JsonValue::Number)
                        .unwrap_or(JsonValue::Null)
                }
            }
            Value::String(value) => JsonValue::String(value),
            Value::Array(values) => {
                JsonValue::Array(values.into_iter().map(Value::into_json).collect())
            }
            Value::Object(map) => JsonValue::Object(
                map.into_iter()
                    .map(|(key, value)| (key, value.into_json()))
                    .collect(),
            ),
        }
    }
}

pub fn without_empty(values: ObjectMap) -> ObjectMap {
    values
        .into_iter()
        .filter(|(_, value)| match value {
            Value::Null => false,
            Value::Bool(false) => false,
            _ => true,
        })
        .collect()
}

pub fn optional(value: Option<Value>) -> Value {
    value.unwrap_or(Value::Null)
}

pub fn clean_object(pairs: Vec<(&str, Value)>) -> ObjectMap {
    without_empty(Value::object(pairs).as_object().cloned().unwrap_or_default())
}

impl Value {
    pub fn as_object(&self) -> Option<&ObjectMap> {
        match self {
            Value::Object(map) => Some(map),
            _ => None,
        }
    }
}

pub fn empty_object() -> Value {
    Value::Object(ObjectMap::new())
}
