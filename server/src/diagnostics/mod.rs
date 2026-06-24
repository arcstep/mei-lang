//! Materialization diagnostics: disk, MCG/MRG registry, compile cache counters.

mod collect;
mod report;

pub use collect::collect_materialization_diagnostics;
