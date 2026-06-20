use leptos::prelude::*;
use mei_lang_kernel::{
    resolve_runtime_metric_def_key, CompiledApp, WorldSemanticDataset, WorldSemanticExplainBlock,
    WorldSemanticMetric,
};
use serde_json::Value;

use super::super::compile_status::{is_world_capsule_target, world_capsule_companion_scene};
use super::super::manage_routing::{build_preview_href, WorldSemanticQuery};

fn contract_summary_lines(contract: &Value) -> Vec<(String, String)> {
    let mut lines = Vec::new();
    if let Some(title) = contract
        .get("title")
        .or_else(|| contract.get("focus_node_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        lines.push(("标题".to_string(), title.to_string()));
    }
    if let Some(note) = contract
        .get("note")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        lines.push(("说明".to_string(), note.to_string()));
    }
    if let Some(tabs) = contract.get("tabs").and_then(Value::as_array) {
        let joined = tabs
            .iter()
            .filter_map(|tab| {
                tab.get("id")
                    .or_else(|| tab.get("label"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join(" · ");
        if !joined.is_empty() {
            lines.push(("分析页签".to_string(), joined));
        }
    }
    if let Some(blocks) = contract.get("blocks").and_then(Value::as_array) {
        let joined = blocks
            .iter()
            .filter_map(|block| block.get("kind").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" · ");
        if !joined.is_empty() {
            lines.push(("块类型".to_string(), joined));
        }
    }
    lines
}

fn find_world_metrics_dataset<'a>(
    compiled: &'a CompiledApp,
    file_path: &str,
) -> Option<&'a mei_lang_kernel::DatasetView> {
    let namespaced = format!("__world_metrics__::{file_path}::metrics");
    compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__" || resource.id == namespaced)
        .and_then(|resource| resource.dataset.as_ref())
}

fn metric_scalar_display(metric: &mei_lang_kernel::MetricContract) -> String {
    if let Some(value) = metric.value.get("value") {
        return value.to_string().trim_matches('"').to_string();
    }
    if metric.value.is_number() || metric.value.is_string() {
        return metric.value.to_string().trim_matches('"').to_string();
    }
    String::new()
}

fn lookup_metric_contract<'a>(
    compiled: &'a CompiledApp,
    dataset: &'a mei_lang_kernel::DatasetView,
    resource_id: &str,
    metric_id: &str,
) -> Option<&'a mei_lang_kernel::MetricContract> {
    if let Some(entry) = compiled.world_metrics.get(metric_id) {
        return Some(&entry.metric);
    }
    let canonical = resolve_runtime_metric_def_key(resource_id, metric_id, &dataset.runtime_metric_defs)
        .unwrap_or_else(|| metric_id.to_string());
    dataset.metrics.get(&canonical).or_else(|| {
        dataset
            .metrics
            .iter()
            .find(|(key, _)| key.ends_with(metric_id) || key.contains(&format!("::{metric_id}")))
            .map(|(_, metric)| metric)
    })
}

fn lookup_analysis_contract(
    dataset: &mei_lang_kernel::DatasetView,
    resource_id: &str,
    metric_id: &str,
) -> Option<Value> {
    let canonical = resolve_runtime_metric_def_key(resource_id, metric_id, &dataset.runtime_metric_defs)
        .unwrap_or_else(|| metric_id.to_string());
    dataset.runtime_analysis_contracts.get(&canonical).cloned()
}

fn metric_inspector_body(
    metric: &WorldSemanticMetric,
    explain: Option<&WorldSemanticExplainBlock>,
    compiled: &CompiledApp,
    file_path: &str,
    resource_id: &str,
) -> AnyView {
    let dataset = find_world_metrics_dataset(compiled, file_path);
    let metric_contract = dataset
        .and_then(|dataset| lookup_metric_contract(compiled, dataset, resource_id, metric.id.as_str()));
    let scalar = metric_contract
        .map(metric_scalar_display)
        .filter(|value| !value.is_empty());
    let analysis_contract = dataset
        .and_then(|dataset| lookup_analysis_contract(dataset, resource_id, metric.id.as_str()));
    let contract_lines = analysis_contract
        .as_ref()
        .map(contract_summary_lines)
        .unwrap_or_default();
    let explain_block = explain.or_else(|| {
        metric
            .explain
            .first()
            .filter(|_| metric.explain.len() == 1)
    });
    view! {
        <div class="world-semantic-inspector-body grid gap-3 text-xs leading-5 mei-text-body">
            <header class="grid gap-1 border-b border-slate-700/50 pb-2">
                <div class="text-sm font-medium mei-text-inverse">
                    {metric
                        .label
                        .clone()
                        .filter(|text| !text.trim().is_empty())
                        .unwrap_or_else(|| metric.id.clone())}
                </div>
                <div class="font-mono text-[11px] mei-text-muted">{metric.id.clone()}</div>
            </header>
            {scalar
                .map(|value| {
                    let unit = metric.unit.clone().unwrap_or_default();
                    view! {
                        <section class="grid gap-1">
                            <div class="text-[11px] uppercase tracking-wide mei-text-muted">"指标值"</div>
                            <div class="text-lg font-semibold text-sky-100">
                                {value}
                                {(!unit.is_empty()).then(|| view! { <span class="ml-1 text-sm mei-text-muted">{unit}</span> })}
                            </div>
                        </section>
                    }
                        .into_any()
                })
                .unwrap_or_else(|| {
                    view! {
                        <section class="rounded-lg border border-dashed mei-border-muted mei-surface-panel-muted px-3 py-2 text-[11px] mei-text-muted">
                            "当前编译未物化该指标值；请确认 world 胶囊已成功编译，或查看调试页诊断。"
                        </section>
                    }
                        .into_any()
                })}
            {metric
                .note
                .as_ref()
                .filter(|note| !note.trim().is_empty())
                .map(|note| {
                    view! {
                        <section class="grid gap-1">
                            <div class="text-[11px] uppercase tracking-wide mei-text-muted">"口径"</div>
                            <p class="m-0 mei-text-body">{note.clone()}</p>
                        </section>
                    }
                        .into_any()
                })
                .unwrap_or_else(|| view! { <></> }.into_any())}
            {if !contract_lines.is_empty() {
                view! {
                    <section class="grid gap-1.5">
                        <div class="text-[11px] uppercase tracking-wide mei-text-muted">"Analysis Contract"</div>
                        <dl class="m-0 grid gap-1">
                            {contract_lines
                                .into_iter()
                                .map(|(label, value)| {
                                    view! {
                                        <div class="grid gap-0.5">
                                            <dt class="text-[11px] mei-text-muted">{label}</dt>
                                            <dd class="m-0 mei-text-body">{value}</dd>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </dl>
                    </section>
                }
                    .into_any()
            } else {
                view! { <></> }.into_any()
            }}
            {explain_block
                .map(|block| {
                    view! {
                        <section class="grid gap-1.5 rounded-lg border border-sky-500/25 bg-sky-500/5 px-3 py-2">
                            <div class="text-[11px] uppercase tracking-wide text-sky-300/80">"Explain 块"</div>
                            <div class="font-mono text-[11px] mei-text-muted">{block.id.clone()}</div>
                            <div class="mei-text-primary">{block.kind.clone()}</div>
                            {block
                                .label
                                .as_ref()
                                .map(|label| view! { <div>{label.clone()}</div> }.into_any())
                                .unwrap_or_else(|| view! { <></> }.into_any())}
                            {block
                                .by
                                .as_ref()
                                .map(|by| {
                                    view! {
                                        <div class="mei-text-muted">
                                            "维度："
                                            <span class="mei-text-primary">{by.clone()}</span>
                                        </div>
                                    }
                                        .into_any()
                                })
                                .unwrap_or_else(|| view! { <></> }.into_any())}
                        </section>
                    }
                        .into_any()
                })
                .unwrap_or_else(|| view! { <></> }.into_any())}
        </div>
    }
    .into_any()
}

fn dataset_inspector_body(dataset: &WorldSemanticDataset) -> AnyView {
    view! {
        <div class="world-semantic-inspector-body grid gap-3 text-xs leading-5 mei-text-body">
            <header class="grid gap-1 border-b border-slate-700/50 pb-2">
                <div class="text-sm font-medium mei-text-inverse">{dataset.id.clone()}</div>
                {dataset
                    .title
                    .as_ref()
                    .filter(|title| !title.trim().is_empty())
                    .map(|title| view! { <div class="mei-text-muted">{title.clone()}</div> }.into_any())
                    .unwrap_or_else(|| view! { <></> }.into_any())}
            </header>
            <section class="grid gap-1">
                <div class="text-[11px] uppercase tracking-wide mei-text-muted">"Source"</div>
                <div class="mei-text-body">
                    {dataset
                        .source_kind
                        .clone()
                        .unwrap_or_else(|| "—".to_string())}
                </div>
            </section>
            <section class="grid gap-1">
                <div class="text-[11px] uppercase tracking-wide mei-text-muted">"Filters"</div>
                <div class="mei-text-body">{dataset.filter_field_count} " 个字段"</div>
            </section>
            <section class="grid gap-1.5">
                <div class="text-[11px] uppercase tracking-wide mei-text-muted">"Schema"</div>
                {if dataset.schema_columns.is_empty() {
                    view! { <div class="mei-text-muted">"（无列信息）"</div> }.into_any()
                } else {
                    view! {
                        <ul class="m-0 grid list-none gap-0.5 pl-0 font-mono text-[11px] mei-text-body">
                            {dataset
                                .schema_columns
                                .iter()
                                .map(|column| view! { <li>{column.clone()}</li> })
                                .collect_view()}
                        </ul>
                    }
                        .into_any()
                }}
            </section>
        </div>
    }
    .into_any()
}

pub(crate) fn world_semantic_inspector_view(
    compiled: &CompiledApp,
    app_path: &str,
    file_path: &str,
    semantic: WorldSemanticQuery<'_>,
) -> AnyView {
    if !is_world_capsule_target(file_path) || !semantic.has_selection() {
        return view! { <></> }.into_any();
    }
    let index = compiled.world_semantic_by_file.get(file_path);
    let body = if let Some(dataset_id) = semantic
        .world_dataset
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        index
            .and_then(|index| index.datasets.iter().find(|dataset| dataset.id == dataset_id))
            .map(dataset_inspector_body)
            .unwrap_or_else(|| {
                view! {
                    <div class="text-xs mei-text-muted">
                        "未找到 dataset `"
                        {dataset_id.to_string()}
                        "` 的语义索引。"
                    </div>
                }
                .into_any()
            })
    } else if let Some(metric_id) = semantic
        .world_metric
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let explain = semantic
            .explain
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|explain_id| {
                index.and_then(|index| {
                    index.metrics.iter().find_map(|metric| {
                        if metric.id != metric_id {
                            return None;
                        }
                        metric.explain.iter().find(|block| block.id == explain_id)
                    })
                })
            });
        index
            .and_then(|index| index.metrics.iter().find(|metric| metric.id == metric_id))
            .map(|metric| {
                let resource_id = index
                    .map(|index| index.resource_id.as_str())
                    .unwrap_or("__world_metrics__");
                metric_inspector_body(metric, explain, compiled, file_path, resource_id)
            })
            .unwrap_or_else(|| {
                view! {
                    <div class="text-xs mei-text-muted">
                        "未找到 metric `"
                        {metric_id.to_string()}
                        "` 的语义索引。"
                    </div>
                }
                .into_any()
            })
    } else {
        view! { <></> }.into_any()
    };

    let companion = world_capsule_companion_scene(file_path);
    let companion_href = companion.as_ref().map(|scene_file| {
        build_preview_href(
            app_path,
            Some(scene_file.as_str()),
            None,
            Some("preview"),
            None,
            WorldSemanticQuery::default(),
        )
    });

    view! {
        <aside class="sidebar right workspace-panel workspace-panel-side workspace-panel-inspector h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-4 py-2.5">
            <div class="mb-2 flex items-center justify-between gap-2 border-b border-slate-700/50 pb-2">
                <div>
                    <div class="text-[11px] uppercase tracking-wide mei-text-muted">"语义检视"</div>
                    <div class="text-sm font-medium mei-text-inverse">"World 胶囊"</div>
                </div>
            </div>
            <div class="sidebar-scroll flex-1 min-h-0 overflow-auto">
                {body}
                {companion_href
                    .map(|href| {
                        view! {
                            <section class="mt-4 border-t border-slate-700/50 pt-3 text-[11px] leading-5 mei-text-muted">
                                <div class="mb-1 mei-text-muted">"关联场景"</div>
                                <a class="text-sky-300 hover:text-sky-200" href=href>
                                    {companion.clone().unwrap_or_default()}
                                </a>
                            </section>
                        }
                            .into_any()
                    })
                    .unwrap_or_else(|| view! { <></> }.into_any())}
            </div>
        </aside>
    }
    .into_any()
}

pub(crate) fn should_show_world_semantic_inspector(
    node: &mei_lang_kernel::BuildNodeId,
    file_path: &str,
    semantic: WorldSemanticQuery<'_>,
) -> bool {
    use mei_lang_kernel::BuildNodeKind;
    match node.kind {
        BuildNodeKind::WorldFile
        | BuildNodeKind::WorldDataset
        | BuildNodeKind::WorldMetric
        | BuildNodeKind::WorldExplain
        | BuildNodeKind::Dataset => is_world_capsule_target(file_path) || semantic.has_selection(),
        _ => false,
    }
}
