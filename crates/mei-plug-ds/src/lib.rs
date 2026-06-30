//! Data-source plugin: parquet import, dataset query, metric eval.

mod cli;
mod client_bootstrap_refresh;
mod commands;
mod dataset_api;
mod eval;
mod eval_pipeline;
mod http;
mod memory_warmup;
mod plugin;
mod smart_warmup;
mod warmup;
mod warmup_orchestrator;

pub use cli::{Cli, Command, ServeArgs, WarmupArgs};
pub use commands::{resolve_warmup_targets, run_serve, run_warmup};
pub use http::router as http_router;

pub use smart_warmup::{maybe_trigger_smart_warmup, run_activation_warmup, run_smart_warmup};

pub use eval::{eval_metric_ids, load_compiled_for_warmup};
pub use mei_host_graph::WarmupTier;
pub use plugin::{materialize_targets, query_dataset, query_metrics, DsPluginImpl};
pub use warmup::{
    collect_all_board_scenes, collect_warmup_targets, frontier_targets_from_metrics, WarmupTarget,
};
pub use warmup_orchestrator::{
    hydrate_existing_l1_slots, run_warmup_targets_with_tier, WarmupOrchestratorReport,
};
