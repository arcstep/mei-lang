//! Synthesize access navigation GraphBlocks from discovered Stage Programs (0119 graph closure).

use mei_syntax::{discover_stage_programs, DiscoveredStageProgram};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;

use crate::lower::GraphBlock;

const NAV_SCHEMA: &str = "mei-navigation-artifact-v1";

/// Append compiler-synthesized `navigation:access:{stage_id}` blocks for Stage Programs
/// that do not already have an author-written access navigation with the same key.
pub fn synthesize_stage_access_navigation(
    app_root: &Path,
    app_id: &str,
    blocks: &mut Vec<GraphBlock>,
) {
    let programs = discover_stage_programs(app_root);
    if programs.is_empty() {
        return;
    }
    let existing_access_keys = existing_access_navigation_keys(blocks);
    for prog in &programs {
        let access_key = format!("access:{}", prog.stage_id);
        if existing_access_keys.contains(access_key.as_str()) {
            continue;
        }
        blocks.push(synthesized_access_navigation(app_id, prog));
    }
}

fn existing_access_navigation_keys(blocks: &[GraphBlock]) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for block in blocks {
        if block.kind != "navigation" {
            continue;
        }
        let key = block
            .payload
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if key.starts_with("access:") {
            keys.insert(key.to_string());
        }
        if block.block_id.starts_with("navigation:access:") {
            keys.insert(
                block
                    .block_id
                    .strip_prefix("navigation:")
                    .unwrap_or(block.block_id.as_str())
                    .to_string(),
            );
        }
    }
    keys
}

fn synthesized_access_navigation(app_id: &str, prog: &DiscoveredStageProgram) -> GraphBlock {
    let access_key = format!("access:{}", prog.stage_id);
    let block_id = format!("navigation:{access_key}");
    let url = format!("/apps/{app_id}/{}", prog.stage_id);
    GraphBlock {
        kind: "navigation".to_string(),
        block_id,
        schema: NAV_SCHEMA.to_string(),
        payload: json!({
            "key": access_key,
            "scene": prog.stage_id,
            "stage": prog.stage_id,
            "url": url,
            "assembly": {
                "__ref": "assembly_ref",
                "__args": { "arg0": prog.assembly_key },
            },
            "__mei_synthesized_stage_closure": true,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn synthesizes_access_home_when_author_has_no_navigation() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        fs::create_dir_all(app.join("src/stage")).unwrap();
        fs::create_dir_all(app.join("src/scene")).unwrap();
        fs::write(
            app.join("src/stage/home.stage.mdx"),
            r#"---
stage_id: home
profile: cockpit
title: Home
---
@scene(use="scene/home")
"#,
        )
        .unwrap();
        fs::write(app.join("src/scene/home.mei"), "scene(id=\"home\")\n").unwrap();

        let mut blocks = Vec::new();
        synthesize_stage_access_navigation(app, "demo", &mut blocks);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_id, "navigation:access:home");
        assert_eq!(
            blocks[0]
                .payload
                .get("assembly")
                .and_then(|a| a.get("__args"))
                .and_then(|a| a.get("arg0"))
                .and_then(|v| v.as_str()),
            Some("home@src/scene/home.mei")
        );
        assert_eq!(
            blocks[0]
                .payload
                .get("__mei_synthesized_stage_closure")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn skips_when_author_already_wrote_access_home() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        fs::create_dir_all(app.join("src/stage")).unwrap();
        fs::write(
            app.join("src/stage/home.stage.mdx"),
            r#"---
stage_id: home
profile: cockpit
---
@scene(use="scene/home")
"#,
        )
        .unwrap();

        let mut blocks = vec![GraphBlock {
            kind: "navigation".to_string(),
            block_id: "navigation:access:home".to_string(),
            schema: NAV_SCHEMA.to_string(),
            payload: json!({
                "key": "access:home",
                "scene": "home",
                "url": "/apps/demo/home",
                "assembly": {
                    "__ref": "assembly_ref",
                    "__args": { "arg0": "home@src/scene/home.mei" },
                },
            }),
        }];
        synthesize_stage_access_navigation(app, "demo", &mut blocks);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0]
            .payload
            .get("__mei_synthesized_stage_closure")
            .is_none());
    }
}
