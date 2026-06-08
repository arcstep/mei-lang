use std::collections::BTreeSet;

use mei_lang_kernel::DatasetView;

pub(crate) const DATASET_QUERY_DEFAULT_LIMIT: usize = 10;
pub(crate) const DATASET_QUERY_MAX_LIMIT: usize = 50;
pub(crate) const DATASET_QUERY_DEFAULT_COLUMNS: usize = 10;
pub(crate) const DATASET_QUERY_MAX_COLUMNS: usize = 10;

fn normalize_limit(limit: Option<usize>, default: usize, max: usize) -> usize {
    limit.unwrap_or(default).clamp(1, max)
}

pub(crate) fn normalize_dataset_limit(limit: Option<usize>) -> usize {
    normalize_limit(limit, DATASET_QUERY_DEFAULT_LIMIT, DATASET_QUERY_MAX_LIMIT)
}

fn dataset_available_columns(dataset: &DatasetView) -> Vec<String> {
    if !dataset.columns.is_empty() {
        return dataset.columns.clone();
    }
    dataset.schema.iter().map(|c| c.name.clone()).collect()
}

pub fn normalize_dataset_columns(
    dataset: &DatasetView,
    requested: Option<&[String]>,
) -> Vec<String> {
    let available = dataset_available_columns(dataset);
    let available_set = available.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = Vec::new();

    if let Some(req) = requested {
        for col in req {
            let name = col.trim();
            if name.is_empty() {
                continue;
            }
            if available_set.contains(name) && !selected.iter().any(|v| v == name) {
                selected.push(name.to_string());
            }
            if selected.len() >= DATASET_QUERY_MAX_COLUMNS {
                break;
            }
        }
    }

    if selected.is_empty() {
        selected = available
            .into_iter()
            .take(DATASET_QUERY_DEFAULT_COLUMNS)
            .collect();
    }
    selected
}
