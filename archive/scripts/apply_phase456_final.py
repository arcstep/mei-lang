#!/usr/bin/env python3
"""One-shot Phase 4-6 dataset + execute fixes."""
from __future__ import annotations

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def git_lines(rel: str) -> list[str]:
    return subprocess.check_output(["git", "show", f"HEAD:{rel}"], text=True).splitlines(keepends=True)


def sl(lines: list[str], start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    n = content.count("\n")
    print(f"  {'WARN' if n > 501 else 'OK'} {path.relative_to(ROOT)}: {n}")


def promote(content: str) -> str:
    out: list[str] = []
    for line in content.splitlines(keepends=True):
        s = line.lstrip()
        ind = line[: len(line) - len(s)]
        if not ind and s.startswith(("fn ", "struct ")) and not s.startswith("pub"):
            out.append(f"pub(super) {s}")
        elif s.startswith("fn ") and ind == "    " and not s.startswith("pub"):
            out.append(f"{ind}pub(super) {s}")
        else:
            out.append(line)
    return "".join(out)


def split_datasets() -> None:
    print("datasets")
    ra = git_lines("crates/datasets/src/result_artifact.rs")
    out = ROOT / "crates/datasets/src/result_artifact"
    h = sl(ra, 1, 38)
    for name, a, b in [("core", 39, 258), ("index_a", 259, 507), ("index_b", 508, 732), ("store", 733, len(ra))]:
        write(out / f"{name}.rs", h + promote(sl(ra, a, b)))
    write(
        out / "mod.rs",
        """mod core; mod index_a; mod index_b; mod store;
pub use core::{default_result_artifact_scope,load_metric_response_result_artifact,metric_dataframe_result_artifact_exists,metric_response_result_artifact_exists,store_metric_dataframe_result_artifact,store_metric_response_result_artifact,take_metric_response_index_stats};
pub use index_a::{invalidate_prebuild_metric_response_index,prebuild_metric_response_index_covers_key};
pub use index_b::{load_prebuild_metric_response_artifact_dataset_fallback,preload_prebuild_metric_response_index,rebuild_and_install_prebuild_metric_response_index};
pub use store::load_metric_dataframe_result_artifact;
""",
    )
    (ROOT / "crates/datasets/src/result_artifact.rs").unlink(missing_ok=True)

    md = git_lines("crates/datasets/src/metric_dataframe.rs")
    out = ROOT / "crates/datasets/src/metric_dataframe"
    h = sl(md, 1, 87)
    write(out / "cache.rs", h + promote(sl(md, 88, 317)))
    write(out / "materialize.rs", h + promote(sl(md, 798, len(md))))

    imports = """use std::path::Path;
use std::time::Instant;
use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::{coerce_calendar_columns_in_rows, resolve_runtime_metric_def_key, runtime_eval_node_cache_enabled, ColumnSchema, CompiledApp, DatasetView, EvalPlanNodeKind, FilterIntent, MetricContract, MetricShape, QueryState};
use serde_json::Value;
use super::cache::{hash_fingerprint, metric_output_pagination_options, store_cached_metric_dataframe_materialized, store_cached_metric_dataframe_result, synthetic_scalar_rowset_parent, take_cached_metric_dataframe_materialized, take_cached_metric_dataframe_result, MIN_MATERIALIZED_METRIC_ROWS_TO_CACHE};
use super::materialize::{extract_dataframe_rows, paginate_materialized_metric_dataframe};
use super::eval_artifact::{eval_artifact_hydrate_dataset_ids, load_or_build_runtime_metric_workset_artifact};
use super::eval_execute::execute_runtime_eval_plan_artifacts;
use super::metric_hydrate::hydrate_file_backed_datasets_for_metric_defs;
use super::metric_hydrate::{resolve_dataset_query_bindings_from_state, unique_dataset_views};
use super::metric_locate::locate_runtime_metric_resource;
use super::query::query_dataset_rows;
use super::result_artifact::{default_result_artifact_scope, load_metric_dataframe_result_artifact, store_metric_dataframe_result_artifact};
use super::table_contract::{column_meta_for_row_schema, format_rows_with_dataset_schema};
use super::types::{parse_source_meta, DatasetQueryOptions, DatasetQueryResult};
use super::util::elapsed_ms;
use super::{build_compiled_datasets_map, metric_dataframe_artifact_lookup_cache_keys, metric_dataframe_result_cache_key, metric_request_revision_fingerprint_for_compiled, metric_scope_cache_key, query_state_from_request, runtime_metric_eval_scope, serialize_cache_value};

"""
    query_body = sl(md, 318, 797)
    # extract artifact block to shortcut.rs
    marker = "    if result_artifact_candidate {\n        let mut loaded_artifact = None;"
    start = query_body.index(marker)
    end = query_body.index("    let meta = parse_source_meta", start)
    block = query_body[start:end].replace("    if result_artifact_candidate {\n", "", 1)
    block = block.replace("        return Ok(artifact);", "        return Ok(Some(artifact));")
    if block.rstrip().endswith("}"):
        block = block.rstrip()[:-1].rstrip() + "\n"
    shortcut_fn = (
        "pub(super) fn try_load_dataframe_result_artifact(app_root: &Path, lookup_cache_keys: &[String], response_cache_lookup_started: Instant) -> Result<Option<DatasetQueryResult>> {\n"
        + block
        + "    Ok(None)\n}\n"
    )
    shortcut_imports = """use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;
use anyhow::Result;
use super::cache::{hash_fingerprint, store_cached_metric_dataframe_result};
use super::result_artifact::load_metric_dataframe_result_artifact;
use super::types::DatasetQueryResult;
use super::util::elapsed_ms;

"""
    write(out / "shortcut.rs", shortcut_imports + shortcut_fn)
    query_body = (
        query_body[:start]
        + "    if result_artifact_candidate {\n        if let Some(artifact) = try_load_dataframe_result_artifact(app_root, &lookup_cache_keys, response_cache_lookup_started)? { return Ok(artifact); }\n    }\n"
        + query_body[end:]
    )
    write(out / "query.rs", imports + query_body)
    write(
        out / "mod.rs",
        "mod cache;\nmod materialize;\nmod query;\nmod shortcut;\npub use cache::metric_dataframe_result_cache_key;\npub use query::query_metric_dataframe;\n",
    )
    (ROOT / "crates/datasets/src/metric_dataframe.rs").unlink(missing_ok=True)

    pg = git_lines("crates/datasets/src/paginate.rs")
    out = ROOT / "crates/datasets/src/paginate"
    h = sl(pg, 1, 10)
    for name, a, b in [("core", 11, 165), ("sort", 166, 287), ("filter", 288, 504), ("helpers", 505, len(pg))]:
        write(out / f"{name}.rs", h + promote(sl(pg, a, b)))
    write(
        out / "mod.rs",
        """mod core; mod filter; mod helpers; mod sort;
pub(crate) use core::{paginate_rows,paginate_rows_iter};
pub(crate) use helpers::{apply_normalize,infer_columns,output_columns,row_matches,QueryWindow};
pub(crate) use sort::normalize_search;
""",
    )
    (ROOT / "crates/datasets/src/paginate.rs").unlink(missing_ok=True)

    ck = git_lines("crates/datasets/src/metric_cache_key/cache_key.rs")
    out = ROOT / "crates/datasets/src/metric_cache_key/cache_key"
    h = sl(ck, 1, 57)
    write(out / "identity.rs", h + promote(sl(ck, 58, 260)))
    write(out / "scope.rs", h + "use super::identity::*;\n\n" + promote(sl(ck, 261, 457)))
    write(out / "lookup.rs", h + "use super::identity::*;\nuse super::scope::*;\n\n" + promote(sl(ck, 458, len(ck))))
    write(
        out / "mod.rs",
        """mod identity; mod lookup; mod scope;
pub(crate) use identity::{dataset_metric_identity_key,dataset_resource_lookup_aliases,effective_compile_revision_for_slot,equivalent_dataset_resource_ids,eval_node_cache_key,metric_request_revision_fingerprint,metric_request_revision_fingerprint_for_compiled,metric_scope_cache_key,serialize_cache_value,stable_slot_hash};
pub(crate) use lookup::{equivalent_dataframe_metric_scope_tokens,lookup_compiled_dataset_view,metric_dataframe_artifact_lookup_cache_keys,metric_response_artifact_lookup_cache_keys};
pub(crate) use scope::runtime_metric_eval_scope;
""",
    )
    (ROOT / "crates/datasets/src/metric_cache_key/cache_key.rs").unlink(missing_ok=True)

    mod_path = ROOT / "crates/datasets/src/metric_cache_key/mod.rs"
    text = mod_path.read_text(encoding="utf-8")
    if "#[cfg(test)]\nmod tests;" not in text:
        idx = text.index("#[cfg(test)]")
        inner = text[idx:].replace("#[cfg(test)]\nmod tests {\n", "", 1)
        if inner.endswith("}\n"):
            inner = inner[:-2]
        tl = inner.splitlines(keepends=True)
        mid = len(tl) // 2
        for i in range(mid, len(tl)):
            if tl[i].lstrip().startswith("#[test]"):
                mid = i
                break
        mod_path.write_text(text[:idx].rstrip() + "\n\n#[cfg(test)]\nmod tests;\n", encoding="utf-8")
        outt = ROOT / "crates/datasets/src/metric_cache_key/tests"
        write(outt / "a.rs", "    use super::*;\n\n" + "".join(tl[:mid]))
        write(outt / "b.rs", "    use super::*;\n\n" + "".join(tl[mid:]))
        write(outt / "mod.rs", "mod a;\nmod b;\n")


def fix_execute() -> None:
    print("execute")
    prelude = """use std::time::Instant;
use crate::http::pages::metric_api::assembly::{hash_metric_response_cache_key, metric_eval_diagnostic_code, write_dag_perf, MetricQueryGroupRequest, MetricQueryGroupResponse};
use crate::http::pages::scene_qualified::locate_dataset_resource;
use crate::http::pages::util::elapsed_ms;
use crate::http::observation::EvalObservation;
use crate::AppError;
use mei_lang_datasets::{collect_all_query_options, default_result_artifact_scope, evaluate_runtime_metrics_from_plan, load_metric_response_result_artifact, load_prebuild_metric_response_artifact_dataset_fallback, metric_response_artifact_lookup_cache_keys, metric_response_cache_scope_key, plan_access_metric_eval_for_ids, populate_l1_from_loaded_metric_artifact, project_requested_metrics, run_metric_response_artifact_load_singleflight, store_cached_metric_response_aliases, store_metric_response_result_artifact, take_cached_metric_response, take_metric_response_index_stats, runtime_metric_workset, RuntimeMetricEvalMode};
use super::helpers::{requested_metric_ids_label, write_runtime_policy_perf};
use super::types::*;

"""
    body = sl(git_lines("server/src/http/pages/metric_api/handlers.rs"), 607, 1086)
    body = body.replace("fn execute_metric_query_group", "pub(super) fn execute_metric_query_group", 1)
    write(ROOT / "server/src/http/pages/metric_api/handlers/execute.rs", prelude + body)


def main() -> None:
    split_datasets()
    fix_execute()
    print("done")


if __name__ == "__main__":
    main()
