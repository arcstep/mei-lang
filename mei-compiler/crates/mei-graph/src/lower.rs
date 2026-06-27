use mei_syntax::v2::{CallArgs, V2Expr, V2Item, V2SourceFile};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LowerGraphError {
    #[error("{0}")]
    Lower(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphBlock {
    pub kind: String,
    pub block_id: String,
    pub schema: String,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphOutcome {
    pub graph_schema_version: String,
    pub source_file: String,
    pub blocks: Vec<GraphBlock>,
}

pub fn lower_v2_file(source_file: &str, file: &V2SourceFile) -> Result<GraphOutcome, LowerGraphError> {
    let mut blocks = Vec::new();
    for item in &file.items {
        if let V2Item::TopLevel { name, args } = item {
            blocks.push(lower_top_level(name, args)?);
        }
    }
    Ok(GraphOutcome {
        graph_schema_version: "mei-compiler-graph-v2".to_string(),
        source_file: source_file.to_string(),
        blocks,
    })
}

fn lower_top_level(name: &str, args: &CallArgs) -> Result<GraphBlock, LowerGraphError> {
    let payload = call_args_to_json(args)?;
    let block_id = derive_block_id(name, &payload)?;
    let schema = schema_for_constructor(name);
    Ok(GraphBlock {
        kind: name.to_string(),
        block_id,
        schema: schema.to_string(),
        payload,
    })
}

fn schema_for_constructor(name: &str) -> &'static str {
    match name {
        "app_skeleton" => "mei-app-skeleton-artifact-v1",
        "assembly_view" | "board_assembly" => "mei-projection-assembly-v1",
        "panel_contract" => "mei-panel-contract-artifact-v1",
        "metric_def_bundle" => "mei-metric-def-bundle-artifact-v1",
        "navigation" | "link_decl" => "mei-navigation-artifact-v1",
        "warmup_policy" => "mei-warmup-policy-artifact-v1",
        _ => "mei-graph-block-v2",
    }
}

fn derive_block_id(name: &str, payload: &JsonValue) -> Result<String, LowerGraphError> {
    let obj = payload
        .as_object()
        .ok_or_else(|| LowerGraphError::Lower("payload must be object".into()))?;
    match name {
        "app_skeleton" => kw_string(obj, "id").map(|id| format!("app_skeleton:{id}")),
        "navigation" | "link_decl" => kw_string(obj, "key").map(|key| format!("{name}:{key}")),
        "assembly_view" | "board_assembly" => kw_string(obj, "key").map(|key| format!("assembly_view:{key}")),
        "panel_contract" => {
            let id = kw_string(obj, "id")?;
            if let Some(scope) = obj.get("scope").and_then(|v| v.as_str()) {
                Ok(format!("panel_contract:{scope}:{id}"))
            } else {
                Ok(format!("panel_contract:{id}"))
            }
        }
        "metric_def_bundle" => kw_string(obj, "key").map(|key| format!("metric_def_bundle:{key}")),
        "warmup_policy" => {
            let scope = obj.get("scope").cloned().unwrap_or(JsonValue::Null);
            Ok(format!("warmup_policy:{scope}"))
        }
        other => Ok(format!("{other}:anonymous")),
    }
}

fn kw_string(obj: &Map<String, JsonValue>, key: &str) -> Result<String, LowerGraphError> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| LowerGraphError::Lower(format!("missing string field `{key}`")))
}

fn call_args_to_json(args: &CallArgs) -> Result<JsonValue, LowerGraphError> {
    let mut map = Map::new();
    for (idx, expr) in args.positional.iter().enumerate() {
        map.insert(format!("arg{idx}"), expr_to_json(expr)?);
    }
    for (name, expr) in &args.keywords {
        map.insert(name.clone(), expr_to_json(expr)?);
    }
    Ok(JsonValue::Object(map))
}

fn expr_to_json(expr: &V2Expr) -> Result<JsonValue, LowerGraphError> {
    match expr {
        V2Expr::String(s) => Ok(JsonValue::String(s.clone())),
        V2Expr::Number(n) => {
            if n.fract() == 0.0 {
                Ok(JsonValue::Number(serde_json::Number::from(*n as i64)))
            } else {
                serde_json::Number::from_f64(*n)
                    .map(JsonValue::Number)
                    .ok_or_else(|| LowerGraphError::Lower("invalid number".into()))
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
    }
}
