use std::{fs, path::Path};

use anyhow::{Context, Result};
use mei_surface::surface_catalog;
use serde_json::{json, Value as JsonValue};

use crate::mei_config::{forbidden_authoring_tokens, validate_authoring_policy};

pub fn describe_dsl() -> JsonValue {
    let public_surface: Vec<&str> = surface_catalog()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    json!({
        "schema_version": "0.2.0",
        "source": ["mei-surface/catalog"],
        "runtime": {
            "evaluator": "mei-lower",
            "dialect": "mei-surface/native-v2",
        },
        "forbidden_tokens": forbidden_authoring_tokens(),
        "public_surface": public_surface,
    })
}

fn validate_policy(source: &str) -> Result<()> {
    validate_authoring_policy(source)
}

pub fn evaluate_mei_source(filename: &str, source: &str) -> Result<JsonValue> {
    validate_policy(source)?;
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
    default_stage = "home",
    scene = scene_ref(scene_file = "scenes/home.mei"),
)
"#;
        let value = evaluate_mei_source("minimal-app.mei", source).expect("lower v2 source");
        let exports = value.as_array().expect("exports array");
        assert_eq!(exports.len(), 1);
        assert_eq!(
            exports[0].get("kind").and_then(JsonValue::as_str),
            Some("app")
        );
        assert_eq!(
            exports[0].get("id").and_then(JsonValue::as_str),
            Some("minimal-app")
        );
        assert_eq!(
            exports[0].get("default_stage").and_then(JsonValue::as_str),
            Some("home")
        );
    }
}
