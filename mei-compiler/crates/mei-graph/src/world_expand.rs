//! Compile-time expansion of `for` / `enum` in `*.world.mei` before lower.

use std::collections::BTreeMap;
use std::path::Path;

use mei_syntax::v2::{CallArgs, V2Expr, V2Item, V2SourceFile};
use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorldExpandError {
    #[error("{0}")]
    Expand(String),
}

#[derive(Debug, Clone, Default)]
pub struct WorldContextCatalog {
    pub datasets: BTreeMap<String, Vec<Map<String, Value>>>,
}

impl WorldContextCatalog {
    pub fn load_from_app(app_root: &Path) -> Self {
        let mut out = Self::default();
        for dir in [
            app_root.join("src/context"),
            app_root.join("assets/context"),
        ] {
            if !dir.is_dir() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let Some(id) = value.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(rows) = value.get("rows").and_then(|v| v.as_array()) else {
                    continue;
                };
                let parsed = rows
                    .iter()
                    .filter_map(|row| row.as_object().cloned())
                    .collect::<Vec<_>>();
                out.datasets.insert(id.to_string(), parsed);
            }
        }
        out
    }
}

pub fn expand_world_v2_file(
    file: &V2SourceFile,
    catalog: &WorldContextCatalog,
) -> Result<V2SourceFile, WorldExpandError> {
    let mut items = Vec::new();
    for item in &file.items {
        items.push(expand_world_item(item, catalog)?);
    }
    Ok(V2SourceFile { items })
}

fn expand_world_item(
    item: &V2Item,
    catalog: &WorldContextCatalog,
) -> Result<V2Item, WorldExpandError> {
    match item {
        V2Item::TopLevel { name, args } if name == "world" => Ok(V2Item::TopLevel {
            name: name.clone(),
            args: expand_world_call_args(args, catalog)?,
        }),
        other => Ok(other.clone()),
    }
}

fn expand_world_call_args(
    args: &CallArgs,
    catalog: &WorldContextCatalog,
) -> Result<CallArgs, WorldExpandError> {
    let mut positional = Vec::new();
    for expr in &args.positional {
        positional.extend(expand_world_expr(expr, catalog)?);
    }
    let mut keywords = Vec::new();
    for (name, expr) in &args.keywords {
        let expanded = expand_world_expr(expr, catalog)?;
        if expanded.len() == 1 {
            keywords.push((name.clone(), expanded[0].clone()));
        } else {
            return Err(WorldExpandError::Expand(format!(
                "keyword `{name}` expanded to multiple expressions; use positional for/for blocks"
            )));
        }
    }
    Ok(CallArgs {
        positional,
        keywords,
    })
}

fn expand_world_expr(
    expr: &V2Expr,
    catalog: &WorldContextCatalog,
) -> Result<Vec<V2Expr>, WorldExpandError> {
    match expr {
        V2Expr::ForIn { var, source, body } => {
            let rows = resolve_dataset_rows(source, catalog)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(substitute_row_vars(body, var, &row)?);
            }
            Ok(out)
        }
        V2Expr::EnumMatch {
            subject,
            cases,
            default,
        } => {
            let key = eval_match_key(subject)?;
            for (case_key, body) in cases {
                if eval_match_key(case_key)? == key {
                    return expand_world_expr(body, catalog);
                }
            }
            if let Some(body) = default {
                return expand_world_expr(body, catalog);
            }
            Ok(Vec::new())
        }
        other => Ok(vec![other.clone()]),
    }
}

fn resolve_dataset_rows(
    source: &V2Expr,
    catalog: &WorldContextCatalog,
) -> Result<Vec<Map<String, Value>>, WorldExpandError> {
    let dataset_id = resolve_ref_id(source, &["dataset_ref", "dataframe_ref"])?;
    catalog.datasets.get(&dataset_id).cloned().ok_or_else(|| {
        WorldExpandError::Expand(format!(
            "dataset `{dataset_id}` not found in context catalog (src/context or assets/context)"
        ))
    })
}

fn resolve_ref_id(expr: &V2Expr, allowed: &[&str]) -> Result<String, WorldExpandError> {
    match expr {
        V2Expr::RefCall { name, args } if allowed.contains(&name.as_str()) => {
            let id = args
                .keywords
                .iter()
                .find(|(k, _)| k == "id" || k == "key")
                .and_then(|(_, v)| match v {
                    V2Expr::String(s) => Some(s.clone()),
                    _ => None,
                })
                .or_else(|| {
                    args.positional.first().and_then(|v| match v {
                        V2Expr::String(s) => Some(s.clone()),
                        _ => None,
                    })
                });
            id.ok_or_else(|| WorldExpandError::Expand(format!("{name} requires id")))
        }
        V2Expr::String(s) => Ok(s.clone()),
        _ => Err(WorldExpandError::Expand(
            "expected dataset_ref(id = \"...\")".into(),
        )),
    }
}

fn eval_match_key(expr: &V2Expr) -> Result<String, WorldExpandError> {
    match expr {
        V2Expr::String(s) => Ok(s.clone()),
        V2Expr::VarRef(name) => Ok(name.clone()),
        V2Expr::Member { object, field } => {
            if let V2Expr::VarRef(row_name) = object.as_ref() {
                return Ok(format!("{row_name}.{field}"));
            }
            Err(WorldExpandError::Expand(
                "enum subject must be string literal or row.field".into(),
            ))
        }
        _ => Err(WorldExpandError::Expand(
            "enum subject must be string literal or row.field".into(),
        )),
    }
}

fn substitute_row_vars(
    expr: &V2Expr,
    row_var: &str,
    row: &Map<String, Value>,
) -> Result<V2Expr, WorldExpandError> {
    match expr {
        V2Expr::Member { object, field } => {
            if let V2Expr::VarRef(name) = object.as_ref() {
                if name == row_var {
                    return value_to_expr(row.get(field).unwrap_or(&Value::Null));
                }
            }
            Ok(V2Expr::Member {
                object: Box::new(substitute_row_vars(object, row_var, row)?),
                field: field.clone(),
            })
        }
        V2Expr::Call { path, args } => Ok(V2Expr::Call {
            path: path.clone(),
            args: substitute_call_args(args, row_var, row)?,
        }),
        V2Expr::RefCall { name, args } => Ok(V2Expr::RefCall {
            name: name.clone(),
            args: substitute_call_args(args, row_var, row)?,
        }),
        V2Expr::List(items) => Ok(V2Expr::List(
            items
                .iter()
                .map(|item| substitute_row_vars(item, row_var, row))
                .collect::<Result<_, _>>()?,
        )),
        V2Expr::Dict(entries) => Ok(V2Expr::Dict(
            entries
                .iter()
                .map(|(k, v)| Ok((k.clone(), substitute_row_vars(v, row_var, row)?)))
                .collect::<Result<_, _>>()?,
        )),
        V2Expr::BinOp { op, left, right } => Ok(V2Expr::BinOp {
            op: *op,
            left: Box::new(substitute_row_vars(left, row_var, row)?),
            right: Box::new(substitute_row_vars(right, row_var, row)?),
        }),
        V2Expr::ForIn { .. } | V2Expr::EnumMatch { .. } => Err(WorldExpandError::Expand(
            "nested for/enum inside for body is not supported in v1".into(),
        )),
        other => Ok(other.clone()),
    }
}

fn substitute_call_args(
    args: &CallArgs,
    row_var: &str,
    row: &Map<String, Value>,
) -> Result<CallArgs, WorldExpandError> {
    Ok(CallArgs {
        positional: args
            .positional
            .iter()
            .map(|e| substitute_row_vars(e, row_var, row))
            .collect::<Result<_, _>>()?,
        keywords: args
            .keywords
            .iter()
            .map(|(k, v)| Ok((k.clone(), substitute_row_vars(v, row_var, row)?)))
            .collect::<Result<_, _>>()?,
    })
}

fn value_to_expr(value: &Value) -> Result<V2Expr, WorldExpandError> {
    match value {
        Value::String(s) => Ok(V2Expr::String(s.clone())),
        Value::Number(n) => Ok(V2Expr::Number(n.as_f64().unwrap_or(0.0))),
        Value::Bool(b) => Ok(V2Expr::Bool(*b)),
        Value::Null => Ok(V2Expr::None),
        _ => Err(WorldExpandError::Expand(
            "row field must be scalar for substitution in v1".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_syntax::v2::parse_world_v2_source;

    #[test]
    fn expand_for_in_buildings_from_catalog() {
        let mut catalog = WorldContextCatalog::default();
        catalog.datasets.insert(
            "building_catalog".to_string(),
            vec![Map::from_iter([
                ("entity_id".to_string(), Value::String("play_zone".into())),
                ("height".to_string(), Value::from(14.0)),
                ("shell_color".to_string(), Value::String("#f472b6".into())),
                ("shell_opacity".to_string(), Value::from(0.88)),
                ("fill_opacity".to_string(), Value::from(0.86)),
                ("shell_lift".to_string(), Value::from(0.038)),
            ])],
        );
        let source = r#"
world(
    id = "park_world",
    for row in dataset_ref(id = "building_catalog") {
        building(
            id = row.entity_id,
            height = row.height,
            shell = surface(color = row.shell_color, opacity = row.shell_opacity),
            footprint = feature_ref("footprint", entity_id = row.entity_id),
            map_view = fill_extrusion(fill_opacity = row.fill_opacity),
            world_view = footprint_shell(lift = row.shell_lift),
        )
    },
)
"#;
        let parsed = parse_world_v2_source(source).expect("parse");
        let expanded = expand_world_v2_file(&parsed, &catalog).expect("expand");
        let world = expanded
            .items
            .iter()
            .find_map(|item| match item {
                V2Item::TopLevel { name, args } if name == "world" => Some(args),
                _ => None,
            })
            .expect("world");
        assert_eq!(world.positional.len(), 1);
        match &world.positional[0] {
            V2Expr::Call { path, .. } => assert_eq!(path, &["building".to_string()]),
            other => panic!("expected building call, got {other:?}"),
        }
    }
}
