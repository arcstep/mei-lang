//! Parse panel `.panel.mei` module constants via mei-syntax v2 (supports multi-line objects and ref calls).

use std::collections::BTreeMap;

use mei_syntax::v2::{parse_v2_source, CallArgs, V2Expr, V2Item};
use serde_json::{json, Map, Number, Value};

pub fn parse_panel_constants_from_source(content: &str) -> BTreeMap<String, Value> {
    let file = match parse_v2_source(content) {
        Ok(file) => file,
        Err(_) => return BTreeMap::new(),
    };

    let mut const_exprs = BTreeMap::new();
    for item in &file.items {
        if let V2Item::ModuleConst { name, value } = item {
            const_exprs.insert(name.clone(), value.clone());
        }
    }

    let mut evaluated_exprs = BTreeMap::new();
    for (name, expr) in &const_exprs {
        if let Ok(evaluated) = eval_panel_const_expr(expr, &const_exprs, &evaluated_exprs) {
            evaluated_exprs.insert(name.clone(), evaluated);
        }
    }

    let mut out = BTreeMap::new();
    for (name, expr) in evaluated_exprs {
        if let Ok(value) = panel_expr_to_json(&expr) {
            out.insert(name, value);
        }
    }
    out
}

fn eval_panel_const_expr(
    expr: &V2Expr,
    all_consts: &BTreeMap<String, V2Expr>,
    evaluated: &BTreeMap<String, V2Expr>,
) -> Result<V2Expr, String> {
    match expr {
        V2Expr::VarRef(name) => evaluated
            .get(name)
            .cloned()
            .or_else(|| all_consts.get(name).cloned())
            .and_then(|inner| eval_panel_const_expr(&inner, all_consts, evaluated).ok())
            .ok_or_else(|| format!("unbound panel const `{name}`")),
        V2Expr::BinOp {
            op: mei_syntax::v2::BinOp::Add,
            left,
            right,
        } => {
            let left = eval_panel_const_expr(left, all_consts, evaluated)?;
            let right = eval_panel_const_expr(right, all_consts, evaluated)?;
            match (left, right) {
                (V2Expr::String(a), V2Expr::String(b)) => Ok(V2Expr::String(format!("{a}{b}"))),
                (l, r) => Ok(V2Expr::BinOp {
                    op: mei_syntax::v2::BinOp::Add,
                    left: Box::new(l),
                    right: Box::new(r),
                }),
            }
        }
        V2Expr::List(items) => Ok(V2Expr::List(
            items
                .iter()
                .map(|item| eval_panel_const_expr(item, all_consts, evaluated))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        V2Expr::Dict(entries) => {
            let mut out = Vec::new();
            for (key, value) in entries {
                out.push((
                    key.clone(),
                    eval_panel_const_expr(value, all_consts, evaluated)?,
                ));
            }
            Ok(V2Expr::Dict(out))
        }
        V2Expr::RefCall { .. } | V2Expr::Call { .. } => Ok(expr.clone()),
        other => Ok(other.clone()),
    }
}

fn call_args_to_json(args: &CallArgs) -> Result<Value, String> {
    let mut map = Map::new();
    for (idx, expr) in args.positional.iter().enumerate() {
        map.insert(format!("arg{idx}"), panel_expr_to_json(expr)?);
    }
    for (name, expr) in &args.keywords {
        map.insert(name.clone(), panel_expr_to_json(expr)?);
    }
    Ok(Value::Object(map))
}

fn panel_expr_to_json(expr: &V2Expr) -> Result<Value, String> {
    match expr {
        V2Expr::String(s) => Ok(json!(s)),
        V2Expr::Number(n) => {
            if n.fract() == 0.0 {
                Ok(Value::Number(Number::from(*n as i64)))
            } else {
                Number::from_f64(*n)
                    .map(Value::Number)
                    .ok_or_else(|| "invalid number".to_string())
            }
        }
        V2Expr::Bool(b) => Ok(Value::Bool(*b)),
        V2Expr::None => Ok(Value::Null),
        V2Expr::VarRef(name) => Ok(json!({ "__var": name })),
        V2Expr::BinOp { op, left, right } => Ok(json!({
            "__binop": format!("{op:?}"),
            "left": panel_expr_to_json(left)?,
            "right": panel_expr_to_json(right)?,
        })),
        V2Expr::List(items) => Ok(Value::Array(
            items
                .iter()
                .map(panel_expr_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        V2Expr::Dict(entries) => {
            let mut map = Map::new();
            for (key, value) in entries {
                map.insert(key.clone(), panel_expr_to_json(value)?);
            }
            Ok(Value::Object(map))
        }
        V2Expr::Call { path, args } => Ok(json!({
            "__call": path.join("."),
            "__args": call_args_to_json(args)?,
        })),
        V2Expr::RefCall { name, args } => Ok(json!({
            "__ref": name,
            "__args": call_args_to_json(args)?,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gis_map_panel_constants_with_refs() {
        let src = r#"
GEO_OUTLINE = ops_param_ref("shapingba_outline_url")
MAP_BUNDLE = "metrics/map.bundle.mei"
MAP_SPEC = {
    "basemap": basemap_ref("shapingba"),
    "layers": [
        {
            "id": "district_outline",
            "url": GEO_OUTLINE,
        },
    ],
}
panel_contract(id = "gis-map")
"#;
        let constants = parse_panel_constants_from_source(src);
        assert_eq!(
            constants.get("MAP_BUNDLE"),
            Some(&json!("metrics/map.bundle.mei"))
        );
        let outline = constants.get("GEO_OUTLINE").expect("GEO_OUTLINE");
        assert_eq!(outline["__ref"], json!("ops_param_ref"));
        let map_spec = constants.get("MAP_SPEC").expect("MAP_SPEC");
        assert!(
            map_spec["basemap"].get("__ref").and_then(Value::as_str) == Some("basemap_ref")
                || map_spec["basemap"].get("__call").and_then(Value::as_str) == Some("basemap_ref")
        );
        let url = &map_spec["layers"][0]["url"];
        assert!(
            url.get("__ref").and_then(Value::as_str) == Some("ops_param_ref")
                || url.get("__call").and_then(Value::as_str) == Some("ops_param_ref")
        );
    }
}
