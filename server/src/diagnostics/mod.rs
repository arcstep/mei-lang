//! Materialization diagnostics: disk, MCG/MRG registry, compile cache counters.

mod build;
mod collect;
mod format;
mod report;

pub use build::persist_last_build_summary;
pub use collect::collect_materialization_diagnostics;
pub use format::{format_age_ms, format_bytes_human};
pub use report::{LastBuildSummary, MaterializationDiagnosticsReport};
