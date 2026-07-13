use mei_syntax::v2::{CallArgs, V2Item, V2SourceFile};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use thiserror::Error;

use crate::artifact_expand;

#[derive(Debug, Error)]
pub enum LowerGraphError {
    #[error("{0}")]
    Lower(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphBlock {
    pub kind: String,
    pub block_id: String,
    pub schema: String,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphOutcome {
    pub graph_schema_version: String,
    pub source_file: String,
    pub blocks: Vec<GraphBlock>,
}

pub fn lower_v2_file(
    source_file: &str,
    file: &V2SourceFile,
) -> Result<GraphOutcome, LowerGraphError> {
    let mut blocks = Vec::new();
    for item in &file.items {
        if let V2Item::TopLevel { name, args } = item {
            blocks.push(lower_top_level(source_file, name, args)?);
        }
    }
    Ok(GraphOutcome {
        graph_schema_version: "mei-compiler-graph-v2".to_string(),
        source_file: source_file.to_string(),
        blocks,
    })
}

fn lower_top_level(
    source_file: &str,
    name: &str,
    args: &CallArgs,
) -> Result<GraphBlock, LowerGraphError> {
    let mut payload = call_args_to_json(args)?;
    if matches!(
        name,
        "scene"
            | "presentation"
            | "plane_layout"
            | "region_layout"
            | "section_layout"
            | "slide_layout"
            | "map_spec"
            | "view_spec"
            | "page_instance"
    ) {
        if let Some(obj) = payload.as_object_mut() {
            obj.entry("source_file".to_string())
                .or_insert(JsonValue::String(source_file.to_string()));
            if obj.get("key").is_none() {
                let block_id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        LowerGraphError::Lower(format!(
                            "{name} top-level must declare non-empty `id`"
                        ))
                    })?;
                obj.insert(
                    "key".to_string(),
                    JsonValue::String(format!("{block_id}@{source_file}")),
                );
            }
        }
    }
    if name == "slide_layout" {
        validate_slide_layout_payload(&payload)?;
    }
    let block_id = derive_block_id(name, source_file, &payload)?;
    let schema = schema_for_constructor(name);
    Ok(GraphBlock {
        kind: name.to_string(),
        block_id,
        schema: schema.to_string(),
        payload,
    })
}

fn validate_slide_layout_payload(payload: &JsonValue) -> Result<(), LowerGraphError> {
    let obj = payload
        .as_object()
        .ok_or_else(|| LowerGraphError::Lower("slide_layout payload must be object".into()))?;
    if let Some(pattern) = obj.get("pattern").and_then(|v| v.as_str()) {
        if mei_syntax::v2::slide_pattern_areas(pattern).is_none() {
            return Err(LowerGraphError::Lower(format!(
                "slide_layout unknown pattern `{pattern}`; expected one of: {}",
                mei_syntax::v2::SLIDE_PATTERNS.join(", ")
            )));
        }
    }
    Ok(())
}

fn schema_for_constructor(name: &str) -> &'static str {
    match name {
        "app_skeleton" => "mei-app-skeleton-artifact-v1",
        "scene" => "mei-scene-semantic-v1",
        "presentation" => "mei-presentation-semantic-v1",
        "plane_layout" | "region_layout" | "section_layout" => "mei-scene-layout-fragment-v1",
        "slide_layout" => "mei-presentation-slide-fragment-v1",
        "map_spec" => "mei-map-spec-v1",
        "view_spec" => "mei-view-spec-v1",
        "page_instance" => "mei-projection-assembly-v1",
        "content_panel" => "mei-panel-contract-artifact-v1",
        "metric_def_bundle" => "mei-metric-def-bundle-artifact-v1",
        "navigation" | "link_decl" => "mei-navigation-artifact-v1",
        "warmup_policy" => "mei-warmup-policy-artifact-v1",
        _ => "mei-graph-block-v2",
    }
}

fn derive_block_id(
    name: &str,
    source_file: &str,
    payload: &JsonValue,
) -> Result<String, LowerGraphError> {
    let obj = payload
        .as_object()
        .ok_or_else(|| LowerGraphError::Lower("payload must be object".into()))?;
    match name {
        "app_skeleton" => kw_string(obj, "id").map(|id| format!("app_skeleton:{id}")),
        "scene" | "presentation" => {
            let stage_id = kw_string(obj, "id")?;
            let key = obj
                .get("key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{stage_id}@{source_file}"));
            Ok(format!("{name}:{key}"))
        }
        "plane_layout" | "region_layout" | "section_layout" | "slide_layout" | "map_spec"
        | "view_spec" => {
            let id = kw_string(obj, "id")?;
            let key = obj
                .get("key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{id}@{source_file}"));
            Ok(format!("{name}:{key}"))
        }
        "navigation" | "link_decl" => kw_string(obj, "key").map(|key| format!("{name}:{key}")),
        "page_instance" => kw_string(obj, "key").map(|key| format!("page_instance:{key}")),
        "content_panel" => {
            if let Some(key) = obj.get("key").and_then(|value| value.as_str()) {
                return Ok(format!("content_panel:{key}"));
            }
            let id = kw_string(obj, "id")?;
            if let Some(scope) = obj.get("scope").and_then(|v| v.as_str()) {
                Ok(format!("content_panel:{scope}:{id}"))
            } else {
                Ok(format!("content_panel:{id}"))
            }
        }
        "metric_def_bundle" => kw_string(obj, "key").map(|key| format!("metric_def_bundle:{key}")),
        "world" => kw_string(obj, "id").map(|id| format!("world_model:{id}")),
        "warmup_policy" => {
            let scope = obj.get("scope").cloned().unwrap_or(JsonValue::Null);
            Ok(format!("warmup_policy:{scope}"))
        }
        other => Ok(format!("{other}:anonymous")),
    }
}

fn kw_string(obj: &Map<String, JsonValue>, key: &str) -> Result<String, LowerGraphError> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| LowerGraphError::Lower(format!("missing string field `{key}`")))
}

fn call_args_to_json(args: &CallArgs) -> Result<JsonValue, LowerGraphError> {
    let mut map = Map::new();
    for (idx, expr) in args.positional.iter().enumerate() {
        map.insert(
            format!("arg{idx}"),
            artifact_expand::expr_to_json(expr)
                .map_err(|e| LowerGraphError::Lower(e.to_string()))?,
        );
    }
    for (name, expr) in &args.keywords {
        map.insert(
            name.clone(),
            artifact_expand::expr_to_json(expr)
                .map_err(|e| LowerGraphError::Lower(e.to_string()))?,
        );
    }
    Ok(JsonValue::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_syntax::v2::parse_v2_source;

    #[test]
    fn lowers_presentation_and_slide_schemas() {
        let source = r#"
presentation(
    id = "intro",
    planes = [plane_ref(id = "p")],
)

slide_layout(
    id = "slide-01-cover",
    title = "Cover",
    pattern = "full_bleed",
    regions = [region_ref(id = "r-main")],
)
"#;
        let file = parse_v2_source(source).expect("parse");
        let outcome = lower_v2_file("presentation/intro/presentation.mei", &file).expect("lower");
        let presentation = outcome
            .blocks
            .iter()
            .find(|b| b.kind == "presentation")
            .expect("presentation block");
        assert_eq!(presentation.schema, "mei-presentation-semantic-v1");
        assert!(presentation.block_id.starts_with("presentation:"));
        let slide = outcome
            .blocks
            .iter()
            .find(|b| b.kind == "slide_layout")
            .expect("slide_layout block");
        assert_eq!(slide.schema, "mei-presentation-slide-fragment-v1");
        assert!(slide.block_id.starts_with("slide_layout:"));
    }

    #[test]
    fn rejects_unknown_slide_pattern() {
        let source = r#"
slide_layout(
    id = "slide-bad",
    pattern = "two_columns",
    regions = [region_ref(id = "r-main")],
)
"#;
        let file = parse_v2_source(source).expect("parse");
        let err = lower_v2_file("p/slide-bad.mei", &file).expect_err("unknown pattern");
        let msg = err.to_string();
        assert!(
            msg.contains("unknown pattern") && msg.contains("two_columns"),
            "unexpected error: {msg}"
        );
    }
}
