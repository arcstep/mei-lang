#!/usr/bin/env python3
"""Split datasets crates using include! for shared private scope."""
from __future__ import annotations

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


def split_mck() -> None:
    mod_path = ROOT / "crates/datasets/src/metric_cache_key/mod.rs"
    text = mod_path.read_text(encoding="utf-8")
    if "#[cfg(test)]\nmod tests;" in text:
        return
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
    out = ROOT / "crates/datasets/src/metric_cache_key/tests"
    write(out / "a.rs", "    use super::*;\n\n" + "".join(tl[:mid]))
    write(out / "b.rs", "    use super::*;\n\n" + "".join(tl[mid:]))
    write(out / "mod.rs", "mod a;\nmod b;\n")


def fix_execute() -> None:
    lines = git_lines("server/src/http/pages/metric_api/handlers.rs")
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
    body = sl(lines, 607, len(lines)).replace(
        "fn execute_metric_query_group", "pub(super) fn execute_metric_query_group", 1
    )
    write(ROOT / "server/src/http/pages/metric_api/handlers/execute.rs", prelude + body)


def main() -> None:
    print("split datasets (include!)")

    ra_lines = git_lines("crates/datasets/src/result_artifact.rs")
    ra_out = ROOT / "crates/datasets/src/result_artifact"
    write(ra_out / "core.rs", sl(ra_lines, 39, 258))
    write(ra_out / "index_a.rs", sl(ra_lines, 259, 507))
    write(ra_out / "index_b.rs", sl(ra_lines, 508, 732))
    write(ra_out / "store.rs", sl(ra_lines, 733, len(ra_lines)))
    write(
        ra_out / "mod.rs",
        sl(ra_lines, 1, 38)
        + 'include!("core.rs");\ninclude!("index_a.rs");\ninclude!("index_b.rs");\ninclude!("store.rs");\n',
    )
    (ROOT / "crates/datasets/src/result_artifact.rs").unlink(missing_ok=True)

    md_lines = git_lines("crates/datasets/src/metric_dataframe.rs")
    md_out = ROOT / "crates/datasets/src/metric_dataframe"
    write(md_out / "cache.rs", sl(md_lines, 88, 317))
    write(md_out / "query.rs", sl(md_lines, 318, 797))
    write(md_out / "materialize.rs", sl(md_lines, 798, len(md_lines)))
    write(
        md_out / "mod.rs",
        sl(md_lines, 1, 87)
        + 'include!("cache.rs");\ninclude!("query.rs");\ninclude!("materialize.rs");\n',
    )
    (ROOT / "crates/datasets/src/metric_dataframe.rs").unlink(missing_ok=True)

    pg_lines = git_lines("crates/datasets/src/paginate.rs")
    pg_out = ROOT / "crates/datasets/src/paginate"
    write(pg_out / "core.rs", sl(pg_lines, 11, 165))
    write(pg_out / "sort.rs", sl(pg_lines, 166, 287))
    write(pg_out / "filter.rs", sl(pg_lines, 288, 504))
    write(pg_out / "helpers.rs", sl(pg_lines, 505, len(pg_lines)))
    write(
        pg_out / "mod.rs",
        sl(pg_lines, 1, 10)
        + 'include!("core.rs");\ninclude!("sort.rs");\ninclude!("filter.rs");\ninclude!("helpers.rs");\n',
    )
    (ROOT / "crates/datasets/src/paginate.rs").unlink(missing_ok=True)

    ck_lines = git_lines("crates/datasets/src/metric_cache_key/cache_key.rs")
    ck_out = ROOT / "crates/datasets/src/metric_cache_key/cache_key"
    write(ck_out / "identity.rs", sl(ck_lines, 58, 260))
    write(ck_out / "scope.rs", sl(ck_lines, 261, 457))
    write(ck_out / "lookup.rs", sl(ck_lines, 458, len(ck_lines)))
    write(
        ck_out / "mod.rs",
        sl(ck_lines, 1, 57)
        + 'include!("identity.rs");\ninclude!("scope.rs");\ninclude!("lookup.rs");\n',
    )
    (ROOT / "crates/datasets/src/metric_cache_key/cache_key.rs").unlink(missing_ok=True)

    split_mck()
    fix_execute()
    print("done")


if __name__ == "__main__":
    main()
