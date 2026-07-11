use std::path::Path;

use chumsky::prelude::*;

use crate::ast::*;
use crate::policy::{validate_authoring_policy, validate_authoring_policy_for_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span_start: usize,
    pub span_end: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse_source_file(path: &Path) -> Result<SourceFile, ParseError> {
    let source = std::fs::read_to_string(path).map_err(|error| ParseError {
        message: format!("failed to read {}: {error}", path.display()),
        span_start: 0,
        span_end: 0,
    })?;
    if let Err(forbidden) = validate_authoring_policy_for_path(path, &source) {
        return Err(ParseError {
            message: forbidden.to_string(),
            span_start: 0,
            span_end: source.len().min(1),
        });
    }
    parse_source(&source)
}

pub fn parse_source(source: &str) -> Result<SourceFile, ParseError> {
    if let Err(forbidden) = validate_authoring_policy(source) {
        return Err(ParseError {
            message: forbidden.to_string(),
            span_start: 0,
            span_end: source.len().min(1),
        });
    }
    let stripped = strip_comments(source);
    let parser = source_file_parser();
    parser.parse(stripped.as_str()).map_err(|errors| {
        let error = errors
            .first()
            .cloned()
            .unwrap_or_else(|| Simple::custom(0..0, "parse error"));
        let span = error.span();
        ParseError {
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
            none_of('"')
                .or(just('\\').ignore_then(any()))
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

fn expr_parser() -> impl Parser<char, Expr, Error = Simple<char>> + Clone {
    recursive(|expr| {
        let call_args = call_args_parser(expr.clone());
        let atom = choice((
            string_parser().map(Expr::String),
            number_parser().map(Expr::Number),
            text::keyword("true").to(Expr::Bool(true)),
            text::keyword("false").to(Expr::Bool(false)),
            text::keyword("None").to(Expr::None),
            path_parser()
                .then(call_args.clone())
                .map(|(path, args)| Expr::Call { path, args }),
        ));
        let list = atom
            .clone()
            .padded()
            .separated_by(just(',').padded())
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just('[').padded(), just(']').padded())
            .map(Expr::List);
        choice((list, atom))
    })
}

fn call_args_parser(
    expr: impl Parser<char, Expr, Error = Simple<char>> + Clone,
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

enum Either<L, R> {
    Left(L),
    Right(R),
}

fn top_level_call_parser() -> impl Parser<char, TopLevelCall, Error = Simple<char>> {
    path_parser()
        .then(call_args_parser(expr_parser()))
        .map_with_span(|(path, args), span| TopLevelCall {
            path,
            args,
            span_start: span.start,
            span_end: span.end,
        })
        .padded()
}

fn source_file_parser() -> impl Parser<char, SourceFile, Error = Simple<char>> {
    top_level_call_parser()
        .padded()
        .repeated()
        .at_least(1)
        .map(|statements| SourceFile { statements })
        .then_ignore(end())
        .padded()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hello_main() {
        let source = include_str!("../../../../../workspaces/ws-hello/apps/hello/src/main.mei");
        let file = parse_source(source).expect("parse main.mei");
        assert_eq!(file.statements.len(), 1);
        assert_eq!(file.statements[0].path, vec!["app"]);
    }

    #[test]
    fn parses_hello_home() {
        let source =
            include_str!("../../../../../workspaces/ws-hello/apps/hello/src/scenes/home.mei");
        let file = parse_source(source).expect("parse home.mei");
        assert_eq!(file.statements.len(), 4);
        assert_eq!(file.statements[3].path, vec!["frame", "add_panel"]);
    }
}
