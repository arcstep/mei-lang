use std::path::Path;

use chumsky::prelude::*;

use super::ast::*;
use crate::policy::{
    validate_authoring_policy, validate_authoring_policy_for_path, validate_world_authoring_policy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V2ParseError {
    pub message: String,
    pub span_start: usize,
    pub span_end: usize,
}

impl std::fmt::Display for V2ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for V2ParseError {}

pub fn parse_v2_source_file(path: &Path) -> Result<V2SourceFile, V2ParseError> {
    let source = std::fs::read_to_string(path).map_err(|error| V2ParseError {
        message: format!("failed to read {}: {error}", path.display()),
        span_start: 0,
        span_end: 0,
    })?;
    if let Err(forbidden) = validate_authoring_policy_for_path(path, &source) {
        return Err(V2ParseError {
            message: forbidden.to_string(),
            span_start: 0,
            span_end: source.len().min(1),
        });
    }
    parse_v2_source_unchecked(&source)
}

pub fn parse_v2_source(source: &str) -> Result<V2SourceFile, V2ParseError> {
    parse_v2_source_with_policy(source, validate_authoring_policy)
}

pub fn parse_world_v2_source(source: &str) -> Result<V2SourceFile, V2ParseError> {
    parse_v2_source_with_policy(source, validate_world_authoring_policy)
}

fn parse_v2_source_with_policy(
    source: &str,
    validate: fn(&str) -> Result<(), crate::policy::ForbiddenTokenError>,
) -> Result<V2SourceFile, V2ParseError> {
    let stripped = strip_comments(source);
    if let Err(forbidden) = validate(&stripped) {
        return Err(V2ParseError {
            message: forbidden.to_string(),
            span_start: 0,
            span_end: stripped.len().min(1),
        });
    }
    parse_v2_source_stripped(&stripped)
}

fn parse_v2_source_unchecked(source: &str) -> Result<V2SourceFile, V2ParseError> {
    parse_v2_source_stripped(&strip_comments(source))
}

fn parse_v2_source_stripped(stripped: &str) -> Result<V2SourceFile, V2ParseError> {
    let parser = v2_source_file_parser();
    parser.parse(stripped).map_err(|errors| {
        let error = errors
            .first()
            .cloned()
            .unwrap_or_else(|| Simple::custom(0..0, "parse error"));
        let span = error.span();
        V2ParseError {
            message: format!("{error}"),
            span_start: span.start,
            span_end: span.end,
        }
    })
}

fn strip_comments(source: &str) -> String {
    let mut result = String::new();
    let mut in_string = None::<char>;
    let mut escaped = false;
    for line in source.lines() {
        let mut line_out = String::new();
        for ch in line.chars() {
            if let Some(quote) = in_string {
                line_out.push(ch);
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == quote {
                    in_string = None;
                }
                continue;
            }
            if ch == '"' || ch == '\'' {
                in_string = Some(ch);
                line_out.push(ch);
                continue;
            }
            if ch == '#' {
                break;
            }
            line_out.push(ch);
        }
        result.push_str(&line_out);
        result.push('\n');
    }
    result
}

fn identifier_parser() -> impl Parser<char, String, Error = Simple<char>> + Clone {
    text::ident::<char, Simple<char>>().map(|ident: String| ident)
}

fn path_parser() -> impl Parser<char, Vec<String>, Error = Simple<char>> + Clone {
    identifier_parser()
        .separated_by(just('.'))
        .at_least(1)
        .collect::<Vec<_>>()
}

fn string_parser() -> impl Parser<char, String, Error = Simple<char>> + Clone {
    just('"')
        .ignore_then(
            choice((
                just('\\').ignore_then(any()).map(|ch| match ch {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '0' => '\0',
                    other => other,
                }),
                none_of('"'),
            ))
            .repeated()
            .collect::<String>(),
        )
        .then_ignore(just('"'))
}

fn number_parser() -> impl Parser<char, f64, Error = Simple<char>> + Clone {
    text::int(10)
        .then(just('.').ignore_then(text::digits(10)).or_not())
        .map(|(whole, frac): (String, Option<String>)| {
            let text = if let Some(frac) = frac {
                format!("{whole}.{frac}")
            } else {
                whole
            };
            text.parse::<f64>().unwrap_or(0.0)
        })
}

fn ref_keyword_parser() -> impl Parser<char, String, Error = Simple<char>> + Clone {
    choice((
        just("panel_ref").to("panel_ref"),
        just("metric_ref").to("metric_ref"),
        just("assembly_ref").to("assembly_ref"),
        just("link_ref").to("link_ref"),
        just("template_ref").to("template_ref"),
        just("asset_ref").to("asset_ref"),
        just("source_ref").to("source_ref"),
        just("theme_ref").to("theme_ref"),
        just("metric_bundle_ref").to("metric_bundle_ref"),
        just("explain_ref").to("explain_ref"),
        just("ops_param_ref").to("ops_param_ref"),
        just("board_ref").to("board_ref"),
        just("param_ref").to("param_ref"),
        just("dataset_ref").to("dataset_ref"),
        just("dataframe_ref").to("dataframe_ref"),
        just("source_feature_ref").to("source_feature_ref"),
        just("feature_ref").to("feature_ref"),
    ))
    .map(str::to_string)
}

fn call_args_parser(
    expr: impl Parser<char, V2Expr, Error = Simple<char>> + Clone,
) -> impl Parser<char, CallArgs, Error = Simple<char>> + Clone {
    let kw = identifier_parser()
        .then_ignore(just('=').padded())
        .then(expr.clone().padded())
        .map(|(name, value)| (name, value));
    let positional = expr.padded();
    choice((kw.map(Either::Right), positional.map(Either::Left)))
        .padded()
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just('(').padded(), just(')').padded())
        .map(|items| {
            let mut positional = Vec::new();
            let mut keywords = Vec::new();
            for item in items {
                match item {
                    Either::Left(value) => positional.push(value),
                    Either::Right((name, value)) => keywords.push((name, value)),
                }
            }
            CallArgs {
                positional,
                keywords,
            }
        })
}

fn template_params_parser(
    expr: impl Parser<char, V2Expr, Error = Simple<char>> + Clone,
) -> impl Parser<char, Vec<TemplateParam>, Error = Simple<char>> + Clone {
    identifier_parser()
        .then(
            just('=')
                .padded()
                .ignore_then(expr.clone().padded())
                .or_not(),
        )
        .map(|(name, default)| TemplateParam { name, default })
        .padded()
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just('(').padded(), just(')').padded())
}

fn expr_parser() -> impl Parser<char, V2Expr, Error = Simple<char>> + Clone {
    recursive(|expr| {
        let call_args = call_args_parser(expr.clone());
        let var_ref = identifier_parser()
            .then(call_args.clone().or_not())
            .try_map(|(name, args), _span| {
                if let Some(args) = args {
                    Ok(V2Expr::Call {
                        path: vec![name],
                        args,
                    })
                } else {
                    Ok(V2Expr::VarRef(name))
                }
            });
        let ref_call = ref_keyword_parser()
            .then(call_args.clone())
            .map(|(name, args)| V2Expr::RefCall { name, args });
        let path_call = path_parser()
            .then(call_args.clone())
            .map(|(path, args)| V2Expr::Call { path, args });
        let atom = choice((
            string_parser().map(V2Expr::String),
            number_parser().map(V2Expr::Number),
            just("true").to(V2Expr::Bool(true)),
            just("True").to(V2Expr::Bool(true)),
            just("false").to(V2Expr::Bool(false)),
            just("False").to(V2Expr::Bool(false)),
            text::keyword("None").to(V2Expr::None),
            ref_call,
            path_call,
            var_ref,
        ));
        let list = expr
            .clone()
            .padded()
            .separated_by(just(',').padded())
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('[').padded(), just(']').padded())
            .map(V2Expr::List);
        let dict_key = choice((string_parser(), identifier_parser()));
        let dict = dict_key
            .clone()
            .then_ignore(just(':').padded())
            .then(expr.clone().padded())
            .padded()
            .separated_by(just(',').padded())
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('{').padded(), just('}').padded())
            .map(V2Expr::Dict);
        let for_in = just("for")
            .padded()
            .ignore_then(identifier_parser())
            .then_ignore(just("in").padded())
            .then(expr.clone())
            .then(
                expr.clone()
                    .delimited_by(just('{').padded(), just('}').padded()),
            )
            .map(|((var, source), body)| V2Expr::ForIn {
                var,
                source: Box::new(source),
                body: Box::new(body),
            });
        let enum_match = just("enum")
            .padded()
            .ignore_then(expr.clone())
            .then(
                dict_key
                    .then_ignore(just("=>").padded())
                    .then(expr.clone().padded())
                    .padded()
                    .separated_by(just(',').padded())
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just('{').padded(), just('}').padded()),
            )
            .map(|(subject, cases)| {
                let cases: Vec<(V2Expr, V2Expr)> = cases
                    .into_iter()
                    .map(|(key, body)| (V2Expr::String(key), body))
                    .collect();
                let default = cases.iter().position(|(key, _)| match key {
                    V2Expr::String(s) => s == "default",
                    V2Expr::VarRef(s) => s == "default",
                    _ => false,
                });
                let default_body =
                    default.and_then(|idx| cases.get(idx).map(|(_, body)| body.clone()));
                let filtered_cases: Vec<_> = cases
                    .into_iter()
                    .enumerate()
                    .filter(|(idx, _)| default.map(|d| d != *idx).unwrap_or(true))
                    .map(|(_, pair)| pair)
                    .collect();
                V2Expr::EnumMatch {
                    subject: Box::new(subject),
                    cases: filtered_cases,
                    default: default_body.map(Box::new),
                }
            });
        let primary = choice((enum_match, for_in, dict, list, atom));
        let with_members = primary
            .clone()
            .then(
                just('.')
                    .padded()
                    .ignore_then(identifier_parser())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .map(|(base, fields)| {
                fields.into_iter().fold(base, |acc, field| V2Expr::Member {
                    object: Box::new(acc),
                    field,
                })
            });
        with_members
            .clone()
            .then(
                choice((just('+').to(BinOp::Add), just('|').to(BinOp::Merge)))
                    .padded()
                    .then(primary.clone())
                    .repeated(),
            )
            .map(|(left, rest)| {
                rest.into_iter()
                    .fold(left, |acc, (op, right)| V2Expr::BinOp {
                        op,
                        left: Box::new(acc),
                        right: Box::new(right),
                    })
            })
    })
}

enum Either<L, R> {
    Left(L),
    Right(R),
}

fn use_template_parser() -> impl Parser<char, V2Item, Error = Simple<char>> + Clone {
    just("use")
        .padded()
        .ignore_then(just("template"))
        .padded()
        .ignore_then(string_parser())
        .then(
            just("as")
                .padded()
                .ignore_then(identifier_parser())
                .or_not(),
        )
        .map(|(path, alias)| V2Item::UseTemplate { path, alias })
        .padded()
}

fn template_decl_parser(
    expr: impl Parser<char, V2Expr, Error = Simple<char>> + Clone,
) -> impl Parser<char, V2Item, Error = Simple<char>> + Clone {
    just("template")
        .padded()
        .ignore_then(identifier_parser())
        .then(template_params_parser(expr.clone()))
        .then_ignore(just(':').padded())
        .then(expr.padded())
        .map(|((name, params), body)| V2Item::TemplateDecl { name, params, body })
        .padded()
}

fn module_const_parser(
    expr: impl Parser<char, V2Expr, Error = Simple<char>> + Clone,
) -> impl Parser<char, V2Item, Error = Simple<char>> + Clone {
    identifier_parser()
        .then_ignore(just('=').padded())
        .then(expr)
        .map(|(name, value)| V2Item::ModuleConst { name, value })
        .padded()
}

fn top_level_parser(
    expr: impl Parser<char, V2Expr, Error = Simple<char>> + Clone,
) -> impl Parser<char, V2Item, Error = Simple<char>> + Clone {
    identifier_parser()
        .then(call_args_parser(expr))
        .try_map(|(name, args), span| {
            if V2_TOP_LEVEL_CONSTRUCTORS.contains(&name.as_str()) {
                Ok(V2Item::TopLevel { name, args })
            } else {
                Err(Simple::custom(
                    span,
                    format!("unknown top-level constructor `{name}`"),
                ))
            }
        })
        .padded()
}

fn v2_source_file_parser() -> impl Parser<char, V2SourceFile, Error = Simple<char>> {
    let expr = expr_parser();
    let item = choice((
        use_template_parser(),
        module_const_parser(expr.clone()),
        template_decl_parser(expr.clone()),
        top_level_parser(expr.clone()),
    ));
    item.repeated()
        .at_least(1)
        .map(|items| V2SourceFile { items })
        .then_ignore(end().or_not())
        .padded()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_metric_card_with_surface() {
        // `metric_card` is no longer a top-level constructor; exercise a current whitelist name.
        let src2 = r#"
content_panel(id = "demo", chrome = "bare", blocks = [])
"#;
        let file = parse_v2_source(src2).expect("parse");
        let item = &file.items[0];
        match item {
            V2Item::TopLevel { name, args, .. } => {
                assert_eq!(name, "content_panel");
                let chrome = args
                    .keywords
                    .iter()
                    .find(|(k, _)| k == "chrome")
                    .map(|(_, v)| v)
                    .expect("chrome");
                match chrome {
                    V2Expr::String(s) => assert_eq!(s, "bare"),
                    other => panic!("expected String, got {other:?}"),
                }
            }
            _ => panic!("expected top-level"),
        }
    }

    #[test]
    fn parses_v2_app_skeleton() {
        let source = r#"
app_skeleton(
    id = "data-demo",
    title = "Data Demo",
    default_stage = "home",
)
"#;
        let file = parse_v2_source(source).expect("parse");
        assert_eq!(file.items.len(), 1);
        match &file.items[0] {
            V2Item::TopLevel { name, .. } => assert_eq!(name, "app_skeleton"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_v2_template_with_const_and_plus() {
        let source = r#"
TPL = "/assets/panel"

template panel_title_decor(
    caret_url = TPL + "/caret.svg",
):
    {
        "url": caret_url,
    }
"#;
        let file = parse_v2_source(source).expect("parse");
        assert_eq!(file.items.len(), 2);
    }

    #[test]
    fn parses_multiple_templates_after_dict_merge() {
        let source = r#"
template slot_metric_shell(id = "x", child = None, shell_props = {}):
    panel(
        props = {"a": "1"} | shell_props,
        blocks = [child],
    )
template slot_metric_card(id = "y"):
    panel(id = id)
"#;
        let file = parse_v2_source(source).expect("parse");
        let templates: Vec<_> = file
            .items
            .iter()
            .filter_map(|item| {
                if let V2Item::TemplateDecl { name, .. } = item {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            templates,
            vec!["slot_metric_shell", "slot_metric_card"],
            "dict merge must not truncate following templates"
        );
    }

    #[test]
    fn parses_v2_panel_with_refs() {
        let source = include_str!("../../tests/fixtures/v2/panel_with_refs.mei");
        let file = parse_v2_source(source).expect("parse panel_with_refs.mei");
        assert!(file
            .items
            .iter()
            .any(|item| matches!(item, V2Item::UseTemplate { .. })));
        assert!(file
            .items
            .iter()
            .any(|item| matches!(item, V2Item::TopLevel { name, .. } if name == "content_panel")));
    }

    #[test]
    fn parses_cockpit_metric_gallery_scene() {
        let source = include_str!("../../tests/fixtures/v2/metric_gallery.mei");
        let file = parse_v2_source(source).expect("parse metric_gallery.mei");
        assert!(file
            .items
            .iter()
            .any(|item| matches!(item, V2Item::UseTemplate { .. })));
        assert!(
            !file.items.is_empty(),
            "metric gallery should produce AST items"
        );
    }

    #[test]
    fn parses_grid_only_authoring_example() {
        let source = include_str!("../../tests/fixtures/v2/grid_authoring.mei");
        let file = parse_v2_source(source).expect("parse grid_authoring.mei");
        assert!(file
            .items
            .iter()
            .any(|item| matches!(item, V2Item::UseTemplate { .. })));
        assert!(
            !file.items.is_empty(),
            "authoring example should produce AST items"
        );
    }
}

#[cfg(test)]
mod phase5_constructor_parse {
    use super::*;

    #[test]
    fn parses_bare_content_panel() {
        let source = r#"
content_panel(
    id = "map_stage",
    chrome = "bare",
    blocks = [],
)
"#;
        let file = parse_v2_source(source).expect("bare content_panel should parse");
        assert!(file.items.iter().any(|item| {
            matches!(item, V2Item::TopLevel { name, .. } if name == "content_panel")
        }));
    }

    #[test]
    fn parses_bare_page_instance() {
        let source = r#"
page_instance(
    key = "x",
    scene = "y",
)
"#;
        let file = parse_v2_source(source).expect("bare page_instance should parse");
        assert!(file.items.iter().any(|item| {
            matches!(item, V2Item::TopLevel { name, .. } if name == "page_instance")
        }));
    }

    #[test]
    fn parses_presentation_and_slide_layout() {
        let source = r#"
presentation(
    id = "intro",
    summary = "MeiLang tutorial",
    theme = "presentation",
    planes = [plane_ref(id = "p")],
)

plane_layout(
    id = "p",
    tier = "p",
    slides = [
        slide_ref(id = "slide-01-cover"),
        slide_ref(id = "slide-02-why"),
    ],
)

slide_layout(
    id = "slide-01-cover",
    title = "Cover",
    chapter = "开场",
    pattern = "full_bleed",
    regions = [region_ref(id = "r-main")],
)
"#;
        let file = parse_v2_source(source).expect("presentation constructors should parse");
        let names: Vec<&str> = file
            .items
            .iter()
            .filter_map(|item| match item {
                V2Item::TopLevel { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(names.contains(&"presentation"));
        assert!(names.contains(&"plane_layout"));
        assert!(names.contains(&"slide_layout"));
    }
}

#[cfg(test)]
mod escape_tests {
    use super::*;
    use crate::v2::ast::{V2Expr, V2Item};

    #[test]
    fn string_parser_interprets_common_escapes() {
        let file = parse_v2_source(
            r#"
content_panel(
    id = "x",
    props = {"content": "a\nb"},
)
"#,
        )
        .expect("parse");
        let args = file
            .items
            .iter()
            .find_map(|item| match item {
                V2Item::TopLevel { name, args } if name == "content_panel" => Some(args),
                _ => None,
            })
            .expect("content_panel");
        let props = args
            .keywords
            .iter()
            .find(|(key, _)| key == "props")
            .map(|(_, expr)| expr)
            .expect("props");
        let V2Expr::Dict(entries) = props else {
            panic!("expected dict props, got {props:?}");
        };
        let content = entries
            .iter()
            .find_map(|(key, value)| match (key.as_str(), value) {
                ("content", V2Expr::String(text)) => Some(text.as_str()),
                _ => None,
            })
            .expect("content string");
        assert_eq!(content, "a\nb");
    }
}
