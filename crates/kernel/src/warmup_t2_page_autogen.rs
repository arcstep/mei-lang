//! T2 page / board scene_export 自动推导 deferred warmup 条目。

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use crate::mei_config::WorkspaceWarmupDatasetConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedWarmupDatasetRequest {
    pub scene_id: String,
    pub focus: String,
    pub dataset_id: String,
    pub metric_id: Option<String>,
    pub priority: String,
}

pub fn board_warmup_autogen_enabled() -> bool {
    !matches!(
        std::env::var("MEI_WARMUP_BOARD_AUTOGEN")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("0" | "false" | "off" | "no")
    )
}

pub fn discover_board_warmup_suggestions(
    app_root: &Path,
) -> Result<Vec<SuggestedWarmupDatasetRequest>> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in WalkDir::new(app_root)
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || !e
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with('.') || name == "node_modules")
        })
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(rel) = path.strip_prefix(app_root).ok().and_then(|p| p.to_str()) else {
            continue;
        };
        if !is_t2_page_capsule(rel) {
            continue;
        }
        let focus = rel.replace('\\', "/");
        let source = fs::read_to_string(path)?;
        for block in split_scene_export_blocks(source.as_str()) {
            let Some(scene_id) = extract_quoted_after_key(block, "id") else {
                continue;
            };
            let Some(dataset_id) = extract_example_rowset_dataset_id(block) else {
                continue;
            };
            let metric_id = extract_example_metric_ref(block);
            let key = format!("{scene_id}|{focus}|{dataset_id}");
            if !seen.insert(key) {
                continue;
            }
            out.push(SuggestedWarmupDatasetRequest {
                scene_id,
                focus: focus.clone(),
                dataset_id,
                metric_id,
                priority: "deferred".to_string(),
            });
        }
    }
    out.sort_by(|a, b| {
        a.scene_id
            .cmp(&b.scene_id)
            .then(a.focus.cmp(&b.focus))
            .then(a.dataset_id.cmp(&b.dataset_id))
    });
    Ok(out)
}

pub fn merge_workspace_and_board_warmup_requests(
    manual: &[WorkspaceWarmupDatasetConfig],
    app_root: &Path,
) -> Result<Vec<WorkspaceWarmupDatasetConfig>> {
    let mut merged = manual.to_vec();
    if !board_warmup_autogen_enabled() {
        return Ok(merged);
    }
    let mut override_keys = BTreeSet::new();
    for request in manual {
        let scene = request.scene_id.as_deref().unwrap_or("").trim();
        let dataset = request.dataset_id.trim();
        if !scene.is_empty() && !dataset.is_empty() {
            override_keys.insert(format!("{scene}|{dataset}"));
        }
    }
    for suggestion in discover_board_warmup_suggestions(app_root)? {
        let key = format!("{}|{}", suggestion.scene_id, suggestion.dataset_id);
        if override_keys.contains(key.as_str()) {
            continue;
        }
        merged.push(WorkspaceWarmupDatasetConfig {
            scene_id: Some(suggestion.scene_id),
            dataset_id: suggestion.dataset_id,
            metric_id: suggestion.metric_id,
            metric_ids: Vec::new(),
            focus: Some(suggestion.focus),
            priority: Some(suggestion.priority),
        });
    }
    Ok(merged)
}

fn split_scene_export_blocks(source: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = source;
    while let Some(idx) = rest.find("scene_export(") {
        let tail = &rest[idx..];
        let end = find_matching_paren_end(tail, "scene_export(".len() - 1).unwrap_or(tail.len());
        blocks.push(&tail[..end]);
        rest = &tail[end..];
    }
    blocks
}

fn find_matching_paren_end(text: &str, open_idx: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    for (i, ch) in text.char_indices().skip(open_idx) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        if i >= bytes.len() {
            break;
        }
    }
    None
}

fn extract_quoted_after_key(block: &str, key: &str) -> Option<String> {
    let needle = format!("{key} = \"");
    let start = block.find(needle.as_str())? + needle.len();
    let rest = &block[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_example_rowset_dataset_id(block: &str) -> Option<String> {
    if let Some(value) = extract_json_string_field(block, "rowset_dataset_id") {
        return Some(value);
    }
    let needle = "rowset_dataset_id = \"";
    let start = block.find(needle)? + needle.len();
    let rest = &block[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_json_string_field(block: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\": \"");
    let start = block.find(needle.as_str())? + needle.len();
    let rest = &block[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_example_metric_ref(block: &str) -> Option<String> {
    let needle = "metric_ref(\"";
    let start = block.find(needle)? + needle.len();
    let rest = &block[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn is_t2_page_capsule(rel: &str) -> bool {
    rel.ends_with(".page.mei") || rel.ends_with(".board.mei")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_t2_page_export_fields_from_block() {
        let block = r#"
scene_export(
    id = "penalty_total_analytics_page",
    examples = [
        {
            "params": {
                "metric": metric_ref("penalties_total_count"),
                "rowset_dataset_id": "penalty_result_dashboard_ds",
            },
        },
    ],
)
"#;
        assert_eq!(
            extract_quoted_after_key(block, "id").as_deref(),
            Some("penalty_total_analytics_page")
        );
        assert_eq!(
            extract_example_rowset_dataset_id(block).as_deref(),
            Some("penalty_result_dashboard_ds")
        );
        assert_eq!(
            extract_example_metric_ref(block).as_deref(),
            Some("penalties_total_count")
        );
    }
}
