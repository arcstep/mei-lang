//! T2 page / board scene_export 自动推导 deferred warmup 条目。

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use crate::mei_config::{is_plane_structure_mei_path, WorkspaceWarmupDatasetConfig};

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
        let focus = rel.replace('\\', "/");
        let legacy_capsule = is_legacy_t2_page_capsule(focus.as_str());
        let plane_capsule = is_plane_structure_mei_path(focus.as_str());
        if !legacy_capsule && !plane_capsule {
            continue;
        }
        let source = fs::read_to_string(path)?;
        if plane_capsule && !legacy_capsule && !source.contains("page_instance(") {
            // Pure plane layout files (no page_instance) are not T2 warmup capsules.
            continue;
        }
        let blocks = if legacy_capsule {
            split_named_blocks(source.as_str(), "scene_export")
        } else {
            split_named_blocks(source.as_str(), "page_instance")
        };
        for block in blocks {
            let scene_id = if legacy_capsule {
                extract_quoted_after_key(block, "id")
            } else {
                extract_quoted_after_key(block, "scene")
                    .or_else(|| extract_quoted_after_key(block, "id"))
            };
            let Some(scene_id) = scene_id else {
                continue;
            };
            let dataset_id = extract_example_rowset_dataset_id(block).unwrap_or_default();
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
        if suggestion.dataset_id.trim().is_empty() {
            continue;
        }
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

fn split_named_blocks<'a>(source: &'a str, name: &str) -> Vec<&'a str> {
    let needle = format!("{name}(");
    let mut blocks = Vec::new();
    let mut rest = source;
    while let Some(idx) = rest.find(needle.as_str()) {
        let tail = &rest[idx..];
        let end = find_matching_paren_end(tail, needle.len() - 1).unwrap_or(tail.len());
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

fn is_legacy_t2_page_capsule(rel: &str) -> bool {
    rel.ends_with(".page.mei") || rel.ends_with(".board.mei")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn temp_app_root(tag: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "mei-warmup-t2-{}-{}-{}",
            tag,
            std::process::id(),
            stamp
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("app root");
        root
    }

    #[test]
    fn discover_suggests_plane_page_instance_and_skips_pure_plane() {
        let app_root = temp_app_root("plane-pi");
        let section = app_root.join("src/scene/home/t1/region-main/section-metric");
        fs::create_dir_all(&section).expect("section dir");
        fs::write(
            section.join("plane-analytics.mei"),
            r#"
plane_layout(id = "p-analytics", key = "demo/plane-analytics", tier = "t2", layout = grid(rows = ["1fr"], columns = ["1fr"], areas = [["main"]]), regions = [])
page_instance(
    key = "demo/plane-analytics",
    scene = "fx_analytics_page",
    examples = [
        {
            "params": {
                "rowset_dataset_id": "fx_rows",
            },
        },
    ],
)
"#,
        )
        .expect("plane-analytics");
        fs::create_dir_all(app_root.join("src/scene/home/t1")).expect("t1 dir");
        fs::write(
            app_root.join("src/scene/home/t1/plane.mei"),
            r#"
plane_layout(
    id = "t1",
    key = "demo/home/t1",
    tier = "t1",
    layout = grid(rows = ["1fr"], columns = ["1fr"], areas = [["main"]]),
    regions = [],
)
"#,
        )
        .expect("pure plane");
        fs::create_dir_all(app_root.join("src/overlay")).expect("overlay");
        fs::write(
            app_root.join("src/overlay/legacy.board.mei"),
            r#"
scene_export(
    id = "legacy_board_page",
    examples = [
        {
            "params": {
                "rowset_dataset_id": "legacy_ds",
            },
        },
    ],
)
"#,
        )
        .expect("legacy board");

        let suggestions = discover_board_warmup_suggestions(&app_root).expect("discover");
        assert!(
            suggestions.iter().any(|s| {
                s.scene_id == "fx_analytics_page"
                    && s.focus.ends_with("plane-analytics.mei")
                    && s.dataset_id == "fx_rows"
            }),
            "plane page_instance should be suggested: {suggestions:?}"
        );
        assert!(
            suggestions
                .iter()
                .any(|s| s.scene_id == "legacy_board_page" && s.focus.ends_with(".board.mei")),
            "legacy board should still work: {suggestions:?}"
        );
        assert!(
            suggestions.iter().all(|s| !s.focus.ends_with("/plane.mei")),
            "pure plane.mei must be skipped: {suggestions:?}"
        );
        let _ = fs::remove_dir_all(app_root);
    }
}
