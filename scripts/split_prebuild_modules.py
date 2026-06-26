#!/usr/bin/env python3
"""Split server/src/prebuild.rs into prebuild/ submodules (≤500 lines each)."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "server" / "src" / "prebuild.rs"
OUT = ROOT / "server" / "src" / "prebuild"

MODULE_PREFIX = "use super::prelude::*;\nuse super::*;\n\n"

PRELUDE_RS = """//! Shared imports for prebuild submodules.
pub(crate) use std::collections::{BTreeMap, BTreeSet};
pub(crate) use std::fs;
pub(crate) use std::io::{IsTerminal, Write};
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::atomic::{AtomicUsize, Ordering};
pub(crate) use std::sync::{Arc, Condvar, Mutex, OnceLock};
pub(crate) use std::thread;
pub(crate) use std::time::{Instant, SystemTime, UNIX_EPOCH};
pub(crate) use anyhow::{Context, Result};
pub(crate) use mei_lang_datasets::{
    collect_all_query_options, evaluate_runtime_metrics_from_plan,
    load_metric_dataframe_result_artifact, load_metric_response_result_artifact,
    locate_runtime_metric_resource, metric_dataframe_result_artifact_exists,
    metric_dataframe_result_cache_key, metric_request_revision_fingerprint_for_compiled,
    metric_response_cache_scope_key, metric_response_prebuild_shared_key,
    metric_response_result_artifact_exists, metric_scope_cache_key,
    plan_access_metric_eval_for_ids, prebuild_metric_response_index_covers_key,
    query_metric_dataframe, query_state_from_request, runtime_metric_workset,
    store_cached_metric_response, store_metric_dataframe_result_artifact,
    store_metric_response_result_artifact, DatasetQueryOptions, DatasetQueryResult,
    AccessMetricEvalPlan, LoadedMetricResponseArtifact, RuntimeMetricEvalMode,
};
pub(crate) use mei_lang_kernel::{
    begin_prebuild_generation, clear_prebuild_build_root_override, data_snapshot_import_manifest_path,
    data_snapshot_store_root, finish_prebuild_generation, resolve_app_root,
    resolve_data_snapshot_import_entry, resolve_runtime_warmup_manifest, set_prebuild_build_root_override,
    CompileOptions, CompiledApp, DatasetView, LoadedResource, RuntimeWarmupApp,
    RuntimeWarmupDatasetRequest, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};
pub(crate) use mei_lang_toolchain::{self as toolchain, PublishDataSnapshotsReport, WorldScope};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use serde_json::Value;
pub(crate) use walkdir::WalkDir;
"""

SECTIONS = [
    ("progress", 36, 363),
    ("compile_index", 365, 686),
    ("diagnostics", 688, 983),
    ("optimization", 984, 1359),
    ("types", 1360, 1743),
    ("scope", 1744, 1990),
    ("coverage", 1992, 2151),
    ("run", 2153, 2453),
    ("compile_app", 2454, 3281),
    ("plan", 3282, 3695),
    ("warmup", 3696, 3869),
    ("compile_session", 3871, 4199),
    ("compile_scope_ops", 4200, 4531),
    ("artifact_helpers", 4532, 4902),
    ("artifact_helpers_ctx", 4904, 5001),
    ("artifact_plan", 5002, 5241),
    ("artifact_plan_collect", 5242, 5500),
    ("artifact_metric", 5501, 5941),
    ("artifact_dataframe", 5942, 6318),
    ("artifact_aliases", 6319, 6549),
    ("parallel", 6550, 6624),
    ("scoped_materialize", 6625, 6756),
]

MOD_RS = """//! Prebuild orchestration: compile scopes, warm artifacts, diagnostics.
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
mod parallel;
mod scoped_materialize;
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
pub(crate) use parallel::*;

#[allow(unused_imports)]
pub use types::{
    PrebuildAppReport, PrebuildAppSummary, PrebuildCompileIndexStatsReport,
    PrebuildCoverageReport, PrebuildDiagnosticsReport, PrebuildDiskUsageReport,
    PrebuildEvalArtifactDiskReport, PrebuildMode, PrebuildNodeBudgetReport,
    PrebuildOptions, PrebuildPlanNodeStatsReport, PrebuildReport, PrebuildReportSummary,
    PrebuildScopeProfile, PrebuildScopeReport, PrebuildScopeSummary, PrebuildSessionEntryStatsReport,
    PrebuildSlowMetricDiagnostic, PrebuildSlowScopeDiagnostic, PrebuildTimingReport,
    PrebuildWarmupDiagnosticReport, PrebuildWarningReport,
};
pub use run::run_prebuild;
pub use scoped_materialize::{materialize_scope_after_compile, ScopedMaterializeReport};
pub(crate) use plan::app_has_deferred_warmup_work;
"""

FINISH_HEADER = """use super::prelude::*;
use super::*;

pub(crate) struct PrebuildAppAfterCompile {
    pub app_started: Instant,
    pub app_root: PathBuf,
    pub components_root: PathBuf,
    pub diagnostics: Arc<PrebuildDiagnostics>,
    pub manifest_plan: PrebuildManifestPlan,
    pub warmup_requests: Vec<AggregatedWarmupRequest>,
    pub max_parallelism: usize,
    pub pre_mcg_bundle_revisions: BTreeMap<String, String>,
    pub initial_scope_count: usize,
    pub compile_scopes_ms: u64,
    pub compile_reports: Vec<PrebuildScopeReport>,
    pub prepared_outcomes: Vec<PreparedCompileOutcome>,
    pub compile_session: Arc<Mutex<PrebuildCompileSession>>,
    pub warnings: Vec<PrebuildWarningReport>,
}

pub(crate) fn finish_run_prebuild_for_app(
    source_root: &Path,
    app: &RuntimeWarmupApp,
    mode: PrebuildMode,
    ctx: PrebuildAppAfterCompile,
) -> Result<PrebuildAppReport> {
    let PrebuildAppAfterCompile {
        app_started,
        app_root,
        components_root,
        diagnostics,
        manifest_plan,
        warmup_requests,
        max_parallelism,
        pre_mcg_bundle_revisions,
        initial_scope_count,
        compile_scopes_ms,
        compile_reports,
        prepared_outcomes,
        compile_session,
        mut warnings,
    } = ctx;

"""

FINISH_CALL = """
    finish_run_prebuild_for_app(
        source_root,
        app,
        mode,
        PrebuildAppAfterCompile {
            app_started,
            app_root,
            components_root,
            diagnostics,
            manifest_plan,
            warmup_requests,
            max_parallelism,
            pre_mcg_bundle_revisions,
            initial_scope_count,
            compile_scopes_ms,
            compile_reports,
            prepared_outcomes,
            compile_session,
            warnings,
        },
    )
}
"""


def promote_pub_crate(content: str) -> str:
    out: list[str] = []
    in_struct = False
    field_indent: str | None = None
    for line in content.splitlines(keepends=True):
        stripped = line.lstrip()
        indent = line[: len(line) - len(stripped)]
        if in_struct:
            if stripped.startswith("}"):
                if field_indent is None or len(indent) < len(field_indent):
                    in_struct = False
                    field_indent = None
                out.append(line)
                continue
            if field_indent is None and stripped and stripped != "{":
                field_indent = indent
            if (
                field_indent is not None
                and indent == field_indent
                and ":" in stripped
                and not stripped.startswith("pub")
                and not stripped.startswith("#")
            ):
                out.append(f"{indent}pub(crate) {stripped}")
                continue
            out.append(line)
            continue
        if line and not line[0].isspace():
            if stripped.startswith("struct "):
                in_struct = True
                field_indent = None
                out.append(f"pub(crate) {stripped}")
                continue
            if not stripped.startswith("pub") and stripped.startswith(
                ("fn ", "enum ", "const ", "type ")
            ):
                out.append(f"pub(crate) {stripped}")
                continue
        if stripped.startswith("fn ") and indent == "    " and not stripped.startswith("pub"):
            if stripped.startswith("fn default(") or stripped.startswith("fn drop("):
                out.append(line)
            else:
                out.append(f"{indent}pub(crate) {stripped}")
            continue
        out.append(line)
    return "".join(out)


def main() -> None:
    if not SRC.is_file():
        raise SystemExit(f"missing {SRC}")
    lines = SRC.read_text(encoding="utf-8").splitlines(keepends=True)
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "prelude.rs").write_text(PRELUDE_RS, encoding="utf-8")

    tests_start = next(i for i, l in enumerate(lines) if l.strip() == "#[cfg(test)]")
    tests_content = "".join(lines[tests_start:])
    tests_content = tests_content.replace("mod tests {", "mod tests {\n    use super::*;\n")
    (OUT / "tests.rs").write_text(tests_content, encoding="utf-8")

    for name, start, end in SECTIONS:
        body = promote_pub_crate(extract(lines, start, end))
        (OUT / f"{name}.rs").write_text(MODULE_PREFIX + body, encoding="utf-8")

    # split compile_app into compile + finish
    app_path = OUT / "compile_app.rs"
    app_lines = app_path.read_text(encoding="utf-8").splitlines(keepends=True)
    split_at = 435  # after compile phase progress line
    head = "".join(app_lines[:split_at]) + FINISH_CALL
    tail = FINISH_HEADER + "".join(app_lines[split_at:])
    app_path.write_text(head, encoding="utf-8")
    (OUT / "compile_app_finish.rs").write_text(tail, encoding="utf-8")

    # manual visibility fixes
    prog = OUT / "progress.rs"
    prog.write_text(
        prog.read_text(encoding="utf-8").replace(
            "impl PrebuildProgressSession {\n    fn begin()",
            "impl PrebuildProgressSession {\n    pub(crate) fn begin()",
        ),
        encoding="utf-8",
    )
    cov = OUT / "coverage.rs"
    cov.write_text(
        cov.read_text(encoding="utf-8").replace(
            "pub(crate) fn default() -> Self {", "fn default() -> Self {"
        ),
        encoding="utf-8",
    )
    cs = OUT / "compile_session.rs"
    cs.write_text(
        cs.read_text(encoding="utf-8").replace(
            "pub(crate) struct PrebuildCompileSession {",
            "#[derive(Default)]\npub(crate) struct PrebuildCompileSession {",
        ),
        encoding="utf-8",
    )
    types = OUT / "types.rs"
    types.write_text(
        types.read_text(encoding="utf-8").replace("use super::*;\n\n", ""),
        encoding="utf-8",
    )

    (OUT / "mod.rs").write_text(MOD_RS, encoding="utf-8")
    SRC.unlink()
    print(f"Wrote {OUT}/")


def extract(lines: list[str], start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


if __name__ == "__main__":
    main()
