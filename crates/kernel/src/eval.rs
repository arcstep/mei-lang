use std::cell::RefCell;
use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde_json::{json, Value as JsonValue};

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
            "evaluator": "mei-lower",
            "dialect": "mei-surface/native-v2",
        },
        "forbidden_tokens": forbidden_authoring_tokens(),
        "public_surface": merged_public_surface(helpers),
    })
}

fn validate_policy(source: &str) -> Result<()> {
    validate_authoring_policy(source)
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
    if let Some(helpers) = helpers {
        if !helpers.fingerprint.trim().is_empty()
            || !helpers.prelude_suffix.trim().is_empty()
            || !helpers.public_functions.is_empty()
        {
            eprintln!(
                "warning: native v2 evaluator ignores workspace authoring helpers (file={}, fingerprint={})",
                filename,
                helpers.fingerprint
            );
        }
    }
    let lowered = mei_lower::lower_source(source)
        .with_context(|| format!("failed to lower {filename} with native v2 pipeline"))?;
    Ok(JsonValue::Array(lowered.exports))
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

    #[test]
    fn evaluate_mei_source_lowers_native_v2_app_decl() {
        let source = r#"
app(
    id = "minimal-app",
    title = "Minimal App",
    default_scene = "home",
    scene = scene_ref(scene_file = "scenes/home.mei"),
)
"#;
        let value = evaluate_mei_source("minimal-app.mei", source).expect("lower v2 source");
        let exports = value.as_array().expect("exports array");
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].get("kind").and_then(JsonValue::as_str), Some("app"));
        assert_eq!(
            exports[0].get("id").and_then(JsonValue::as_str),
            Some("minimal-app")
        );
        assert_eq!(
            exports[0].get("default_scene").and_then(JsonValue::as_str),
            Some("home")
        );
    }
}
