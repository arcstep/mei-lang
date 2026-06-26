use std::path::Path;

use mei_lang_kernel::{ReachabilityTreeNode, ReachabilityTreeRoot};

use crate::diagnostics::collect_materialization_diagnostics;
use crate::graph::mrg::registry::MrgRegistryWriter;

pub fn build_runtime_observability_roots(source_root: &Path, app_id: &str) -> Vec<ReachabilityTreeRoot> {
    vec![
        build_mrg_slots_root(source_root, app_id),
        build_warmup_root(source_root, app_id),
        build_l1_root(source_root, app_id),
        build_logs_root(source_root, app_id),
    ]
    .into_iter()
    .filter(|root| !root.children.is_empty() || root.group == "logs")
    .collect()
}

fn build_mrg_slots_root(source_root: &Path, app_id: &str) -> ReachabilityTreeRoot {
    let mrg = MrgRegistryWriter::load(source_root, app_id);
    let mut slots = mrg.slots;
    slots.sort_by(|left, right| {
        left.slot_id
            .node
            .key
            .cmp(&right.slot_id.node.key)
            .then(left.slot_id.scope_key.cmp(&right.slot_id.scope_key))
    });
    let children = slots
        .into_iter()
        .map(|slot| {
            let key = format!("{}@{}", slot.slot_id.node.key, slot.slot_id.scope_key);
            let mut badges = vec![material_state_label(slot.state).to_string()];
            if let Some(eval) = slot.last_eval.as_ref() {
                if eval.artifact_hit {
                    badges.push("artifact_hit".to_string());
                }
                if !eval.cache_layer.is_empty() {
                    badges.push(format!("cache:{}", eval.cache_layer));
                }
            }
            ReachabilityTreeNode {
                id: format!("mrg-slot-{key}"),
                node_id: format!("mrg-slot:{key}"),
                kind: "mrg_slot".to_string(),
                label: key,
                badges,
                compile_scene: String::new(),
                compile_target: String::new(),
                board_layout_zone: String::new(),
                children: Vec::new(),
            }
        })
        .collect();
    ReachabilityTreeRoot {
        group: "mrg_slots".to_string(),
        label: "MRG · Materialization".to_string(),
        default_open: true,
        children,
    }
}

fn build_warmup_root(source_root: &Path, app_id: &str) -> ReachabilityTreeRoot {
    let report = collect_materialization_diagnostics(
        source_root,
        app_id,
        &["build".to_string(), "mrg".to_string()],
    );
    let mut children = Vec::new();
    if report.mrg.slot_count > 0 {
        children.push(summary_node(
            "warmup-slots",
            "mrg-slot-summary",
            format!(
                "slots {} (ready {} / stale {} / failed {})",
                report.mrg.slot_count,
                report.mrg.ready_slots,
                report.mrg.stale_slots,
                report.mrg.failed_slots
            ),
            vec![format!("stale_ratio:{:.0}%", report.mrg.stale_ratio * 100.0)],
        ));
    }
    if let Some(skips) = report.build.mrg_eval_skips {
        children.push(summary_node(
            "warmup-mrg-skips",
            "warmup-mrg-skips",
            format!("mrg_eval_skips={skips}"),
            Vec::new(),
        ));
    }
    if let Some(source) = report.build.source.strip_prefix("startup:") {
        if !source.is_empty() {
            children.push(summary_node(
                "warmup-source",
                "warmup-source",
                format!("prebuild source={source}"),
                Vec::new(),
            ));
        }
    }
    ReachabilityTreeRoot {
        group: "mrg_warmup".to_string(),
        label: "MRG · Warmup".to_string(),
        default_open: false,
        children,
    }
}

fn build_l1_root(source_root: &Path, app_id: &str) -> ReachabilityTreeRoot {
    let report = collect_materialization_diagnostics(
        source_root,
        app_id,
        &["cache".to_string(), "eval".to_string(), "build".to_string()],
    );
    let children = vec![
        summary_node(
            "l1-server-policy",
            "l1-server-policy",
            format!(
                "graph_dedup={} slim={} canonical_persist={}",
                report.cache.graph_registry_dedup,
                report.cache.access_slim_artifacts,
                report.cache.canonical_artifact_persist
            ),
            Vec::new(),
        ),
        summary_node(
            "l1-eval-index",
            "l1-eval-index",
            format!(
                "eval_files={} response_files={} mrg_eval_skips={}",
                report.eval.eval_total_files,
                report.eval.metric_response_files,
                report.build.mrg_eval_skips.unwrap_or(0)
            ),
            Vec::new(),
        ),
    ];
    ReachabilityTreeRoot {
        group: "l1_cache".to_string(),
        label: "L1 · Cache".to_string(),
        default_open: false,
        children,
    }
}

fn build_logs_root(source_root: &Path, app_id: &str) -> ReachabilityTreeRoot {
    let report = collect_materialization_diagnostics(source_root, app_id, &[]);
    let mut children: Vec<ReachabilityTreeNode> = report
        .alerts
        .into_iter()
        .enumerate()
        .map(|(index, alert)| {
            summary_node(
                format!("log-{index}"),
                format!("log:{index}"),
                alert,
                Vec::new(),
            )
        })
        .collect();
    if children.is_empty() {
        children.push(summary_node(
            "log-empty",
            "log:empty",
            "无活跃告警".to_string(),
            Vec::new(),
        ));
    }
    ReachabilityTreeRoot {
        group: "logs".to_string(),
        label: "Logs / Events".to_string(),
        default_open: false,
        children,
    }
}

fn material_state_label(state: crate::graph::types::MaterialState) -> &'static str {
    use crate::graph::types::MaterialState;
    match state {
        MaterialState::Missing => "missing",
        MaterialState::Warming => "warming",
        MaterialState::Ready => "ready",
        MaterialState::Stale => "stale",
        MaterialState::Failed => "failed",
    }
}

fn summary_node(
    id: impl Into<String>,
    node_id: impl Into<String>,
    label: impl Into<String>,
    badges: Vec<String>,
) -> ReachabilityTreeNode {
    ReachabilityTreeNode {
        id: id.into(),
        node_id: node_id.into(),
        kind: "runtime_summary".to_string(),
        label: label.into(),
        badges,
        compile_scene: String::new(),
        compile_target: String::new(),
        board_layout_zone: String::new(),
        children: Vec::new(),
    }
}
