//! MCG/MRG dual-graph registry (see docs `80` / `80b`).

pub mod bridge;
pub mod feature;
pub mod integration;
pub mod io;
pub mod mcg;
pub mod mrg;
pub mod paths;
pub mod types;

#[cfg(test)]
mod tests;

pub use integration::{
    app_graph_fingerprint, bundle_unchanged_owners, maybe_update_graph_after_compile,
    record_prebuild_slot, runtime_payloads_from_compiled, schedule_warmup_frontier,
};
pub use mrg::registry::MrgRegistryWriter;
pub use types::MaterialState;
