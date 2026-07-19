//! Lower Cockpit `*.stage.mdx` into Graph blocks (Phase 4).

use mei_syntax::{parse_cockpit_stage_file, CockpitStageFile, StageMdxError};
use serde_json::json;
use std::path::Path;

use crate::lower::{GraphBlock, GraphOutcome};

pub fn cockpit_stage_to_graph(source_file: &str, doc: &CockpitStageFile) -> GraphOutcome {
    let fills: Vec<_> = doc
        .fills
        .iter()
        .map(|f| {
            json!({
                "slot": f.slot,
                "content": f.content,
                "line": f.line,
            })
        })
        .collect();
    let steps: Vec<_> = doc
        .steps
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "target": s.target,
                "caption": s.caption.as_ref().map(|m| m.markdown.clone()),
                "speaker_notes": s.speaker_notes.as_ref().map(|m| m.markdown.clone()),
                "line": s.line,
            })
        })
        .collect();
    let payload = json!({
        "stage_id": doc.frontmatter.stage_id,
        "profile": doc.frontmatter.profile,
        "title": doc.frontmatter.title,
        "scene_use": doc.scene_use,
        "fills": fills,
        "steps": steps,
        "source_file": source_file,
    });
    let block_id = format!("stage_mdx:{}", doc.frontmatter.stage_id);
    GraphOutcome {
        graph_schema_version: "mei-compiler-graph-v2".to_string(),
        source_file: source_file.to_string(),
        blocks: vec![GraphBlock {
            kind: "stage_mdx".to_string(),
            block_id,
            schema: "mei-stage-mdx-cockpit-v1".to_string(),
            payload,
        }],
    }
}

pub fn compile_cockpit_stage_file(
    path: &Path,
    source_file: &str,
) -> Result<GraphOutcome, StageMdxError> {
    let doc = parse_cockpit_stage_file(path)?;
    Ok(cockpit_stage_to_graph(source_file, &doc))
}
