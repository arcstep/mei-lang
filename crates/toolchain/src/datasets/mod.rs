#[path = "../../../../server/src/http/datasets/csv_dataset.rs"]
mod csv_dataset;
#[path = "../../../../server/src/http/datasets/db_dataset.rs"]
mod db_dataset;
#[path = "../../../../server/src/http/datasets/file_cache.rs"]
#[allow(dead_code)] // Compatibility import: toolchain reuses the host dataset stack but only exposes an access-side subset today.
mod file_cache;
#[path = "../../../../server/src/http/datasets/geojson_dataset.rs"]
mod geojson_dataset;
#[path = "../../../../server/src/http/datasets/json_dataset.rs"]
mod json_dataset;
#[path = "../../../../server/src/http/datasets/metric_cache_key.rs"]
#[allow(dead_code)] // Compatibility import: access-side evaluation only needs part of this shared metric cache helper surface.
mod metric_cache_key;
#[path = "../../../../server/src/http/datasets/metric_hydrate.rs"]
mod metric_hydrate;
mod metric_locate;
#[path = "../../../../server/src/http/datasets/paginate.rs"]
mod paginate;
#[path = "../../../../server/src/http/datasets/paths.rs"]
mod paths;
#[path = "../../../../server/src/http/datasets/query.rs"]
mod query;
#[path = "../../../../server/src/http/datasets/table_contract.rs"]
#[allow(dead_code)] // Compatibility import: table API contract helpers stay shared even when the access path uses only row querying.
pub mod table_contract;
#[path = "../../../../server/src/http/datasets/types.rs"]
mod types;
#[path = "../../../../server/src/http/datasets/util.rs"]
mod util;
#[path = "../../../../server/src/http/datasets/xlsx_dataset.rs"]
mod xlsx_dataset;

pub(crate) use metric_cache_key::{
    metric_request_revision_fingerprint, normalize_query_filters, normalize_query_search,
    query_state_from_request, runtime_metric_eval_scope, runtime_metric_workset,
};
pub(crate) use metric_hydrate::hydrate_file_backed_datasets_for_metric_defs;
pub(crate) use metric_locate::{
    locate_runtime_metric_resource, metric_ids_visible_for_dataset, plan_access_metric_eval_for_ids,
};
pub(crate) use query::query_dataset_rows;
pub(crate) use types::DatasetQueryOptions;
