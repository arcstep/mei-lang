//! Host SSR：组件 `data-props` 只保留 schema / 运行时引用，不内联行集与大块指标值。

use mei_lang_kernel::{DatasetView, MetricContract, MetricShape};
use serde_json::Value;

pub(crate) fn dataset_for_host_ssr(dataset: &DatasetView) -> Value {
    let mut slim = dataset.clone();
    slim.rows.clear();
    for metric in slim.metrics.values_mut() {
        if metric.shape != MetricShape::Scalar {
            metric.value = Value::Null;
        }
    }
    serde_json::to_value(slim).unwrap_or(Value::Null)
}

pub(crate) fn metric_for_host_ssr(metric: &MetricContract) -> Value {
    let mut slim = metric.clone();
    if slim.shape != MetricShape::Scalar {
        slim.value = Value::Null;
    }
    serde_json::to_value(slim).unwrap_or(Value::Null)
}
