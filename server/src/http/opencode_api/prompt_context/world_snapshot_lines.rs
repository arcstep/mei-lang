use std::path::Path as FsPath;

use super::super::super::scene_api::{build_world_context_snapshot, WorldScope};

pub(super) fn append_world_context_lines(
    lines: &mut Vec<String>,
    source_root: &FsPath,
    app_id: &str,
    scope: &WorldScope,
) {
    let snapshot = match build_world_context_snapshot(source_root, app_id, Some(scope)) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(app_id = %app_id, %error, "failed to build world context snapshot");
            return;
        }
    };

    lines.push(String::new());
    lines.push("[World Snapshot]".to_string());
    lines.push(format!("scene_id: {}", snapshot.world_snapshot.scene_id));
    lines.push(format!("entry_target: {}", snapshot.entry_target));
    lines.push(format!(
        "world_id: {}",
        snapshot
            .world_snapshot
            .world_id
            .as_deref()
            .unwrap_or("unknown")
    ));
    lines.push(format!(
        "resource_count: {}",
        snapshot.world_snapshot.world_resource_count
    ));
    lines.push(format!(
        "entity_count: {}",
        snapshot.world_snapshot.world_entity_count
    ));
    if let Some(topology) = snapshot.world_snapshot.world_topology.as_deref() {
        lines.push(format!("topology: {topology}"));
    }
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
    if !snapshot.world_snapshot.world_entity_kind_counts.is_empty() {
        lines.push(format!(
            "entity_kind_counts: {}",
            serde_json::to_string(&snapshot.world_snapshot.world_entity_kind_counts)
                .unwrap_or_else(|_| "{}".to_string())
        ));
    }
    if !snapshot.world_snapshot.world_key_resource_ids.is_empty() {
        lines.push(format!(
            "key_resource_ids: {}",
            snapshot.world_snapshot.world_key_resource_ids.join(", ")
        ));
    }
    if !snapshot.world_snapshot.world_key_entity_ids.is_empty() {
        lines.push(format!(
            "key_entity_ids: {}",
            snapshot.world_snapshot.world_key_entity_ids.join(", ")
        ));
    }

    lines.push(String::new());
    lines.push("[Runtime Summary]".to_string());
    lines.push(format!("phase: {}", snapshot.runtime_summary.phase));
    lines.push(format!("result: {}", snapshot.runtime_summary.result));
    lines.push(format!("countdown: {}", snapshot.runtime_summary.countdown));
    lines.push(format!(
        "scene_view_entities: {}",
        snapshot.runtime_summary.scene_view_entities
    ));
    lines.push(format!(
        "scene_view_cells: {}",
        snapshot.runtime_summary.scene_view_cells
    ));
    if !snapshot.runtime_summary.available_actions.is_empty() {
        lines.push(format!(
            "available_actions: {}",
            snapshot.runtime_summary.available_actions.join(", ")
        ));
    }
    if !snapshot.runtime_summary.recent_trace_messages.is_empty() {
        lines.push(format!(
            "recent_trace_messages: {}",
            snapshot.runtime_summary.recent_trace_messages.join(" | ")
        ));
    }

    lines.push(String::new());
    lines.push("[World Query Capabilities]".to_string());
    for capability in snapshot.query_capabilities {
        lines.push(format!(
            "- id: {} | status: {} | purpose: {}",
            capability.id, capability.status, capability.purpose
        ));
        lines.push(format!("  input: {}", capability.input));
        lines.push(format!("  output: {}", capability.output));
    }

    lines.push(String::new());
    lines.push("[World Query Skill]".to_string());
    lines.push(
        "1) 先基于 world_snapshot 与 runtime_summary 回答；如果信息不足，再选择对应 world 查询能力。"
            .to_string(),
    );
    lines.push(
        "2) 优先围绕 world 核心资产（entity/resource/topology/relation）推理，不要回退到整个工作区源码。"
            .to_string(),
    );
    lines.push(
        "3) 访问侧默认只读，不直接改写正式作者态；涉及结构修改时，先提出 session patch 建议。"
            .to_string(),
    );
    lines.push(
        "4) 如需显式触发查询，可使用 /world 指令：/world context | /world assets [kind] [limit] | /world asset <id> | /world runtime [trace_limit]。"
            .to_string(),
    );
    lines.push(
        "5) world 查询默认绑定当前 scene；当 app 内存在多个 scene 时，不要跨 scene 混合推理。"
            .to_string(),
    );
}
