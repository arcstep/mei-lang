use leptos::prelude::*;
use mei_lang_kernel::{
    resolve_dataset_resource_id, CompiledApp, MetricContract, WorldSemanticMetric,
};
use serde_json::Value;

use crate::ui::preview::resolve::{metric_for_host_ssr, with_runtime_ref};
use crate::ui::preview::PreviewRuntimeContext;

use super::lookup::*;

pub(super) fn dataset_table_preview(
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
                <section class="world-capsule-preview world-capsule-preview-empty rounded-[14px] border border-amber-500/25 mei-surface-panel-muted p-4 text-sm mei-text-body">
                    <p class="m-0">"无法解析 dataset："{detail}</p>
                </section>
            }
            .into_any();
        }
    };
    let Some(resource) = runtime_ctx.resources.get(&resolved_id) else {
        return view! {
            <section class="world-capsule-preview world-capsule-preview-empty rounded-[14px] border border-amber-500/25 mei-surface-panel-muted p-4 text-sm mei-text-body">
                <p class="m-0">"未找到已物化的 dataset `"{dataset_id}"`。"</p>
            </section>
        }
        .into_any();
    };
    let Some(dataset) = resource.dataset.as_ref() else {
        return view! {
            <section class="world-capsule-preview world-capsule-preview-empty rounded-[14px] border border-amber-500/25 mei-surface-panel-muted p-4 text-sm mei-text-body">
                <p class="m-0">"资源 `"{resolved_id}"` 不含 dataset 视图。"</p>
            </section>
        }
        .into_any();
    };
    let anchor = runtime_scene_anchor(compiled, file_path);
    let mut data = prepare_dataset_table_data(
        dataset,
        resolved_id.as_str(),
        &anchor,
        runtime_ctx.host_ssr_slim_payload,
    );
    if let Some(title) = title.filter(|text| !text.trim().is_empty()) {
        if let Some(map) = data.as_object_mut() {
            map.insert("title".to_string(), Value::String(title.to_string()));
        }
    }
    let html = table_host_html(compiled, app_path, file_path, data);
    view! {
        <section class="world-capsule-preview world-capsule-dataset-table preview-surface min-h-0 p-3">
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}

pub(super) fn scalar_form_field(label: String, value: AnyView) -> AnyView {
    view! {
        <label class="world-capsule-scalar-field">
            <span class="world-capsule-scalar-field__label">{label}</span>
            <span class="world-capsule-scalar-field__value">{value}</span>
        </label>
    }
    .into_any()
}

pub(super) fn scalar_metric_form_preview(
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
                view! { <span class="text-lg font-semibold text-sky-100">{value}</span> }
                    .into_any(),
            )
        })
        .unwrap_or_else(|| {
            scalar_form_field(
                "指标值".to_string(),
                view! { <span class="mei-text-muted">"（未物化）"</span> }.into_any(),
            )
        });
    let mut fields = vec![
        scalar_form_field("标识".to_string(), view! { {metric.id.clone()} }.into_any()),
        scalar_form_field(
            "标签".to_string(),
            view! { {display_label.clone()} }.into_any(),
        ),
        scalar_form_field("形状".to_string(), view! { {shape_label} }.into_any()),
    ];
    if !unit.is_empty() {
        fields.push(scalar_form_field(
            "单位".to_string(),
            view! { {unit} }.into_any(),
        ));
    }
    fields.push(value_field);
    if let Some(note) = metric.note.as_ref().filter(|note| !note.trim().is_empty()) {
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
                <ul class="m-0 list-none pl-0 font-mono text-[11px] mei-text-body">
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
        <section class="world-capsule-preview world-capsule-scalar-form preview-surface rounded-[14px] border mei-border-default mei-surface-panel-muted p-4">
            <header class="mb-4 border-b border-slate-700/50 pb-3">
                <h3 class="m-0 text-base font-semibold mei-text-inverse">{display_label}</h3>
                <div class="mt-1 font-mono text-[11px] mei-text-muted">{metric.id.clone()}</div>
            </header>
            <form class="world-capsule-scalar-form__grid" on:submit=|ev| { ev.prevent_default(); }>
                {fields}
            </form>
        </section>
    }
    .into_any()
}

pub(super) fn metric_table_preview(
    compiled: &CompiledApp,
    app_path: &str,
    file_path: &str,
    lookup_metric_id: &str,
    contract: &MetricContract,
    resource_id: &str,
    host_ssr_slim_payload: bool,
) -> AnyView {
    let anchor = runtime_scene_anchor(compiled, file_path);
    let metric_id = contract.id.as_str();
    let payload = if host_ssr_slim_payload {
        metric_for_host_ssr(contract)
    } else {
        serde_json::to_value(contract).unwrap_or(Value::Null)
    };
    let mut data = with_runtime_ref(
        payload,
        anchor.runtime_ref_extra("metric", resource_id, Some(metric_id), None),
    );
    let title = contract
        .label
        .clone()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| lookup_metric_id.to_string());
    if let Some(map) = data.as_object_mut() {
        map.insert("title".to_string(), Value::String(title));
    }
    let html = table_host_html(compiled, app_path, file_path, data);
    view! {
        <section class="world-capsule-preview world-capsule-dataset-table preview-surface min-h-0 p-3">
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}
