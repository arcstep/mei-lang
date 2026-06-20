use std::collections::BTreeMap;

use mei_lang_kernel::{build_runtime_eval_plan, CompiledApp, DatasetView, RuntimeMetricEvalScope};
use mei_lang_toolchain::format_eval_plan_markdown;

pub fn eval_plan_markdown_for_metric(
    _compiled: &CompiledApp,
    dataset: &DatasetView,
    metric_id: &str,
) -> Result<String, String> {
    let scope = RuntimeMetricEvalScope::default();
    let mut datasets = BTreeMap::new();
    datasets.insert(dataset.id.clone(), dataset.clone());
    let plan = build_runtime_eval_plan(
        &dataset.runtime_metric_defs,
        Some(&[metric_id.to_string()]),
        &datasets,
        &scope,
    );
    Ok(format_eval_plan_markdown(&plan))
}
