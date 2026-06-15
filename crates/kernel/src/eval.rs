use std::cell::RefCell;
use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde_json::{json, Value as JsonValue};
use starlark::{
    environment::{GlobalsBuilder, Module},
    eval::Evaluator,
    syntax::{AstModule, Dialect},
};

use crate::mei_config::{forbidden_authoring_tokens, validate_authoring_policy, AuthoringHelpers};
const PRELUDE_SOURCE_FILES: &[&str] = &[
    "crates/kernel/src/prelude/core.star",
    "crates/kernel/src/prelude/ds.star",
    "crates/kernel/src/prelude/link.star",
    "crates/kernel/src/prelude/ui.star",
    "crates/kernel/src/prelude/doc.star",
    "crates/kernel/src/prelude/text.star",
    "crates/kernel/src/prelude/metric.star",
    "crates/kernel/src/prelude/assembly.star",
];
const MEILANG_PRELUDE: &str = concat!(
    include_str!("prelude/core.star"),
    "\n",
    include_str!("prelude/ds.star"),
    "\n",
    include_str!("prelude/link.star"),
    "\n",
    include_str!("prelude/ui.star"),
    "\n",
    include_str!("prelude/doc.star"),
    "\n",
    include_str!("prelude/text.star"),
    "\n",
    include_str!("prelude/metric.star"),
    "\n",
    include_str!("prelude/assembly.star"),
);

thread_local! {
    static ACTIVE_AUTHORING_HELPERS: RefCell<Option<AuthoringHelpers>> = const { RefCell::new(None) };
}

fn public_functions_from_source(source: &str) -> Vec<String> {
    source
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

fn public_prelude_functions() -> Vec<String> {
    public_functions_from_source(MEILANG_PRELUDE)
}

fn merged_public_surface(helpers: Option<&AuthoringHelpers>) -> Vec<String> {
    let mut out = public_prelude_functions();
    if let Some(helpers) = helpers {
        out.extend(helpers.public_functions.iter().cloned());
    }
    out.sort();
    out.dedup();
    out
}

pub fn active_authoring_helpers() -> Option<AuthoringHelpers> {
    ACTIVE_AUTHORING_HELPERS.with(|slot| slot.borrow().clone())
}

pub fn active_authoring_fingerprint() -> String {
    active_authoring_helpers()
        .map(|helpers| helpers.fingerprint)
        .unwrap_or_default()
}

/// Install workspace authoring helpers for the current thread until the guard drops.
pub fn push_authoring_helpers(helpers: AuthoringHelpers) -> AuthoringEvalGuard {
    ACTIVE_AUTHORING_HELPERS.with(|slot| {
        *slot.borrow_mut() = Some(helpers);
    });
    AuthoringEvalGuard
}

pub struct AuthoringEvalGuard;

impl Drop for AuthoringEvalGuard {
    fn drop(&mut self) {
        ACTIVE_AUTHORING_HELPERS.with(|slot| {
            *slot.borrow_mut() = None;
        });
    }
}

pub fn describe_dsl() -> JsonValue {
    describe_dsl_with_helpers(active_authoring_helpers().as_ref())
}

pub fn describe_dsl_with_helpers(helpers: Option<&AuthoringHelpers>) -> JsonValue {
    let mut source = PRELUDE_SOURCE_FILES
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if let Some(helpers) = helpers {
        source.extend(helpers.source_files.iter().cloned());
    }
    json!({
        "schema_version": "0.1.0",
        "source": source,
        "runtime": {
            "evaluator": "starlark",
            "dialect": "starlark::syntax::Dialect::Standard",
        },
        "forbidden_tokens": forbidden_authoring_tokens(),
        "public_surface": merged_public_surface(helpers),
    })
}

fn validate_policy(source: &str) -> Result<()> {
    validate_authoring_policy(source)
}

fn rewrite_namespaces(source: &str) -> String {
    source
        .replace("app.add_scene(", "app_add_scene(")
        .replace("scene.set_world(", "world(")
        .replace("scene.set_flow(", "flow(")
        .replace("scene.set_frame(", "frame(")
        .replace("world.add_resource(", "world_add_resource(")
        .replace("world.add_dataset(", "world_add_dataset(")
        .replace("world.add_dataset_view(", "world_add_dataset_view(")
        .replace("world.add_metric(", "world_add_metric(")
        .replace("world.add_metric_pack(", "world_add_metric_pack(")
        .replace("world.add_entity(", "world_add_entity(")
        .replace("world.set_topology(", "world_set_topology(")
        .replace("frame.set_layout(", "frame_set_layout(")
        .replace("frame.add_panel(", "panel_decl(")
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

fn compose_prelude(helpers: Option<&AuthoringHelpers>) -> String {
    let mut prelude = MEILANG_PRELUDE.to_string();
    if let Some(helpers) = helpers {
        if !helpers.prelude_suffix.trim().is_empty() {
            prelude.push_str("\n\n");
            prelude.push_str(helpers.prelude_suffix.trim());
        }
    }
    prelude
}

pub fn evaluate_mei_source(filename: &str, source: &str) -> Result<JsonValue> {
    evaluate_mei_source_with_helpers(filename, source, active_authoring_helpers().as_ref())
}

pub fn evaluate_mei_source_with_helpers(
    filename: &str,
    source: &str,
    helpers: Option<&AuthoringHelpers>,
) -> Result<JsonValue> {
    validate_policy(source)?;
    let source = rewrite_namespaces(source);
    let prelude = compose_prelude(helpers);
    let source = format!("{prelude}\n\n{source}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_authoring_policy_rejects_import_token() {
        let err = validate_authoring_policy("import foo").expect_err("import should be rejected");
        assert!(err.to_string().contains("import"));
    }
}
