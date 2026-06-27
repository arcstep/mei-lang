//! Prebuild orchestration: compile scopes, warm artifacts, diagnostics.
mod prelude;
mod progress;
mod compile_index;
mod diagnostics;
mod optimization;
mod types;
mod scope;
mod coverage;
mod run;
mod compile_app;
mod compile_app_finish;
mod plan;
mod warmup;
mod compile_session;
mod compile_scope_ops;
mod artifact_helpers;
mod artifact_helpers_ctx;
mod artifact_plan;
mod artifact_plan_collect;
mod artifact_metric;
mod artifact_dataframe;
mod artifact_aliases;
mod mrg_plan;
mod parallel;
mod scoped_materialize;
mod phase_tracker;
mod mcg_target_plan;
mod owner_batch_eval;
mod worker;
#[cfg(test)]
mod tests;

pub(crate) use progress::*;
pub(crate) use compile_index::*;
pub(crate) use diagnostics::*;
pub(crate) use optimization::*;
pub(crate) use types::*;
pub(crate) use scope::*;
pub(crate) use coverage::*;
pub(crate) use run::*;
pub(crate) use compile_app::*;
pub(crate) use compile_app_finish::*;
pub(crate) use plan::*;
pub(crate) use warmup::*;
pub(crate) use compile_session::*;
pub(crate) use compile_scope_ops::*;
pub(crate) use artifact_helpers::*;
pub(crate) use artifact_helpers_ctx::*;
pub(crate) use artifact_plan::*;
pub(crate) use artifact_plan_collect::*;
pub(crate) use artifact_metric::*;
pub(crate) use artifact_dataframe::*;
pub(crate) use artifact_aliases::*;
pub(crate) use mrg_plan::*;
pub(crate) use parallel::*;
pub(crate) use phase_tracker::*;
pub(crate) use mcg_target_plan::*;
pub(crate) use owner_batch_eval::*;
pub(crate) use worker::*;

#[allow(unused_imports)]
pub use types::{
    PrebuildAppReport, PrebuildAppSummary, PrebuildCompileIndexStatsReport,
    PrebuildCoverageReport, PrebuildDiagnosticsReport, PrebuildDiskUsageReport,
    PrebuildEvalArtifactDiskReport, PrebuildMode, PrebuildNodeBudgetReport,
    PrebuildOptions, PrebuildPlanNodeStatsReport, PrebuildReport, PrebuildReportSummary,
    PrebuildScopeProfile, PrebuildScopeReport, PrebuildScopeSummary, PrebuildSessionEntryStatsReport,
    PrebuildSlowMetricDiagnostic, PrebuildSlowScopeDiagnostic, PrebuildTimingReport,
    PrebuildWarmupDiagnosticReport, PrebuildWarningReport, effective_prebuild_scope_profile,
};
pub use run::{
    clean_workspace_prebuild_artifacts, load_prebuild_report, persist_prebuild_report,
    run_prebuild,
};
pub use mrg_plan::build_mrg_eval_frontier;
pub use scoped_materialize::{materialize_scope_after_compile, ScopedMaterializeReport};
