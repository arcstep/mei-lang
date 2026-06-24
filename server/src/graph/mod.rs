//! MCG/MRG dual-graph registry (see docs `80` / `80b`).

pub mod bridge;
pub mod dedup;
pub mod feature;
pub mod integration;
pub mod io;
pub mod mcg;
pub mod mrg;
pub mod paths;
pub mod types;

#[cfg(test)]
mod tests;

pub use dedup::{
    load_mrg_registry, metric_bundle_revision_unchanged, mrg_eval_scope_key,
    mrg_slot_covers_dataframe_eval, mrg_slot_covers_eval,
};
pub use integration::{
    app_graph_fingerprint, bundle_unchanged_owners, maybe_update_graph_after_compile,
    record_prebuild_dataframe_slot, record_prebuild_slot, runtime_payloads_from_compiled,
    schedule_warmup_frontier, try_assemble_scope_from_scene_payload,
};
