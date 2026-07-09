use leptos::prelude::*;
use mei_lang_kernel::{BuildExecScope, BuildNodeId, BuildNodeKind, BuildViewTab, ReachabilityTreeNode};

use super::manage_routing::{build_node_href, runtime_node_href};
use super::runtime_snapshot_view::RuntimeSnapshotView;

pub(crate) fn runtime_overview_panel(
    snapshot: Option<&RuntimeSnapshotView>,
    selected: Option<&ReachabilityTreeNode>,
    active_node_id: &str,
    app_path: &str,
) -> impl IntoView {
    let title = selected
        .map(|node| node.label.clone())
        .unwrap_or_else(|| "运行态概览".to_string());
    let node_id = selected
        .map(|node| node.node_id.clone())
        .unwrap_or_else(|| active_node_id.to_string());
    let kind = selected.map(|node| node.kind.clone()).unwrap_or_default();
    let badges = selected
        .map(|node| node.badges.clone())
        .unwrap_or_default();
    let build_cross_link = mcg_build_href_for_runtime_node(app_path, active_node_id);
    view! {
        <section class="build-overview build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 mei-text-body">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="flex flex-col gap-0.5">
                    <strong class="build-panel-title mei-text-primary">{title.clone()}</strong>
                    {snapshot.map(|snap| view! {
                        <span class="mei-font-1 mei-text-muted">{format!("app={}", snap.app_id)}</span>
                    })}
                </div>
                <div class="flex flex-wrap gap-2">
                    <a class="build-toolbar-btn" href=runtime_node_href(app_path, active_node_id, Some("json"))>"查看 JSON"</a>
                </div>
            </div>
            {build_cross_link.map(|href| view! {
                <div class="flex flex-col gap-1 rounded-lg border border-white/10 bg-black/10 p-3">
                    <span class="mei-font-1 mei-text-muted">"对应 MCG 检查点可在构建视图继续 drill-down。"</span>
                    <a class="build-toolbar-btn inline-flex w-fit" href=href>"在构建视图打开 MCG 节点"</a>
                </div>
            })}
            <dl class="grid gap-2" id="runtime-node-fields">
                <div class="flex gap-2">
                    <dt class="w-20 shrink-0 mei-text-muted">"Node"</dt>
                    <dd class="font-mono mei-font-1 break-all">{node_id.clone()}</dd>
                </div>
                {(!kind.is_empty()).then(|| view! {
                    <div class="flex gap-2">
                        <dt class="w-20 shrink-0 mei-text-muted">"Kind"</dt>
                        <dd>{kind.clone()}</dd>
                    </div>
                })}
                {(!badges.is_empty()).then(|| view! {
                    <div class="flex flex-col gap-0.5">
                        <dt class="mei-text-muted">"状态"</dt>
                        <dd class="flex flex-wrap gap-2">
                            {badges.into_iter().map(|badge| view! {
                                <span class="build-tree-badge build-tree-badge--meta">{badge}</span>
                            }).collect_view()}
                        </dd>
                    </div>
                })}
            </dl>
            {snapshot.map(|snap| runtime_layer_metrics_grid(snap, selected)).unwrap_or_else(|| view! {
                <p class="mei-font-1 mei-text-muted" id="runtime-layer-metrics-loading">"正在加载 L1–L4 指标…"</p>
            }.into_any())}
            <p class="mei-text-muted mei-font-1">"左侧树按运行对象与证据链分层列出；选择节点在此查看摘要，并可切到「当前节点 JSON / 完整快照 JSON」继续排障。"</p>
        </section>
    }
}

fn runtime_layer_metrics_grid(
    snapshot: &RuntimeSnapshotView,
    selected: Option<&ReachabilityTreeNode>,
) -> AnyView {
    let tree_nodes = snapshot.roots.iter().map(|root| root.children.len()).sum::<usize>();
    let selected_id = selected
        .map(|node| node.node_id.as_str())
        .unwrap_or("-");
    let host = &snapshot.host;
    let prebuild = &snapshot.prebuild;
    let diag = &snapshot.diagnostics;
    view! {
        <div class="runtime-layer-metrics grid gap-3 md:grid-cols-2" id="runtime-layer-metrics">
            <section class="rounded-lg border border-white/10 bg-black/10 p-3">
                <h3 class="mb-2 mei-font-2 mei-text-primary">"宿主 / 构建"</h3>
                <dl class="grid gap-1 mei-font-1">
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"宿主 phase"</dt><dd>{host.phase.clone()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"App phase"</dt><dd>{host.app_phase.clone()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"access_ready"</dt><dd>{bool_label(host.access_ready)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"wall"</dt><dd>{ms_label(prebuild.total_wall_ms.or(host.last_build_total_ms))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"compile"</dt><dd>{ms_label(prebuild.compile_scopes_ms.or(host.last_build_compile_ms))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"warmup"</dt><dd>{ms_label(host.last_build_warmup_ms)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"scope_gate"</dt><dd>{bool_label(host.scope_gate_ready)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"peak RSS"</dt><dd>{bytes_label(prebuild.peak_rss_bytes)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"current RSS"</dt><dd>{bytes_label(prebuild.current_rss_bytes)}</dd></div>
                </dl>
            </section>
            <section class="rounded-lg border border-white/10 bg-black/10 p-3">
                <h3 class="mb-2 mei-font-2 mei-text-primary">"L1 · Cache"</h3>
                <dl class="grid gap-1 mei-font-1">
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"dedup"</dt><dd>{format!("{} slim={} persist={}", bool_label(diag.cache.graph_registry_dedup), bool_label(diag.cache.access_slim_artifacts), bool_label(diag.cache.canonical_artifact_persist))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"compile_index"</dt><dd>{format!("{} hit / {} miss / {} stale / {} entries", diag.build.compile_index_hits.unwrap_or(0), diag.build.compile_index_misses.unwrap_or(0), diag.build.compile_index_stale_entries.unwrap_or(0), diag.build.compile_index_entries.unwrap_or(0))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"dataframe_skips"</dt><dd>{diag.build.dataframe_eval_skips.unwrap_or(0).to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"mrg_eval_skips"</dt><dd>{diag.build.mrg_eval_skips.unwrap_or(0).to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"eval"</dt><dd>{format!("response {} / {} | dataframe {} / {}", diag.eval.metric_response_files, format_bytes_human(diag.eval.metric_response_bytes), diag.eval.metric_dataframe_files, format_bytes_human(diag.eval.metric_dataframe_bytes))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"eval total"</dt><dd>{format!("{} / {}", diag.eval.eval_total_files, format_bytes_human(diag.eval.eval_total_bytes))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"CAS store"</dt><dd>{format!("{} / {} kinds", format_bytes_human(diag.content_store.bytes), diag.content_store.files_by_kind.len())}</dd></div>
                </dl>
            </section>
            <section class="rounded-lg border border-white/10 bg-black/10 p-3">
                <h3 class="mb-2 mei-font-2 mei-text-primary">"L2 · Navigation"</h3>
                <dl class="grid gap-1 mei-font-1">
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"L2 miss"</dt><dd>{gate_count(host.gate_l2_miss, diag.scope_gate_sweep.l2_miss)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"nav nodes"</dt><dd>{diag.mrg.navigation_node_count.unwrap_or(0).to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"nav dup"</dt><dd>{diag.mrg.navigation_duplicate_keys.unwrap_or(0).to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"nav orphan"</dt><dd>{diag.mrg.navigation_orphan_urls.unwrap_or(0).to_string()}</dd></div>
                </dl>
            </section>
            <section class="rounded-lg border border-white/10 bg-black/10 p-3">
                <h3 class="mb-2 mei-font-2 mei-text-primary">"L3 · Assembly"</h3>
                <dl class="grid gap-1 mei-font-1">
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"MCG nodes"</dt><dd>{format!("{} rev={}", diag.mcg.node_count, diag.mcg.registry_revision)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"scene_payload"</dt><dd>{format!("{} files / {}", diag.mcg.scene_payload_nodes, diag.disk.scene_payload_file_count)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"bundles"</dt><dd>{format!("{} skeleton={}", diag.mcg.metric_def_bundle_nodes, bool_label(diag.mcg.app_skeleton_present))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"L3 fail"</dt><dd>{gate_count(host.gate_l3_fail, diag.scope_gate_sweep.l3_fail)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"payload disk"</dt><dd>{format_bytes_human(diag.disk.scene_payload_bytes)}</dd></div>
                </dl>
            </section>
            <section class="rounded-lg border border-white/10 bg-black/10 p-3">
                <h3 class="mb-2 mei-font-2 mei-text-primary">"L4 · Materialization"</h3>
                <dl class="grid gap-1 mei-font-1">
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"slots"</dt><dd>{format!("{} total / {} ready / {} fail", diag.mrg.slot_count, diag.mrg.ready_slots, diag.mrg.failed_slots)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"eval artifacts"</dt><dd>{format!("{} / {}", diag.disk.eval_artifact_file_count, format_bytes_human(diag.disk.eval_artifact_bytes))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"stale"</dt><dd>{format!("{} ({:.0}%)", diag.mrg.stale_slots, diag.mrg.stale_ratio * 100.0)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"L4 stale"</dt><dd>{gate_count(host.gate_l4_stale, diag.scope_gate_sweep.l4_stale)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"data snapshots"</dt><dd>{format_bytes_human(diag.disk.data_snapshots_bytes)}</dd></div>
                </dl>
            </section>
            <section class="rounded-lg border border-white/10 bg-black/10 p-3">
                <h3 class="mb-2 mei-font-2 mei-text-primary">"磁盘"</h3>
                <dl class="grid gap-1 mei-font-1">
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"app_root"</dt><dd>{format_bytes_human(diag.disk.app_root_bytes)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"compiled_app"</dt><dd>{format!("{} / {}", diag.disk.compiled_app_file_count, format_bytes_human(diag.disk.compiled_app_bytes))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"graph"</dt><dd>{format_bytes_human(diag.disk.graph_bytes)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"prebuild"</dt><dd>{format_bytes_human(diag.disk.prebuild_bytes)}</dd></div>
                </dl>
            </section>
            {(!diag.alerts.is_empty()).then(|| view! {
                <section class="rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 md:col-span-2">
                    <h3 class="mb-2 mei-font-2 mei-text-primary">{format!("告警 ({})", diag.alerts.len())}</h3>
                    <ul class="grid gap-1 mei-font-1">
                        {diag.alerts.iter().map(|alert| view! {
                            <li>{alert.clone()}</li>
                        }).collect_view()}
                    </ul>
                </section>
            })}
            <section class="rounded-lg border border-white/10 bg-black/10 p-3 md:col-span-2">
                <h3 class="mb-2 mei-font-2 mei-text-primary">"Prebuild 报告"</h3>
                <dl class="grid gap-1 mei-font-1 md:grid-cols-2">
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"profile"</dt><dd>{prebuild.scope_profile.clone().unwrap_or_else(|| "-".to_string())}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"report_age"</dt><dd>{prebuild.report_age.clone().unwrap_or_else(|| "-".to_string())}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"real_compile"</dt><dd>{prebuild.real_compile_count.unwrap_or(0).to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"expansion"</dt><dd>{format!("{:.2}", prebuild.expansion_ratio.unwrap_or(0.0))}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"prebuild ok"</dt><dd>{bool_label(prebuild.ok)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"scope_artifacts"</dt><dd>{ms_label(prebuild.scope_artifacts_ms)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"tree nodes"</dt><dd>{tree_nodes.to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"selected"</dt><dd class="font-mono">{selected_id.to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"cache_hit"</dt><dd>{prebuild.cache_hit_count.unwrap_or(0).to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"in_succeeded_apps"</dt><dd>{bool_label(prebuild.in_succeeded_apps)}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"scope_count"</dt><dd>{prebuild.compile_scope_count.unwrap_or(0).to_string()}</dd></div>
                    <div class="flex justify-between gap-2"><dt class="mei-text-muted">"compile source"</dt><dd>{diag.build.source.clone()}</dd></div>
                </dl>
            </section>
        </div>
    }
    .into_any()
}

pub(crate) fn runtime_json_panel(
    title: &str,
    panel_id: &str,
    snapshot_json: &str,
    hint: Option<&str>,
) -> impl IntoView {
    let title = title.to_string();
    let panel_id = panel_id.to_string();
    let hint = hint.map(str::to_string);
    let body = if snapshot_json.trim().is_empty() {
        "{}".to_string()
    } else {
        serde_json::from_str::<serde_json::Value>(snapshot_json)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or_else(|| snapshot_json.to_string())
    };
    view! {
        <section class="build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 mei-text-body min-h-0 flex flex-col">
            <div class="grid gap-1">
                <strong class="build-panel-title mei-text-primary">{title}</strong>
                {hint.map(|text| view! {
                    <p class="mei-font-1 mei-text-muted">{text}</p>
                })}
            </div>
            <pre class="runtime-detail-json min-h-0 flex-1 overflow-auto rounded bg-black/20 p-3 font-mono mei-font-1 leading-5 mei-text-body" id=panel_id>{body}</pre>
        </section>
    }
}

fn mcg_build_href_for_runtime_node(app_path: &str, runtime_node_id: &str) -> Option<String> {
    let raw_key = runtime_node_id
        .strip_prefix("l3-scene:")
        .or_else(|| {
            runtime_node_id
                .strip_prefix("mrg:slot:")
                .and_then(|rest| rest.split('@').next())
        })
        .or_else(|| {
            runtime_node_id
                .strip_prefix("mrg-slot:")
                .and_then(|rest| rest.split('@').next())
        })?
        .trim();
    if raw_key.is_empty() {
        return None;
    }
    let node_key = if raw_key.starts_with("scene_payload:")
        || raw_key.starts_with("metric_def_bundle:")
        || raw_key.starts_with("semantic_graph:")
        || raw_key.starts_with("page_instance:")
    {
        raw_key.to_string()
    } else if raw_key.starts_with("src/") || raw_key.starts_with("scenes/") {
        format!("scene_payload:{raw_key}")
    } else {
        format!("scene_payload:{raw_key}")
    };
    let node = BuildNodeId::new(BuildNodeKind::McgNode, node_key);
    Some(build_node_href(
        app_path,
        &node,
        BuildViewTab::Overview,
        BuildExecScope::Warmup,
        None,
        None,
        super::manage_routing::BuildReviewAxes::default(),
        super::route::UiRouteMode::Layout,
    ))
}

fn bool_label(value: bool) -> String {
    if value { "true".to_string() } else { "false".to_string() }
}

fn ms_label(value: Option<u64>) -> String {
    value.map(|ms| format!("{ms}ms")).unwrap_or_else(|| "-".to_string())
}

fn bytes_label(value: Option<u64>) -> String {
    value.map(format_bytes_human).unwrap_or_else(|| "-".to_string())
}

fn gate_count(host: Option<usize>, sweep: usize) -> String {
    host.map(|count| count.to_string())
        .unwrap_or_else(|| sweep.to_string())
}

fn format_bytes_human(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KiB", bytes as f64 / KIB);
    }
    if bytes < 1024 * 1024 * 1024 {
        return format!("{:.1} MiB", bytes as f64 / KIB / KIB);
    }
    format!("{:.2} GiB", bytes as f64 / KIB / KIB / KIB)
}
