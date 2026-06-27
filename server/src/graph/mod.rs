//! MCG/MRG dual-graph registry (see docs `80` / `80b`).

pub mod bridge;
pub mod content_store;
pub mod dedup;
pub mod feature;
pub mod hydrate_closure;
pub mod integration;
pub mod io;
pub mod migrate;
pub mod mcg;
pub mod mrg;
pub mod observability;
pub mod paths;
pub mod types;

#[cfg(test)]
mod tests;

pub use dedup::{
    canonical_slot_cache_key_for_workset, load_mcg_bundle_revisions, load_mrg_registry,
    metric_bundle_revision_unchanged, mrg_eval_scope_key, mrg_slot_covers_dataframe_eval,
    mrg_slot_covers_eval, resolve_metric_bundle_revision,
};
pub use integration::{
    app_graph_fingerprint, bundle_unchanged_owners, discover_world_metrics_owner_ids,
    embedded_capsule_target_files, hydrate_compiled_for_prebuild_eval,
    maybe_update_graph_after_compile, record_prebuild_dataframe_slot, record_prebuild_slot,
    record_prebuild_slot_failed, runtime_payloads_from_compiled, schedule_warmup_frontier,
    try_assemble_scope_from_scene_payload,
};
pub use observability::{
    run_graph_doctor, run_graph_inspect, run_graph_status, run_scope_gate_check,
};
