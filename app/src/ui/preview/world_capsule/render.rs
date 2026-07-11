use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, MetricShape};

use crate::ui::manage_routing::WorldSemanticQuery;
use crate::ui::preview::PreviewRuntimeContext;

use super::dataset::*;
use super::lookup::*;

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

    let parent_metric_id = semantic
        .world_metric
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
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
                <section class="world-capsule-preview world-capsule-preview-empty rounded-[14px] border border-amber-500/25 mei-surface-panel-muted p-4 text-sm mei-text-body">
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
