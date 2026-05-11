use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value as JsonValue};
use starlark::{
    environment::{GlobalsBuilder, Module},
    eval::Evaluator,
    syntax::{AstModule, Dialect},
};

const FORBIDDEN_TOKENS: &[&str] = &["for", "while", "lambda", "load", "import", "open"];
const PRELUDE_SOURCE_FILES: &[&str] = &[
    "crates/kernel/src/prelude/core.star",
    "crates/kernel/src/prelude/ds.star",
    "crates/kernel/src/prelude/ui.star",
    "crates/kernel/src/prelude/doc.star",
];
const MEILANG_PRELUDE: &str = concat!(
    include_str!("prelude/core.star"),
    "\n",
    include_str!("prelude/ds.star"),
    "\n",
    include_str!("prelude/ui.star"),
    "\n",
    include_str!("prelude/doc.star"),
);

fn public_prelude_functions() -> Vec<String> {
    MEILANG_PRELUDE
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("def ")?;
            let name = rest.split('(').next()?.trim();
            if name.starts_with('_') || name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

pub fn describe_dsl() -> JsonValue {
    json!({
        "schema_version": "0.1.0",
        "source": PRELUDE_SOURCE_FILES,
        "runtime": {
            "evaluator": "starlark",
            "dialect": "starlark::syntax::Dialect::Standard",
        },
        "forbidden_tokens": FORBIDDEN_TOKENS,
        "public_surface": public_prelude_functions(),
    })
}

fn sanitize_for_policy(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    for ch in source.chars() {
        if let Some(quote) = in_string {
            if ch == '\n' {
                out.push('\n');
                escaped = false;
                continue;
            }
            out.push(' ');
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
        match ch {
            '#' => out.push(' '),
            '"' | '\'' => {
                in_string = Some(ch);
                out.push(' ');
            }
            _ => out.push(ch),
        }
    }
    out
}

fn validate_policy(source: &str) -> Result<()> {
    let sanitized = sanitize_for_policy(source);
    for token in sanitized
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| !token.is_empty())
    {
        if FORBIDDEN_TOKENS.contains(&token) {
            bail!("authoring source contains forbidden token `{token}`");
        }
    }
    Ok(())
}

fn rewrite_namespaces(source: &str) -> String {
    source
        .replace("app.add_scene(", "app_add_scene(")
        .replace("scene.set_world(", "world(")
        .replace("scene.set_flow(", "flow(")
        .replace("scene.set_frame(", "frame(")
        .replace("frame.add_panel(", "panel(")
        .replace("doc.", "")
        .replace("ds.", "")
        .replace("ui.", "")
}

fn normalize_output(raw: &str) -> Result<JsonValue> {
    let value: JsonValue =
        serde_json::from_str(raw).context("Starlark output was not valid JSON")?;
    match value {
        JsonValue::Array(items) => Ok(JsonValue::Array(items)),
        other => Ok(json!([other])),
    }
}

pub fn evaluate_mei_source(filename: &str, source: &str) -> Result<JsonValue> {
    validate_policy(source)?;
    let source = rewrite_namespaces(source);
    let source = format!("{MEILANG_PRELUDE}\n\n{source}");
    let ast = AstModule::parse(filename, source, &Dialect::Standard)
        .map_err(|error| anyhow::anyhow!("failed to parse {filename}: {error}"))?;
    let globals = GlobalsBuilder::standard().build();
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.eval_module(ast, &globals)
        .map_err(|error| anyhow::anyhow!("failed to evaluate {filename}: {error}"))?;
    let exports = module
        .get("exports")
        .context("Starlark file did not produce exports")?;
    let raw_json = exports
        .to_json()
        .context("failed to convert exports to JSON")?;
    normalize_output(&raw_json)
}

pub fn evaluate_mei_file(path: impl AsRef<Path>) -> Result<JsonValue> {
    let path = path.as_ref();
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    evaluate_mei_source(&path.to_string_lossy(), &source)
}
