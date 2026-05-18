use std::path::Path as FsPath;

use super::super::super::scene_api::{
    build_world_context_snapshot, ResourceInventoryItem, WorldScope,
};

fn format_related_inventory_entry(item: &ResourceInventoryItem) -> String {
    let mut s = format!("{}:{}", item.id, item.resource_type);
    if let Some(p) = item
        .source_path
        .as_deref()
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        s.push('|');
        s.push_str(p);
    }
    if let Some(t) = item
        .title
        .as_deref()
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        s.push('|');
        s.push_str(t);
    } else if let Some(sum) = item
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        s.push('|');
        let cap = 80usize;
        if sum.chars().count() > cap {
            s.extend(sum.chars().take(cap));
            s.push('…');
        } else {
            s.push_str(sum);
        }
    }
    s
}

pub(super) fn append_world_context_error_lines(
    lines: &mut Vec<String>,
    app_id: &str,
    message: &str,
) {
    let is_scope_mismatch = message.contains("is not bound to target")
        || message.contains("scene `")
        || message.contains("does not provide a scene contract");
    if is_scope_mismatch {
        tracing::debug!(app_id = %app_id, error = %message, "skip world snapshot due to scope mismatch");
    } else {
        tracing::warn!(app_id = %app_id, error = %message, "failed to build world context snapshot");
    }
    lines.push(String::new());
    lines.push("[World Index]".to_string());
    lines.push(format!("status=unavailable ({})", message));
    lines.push(format!(
        "hint: `read_file` 路径相对于 workspace 根；应用内 `.mei` 常见为 `{app_id}/data/...`（裸写 `data/...` 会解析到 workspace 下错误目录）。dataset 行/结构问题优先用 `dataset_query`，指标值问题优先用 `dataset_metric`（都不要读 `.xlsx`）；若 scope 仍失败，用 `read_file` 读目标 `.mei` 核对其中 `scene(id=...)`。"
    ));
}

pub(super) fn append_world_context_snapshot_lines(
    lines: &mut Vec<String>,
    snapshot: &super::super::super::scene_api::WorldContextSnapshot,
) {
    if !snapshot.prompt_catalog_lines.is_empty() {
        lines.push(String::new());
        lines.extend(snapshot.prompt_catalog_lines.iter().cloned());
    }

    lines.push(String::new());
    lines.push("[World Index — compact]".to_string());
    lines.push(format!(
        "scene={} target_file={} world={} resources={} entities={}",
        snapshot.world_snapshot.scene_id,
        snapshot.active_target_file,
        snapshot
            .world_snapshot
            .world_id
            .as_deref()
            .unwrap_or("unknown"),
        snapshot.world_snapshot.world_resource_count,
        snapshot.world_snapshot.world_entity_count,
    ));
    if !snapshot
        .world_snapshot
        .world_resource_kind_counts
        .is_empty()
    {
        lines.push(format!(
            "resource_kind_counts: {}",
            serde_json::to_string(&snapshot.world_snapshot.world_resource_kind_counts)
                .unwrap_or_else(|_| "{}".to_string())
        ));
    }
    if !snapshot.world_snapshot.world_key_resource_ids.is_empty() {
        lines.push(format!(
            "key_resource_ids: {}",
            snapshot.world_snapshot.world_key_resource_ids.join(", ")
        ));
    }
    const MAX_RELATED_LABELS: usize = 28;
    let related: Vec<String> = snapshot
        .resource_inventory
        .items
        .iter()
        .filter(|item| item.related_to_target)
        .map(format_related_inventory_entry)
        .take(MAX_RELATED_LABELS)
        .collect();
    if !related.is_empty() {
        lines.push(format!("related_items: {}", related.join("; ")));
    }
    let related_total = snapshot
        .resource_inventory
        .items
        .iter()
        .filter(|item| item.related_to_target)
        .count();
    if related_total > MAX_RELATED_LABELS {
        lines.push(format!(
            "related_items_omitted: {} (dataset items use dataset_query)",
            related_total - MAX_RELATED_LABELS
        ));
    }
    lines.push(format!(
        "runtime: phase={} result={} actions=[{}]",
        snapshot.runtime_summary.phase,
        snapshot.runtime_summary.result,
        snapshot.runtime_summary.available_actions.join(", ")
    ));
    lines.push(format!(
        "resource_inventory: related_items below are file/scene hints; [World — catalog] above is authoritative for world.resources ids. For dataset resources, use dataset_query(id) for rows/schema and dataset_metric(id) for metric values."
    ));
}

pub(super) fn append_world_context_lines(
    lines: &mut Vec<String>,
    source_root: &FsPath,
    app_id: &str,
    scope: &WorldScope,
) {
    match build_world_context_snapshot(source_root, app_id, Some(scope)) {
        Ok(snapshot) => append_world_context_snapshot_lines(lines, &snapshot),
        Err(error) => append_world_context_error_lines(lines, app_id, &error.to_string()),
    }
}
