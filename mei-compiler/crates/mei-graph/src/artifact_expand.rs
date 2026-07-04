use std::collections::BTreeMap;
use std::path::Path;

use mei_syntax::v2::{parse_v2_source, BinOp, CallArgs, V2Expr, V2Item};
use serde_json::{Map, Value as JsonValue};

use crate::expand::{expand_artifact_expr, ExpandError};
use crate::registry::{normalize_template_path, MacroRegistry};

pub fn expr_to_json(expr: &V2Expr) -> Result<JsonValue, ExpandError> {
    match expr {
        V2Expr::String(s) => Ok(JsonValue::String(s.clone())),
        V2Expr::Number(n) => {
            if n.fract() == 0.0 {
                Ok(JsonValue::Number(serde_json::Number::from(*n as i64)))
            } else {
                serde_json::Number::from_f64(*n)
                    .map(JsonValue::Number)
                    .ok_or_else(|| ExpandError::Expand("invalid number".into()))
            }
        }
        V2Expr::Bool(b) => Ok(JsonValue::Bool(*b)),
        V2Expr::None => Ok(JsonValue::Null),
        V2Expr::VarRef(name) => Ok(JsonValue::Object(Map::from_iter([(
            "__var".to_string(),
            JsonValue::String(name.clone()),
        )]))),
        V2Expr::BinOp { op, left, right } => Ok(JsonValue::Object(Map::from_iter([
            (
                "__binop".to_string(),
                JsonValue::String(format!("{op:?}")),
            ),
            ("left".to_string(), expr_to_json(left)?),
            ("right".to_string(), expr_to_json(right)?),
        ]))),
        V2Expr::List(items) => Ok(JsonValue::Array(
            items
                .iter()
                .map(expr_to_json)
                .collect::<Result<_, _>>()?,
        )),
        V2Expr::Dict(entries) => {
            let mut map = Map::new();
            for (k, v) in entries {
                map.insert(k.clone(), expr_to_json(v)?);
            }
            Ok(JsonValue::Object(map))
        }
        V2Expr::Call { path, args } => {
            let mut map = Map::new();
            map.insert(
                "__call".to_string(),
                JsonValue::String(path.join(".")),
            );
            map.insert("__args".to_string(), call_args_to_json(args)?);
            Ok(JsonValue::Object(map))
        }
        V2Expr::RefCall { name, args } => {
            let mut map = Map::new();
            map.insert("__ref".to_string(), JsonValue::String(name.clone()));
            map.insert("__args".to_string(), call_args_to_json(args)?);
            Ok(JsonValue::Object(map))
        }
        V2Expr::Member { object, field } => Ok(JsonValue::Object(Map::from_iter([
            ("__member".to_string(), JsonValue::String(field.clone())),
            ("base".to_string(), expr_to_json(object)?),
        ]))),
        V2Expr::ForIn { .. } | V2Expr::EnumMatch { .. } => Err(ExpandError::Expand(
            "for/enum must be expanded before artifact json roundtrip".into(),
        )),
    }
}

pub fn json_to_expr(value: &JsonValue) -> Result<V2Expr, ExpandError> {
    match value {
        JsonValue::String(s) => Ok(V2Expr::String(s.clone())),
        JsonValue::Number(n) => Ok(V2Expr::Number(n.as_f64().unwrap_or(0.0))),
        JsonValue::Bool(b) => Ok(V2Expr::Bool(*b)),
        JsonValue::Null => Ok(V2Expr::None),
        JsonValue::Array(items) => Ok(V2Expr::List(
            items
                .iter()
                .map(json_to_expr)
                .collect::<Result<_, _>>()?,
        )),
        JsonValue::Object(map) => {
            if let Some(name) = map.get("__var").and_then(JsonValue::as_str) {
                return Ok(V2Expr::VarRef(name.to_string()));
            }
            if let Some(op) = map.get("__binop").and_then(JsonValue::as_str) {
                let left = map
                    .get("left")
                    .ok_or_else(|| ExpandError::Expand("binop missing left".into()))?;
                let right = map
                    .get("right")
                    .ok_or_else(|| ExpandError::Expand("binop missing right".into()))?;
                let op = match op {
                    "Add" => BinOp::Add,
                    "Merge" => BinOp::Merge,
                    other => {
                        return Err(ExpandError::Expand(format!(
                            "unsupported binop `{other}`"
                        )))
                    }
                };
                return Ok(V2Expr::BinOp {
                    op,
                    left: Box::new(json_to_expr(left)?),
                    right: Box::new(json_to_expr(right)?),
                });
            }
            if let Some(call) = map.get("__call").and_then(JsonValue::as_str) {
                let args = map
                    .get("__args")
                    .ok_or_else(|| ExpandError::Expand("call missing __args".into()))?;
                return Ok(V2Expr::Call {
                    path: call.split('.').map(str::to_string).collect(),
                    args: json_to_call_args(args)?,
                });
            }
            if let Some(name) = map.get("__ref").and_then(JsonValue::as_str) {
                let args = map
                    .get("__args")
                    .ok_or_else(|| ExpandError::Expand("ref call missing __args".into()))?;
                return Ok(V2Expr::RefCall {
                    name: name.to_string(),
                    args: json_to_call_args(args)?,
                });
            }
            if let Some(field) = map.get("__member").and_then(JsonValue::as_str) {
                let base = map
                    .get("base")
                    .ok_or_else(|| ExpandError::Expand("member missing base".into()))?;
                return Ok(V2Expr::Member {
                    object: Box::new(json_to_expr(base)?),
                    field: field.to_string(),
                });
            }
            let mut entries = Vec::new();
            for (k, v) in map {
                entries.push((k.clone(), json_to_expr(v)?));
            }
            Ok(V2Expr::Dict(entries))
        }
    }
}

fn call_args_to_json(args: &CallArgs) -> Result<JsonValue, ExpandError> {
    let mut map = Map::new();
    for (idx, expr) in args.positional.iter().enumerate() {
        map.insert(format!("arg{idx}"), expr_to_json(expr)?);
    }
    for (name, expr) in &args.keywords {
        map.insert(name.clone(), expr_to_json(expr)?);
    }
    Ok(JsonValue::Object(map))
}

fn json_to_call_args(value: &JsonValue) -> Result<CallArgs, ExpandError> {
    let map = value
        .as_object()
        .ok_or_else(|| ExpandError::Expand("call args must be object".into()))?;
    let mut positional = Vec::new();
    let mut keywords = Vec::new();
    for (key, child) in map {
        if let Some(idx) = key.strip_prefix("arg").and_then(|s| s.parse::<usize>().ok()) {
            if idx == positional.len() {
                positional.push(json_to_expr(child)?);
            } else {
                keywords.push((key.clone(), json_to_expr(child)?));
            }
        } else {
            keywords.push((key.clone(), json_to_expr(child)?));
        }
    }
    Ok(CallArgs {
        positional,
        keywords,
    })
}

pub fn collect_template_imports(stock_templates: &Path) -> BTreeMap<String, String> {
    let mut imports = BTreeMap::new();
    if !stock_templates.is_dir() {
        return imports;
    }
    for entry in walkdir::WalkDir::new(stock_templates)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "mei"))
    {
        let Ok(source) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(file) = parse_v2_source(&source) else {
            continue;
        };
        for item in file.items {
            if let V2Item::UseTemplate { path, alias } = item {
                let norm = normalize_template_path(&path);
                let import_name = alias.unwrap_or_else(|| {
                    norm.rsplit('/').next().unwrap_or(norm.as_str()).to_string()
                });
                imports.insert(import_name, norm);
            }
        }
    }
    imports
}

pub fn expand_artifact_value(
    value: &JsonValue,
    registry: &MacroRegistry,
    imports: &BTreeMap<String, String>,
) -> Result<JsonValue, ExpandError> {
    let expr = json_to_expr(value)?;
    let expanded = expand_artifact_expr(&expr, registry, imports, &BTreeMap::new())?;
    expr_to_json(&expanded)
}

pub fn try_expand_artifact_macro_call(
    value: &JsonValue,
    registry: &MacroRegistry,
    imports: &BTreeMap<String, String>,
) -> Option<JsonValue> {
    let call = value.as_object()?.get("__call")?.as_str()?;
    if matches!(call, "panel" | "component" | "metric_card") {
        return None;
    }
    let expanded = expand_artifact_value(value, registry, imports).ok()?;
    if expanded == *value {
        return None;
    }
    Some(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip_preserves_panel_call() {
        let value = serde_json::json!({
            "__call": "panel",
            "__args": {
                "id": "demo",
                "blocks": []
            }
        });
        let expr = json_to_expr(&value).expect("to expr");
        let back = expr_to_json(&expr).expect("to json");
        assert_eq!(back, value);
    }
}
