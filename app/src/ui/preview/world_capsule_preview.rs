use leptos::prelude::*;
use mei_lang_kernel::{
    resolve_dataset_resource_id, resolve_runtime_metric_def_key, CompiledApp, DatasetView,
    MetricContract, MetricShape, WorldSemanticExplainBlock, WorldSemanticMetric,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use super::nodes::component_html;
use super::resolve::{
    attach_host_meta, dataset_for_host_ssr, metric_for_host_ssr, with_runtime_ref,
    HostMetaOptions, RuntimeSceneAnchor,
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

fn runtime_scene_anchor(compiled: &CompiledApp, file_path: &str) -> RuntimeSceneAnchor {
    RuntimeSceneAnchor {
        scene_id: compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "default".to_string()),
        scene_path: Some(file_path.to_string()),
    }
}

fn table_host_html(
    compiled: &CompiledApp,
    app_path: &str,
    file_path: &str,
    data: Value,
) -> String {
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

fn prepare_dataset_table_data(
    dataset: &DatasetView,
    resolved_id: &str,
    anchor: &RuntimeSceneAnchor,
    host_ssr_slim_payload: bool,
) -> Value {
    let data = if host_ssr_slim_payload {
        dataset_for_host_ssr(dataset)
    } else {
        serde_json::to_value(dataset).unwrap_or(Value::Null)
    };
    with_runtime_ref(
        data,
        anchor.runtime_ref_extra("data", resolved_id, None, None),
    )
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

const SCALAR_ROWSET_SUFFIX: &str = "__scalar_rowset__";

fn canonical_parent_metric_key(
    dataset: &DatasetView,
    resource_id: &str,
    parent_metric_id: &str,
) -> String {
    resolve_runtime_metric_def_key(resource_id, parent_metric_id, &dataset.runtime_metric_defs)
        .unwrap_or_else(|| parent_metric_id.to_string())
}

fn tabular_node_id_from_analysis_contract(
    dataset: &DatasetView,
    resource_id: &str,
    parent_metric_id: &str,
    explain_block_id: &str,
) -> Option<String> {
    let parent_key = canonical_parent_metric_key(dataset, resource_id, parent_metric_id);
    let contract = dataset.runtime_analysis_contracts.get(&parent_key)?;
    let blocks = contract.get("blocks")?.as_array()?;
    for block in blocks {
        let Some(block_obj) = block.as_object() else {
            continue;
        };
        if block_obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            != Some(explain_block_id)
        {
            continue;
        }
        return block_obj
            .get("node_id")
            .or_else(|| block_obj.get("metric_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
    None
}

fn tabular_metric_lookup_candidates(
    parent_metric_id: &str,
    explain_block_id: Option<&str>,
    explain_block: Option<&WorldSemanticExplainBlock>,
    dataset: Option<&DatasetView>,
    resource_id: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(block_id) = explain_block_id.map(str::trim).filter(|value| !value.is_empty()) {
        if let Some(dataset) = dataset {
            if let Some(node_id) =
                tabular_node_id_from_analysis_contract(dataset, resource_id, parent_metric_id, block_id)
            {
                candidates.push(node_id);
            }
        }
        candidates.push(format!("{parent_metric_id}::{block_id}"));
        let role = explain_block
            .and_then(|block| block.support_role.as_deref())
            .unwrap_or_else(|| explain_block.map(|block| block.kind.as_str()).unwrap_or(""));
        if role == "detail" {
            candidates.push(format!("{parent_metric_id}::{SCALAR_ROWSET_SUFFIX}"));
        }
        candidates.push(block_id.to_string());
    } else {
        candidates.push(parent_metric_id.to_string());
    }
    let mut seen = BTreeMap::<String, ()>::new();
    candidates
        .into_iter()
        .filter(|candidate| {
            if candidate.is_empty() || seen.contains_key(candidate) {
                false
            } else {
                seen.insert(candidate.clone(), ());
                true
            }
        })
        .collect()
}

fn resolve_explain_block_id<'a>(
    metric_meta: &'a WorldSemanticMetric,
    explain_block_id: Option<&'a str>,
) -> Option<&'a str> {
    let raw = explain_block_id?.trim();
    if raw.is_empty() {
        return None;
    }
    if metric_meta
        .explain
        .iter()
        .any(|block| block.id == raw)
    {
        return Some(raw);
    }
    if let Some(suffix) = raw.strip_prefix("data_product_") {
        if let Ok(index) = suffix.parse::<usize>() {
            return metric_meta.explain.get(index).map(|block| block.id.as_str());
        }
    }
    Some(raw)
}

fn lookup_metric_contract<'a>(
    compiled: &'a CompiledApp,
    dataset: &'a DatasetView,
    resource_id: &str,
    lookup_candidates: &[String],
    parent_metric_id: Option<&str>,
) -> Option<&'a MetricContract> {
    for candidate in lookup_candidates {
        if let Some(entry) = compiled.world_metrics.get(candidate.as_str()) {
            if metric_contract_is_tabular(&entry.metric) {
                return Some(&entry.metric);
            }
        }
    }
    for candidate in lookup_candidates {
        if let Some(canonical) =
            resolve_runtime_metric_def_key(resource_id, candidate.as_str(), &dataset.runtime_metric_defs)
        {
            if let Some(metric) = dataset.metrics.get(&canonical) {
                return Some(metric);
            }
        }
        if let Some(metric) = dataset.metrics.get(candidate.as_str()) {
            return Some(metric);
        }
    }
    let parent = parent_metric_id.map(str::trim).filter(|value| !value.is_empty())?;
    for candidate in lookup_candidates {
        let metric_id = candidate.as_str();
        if metric_id == parent {
            continue;
        }
        if let Some(metric) = dataset.metrics.iter().find_map(|(key, metric)| {
            if key == metric_id
                || key.ends_with(&format!("::{metric_id}"))
                || (metric_id.contains("::") && key == metric_id)
            {
                Some(metric)
            } else {
                None
            }
        }) {
            return Some(metric);
        }
    }
    None
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

fn metric_contract_is_tabular(contract: &MetricContract) -> bool {
    matches!(
        contract.shape,
        MetricShape::Table | MetricShape::Dataframe | MetricShape::Series
    ) || contract.value.is_array()
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

    let parent_metric_id = semantic.world_metric.map(str::trim).filter(|value| !value.is_empty())?;
    let metric_meta = index
        .metrics
        .iter()
        .find(|metric| metric.id == parent_metric_id)?;
    let resource_id = index.resource_id.as_str();
    let dataset = find_world_metrics_dataset(compiled, file_path);
    let explain_block_id = resolve_explain_block_id(metric_meta, semantic.explain);
    let explain_block = explain_block_id.and_then(|block_id| {
        metric_meta
            .explain
            .iter()
            .find(|block| block.id == block_id)
    });
    let lookup_candidates = tabular_metric_lookup_candidates(
        parent_metric_id,
        explain_block_id,
        explain_block,
        dataset,
        resource_id,
    );
    let lookup_metric_id = lookup_candidates
        .first()
        .map(String::as_str)
        .unwrap_or(parent_metric_id);
    let contract = dataset.and_then(|dataset| {
        lookup_metric_contract(
            compiled,
            dataset,
            resource_id,
            &lookup_candidates,
            Some(parent_metric_id),
        )
    });
    let preview = if contract.is_some_and(metric_contract_is_tabular) {
        metric_table_preview(
            compiled,
            app_path,
            file_path,
            lookup_metric_id,
            contract?,
            resource_id,
            runtime_ctx.host_ssr_slim_payload,
        )
    } else if explain_block_id.is_some() {
        if contract.is_some_and(|entry| entry.shape == MetricShape::Scalar) {
            scalar_metric_form_preview(metric_meta, contract)
        } else {
            view! {
                <section class="world-capsule-preview world-capsule-preview-empty rounded-[14px] border border-amber-500/25 bg-slate-950/35 p-4 text-sm text-slate-300">
                    <p class="m-0">
                        "未找到 explain 块 `"
                        {lookup_metric_id.to_string()}
                        "` 的可表格化物化结果。"
                    </p>
                </section>
            }
            .into_any()
        }
    } else if contract.is_some_and(|entry| entry.shape == MetricShape::Scalar) {
        scalar_metric_form_preview(metric_meta, contract)
    } else {
        scalar_metric_form_preview(metric_meta, contract)
    };
    Some(preview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabular_metric_lookup_candidates_resolve_detail_scalar_rowset() {
        let detail = WorldSemanticExplainBlock {
            id: "detail".to_string(),
            kind: "detail".to_string(),
            label: Some("单位明细".to_string()),
            by: None,
            support_role: Some("detail".to_string()),
        };
        let candidates = tabular_metric_lookup_candidates(
            "enforcement_units_count",
            Some("detail"),
            Some(&detail),
            None,
            "__world_metrics__",
        );
        assert!(
            candidates.iter().any(|key| key == "enforcement_units_count::__scalar_rowset__"),
            "detail explain should fall back to scalar rowset: {candidates:?}"
        );
    }

    #[test]
    fn tabular_metric_lookup_candidates_prefers_analysis_contract_node_id() {
        use mei_lang_kernel::DatasetView;
        use serde_json::json;
        use std::collections::BTreeMap;

        let mut runtime_analysis_contracts = BTreeMap::new();
        runtime_analysis_contracts.insert(
            "enforcement_objects_count".to_string(),
            json!({
                "blocks": [{
                    "id": "enforcement_agency_objects_table",
                    "node_id": "enforcement_objects_count::enforcement_agency_objects_table",
                }]
            }),
        );
        let dataset = DatasetView {
            id: "__world_metrics__".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: mei_lang_kernel::SourceDecl {
                kind: "world_metrics".to_string(),
                path: String::new(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts,
        };
        let block = WorldSemanticExplainBlock {
            id: "enforcement_agency_objects_table".to_string(),
            kind: "data_product".to_string(),
            label: None,
            by: None,
            support_role: None,
        };
        let candidates = tabular_metric_lookup_candidates(
            "enforcement_objects_count",
            Some("enforcement_agency_objects_table"),
            Some(&block),
            Some(&dataset),
            "__world_metrics__",
        );
        assert_eq!(
            candidates.first().map(String::as_str),
            Some("enforcement_objects_count::enforcement_agency_objects_table")
        );
    }

    #[test]
    fn tabular_metric_lookup_candidates_use_parent_for_top_level_metric() {
        let candidates = tabular_metric_lookup_candidates(
            "enterprise_map_rows_2025",
            None,
            None,
            None,
            "__world_metrics__",
        );
        assert_eq!(candidates, vec!["enterprise_map_rows_2025".to_string()]);
    }

    #[test]
    fn resolve_explain_block_id_maps_legacy_data_product_index() {
        let metric = WorldSemanticMetric {
            id: "enforcement_objects_count".to_string(),
            label: None,
            unit: None,
            note: None,
            explain: vec![
                mei_lang_kernel::WorldSemanticExplainBlock {
                    id: "enforcement_venues_table".to_string(),
                    kind: "data_product".to_string(),
                    label: Some("场所".to_string()),
                    by: None,
                    support_role: None,
                },
                mei_lang_kernel::WorldSemanticExplainBlock {
                    id: "enforcement_agency_objects_table".to_string(),
                    kind: "data_product".to_string(),
                    label: Some("机构对象".to_string()),
                    by: None,
                    support_role: None,
                },
            ],
        };
        assert_eq!(
            resolve_explain_block_id(&metric, Some("data_product_0")),
            Some("enforcement_venues_table")
        );
        assert_eq!(
            resolve_explain_block_id(&metric, Some("enforcement_agency_objects_table")),
            Some("enforcement_agency_objects_table")
        );
    }
}
