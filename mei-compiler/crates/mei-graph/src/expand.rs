use std::collections::BTreeMap;

use mei_syntax::v2::{CallArgs, V2Expr, V2Item, V2SourceFile};
use thiserror::Error;

use crate::registry::{normalize_template_path, MacroDef, MacroRegistry, TemplateRoots};

#[derive(Debug, Error)]
pub enum ExpandError {
    #[error("parse error: {0}")]
    Parse(#[from] mei_syntax::V2ParseError),
    #[error("{0}")]
    Expand(String),
}

pub struct ExpandContext<'a> {
    pub registry: &'a MacroRegistry,
    pub imports: BTreeMap<String, String>,
    pub module_consts: BTreeMap<String, V2Expr>,
    /// `(file_path, template_name)` stack for cycle detection.
    pub expand_stack: Vec<(String, String)>,
}

pub fn expand_artifact_expr(
    expr: &V2Expr,
    registry: &MacroRegistry,
    imports: &BTreeMap<String, String>,
    module_consts: &BTreeMap<String, V2Expr>,
) -> Result<V2Expr, ExpandError> {
    let ctx = ExpandContext {
        registry,
        imports: imports.clone(),
        module_consts: module_consts.clone(),
        expand_stack: Vec::new(),
    };
    expand_expr(expr, &ctx)
}

pub fn expand_v2_file(
    file: &V2SourceFile,
    registry: &MacroRegistry,
    roots: &TemplateRoots,
) -> Result<V2SourceFile, ExpandError> {
    let mut imports = BTreeMap::new();
    let mut module_consts = BTreeMap::new();
    let mut registry_owned: Option<MacroRegistry> = None;
    for item in &file.items {
        match item {
            V2Item::UseTemplate { path, alias } => {
                let norm = normalize_template_path(path);
                let import_name = alias
                    .clone()
                    .unwrap_or_else(|| norm.rsplit('/').next().unwrap_or(&norm).to_string());
                imports.insert(import_name, norm);
                if registry.resolve_path(path).is_none() {
                    if let Some(disk) = roots.resolve_file(path) {
                        let nested = mei_syntax::v2::parse_v2_source_file(&disk)?;
                        let reg = registry_owned.get_or_insert_with(|| registry.clone());
                        reg.register_file(&normalize_template_path(path), &nested);
                    }
                }
            }
            V2Item::ModuleConst { name, value } => {
                module_consts.insert(name.clone(), eval_const_expr(value, &module_consts)?);
            }
            _ => {}
        }
    }

    let registry_ref = registry_owned.as_ref().unwrap_or(registry);
    let ctx = ExpandContext {
        registry: registry_ref,
        imports,
        module_consts,
        expand_stack: Vec::new(),
    };

    let mut out = Vec::new();
    for item in &file.items {
        match item {
            V2Item::UseTemplate { .. } | V2Item::ModuleConst { .. } => {}
            V2Item::TemplateDecl { .. } => {
                out.push(item.clone());
            }
            V2Item::TopLevel { name, args } => {
                out.push(V2Item::TopLevel {
                    name: name.clone(),
                    args: expand_call_args(args, &ctx)?,
                });
            }
        }
    }
    Ok(V2SourceFile { items: out })
}

fn expand_call_args(args: &CallArgs, ctx: &ExpandContext<'_>) -> Result<CallArgs, ExpandError> {
    Ok(CallArgs {
        positional: args
            .positional
            .iter()
            .map(|e| expand_expr(e, ctx))
            .collect::<Result<Vec<_>, ExpandError>>()?,
        keywords: args
            .keywords
            .iter()
            .map(|(k, v)| Ok((k.clone(), expand_expr(v, ctx)?)))
            .collect::<Result<Vec<_>, ExpandError>>()?,
    })
}

fn expand_expr(expr: &V2Expr, ctx: &ExpandContext<'_>) -> Result<V2Expr, ExpandError> {
    match expr {
        V2Expr::BinOp { op, left, right } => {
            let left = expand_expr(left, ctx)?;
            let right = expand_expr(right, ctx)?;
            match op {
                mei_syntax::v2::BinOp::Add => {
                    if let (V2Expr::String(a), V2Expr::String(b)) = (&left, &right) {
                        return Ok(V2Expr::String(format!("{a}{b}")));
                    }
                }
                mei_syntax::v2::BinOp::Merge => {
                    if let Some(merged) = merge_dict_expr(&left, &right) {
                        return Ok(merged);
                    }
                }
            }
            Ok(V2Expr::BinOp {
                op: *op,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
        V2Expr::List(items) => Ok(V2Expr::List(
            items
                .iter()
                .map(|e| expand_expr(e, ctx))
                .collect::<Result<Vec<_>, ExpandError>>()?,
        )),
        V2Expr::Dict(entries) => Ok(V2Expr::Dict(
            entries
                .iter()
                .map(|(k, v)| Ok((k.clone(), expand_expr(v, ctx)?)))
                .collect::<Result<Vec<_>, ExpandError>>()?,
        )),
        V2Expr::Call { path, args } => {
            if let Some(expanded) = try_expand_macro_call(path, args, ctx)? {
                return Ok(expanded);
            }
            Ok(V2Expr::Call {
                path: path.clone(),
                args: expand_call_args(args, ctx)?,
            })
        }
        V2Expr::RefCall { name, args } if name == "template_ref" => {
            let path = args
                .positional
                .first()
                .or_else(|| {
                    args.keywords
                        .iter()
                        .find(|(k, _)| k == "path")
                        .map(|(_, v)| v)
                })
                .and_then(|e| match e {
                    V2Expr::String(s) => Some(s.clone()),
                    _ => None,
                })
                .ok_or_else(|| ExpandError::Expand("template_ref requires string path".into()))?;
            let macro_args = CallArgs {
                positional: args.positional.get(1..).unwrap_or(&[]).to_vec(),
                keywords: args.keywords.clone(),
            };
            expand_macro_by_path(&path, &macro_args, ctx)
        }
        V2Expr::RefCall { name, args } => Ok(V2Expr::RefCall {
            name: name.clone(),
            args: expand_call_args(args, ctx)?,
        }),
        V2Expr::VarRef(name) => {
            if let Some(bound) = ctx.module_consts.get(name) {
                expand_expr(bound, ctx)
            } else {
                Ok(V2Expr::VarRef(name.clone()))
            }
        }
        other => Ok(other.clone()),
    }
}

fn try_expand_macro_call(
    path: &[String],
    args: &CallArgs,
    ctx: &ExpandContext<'_>,
) -> Result<Option<V2Expr>, ExpandError> {
    match path {
        [name] => {
            // A template may intentionally export the same name as a built-in
            // UI constructor (for example `ui.panel`). Keep the unqualified
            // form reserved for the constructor so the template can forward
            // to it without recursively expanding itself or hijacking other
            // author files.
            if matches!(name.as_str(), "panel" | "metric_card") {
                return Ok(None);
            }
            if let Some(def) = ctx.registry.resolve_name(name) {
                return Ok(Some(apply_macro(def, args, ctx)?));
            }
            if ctx.imports.contains_key(name) {
                let import_path = ctx.imports.get(name).cloned().unwrap_or_default();
                if let Some(def) = ctx.registry.resolve_path(&import_path) {
                    return Ok(Some(apply_macro(def, args, ctx)?));
                }
            }
            Ok(None)
        }
        [alias, method] => {
            // Qualified calls must resolve via import path first. Never ignore alias
            // by looking up `method` in the global by_name map (same-name recursion).
            if let Some(import_path) = ctx.imports.get(alias) {
                if let Some(def) = ctx.registry.resolve_in_module(import_path, method) {
                    return Ok(Some(apply_macro(def, args, ctx)?));
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn expand_macro_by_path(
    path: &str,
    args: &CallArgs,
    ctx: &ExpandContext<'_>,
) -> Result<V2Expr, ExpandError> {
    let def = ctx
        .registry
        .resolve_path(path)
        .ok_or_else(|| ExpandError::Expand(format!("unknown template macro `{path}`")))?;
    apply_macro(def, args, ctx)
}

fn apply_macro(
    def: &MacroDef,
    args: &CallArgs,
    ctx: &ExpandContext<'_>,
) -> Result<V2Expr, ExpandError> {
    let frame = (def.file_path.clone(), def.name.clone());
    if ctx.expand_stack.iter().any(|item| item == &frame) {
        let chain = ctx
            .expand_stack
            .iter()
            .chain(std::iter::once(&frame))
            .map(|(path, name)| format!("{path}::{name}"))
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(ExpandError::Expand(format!(
            "macro_expansion_cycle: {chain}"
        )));
    }
    let mut expand_stack = ctx.expand_stack.clone();
    expand_stack.push(frame);

    let mut bindings = def.module_consts.clone();
    bindings.extend(ctx.module_consts.clone());
    for param in &def.params {
        if let Some((_, value)) = args.keywords.iter().find(|(k, _)| k == &param.name) {
            bindings.insert(param.name.clone(), expand_expr(value, ctx)?);
        } else if let Some(default) = &param.default {
            let local = ExpandContext {
                registry: ctx.registry,
                imports: ctx.imports.clone(),
                module_consts: bindings.clone(),
                expand_stack: expand_stack.clone(),
            };
            bindings.insert(param.name.clone(), expand_expr(default, &local)?);
        } else if let Some(value) = args.positional.get(
            def.params
                .iter()
                .position(|p| p.name == param.name)
                .unwrap_or(0),
        ) {
            bindings.insert(param.name.clone(), expand_expr(value, ctx)?);
        } else if param.default.is_none() {
            return Err(ExpandError::Expand(format!(
                "missing macro argument `{}` for `{}`",
                param.name, def.name
            )));
        }
    }
    // Expand the defining module's body with THAT module's imports, not the caller's.
    let local = ExpandContext {
        registry: ctx.registry,
        imports: def.module_imports.clone(),
        module_consts: bindings.clone(),
        expand_stack,
    };
    substitute_expr(&def.body, &bindings, &local)
}

fn substitute_expr(
    expr: &V2Expr,
    bindings: &BTreeMap<String, V2Expr>,
    ctx: &ExpandContext<'_>,
) -> Result<V2Expr, ExpandError> {
    match expr {
        V2Expr::VarRef(name) => bindings
            .get(name)
            .cloned()
            .ok_or_else(|| ExpandError::Expand(format!("unbound macro variable `{name}`"))),
        V2Expr::BinOp { op, left, right } => Ok(V2Expr::BinOp {
            op: *op,
            left: Box::new(substitute_expr(left, bindings, ctx)?),
            right: Box::new(substitute_expr(right, bindings, ctx)?),
        }),
        V2Expr::List(items) => Ok(V2Expr::List(
            items
                .iter()
                .map(|e| substitute_expr(e, bindings, ctx))
                .collect::<Result<Vec<_>, ExpandError>>()?,
        )),
        V2Expr::Dict(entries) => Ok(V2Expr::Dict(
            entries
                .iter()
                .map(|(k, v)| Ok((k.clone(), substitute_expr(v, bindings, ctx)?)))
                .collect::<Result<Vec<_>, ExpandError>>()?,
        )),
        V2Expr::Call { path, args } => {
            if let Some(expanded) = try_expand_macro_call(path, args, ctx)? {
                return substitute_expr(&expanded, bindings, ctx);
            }
            Ok(V2Expr::Call {
                path: path.clone(),
                args: CallArgs {
                    positional: args
                        .positional
                        .iter()
                        .map(|e| substitute_expr(e, bindings, ctx))
                        .collect::<Result<Vec<_>, ExpandError>>()?,
                    keywords: args
                        .keywords
                        .iter()
                        .map(|(k, v)| Ok((k.clone(), substitute_expr(v, bindings, ctx)?)))
                        .collect::<Result<Vec<_>, ExpandError>>()?,
                },
            })
        }
        V2Expr::RefCall { name, args } => Ok(V2Expr::RefCall {
            name: name.clone(),
            args: CallArgs {
                positional: args
                    .positional
                    .iter()
                    .map(|e| substitute_expr(e, bindings, ctx))
                    .collect::<Result<Vec<_>, ExpandError>>()?,
                keywords: args
                    .keywords
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), substitute_expr(v, bindings, ctx)?)))
                    .collect::<Result<Vec<_>, ExpandError>>()?,
            },
        }),
        other => Ok(other.clone()),
    }
}

fn merge_dict_expr(left: &V2Expr, right: &V2Expr) -> Option<V2Expr> {
    let V2Expr::Dict(left_entries) = left else {
        return None;
    };
    let V2Expr::Dict(right_entries) = right else {
        return None;
    };
    let mut merged = left_entries.clone();
    for (key, value) in right_entries {
        if let Some((_, existing)) = merged
            .iter_mut()
            .find(|(existing_key, _)| existing_key == key)
        {
            *existing = value.clone();
        } else {
            merged.push((key.clone(), value.clone()));
        }
    }
    Some(V2Expr::Dict(merged))
}

fn eval_const_expr(
    expr: &V2Expr,
    consts: &BTreeMap<String, V2Expr>,
) -> Result<V2Expr, ExpandError> {
    match expr {
        V2Expr::VarRef(name) => consts
            .get(name)
            .cloned()
            .ok_or_else(|| ExpandError::Expand(format!("unbound module const `{name}`"))),
        V2Expr::BinOp {
            op: mei_syntax::v2::BinOp::Add,
            left,
            right,
        } => {
            let left = eval_const_expr(left, consts)?;
            let right = eval_const_expr(right, consts)?;
            match (left, right) {
                (V2Expr::String(a), V2Expr::String(b)) => Ok(V2Expr::String(format!("{a}{b}"))),
                (l, r) => Ok(V2Expr::BinOp {
                    op: mei_syntax::v2::BinOp::Add,
                    left: Box::new(l),
                    right: Box::new(r),
                }),
            }
        }
        V2Expr::BinOp {
            op: mei_syntax::v2::BinOp::Merge,
            left,
            right,
        } => {
            let left = eval_const_expr(left, consts)?;
            let right = eval_const_expr(right, consts)?;
            merge_dict_expr(&left, &right)
                .ok_or_else(|| ExpandError::Expand("dict merge requires two dict literals".into()))
        }
        other => Ok(other.clone()),
    }
}
