//! Data-source plugin: parquet import, dataset query, metric eval.

mod eval;
mod dataset_api;
mod plugin;
mod warmup;

pub use plugin::{materialize_targets, query_dataset, query_metrics, DsPluginImpl};
pub use warmup::{collect_all_board_scenes, collect_warmup_targets, WarmupTarget};
