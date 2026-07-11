use std::path::Path;

use mei_syntax::{parse_source, CallArgs, Expr, SourceFile, TopLevelCall};
use serde_json::Value as JsonValue;
use thiserror::Error;

use crate::builtins;
use crate::value::{ObjectMap, Value};

#[derive(Debug, Error)]
pub enum LowerError {
    #[error("parse error: {0}")]
    Parse(#[from] mei_syntax::ParseError),
    #[error("{0}")]
    Surface(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct LowerOutcome {
    pub decl_ir_schema_version: &'static str,
    pub exports: Vec<JsonValue>,
}

pub fn lower_source_file(path: &Path) -> Result<LowerOutcome, LowerError> {
    let source = std::fs::read_to_string(path)?;
    lower_source(&source)
}

pub fn lower_source(source: &str) -> Result<LowerOutcome, LowerError> {
    let file = parse_source(source)?;
    lower_file(&file)
}

pub fn lower_file(file: &SourceFile) -> Result<LowerOutcome, LowerError> {
    let mut exports = Vec::new();
    for stmt in &file.statements {
        let value = lower_top_level(stmt)?;
        exports.push(value.into_json());
    }
    Ok(LowerOutcome {
        decl_ir_schema_version: "mei-compiler-decl-v0",
        exports,
    })
}

fn lower_top_level(stmt: &TopLevelCall) -> Result<Value, LowerError> {
    let name = desugar_call_name(&stmt.path);
    let value = dispatch_call(&name, &stmt.args).map_err(LowerError::Surface)?;
    Ok(value)
}

pub fn desugar_call_name(path: &[String]) -> String {
    if path.len() >= 2 {
        match (path[0].as_str(), path[1].as_str()) {
            ("frame", "add_panel") => return "panel_decl".to_string(),
            ("doc", name) => return name.to_string(),
            ("ds", name) => return name.to_string(),
            ("ui", name) => return name.to_string(),
            ("app", "add_scene") => return "app_add_scene".to_string(),
            ("scene", "set_world") => return "world".to_string(),
            ("scene", "set_flow") => return "flow".to_string(),
            ("scene", "set_frame") => return "frame".to_string(),
            ("world", "add_resource") => return "world_add_resource".to_string(),
            ("world", "add_dataset") => return "world_add_dataset".to_string(),
            ("world", "add_dataset_view") => return "world_add_dataset_view".to_string(),
            ("world", "add_metric") => return "world_add_metric".to_string(),
            ("world", "add_metric_pack") => return "world_add_metric_pack".to_string(),
            ("world", "add_entity") => return "world_add_entity".to_string(),
            ("world", "set_topology") => return "world_set_topology".to_string(),
            ("frame", "set_layout") => return "frame_set_layout".to_string(),
            _ => {}
        }
    }
    path.join(".")
}

fn dispatch_call(name: &str, args: &CallArgs) -> Result<Value, String> {
    match name {
        "app" => builtins::app(args),
        "scene" | "scene_decl" => builtins::scene(args),
        "world" => builtins::world(args),
        "frame" => builtins::frame(args),
        "panel_decl" => builtins::panel_decl(args),
        "scene_ref" => builtins::scene_ref(args),
        "flex" => builtins::flex(args),
        "markdown" => builtins::markdown(args),
        other => Err(format!(
            "unsupported surface call `{other}` in mei-compiler v0"
        )),
    }
}

pub fn expr_to_value(expr: &Expr) -> Result<Value, String> {
    match expr {
        Expr::String(text) => Ok(Value::String(unescape_string(text))),
        Expr::Number(number) => Ok(Value::Number(*number)),
        Expr::Bool(value) => Ok(Value::Bool(*value)),
        Expr::None => Ok(Value::Null),
        Expr::List(items) => items
            .iter()
            .map(expr_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Expr::Call { path, args } => {
            let name = desugar_call_name(path);
            dispatch_call(&name, args)
        }
    }
}

pub fn keyword_map(args: &CallArgs) -> Result<ObjectMap, String> {
    let mut map = ObjectMap::new();
    for (name, expr) in &args.keywords {
        map.insert(name.clone(), expr_to_value(expr)?);
    }
    Ok(map)
}

fn unescape_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}
