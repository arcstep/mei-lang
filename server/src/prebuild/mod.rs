//! Prebuild orchestration: compile scopes, warm artifacts, diagnostics.
mod artifact_aliases;
mod artifact_dataframe;
mod artifact_helpers;
mod artifact_helpers_ctx;
mod artifact_metric;
mod artifact_plan;
mod artifact_plan_collect;
mod compile_app;
mod compile_app_finish;
mod compile_index;
mod compile_scope_ops;
mod compile_session;
mod coverage;
mod diagnostics;
mod mcg_target_plan;
mod mrg_plan;
mod optimization;
mod owner_batch_eval;
mod parallel;
mod phase_tracker;
mod plan;
mod prelude;
mod progress;
mod run;
mod scope;
mod scoped_materialize;
#[cfg(test)]
mod tests;
mod types;
mod warmup;
mod worker;

pub(crate) use artifact_aliases::*;
pub(crate) use artifact_dataframe::*;
pub(crate) use artifact_helpers::*;
pub(crate) use artifact_helpers_ctx::*;
pub(crate) use artifact_metric::*;
pub(crate) use artifact_plan::*;
pub(crate) use artifact_plan_collect::*;
pub(crate) use compile_app::*;
pub(crate) use compile_app_finish::*;
pub(crate) use compile_index::*;
pub(crate) use compile_scope_ops::*;
pub(crate) use compile_session::*;
pub(crate) use coverage::*;
pub(crate) use diagnostics::*;
pub(crate) use mcg_target_plan::*;
pub(crate) use mrg_plan::*;
pub(crate) use optimization::*;
pub(crate) use owner_batch_eval::*;
pub(crate) use parallel::*;
pub(crate) use phase_tracker::*;
pub(crate) use plan::*;
pub(crate) use progress::*;
pub(crate) use run::*;
pub(crate) use scope::*;
pub(crate) use types::*;
pub(crate) use warmup::*;
pub(crate) use worker::*;

pub use mrg_plan::build_mrg_eval_frontier;
pub use run::{
    clean_workspace_prebuild_artifacts, load_prebuild_report, persist_prebuild_report, run_prebuild,
};
pub use scoped_materialize::{materialize_scope_after_compile, ScopedMaterializeReport};
#[allow(unused_imports)]
pub use types::{
    effective_prebuild_scope_profile, PrebuildAppReport, PrebuildAppSummary,
    PrebuildCompileIndexStatsReport, PrebuildCoverageReport, PrebuildDiagnosticsReport,
    PrebuildDiskUsageReport, PrebuildEvalArtifactDiskReport, PrebuildMode,
    PrebuildNodeBudgetReport, PrebuildOptions, PrebuildPlanNodeStatsReport, PrebuildReport,
    PrebuildReportSummary, PrebuildScopeProfile, PrebuildScopeReport, PrebuildScopeSummary,
    PrebuildSessionEntryStatsReport, PrebuildSlowMetricDiagnostic, PrebuildSlowScopeDiagnostic,
    PrebuildTimingReport, PrebuildWarmupDiagnosticReport, PrebuildWarningReport,
};
