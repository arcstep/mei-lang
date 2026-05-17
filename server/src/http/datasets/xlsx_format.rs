use calamine::Data;
use serde_json::Value;

use super::util::value_to_text;

pub(crate) fn xlsx_cell(value: &Data) -> Value {
    match value {
        Data::Empty => Value::Null,
        Data::String(text) => Value::String(text.clone()),
        Data::Float(number) => serde_json::json!(*number),
        Data::Int(integer) => serde_json::json!(*integer),
        Data::Bool(flag) => Value::Bool(*flag),
        Data::DateTime(date) => Value::String(date.to_string()),
        Data::DateTimeIso(text) | Data::DurationIso(text) => Value::String(text.clone()),
        Data::Error(error) => Value::String(error.to_string()),
    }
}

pub(crate) fn xlsx_header(value: &Data) -> String {
    match xlsx_cell(value) {
        Value::Null => String::new(),
        Value::String(text) => text,
        other => value_to_text(&other),
    }
}
