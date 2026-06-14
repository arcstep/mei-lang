use leptos::prelude::*;
use mei_lang_kernel::{
    resolve_dataset_resource_id, resolve_runtime_metric_def_key, CompiledApp, DatasetView,
    MetricContract, MetricShape, WorldSemanticMetric,
};
use serde_json::{json, Value};

use super::nodes::component_html;
use super::resolve::{
    attach_host_meta, with_runtime_ref, HostMetaOptions, RuntimeSceneAnchor,
};
use super::PreviewRuntimeContext;
use crate::ui::manage_routing::WorldSemanticQuery;

fn component_tag(compiled: &CompiledApp, use_key: &str) -> String {
    compiled
        .component_assets
        .iter()
        .find(|asset| asset.key == use_key)
        .map(|asset| asset.tag.clone())
        .unwrap_or_else(|| match use_key {
            "dataset.table" => "mei-dataset-table".to_string(),
            _ => "mei-missing-component".to_string(),
        })
}

fn prepare_dataset_table_data(
    dataset: &DatasetView,
    resolved_id: &str,
    file_path: &str,
    compiled: &CompiledApp,
) -> Value {
    let anchor = RuntimeSceneAnchor {
        scene_id: compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "default".to_string()),
        scene_path: Some(file_path.to_string()),
    };
    let data = serde_json::to_value(dataset).unwrap_or(Value::Null);
    if dataset.rows.is_empty() {
        return with_runtime_ref(
            data,
            anchor.runtime_ref_extra("data", resolved_id, None, None),
        );
    }
    let mut inline = data;
    if let Some(map) = inline.as_object_mut() {
        map.insert(
            "source".to_string(),
            json!({
                "kind": "derived",
                "path": format!("dataset_view:{}", dataset.id),
            }),
        );
    }
    inline
}

fn find_world_metrics_dataset<'a>(
    compiled: &'a CompiledApp,
    file_path: &str,
) -> Option<&'a DatasetView> {
    let namespaced = format!("__world_metrics__::{file_path}::metrics");
    compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__" || resource.id == namespaced)
        .and_then(|resource| resource.dataset.as_ref())
}

fn lookup_metric_contract<'a>(
    compiled: &'a CompiledApp,
    dataset: &'a DatasetView,
    resource_id: &str,
    metric_id: &str,
) -> Option<&'a MetricContract> {
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

fn metric_scalar_display(metric: &MetricContract) -> String {
    if let Some(value) = metric.value.get("value") {
        return value.to_string().trim_matches('"').to_string();
    }
    if metric.value.is_number() || metric.value.is_string() {
        return metric.value.to_string().trim_matches('"').to_string();
    }
    String::new()
}

fn dataset_table_host_html(
    compiled: &CompiledApp,
    app_path: &str,
    file_path: &str,
    dataset: &DatasetView,
    resolved_id: &str,
    title: Option<&str>,
) -> String {
    let mut data = prepare_dataset_table_data(dataset, resolved_id, file_path, compiled);
    if let Some(title) = title.filter(|text| !text.trim().is_empty()) {
        if let Some(map) = data.as_object_mut() {
            map.insert("title".to_string(), Value::String(title.to_string()));
        }
    }
    let props = attach_host_meta(
        json!({
            "data": data,
            "paging": { "defaultPageSize": 20 },
            "toolbar": { "search": true },
        }),
        compiled,
        app_path,
        &json!({}),
        Some(file_path),
        HostMetaOptions::default(),
    );
    let tag = component_tag(compiled, "dataset.table");
    component_html(tag.as_str(), &props)
}

fn dataset_table_preview(
    compiled: &CompiledApp,
    app_path: &str,
    file_path: &str,
    dataset_id: &str,
    runtime_ctx: &PreviewRuntimeContext,
    title: Option<&str>,
) -> AnyView {
    let resolved_id = match resolve_dataset_resource_id(
        compiled,
        dataset_id,
        Some(&runtime_ctx.index),
    ) {
        Ok(id) => id,
        Err(message) => {
            let detail = message.to_string();
            return view! {
                <section class="world-capsule-preview world-capsule-preview-empty rounded-[14px] border border-amber-500/25 bg-slate-950/35 p-4 text-sm text-slate-300">
                    <p class="m-0">"无法解析 dataset："{detail}</p>
                </section>
            }
            .into_any();
        }
    };
    let Some(resource) = runtime_ctx.resources.get(&resolved_id) else {
        return view! {
            <section class="world-capsule-preview world-capsule-preview-empty rounded-[14px] border border-amber-500/25 bg-slate-950/35 p-4 text-sm text-slate-300">
                <p class="m-0">"未找到已物化的 dataset `"{dataset_id}"`。"</p>
            </section>
        }
        .into_any();
    };
    let Some(dataset) = resource.dataset.as_ref() else {
        return view! {
            <section class="world-capsule-preview world-capsule-preview-empty rounded-[14px] border border-amber-500/25 bg-slate-950/35 p-4 text-sm text-slate-300">
                <p class="m-0">"资源 `"{resolved_id}"` 不含 dataset 视图。"</p>
            </section>
        }
        .into_any();
    };
    let html = dataset_table_host_html(
        compiled,
        app_path,
        file_path,
        dataset,
        resolved_id.as_str(),
        title,
    );
    view! {
        <section class="world-capsule-preview world-capsule-dataset-table preview-surface min-h-0 p-3">
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}

fn scalar_form_field(label: String, value: AnyView) -> AnyView {
    view! {
        <label class="world-capsule-scalar-field">
            <span class="world-capsule-scalar-field__label">{label}</span>
            <span class="world-capsule-scalar-field__value">{value}</span>
        </label>
    }
    .into_any()
}

fn scalar_metric_form_preview(
    metric: &WorldSemanticMetric,
    contract: Option<&MetricContract>,
) -> AnyView {
    let display_label = metric
        .label
        .clone()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| metric.id.clone());
    let scalar_value = contract
        .map(metric_scalar_display)
        .filter(|value| !value.is_empty());
    let unit = metric
        .unit
        .clone()
        .or_else(|| contract.and_then(|entry| entry.unit.clone()))
        .unwrap_or_default();
    let shape_label = contract
        .map(|entry| format!("{:?}", entry.shape).to_ascii_lowercase())
        .unwrap_or_else(|| "scalar".to_string());
    let purpose = contract
        .and_then(|entry| entry.purpose.clone())
        .filter(|text| !text.trim().is_empty());
    let schema_lines = contract
        .map(|entry| {
            entry
                .schema
                .iter()
                .map(|column| format!("{} ({})", column.name, column.type_name))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let value_field = scalar_value
        .map(|value| {
            scalar_form_field(
                "指标值".to_string(),
                view! { <span class="text-lg font-semibold text-sky-100">{value}</span> }.into_any(),
            )
        })
        .unwrap_or_else(|| {
            scalar_form_field(
                "指标值".to_string(),
                view! { <span class="text-slate-500">"（未物化）"</span> }.into_any(),
            )
        });
    let mut fields = vec![
        scalar_form_field("标识".to_string(), view! { {metric.id.clone()} }.into_any()),
        scalar_form_field("标签".to_string(), view! { {display_label.clone()} }.into_any()),
        scalar_form_field("形状".to_string(), view! { {shape_label} }.into_any()),
    ];
    if !unit.is_empty() {
        fields.push(scalar_form_field(
            "单位".to_string(),
            view! { {unit} }.into_any(),
        ));
    }
    fields.push(value_field);
    if let Some(note) = metric
        .note
        .as_ref()
        .filter(|note| !note.trim().is_empty())
    {
        fields.push(scalar_form_field(
            "口径".to_string(),
            view! { {note.clone()} }.into_any(),
        ));
    }
    if let Some(text) = purpose {
        fields.push(scalar_form_field(
            "用途".to_string(),
            view! { {text} }.into_any(),
        ));
    }
    if !schema_lines.is_empty() {
        fields.push(scalar_form_field(
            "Schema".to_string(),
            view! {
                <ul class="m-0 list-none pl-0 font-mono text-[11px] text-slate-300">
                    {schema_lines
                        .into_iter()
                        .map(|line| view! { <li>{line}</li> })
                        .collect_view()}
                </ul>
            }
            .into_any(),
        ));
    }

    view! {
        <section class="world-capsule-preview world-capsule-scalar-form preview-surface rounded-[14px] border border-slate-700/55 bg-slate-950/35 p-4">
            <header class="mb-4 border-b border-slate-700/50 pb-3">
                <h3 class="m-0 text-base font-semibold text-slate-100">{display_label}</h3>
                <div class="mt-1 font-mono text-[11px] text-slate-400">{metric.id.clone()}</div>
            </header>
            <form class="world-capsule-scalar-form__grid" on:submit=|ev| { ev.prevent_default(); }>
                {fields}
            </form>
        </section>
    }
    .into_any()
}

fn metric_table_preview(
    compiled: &CompiledApp,
    app_path: &str,
    file_path: &str,
    metric_id: &str,
    contract: &MetricContract,
    resource_id: &str,
) -> AnyView {
    let anchor = RuntimeSceneAnchor {
        scene_id: compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "default".to_string()),
        scene_path: Some(file_path.to_string()),
    };
    let mut data = with_runtime_ref(
        serde_json::to_value(contract).unwrap_or(Value::Null),
        anchor.runtime_ref_extra("metric", resource_id, Some(metric_id), None),
    );
    let title = contract
        .label
        .clone()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| metric_id.to_string());
    if let Some(map) = data.as_object_mut() {
        map.insert("title".to_string(), Value::String(title));
    }
    let props = attach_host_meta(
        json!({
            "data": data,
            "paging": { "defaultPageSize": 20 },
            "toolbar": { "search": true },
        }),
        compiled,
        app_path,
        &json!({}),
        Some(file_path),
        HostMetaOptions::default(),
    );
    let tag = component_tag(compiled, "dataset.table");
    let html = component_html(tag.as_str(), &props);
    view! {
        <section class="world-capsule-preview world-capsule-dataset-table preview-surface min-h-0 p-3">
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}

pub(crate) fn world_capsule_semantic_preview(
    compiled: &CompiledApp,
    app_path: &str,
    file_path: &str,
    semantic: WorldSemanticQuery<'_>,
    runtime_ctx: &PreviewRuntimeContext,
) -> Option<AnyView> {
    if !semantic.has_selection() {
        return None;
    }
    let index = compiled.world_semantic_by_file.get(file_path)?;

    if let Some(dataset_id) = semantic
        .world_dataset
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let title = index
            .datasets
            .iter()
            .find(|dataset| dataset.id == dataset_id)
            .and_then(|dataset| dataset.title.as_deref());
        return Some(dataset_table_preview(
            compiled,
            app_path,
            file_path,
            dataset_id,
            runtime_ctx,
            title,
        ));
    }

    let metric_id = semantic.world_metric.map(str::trim).filter(|value| !value.is_empty())?;
    let metric_meta = index.metrics.iter().find(|metric| metric.id == metric_id)?;
    let resource_id = index.resource_id.as_str();
    let dataset = find_world_metrics_dataset(compiled, file_path);
    let contract = dataset
        .and_then(|dataset| lookup_metric_contract(compiled, dataset, resource_id, metric_id));
    let preview = match contract.map(|entry| entry.shape) {
        Some(MetricShape::Scalar) => scalar_metric_form_preview(metric_meta, contract),
        Some(MetricShape::Table | MetricShape::Dataframe | MetricShape::Series) => {
            metric_table_preview(
                compiled,
                app_path,
                file_path,
                metric_id,
                contract?,
                resource_id,
            )
        }
        _ => scalar_metric_form_preview(metric_meta, contract),
    };
    Some(preview)
}
