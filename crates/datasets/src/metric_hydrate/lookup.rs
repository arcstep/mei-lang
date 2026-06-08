use std::collections::BTreeMap;

use mei_lang_kernel::{local_dataset_id_from_namespaced_token, DatasetView};

pub(crate) fn lookup_dataset_view<'a>(
    datasets: &'a BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    datasets
        .get(normalized)
        .or_else(|| datasets.get(dataset_id))
        .or_else(|| {
            datasets.iter().find_map(|(key, dataset)| {
                (dataset.id == normalized
                    || key.ends_with(&format!("::{normalized}"))
                    || key.ends_with(&format!("/{normalized}")))
                .then_some(dataset)
            })
        })
        .or_else(|| {
            local_dataset_id_from_namespaced_token(normalized)
                .and_then(|local| lookup_dataset_view(datasets, local))
        })
}

pub(crate) fn lookup_dataset_view_mut<'a>(
    datasets: &'a mut BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a mut DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    if datasets.contains_key(normalized) {
        return datasets.get_mut(normalized);
    }
    if datasets.contains_key(dataset_id) {
        return datasets.get_mut(dataset_id);
    }
    let key = datasets.iter().find_map(|(key, dataset)| {
        (dataset.id == normalized
            || key.ends_with(&format!("::{normalized}"))
            || key.ends_with(&format!("/{normalized}")))
        .then(|| key.clone())
    });
    if let Some(key) = key {
        return datasets.get_mut(key.as_str());
    }
    local_dataset_id_from_namespaced_token(normalized)
        .and_then(|local| lookup_dataset_view_mut(datasets, local))
}
