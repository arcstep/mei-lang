use std::path::Path;

use mei_lang_kernel::{ReachabilityTreeNode, ReachabilityTreeRoot};
use serde::Serialize;

use crate::diagnostics::{
    collect_materialization_diagnostics, format_age_ms, format_bytes_human,
    MaterializationDiagnosticsReport,
};
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::types::{GraphNodeKind, MaterialState};
use crate::http::host_api::registry_snapshot;
use crate::http::startup_run::now_ms_for_host_message;
use crate::prebuild::load_prebuild_report;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostContext {
    pub phase: String,
    pub app_phase: String,
    pub access_ready: bool,
    pub scope_gate_ready: bool,
    pub last_build_total_ms: Option<u64>,
    pub last_build_compile_ms: Option<u64>,
    pub last_build_warmup_ms: Option<u64>,
    pub gate_l2_miss: Option<usize>,
    pub gate_l3_fail: Option<usize>,
    pub gate_l4_stale: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePrebuildContext {
    pub ok: bool,
    pub scope_profile: Option<String>,
    pub total_wall_ms: Option<u64>,
    pub compile_scopes_ms: Option<u64>,
    pub scope_artifacts_ms: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
    pub current_rss_bytes: Option<u64>,
    pub compile_scope_count: Option<usize>,
    pub real_compile_count: Option<usize>,
    pub cache_hit_count: Option<usize>,
    pub expansion_ratio: Option<f64>,
    pub report_age: Option<String>,
    pub in_succeeded_apps: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeObservabilitySnapshot {
    pub app_id: String,
    pub roots: Vec<ReachabilityTreeRoot>,
    pub diagnostics: MaterializationDiagnosticsReport,
    pub host: RuntimeHostContext,
    pub prebuild: RuntimePrebuildContext,
}

pub fn build_runtime_observability_snapshot(
    source_root: &Path,
    app_id: &str,
) -> RuntimeObservabilitySnapshot {
    let report = collect_materialization_diagnostics(source_root, app_id, &[], None, None);
    let host = build_host_context(app_id);
    let prebuild = build_prebuild_context(source_root, app_id, &report);
    let roots = build_layer_roots(source_root, app_id, &report, &host, &prebuild);
    RuntimeObservabilitySnapshot {
        app_id: app_id.to_string(),
        roots,
        diagnostics: report,
        host,
        prebuild,
    }
}

fn build_host_context(app_id: &str) -> RuntimeHostContext {
    let snapshot = registry_snapshot();
    let app = snapshot.apps.iter().find(|app| app.app_id == app_id);
    let (gate_l2_miss, gate_l3_fail, gate_l4_stale) = app
        .and_then(|app| app.gate_summary.as_ref())
        .map(|gate| (Some(gate.l2_miss), Some(gate.l3_fail), Some(gate.l4_stale)))
        .unwrap_or_else(|| {
            snapshot
                .gate_summary
                .as_ref()
                .map(|gate| (Some(gate.l2_miss), Some(gate.l3_fail), Some(gate.l4_stale)))
                .unwrap_or((None, None, None))
        });
    RuntimeHostContext {
        phase: snapshot.phase.clone(),
        app_phase: app
            .map(|app| app.phase.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        access_ready: app.map(|app| app.access_ready).unwrap_or(false),
        scope_gate_ready: snapshot.scope_gate_ready,
        last_build_total_ms: snapshot.last_build_total_ms,
        last_build_compile_ms: snapshot.last_build_compile_ms,
        last_build_warmup_ms: snapshot.last_build_warmup_ms,
        gate_l2_miss,
        gate_l3_fail,
        gate_l4_stale,
    }
}

fn build_prebuild_context(
    source_root: &Path,
    app_id: &str,
    diagnostics: &MaterializationDiagnosticsReport,
) -> RuntimePrebuildContext {
    let workspace_report = load_prebuild_report(source_root).ok().flatten();
    let app_report = workspace_report
        .as_ref()
        .and_then(|report| report.apps.iter().find(|app| app.app_id == app_id));
    let in_succeeded_apps = workspace_report
        .as_ref()
        .map(|report| report.succeeded_apps.iter().any(|id| id == app_id))
        .unwrap_or(false);
    let (peak_rss_bytes, current_rss_bytes, real_compile_count, cache_hit_count, expansion_ratio) =
        if let Some(app) = app_report {
            (
                Some(app.diagnostics.peak_rss_bytes),
                app.diagnostics.current_rss_bytes,
                Some(app.diagnostics.real_compile_count),
                Some(app.diagnostics.cache_hit_count),
                Some(app.diagnostics.expansion_ratio),
            )
        } else if let Some(report) = workspace_report.as_ref() {
            (
                Some(report.diagnostics.peak_rss_bytes),
                report.diagnostics.current_rss_bytes,
                Some(report.diagnostics.real_compile_count),
                Some(report.diagnostics.cache_hit_count),
                Some(report.diagnostics.expansion_ratio),
            )
        } else {
            (
                diagnostics.build.peak_rss_bytes,
                diagnostics.build.current_rss_bytes,
                None,
                diagnostics.build.compile_index_hits,
                None,
            )
        };
    let report_age = diagnostics
        .build
        .recorded_at_ms
        .map(|recorded_at| format_age_ms(recorded_at, now_ms_for_host_message() as u64));
    RuntimePrebuildContext {
        ok: workspace_report.as_ref().map(|report| report.ok).unwrap_or(false),
        scope_profile: workspace_report.as_ref().map(|report| match report.scope_profile {
            crate::prebuild::PrebuildScopeProfile::Full => "full".to_string(),
            crate::prebuild::PrebuildScopeProfile::HotOnly => "hot_only".to_string(),
            crate::prebuild::PrebuildScopeProfile::BlockScoped => "block_scoped".to_string(),
        }),
        total_wall_ms: app_report
            .map(|app| app.timings.total_wall_ms)
            .or_else(|| workspace_report.as_ref().map(|report| report.total_wall_ms)),
        compile_scopes_ms: app_report.map(|app| app.timings.compile_scopes_ms),
        scope_artifacts_ms: app_report.map(|app| app.timings.scope_artifacts_ms),
        peak_rss_bytes,
        current_rss_bytes,
        compile_scope_count: app_report.map(|app| app.compile_scopes.len()),
        real_compile_count,
        cache_hit_count,
        expansion_ratio,
        report_age,
        in_succeeded_apps,
    }
}

fn build_layer_roots(
    source_root: &Path,
    app_id: &str,
    report: &MaterializationDiagnosticsReport,
    host: &RuntimeHostContext,
    prebuild: &RuntimePrebuildContext,
) -> Vec<ReachabilityTreeRoot> {
    vec![
        build_overview_root(report, host, prebuild),
        build_l1_root(report),
        build_l2_root(source_root, app_id, report),
        build_l3_root(source_root, app_id, report),
        build_l4_root(source_root, app_id, report),
        build_build_root(report, prebuild),
        build_logs_root(report),
    ]
}

fn build_overview_root(
    report: &MaterializationDiagnosticsReport,
    host: &RuntimeHostContext,
    prebuild: &RuntimePrebuildContext,
) -> ReachabilityTreeRoot {
    let memory_line = match (prebuild.peak_rss_bytes, prebuild.current_rss_bytes) {
        (Some(peak), Some(current)) => format!(
            "内存 peak={} current={}",
            format_bytes_human(peak),
            format_bytes_human(current)
        ),
        (Some(peak), None) => format!("内存 peak={}", format_bytes_human(peak)),
        _ => "内存 (无 prebuild RSS 记录)".to_string(),
    };
    let timing_line = match (
        prebuild.total_wall_ms,
        prebuild.compile_scopes_ms,
        host.last_build_warmup_ms,
    ) {
        (Some(wall), Some(compile), Some(warmup)) => {
            format!("构建 wall={wall}ms compile={compile}ms warmup={warmup}ms")
        }
        (Some(wall), Some(compile), None) => format!("构建 wall={wall}ms compile={compile}ms"),
        _ => host
            .last_build_total_ms
            .map(|wall| format!("宿主记录 last_build_total={wall}ms"))
            .unwrap_or_else(|| "构建时长 (无 prebuild 报告)".to_string()),
    };
    ReachabilityTreeRoot {
        group: "overview".to_string(),
        label: "概览 · Overview".to_string(),
        default_open: true,
        children: vec![
            summary_node(
                "overview-host",
                "overview-host",
                format!(
                    "宿主 phase={} | app={} access_ready={} scope_gate_ready={}",
                    host.phase, host.app_phase, host.access_ready, host.scope_gate_ready
                ),
                vec![
                    format!("L2_miss={}", host.gate_l2_miss.unwrap_or(0)),
                    format!("L3_fail={}", host.gate_l3_fail.unwrap_or(0)),
                    format!("L4_stale={}", host.gate_l4_stale.unwrap_or(0)),
                ],
            ),
            summary_node(
                "overview-disk",
                "overview-disk",
                format!(
                    "磁盘 app_root={} | content_store={} | prebuild={}",
                    format_bytes_human(report.disk.app_root_bytes),
                    format_bytes_human(report.content_store.bytes),
                    format_bytes_human(report.disk.prebuild_bytes)
                ),
                vec![format!("eval_total={}", format_bytes_human(report.eval.eval_total_bytes))],
            ),
            summary_node("overview-memory", "overview-memory", memory_line, Vec::new()),
            summary_node(
                "overview-timing",
                "overview-timing",
                timing_line,
                prebuild
                    .report_age
                    .as_ref()
                    .map(|age| vec![format!("report_age={age}")])
                    .unwrap_or_default(),
            ),
        ],
    }
}

fn build_l1_root(report: &MaterializationDiagnosticsReport) -> ReachabilityTreeRoot {
    let mut children = vec![
        summary_node(
            "l1-policy",
            "l1-policy",
            format!(
                "策略 dedup={} slim={} canonical_persist={} locked={}",
                report.cache.graph_registry_dedup,
                report.cache.access_slim_artifacts,
                report.cache.canonical_artifact_persist,
                report.cache.locked_on
            ),
            report.cache.env_overrides.clone(),
        ),
        summary_node(
            "l1-compile-index",
            "l1-compile-index",
            format!(
                "compile_index hits={} misses={} stale={} entries={}",
                report.build.compile_index_hits.unwrap_or(0),
                report.build.compile_index_misses.unwrap_or(0),
                report.build.compile_index_stale_entries.unwrap_or(0),
                report.build.compile_index_entries.unwrap_or(0)
            ),
            vec![
                format!("source={}", report.build.source),
                format!(
                    "mrg_eval_skips={}",
                    report.build.mrg_eval_skips.unwrap_or(0)
                ),
            ],
        ),
        summary_node(
            "l1-eval-index",
            "l1-eval-index",
            format!(
                "eval response={} files / {} | dataframe={} files / {}",
                report.eval.metric_response_files,
                format_bytes_human(report.eval.metric_response_bytes),
                report.eval.metric_dataframe_files,
                format_bytes_human(report.eval.metric_dataframe_bytes)
            ),
            vec![format!(
                "eval_total={} files / {}",
                report.eval.eval_total_files,
                format_bytes_human(report.eval.eval_total_bytes)
            )],
        ),
        summary_node(
            "l1-content-store",
            "l1-content-store",
            format!(
                "CAS store {} / {} kinds",
                format_bytes_human(report.content_store.bytes),
                report.content_store.files_by_kind.len()
            ),
            report
                .content_store
                .files_by_kind
                .iter()
                .map(|(kind, count)| format!("{kind}={count}"))
                .collect(),
        ),
    ];
    if let Some(prebuild) = report.prebuild_plan.plan_source.as_ref() {
        children.push(summary_node(
            "l1-prebuild-plan",
            "l1-prebuild-plan",
            format!(
                "prebuild plan source={prebuild} dirty_slots={}",
                report
                    .prebuild_plan
                    .dirty_slot_count
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ),
            Vec::new(),
        ));
    }
    ReachabilityTreeRoot {
        group: "l1_cache".to_string(),
        label: "L1 · Cache".to_string(),
        default_open: true,
        children,
    }
}

fn build_l2_root(
    source_root: &Path,
    app_id: &str,
    report: &MaterializationDiagnosticsReport,
) -> ReachabilityTreeRoot {
    let mut children = vec![summary_node(
        "l2-navigation-summary",
        "l2-navigation-summary",
        format!(
            "MRG navigation nodes={} dup_keys={} orphan_urls={}",
            report
                .mrg
                .navigation_node_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".to_string()),
            report
                .mrg
                .navigation_duplicate_keys
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".to_string()),
            report
                .mrg
                .navigation_orphan_urls
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".to_string())
        ),
        vec![format!(
            "gate_sweep L2_miss={}",
            report.scope_gate_sweep.l2_miss
        )],
    )];
    children.extend(scope_gate_nodes(
        source_root,
        app_id,
        "l2-scope",
        |gate| gate.navigation_ready,
        "L2",
    ));
    ReachabilityTreeRoot {
        group: "l2_navigation".to_string(),
        label: "L2 · Navigation".to_string(),
        default_open: true,
        children,
    }
}

fn build_l3_root(
    source_root: &Path,
    app_id: &str,
    report: &MaterializationDiagnosticsReport,
) -> ReachabilityTreeRoot {
    let mcg = McgRegistryWriter::load(source_root, app_id);
    let mut children = vec![
        summary_node(
            "l3-mcg-summary",
            "l3-mcg-summary",
            format!(
                "MCG nodes={} scene_payload={} bundles={} skeleton={}",
                report.mcg.node_count,
                report.mcg.scene_payload_nodes,
                report.mcg.metric_def_bundle_nodes,
                report.mcg.app_skeleton_present
            ),
            vec![
                format!("revision={}", report.mcg.registry_revision),
                format!(
                    "scene_payload_disk={} / {} files",
                    format_bytes_human(report.disk.scene_payload_bytes),
                    report.disk.scene_payload_file_count
                ),
                format!("gate_sweep L3_fail={}", report.scope_gate_sweep.l3_fail),
            ],
        ),
    ];
    for node in &mcg.nodes {
        if node.id.kind != GraphNodeKind::ScenePayload {
            continue;
        }
        let key = node.id.key.as_str();
        let payload_bytes = node
            .stats
            .as_ref()
            .and_then(|stats| stats.get("payloadBytes").copied())
            .unwrap_or(0);
        children.push(summary_node(
            format!("l3-scene-{}", sanitize_id(key)),
            format!("l3-scene:{key}"),
            key.to_string(),
            vec![
                material_state_label(node.state.clone()).to_string(),
                format!("revision={}", node.revision),
                format!("payload={}", format_bytes_human(payload_bytes)),
            ],
        ));
    }
    children.extend(scope_gate_nodes(
        source_root,
        app_id,
        "l3-scope",
        |gate| gate.assembly_ready,
        "L3",
    ));
    ReachabilityTreeRoot {
        group: "l3_assembly".to_string(),
        label: "L3 · Assembly (MCG)".to_string(),
        default_open: true,
        children,
    }
}

fn build_l4_root(
    source_root: &Path,
    app_id: &str,
    report: &MaterializationDiagnosticsReport,
) -> ReachabilityTreeRoot {
    let mrg = MrgRegistryWriter::load(source_root, app_id);
    let mut slots = mrg.slots;
    slots.sort_by(|left, right| {
        left.slot_id
            .node
            .key
            .cmp(&right.slot_id.node.key)
            .then(left.slot_id.scope_key.cmp(&right.slot_id.scope_key))
    });
    let mut children = vec![summary_node(
        "l4-mrg-summary",
        "l4-mrg-summary",
        format!(
            "MRG slots={} ready={} stale={} failed={} stale_ratio={:.0}%",
            report.mrg.slot_count,
            report.mrg.ready_slots,
            report.mrg.stale_slots,
            report.mrg.failed_slots,
            report.mrg.stale_ratio * 100.0
        ),
        vec![
            format!(
                "eval_artifacts={} / {}",
                report.disk.eval_artifact_file_count,
                format_bytes_human(report.disk.eval_artifact_bytes)
            ),
            format!(
                "data_snapshots={}",
                format_bytes_human(report.disk.data_snapshots_bytes)
            ),
            format!("gate_sweep L4_stale={}", report.scope_gate_sweep.l4_stale),
        ],
    )];
    if slots.is_empty() {
        children.push(summary_node(
            "l4-slots-empty",
            "l4-slots-empty",
            "无 MRG slot（hello 等轻量 app 可能为 0；L4 数据面由 MCG scene_payload 承担）".to_string(),
            Vec::new(),
        ));
    } else {
        for slot in slots {
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
            children.push(summary_node(
                format!("mrg-slot-{}", sanitize_id(&key)),
                format!("mrg-slot:{key}"),
                key,
                badges,
            ));
        }
    }
    for failed in &report.failed_slots {
        children.push(summary_node(
            format!("l4-failed-{}", sanitize_id(&failed.key)),
            format!("l4-failed:{}", failed.key),
            format!("failed {}@{}", failed.key, failed.scope_key),
            vec![failed.error.clone()],
        ));
    }
    children.extend(scope_gate_nodes(
        source_root,
        app_id,
        "l4-scope",
        |gate| gate.data_ready,
        "L4",
    ));
    ReachabilityTreeRoot {
        group: "l4_materialization".to_string(),
        label: "L4 · Materialization (MRG)".to_string(),
        default_open: true,
        children,
    }
}

fn build_build_root(
    report: &MaterializationDiagnosticsReport,
    prebuild: &RuntimePrebuildContext,
) -> ReachabilityTreeRoot {
    let children = vec![
        summary_node(
            "build-prebuild-wall",
            "build-prebuild-wall",
            format!(
                "prebuild ok={} profile={} in_succeeded_apps={}",
                prebuild.ok,
                prebuild
                    .scope_profile
                    .as_deref()
                    .unwrap_or("-"),
                prebuild.in_succeeded_apps
            ),
            vec![
                format!(
                    "wall={}ms compile_scopes={}ms scope_artifacts={}ms",
                    prebuild.total_wall_ms.unwrap_or(0),
                    prebuild.compile_scopes_ms.unwrap_or(0),
                    prebuild.scope_artifacts_ms.unwrap_or(0)
                ),
                format!(
                    "real_compile={} cache_hit={} expansion={:.2}",
                    prebuild.real_compile_count.unwrap_or(0),
                    prebuild.cache_hit_count.unwrap_or(0),
                    prebuild.expansion_ratio.unwrap_or(0.0)
                ),
            ],
        ),
        summary_node(
            "build-compile-index",
            "build-compile-index",
            format!(
                "compile_index source={} path={}",
                report.build.source,
                report
                    .build
                    .report_path
                    .as_deref()
                    .unwrap_or("-")
            ),
            vec![
                format!(
                    "generated={}",
                    report
                        .build
                        .compile_index_generated_at_ms
                        .map(|ms| format_age_ms(ms, now_ms_for_host_message() as u64))
                        .unwrap_or_else(|| "-".to_string())
                ),
                format!(
                    "dataframe_eval_skips={}",
                    report.build.dataframe_eval_skips.unwrap_or(0)
                ),
            ],
        ),
        summary_node(
            "build-disk-graph",
            "build-disk-graph",
            format!(
                "compiled_app={} / {} files | graph={}",
                format_bytes_human(report.disk.compiled_app_bytes),
                report.disk.compiled_app_file_count,
                format_bytes_human(report.disk.graph_bytes)
            ),
            Vec::new(),
        ),
    ];
    ReachabilityTreeRoot {
        group: "build_prebuild".to_string(),
        label: "Build · Prebuild".to_string(),
        default_open: false,
        children,
    }
}

fn build_logs_root(report: &MaterializationDiagnosticsReport) -> ReachabilityTreeRoot {
    let mut children: Vec<ReachabilityTreeNode> = report
        .alerts
        .iter()
        .enumerate()
        .map(|(index, alert)| {
            summary_node(
                format!("log-{index}"),
                format!("log:{index}"),
                alert.clone(),
                vec!["alert".to_string()],
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

fn scope_gate_nodes(
    source_root: &Path,
    app_id: &str,
    id_prefix: &str,
    ready: impl Fn(&crate::readiness::scope_gate::ScopeGateReport) -> bool,
    layer: &str,
) -> Vec<ReachabilityTreeNode> {
    let coords = collect_scope_coords(source_root, app_id);
    coords
        .into_iter()
        .filter_map(|(scene_id, target_file)| {
            let scene = scene_id.as_deref();
            let target = target_file.as_deref();
            let gate = crate::readiness::scope_gate::check_scope_gate_silent(
                source_root,
                app_id,
                scene,
                target,
                true,
            );
            if ready(&gate) {
                return None;
            }
            let scope_label = format!(
                "{}/{}",
                gate.scope.scene_id.trim(),
                gate.scope.target_file.trim()
            );
            Some(summary_node(
                format!("{id_prefix}-{}", sanitize_id(&scope_label)),
                format!("{id_prefix}:{scope_label}"),
                format!("{layer} miss {scope_label}"),
                gate.blockers,
            ))
        })
        .collect()
}

fn collect_scope_coords(
    source_root: &Path,
    app_id: &str,
) -> Vec<(Option<String>, Option<String>)> {
    let mut coords = Vec::new();
    let snapshot = registry_snapshot();
    if let Some(app) = snapshot.apps.iter().find(|app| app.app_id == app_id) {
        for scope in &app.scopes {
            coords.push((scope.scene_id.clone(), scope.target_file.clone()));
        }
    }
    if coords.is_empty() {
        let mcg = McgRegistryWriter::load(source_root, app_id);
        for node in &mcg.nodes {
            if node.id.kind != GraphNodeKind::ScenePayload {
                continue;
            }
            let target = node.id.key.clone();
            let scene = target
                .strip_prefix("src/scenes/")
                .or_else(|| target.strip_prefix("scenes/"))
                .and_then(|rest| rest.strip_suffix(".mei"))
                .map(str::to_string);
            coords.push((scene, Some(target)));
        }
    }
    if coords.is_empty() {
        let default = crate::graph::mrg::navigation::resolve_default_scope(
            source_root,
            app_id,
            crate::readiness::types::UiMode::App,
        );
        coords.push((
            Some(default.scope.scene_id),
            Some(default.scope.target_file),
        ));
    }
    coords.sort();
    coords.dedup();
    coords
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn material_state_label(state: MaterialState) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_includes_all_layer_roots() {
        let root = std::env::temp_dir().join(format!(
            "mei-runtime-snapshot-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        std::fs::create_dir_all(root.join("hello/.mei/build/active")).expect("app dir");
        let snapshot = build_runtime_observability_snapshot(root.as_path(), "hello");
        let groups: Vec<_> = snapshot.roots.iter().map(|root| root.group.as_str()).collect();
        assert!(groups.contains(&"overview"));
        assert!(groups.contains(&"l1_cache"));
        assert!(groups.contains(&"l2_navigation"));
        assert!(groups.contains(&"l3_assembly"));
        assert!(groups.contains(&"l4_materialization"));
        assert!(groups.contains(&"build_prebuild"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
