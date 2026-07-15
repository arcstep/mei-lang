//! Synthesize / overlay `app_skeleton` GraphBlocks from `app.toml` (0120 C4).

use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::lower::GraphBlock;

const SKELETON_SCHEMA: &str = "mei-app-skeleton-artifact-v1";

#[derive(Debug, Default, Deserialize)]
struct AppTomlIdentity {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    default_stage: Option<String>,
    #[serde(default)]
    app_id: Option<String>,
}

/// Ensure an `app_skeleton:{app_id}` block exists and that product fields match `app.toml`.
///
/// - Missing block → synthesize from toml + folder `app_id`.
/// - Author-written block → overlay `title` / `default_stage` from toml when present (C4).
pub fn synthesize_app_skeleton(app_root: &Path, app_id: &str, blocks: &mut Vec<GraphBlock>) {
    let identity = read_app_toml_identity(app_root);
    let id = identity
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(app_id);
    let title = identity
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(app_id)
        .to_string();
    let default_stage = identity
        .default_stage
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("home")
        .to_string();

    let block_id = format!("app_skeleton:{id}");
    if let Some(existing) = blocks.iter_mut().find(|b| b.kind == "app_skeleton") {
        if let Some(obj) = existing.payload.as_object_mut() {
            if identity.title.as_deref().map(str::trim).is_some_and(|s| !s.is_empty()) {
                obj.insert("title".to_string(), json!(title));
            }
            if identity
                .default_stage
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty())
            {
                obj.insert("default_stage".to_string(), json!(default_stage));
            }
            obj.insert("id".to_string(), json!(id));
        }
        existing.block_id = block_id;
        return;
    }

    blocks.push(GraphBlock {
        kind: "app_skeleton".to_string(),
        block_id,
        schema: SKELETON_SCHEMA.to_string(),
        payload: json!({
            "id": id,
            "title": title,
            "default_stage": default_stage,
            "__mei_synthesized_app_skeleton": true,
        }),
    });
}

fn read_app_toml_identity(app_root: &Path) -> AppTomlIdentity {
    let path = app_root.join("app.toml");
    if !path.is_file() {
        return AppTomlIdentity::default();
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return AppTomlIdentity::default();
    };
    toml::from_str(&raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn synthesizes_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        fs::write(
            app.join("app.toml"),
            r#"title = "Hello"
default_stage = "home"
app_id = "hello-mdx"
"#,
        )
        .unwrap();

        let mut blocks = Vec::new();
        synthesize_app_skeleton(app, "hello-mdx", &mut blocks);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_id, "app_skeleton:hello-mdx");
        assert_eq!(
            blocks[0].payload.get("title").and_then(|v| v.as_str()),
            Some("Hello")
        );
        assert_eq!(
            blocks[0]
                .payload
                .get("__mei_synthesized_app_skeleton")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn overlays_toml_fields_on_author_skeleton() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        fs::write(
            app.join("app.toml"),
            r#"title = "From Toml"
default_stage = "home"
"#,
        )
        .unwrap();

        let mut blocks = vec![GraphBlock {
            kind: "app_skeleton".to_string(),
            block_id: "app_skeleton:demo".to_string(),
            schema: SKELETON_SCHEMA.to_string(),
            payload: json!({
                "id": "demo",
                "title": "From Skeleton",
                "default_stage": "other",
            }),
        }];
        synthesize_app_skeleton(app, "demo", &mut blocks);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].payload.get("title").and_then(|v| v.as_str()),
            Some("From Toml")
        );
        assert_eq!(
            blocks[0]
                .payload
                .get("default_stage")
                .and_then(|v| v.as_str()),
            Some("home")
        );
    }
}
