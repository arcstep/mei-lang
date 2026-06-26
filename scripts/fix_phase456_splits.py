#!/usr/bin/env python3
"""Apply Phase 4-6 splits with function-safe boundaries."""
from __future__ import annotations

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read_lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8").splitlines(keepends=True)


def sl(lines: list[str], start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    n = content.count("\n")
    print(f"  {'WARN' if n > 501 else 'OK'} {path.relative_to(ROOT)}: {n}")


def git_lines(rel: str) -> list[str]:
    text = subprocess.check_output(["git", "show", f"HEAD:{rel}"], text=True)
    return text.splitlines(keepends=True)


def promote_pub_super(content: str) -> str:
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
                out.append(f"{indent}pub(super) {stripped}")
                continue
            out.append(line)
            continue
        if line and not line[0].isspace():
            if stripped.startswith("struct "):
                in_struct = True
                field_indent = None
                out.append(f"pub(super) {stripped}")
                continue
            if not stripped.startswith("pub") and (
                stripped.startswith(("fn ", "enum ", "const ", "type ", "async fn "))
            ):
                out.append(f"pub(super) {stripped}")
                continue
        if stripped.startswith("fn ") and indent == "    " and not stripped.startswith("pub"):
            out.append(f"{indent}pub(super) {stripped}")
            continue
        out.append(line)
    return "".join(out)


def split_file_module(src_rel: str, out_name: str, header_end: int, sections: list[tuple[str, int, int]], exports: str) -> None:
    src = ROOT / src_rel
    if not src.exists():
        lines = git_lines(src_rel)
    else:
        lines = read_lines(src)
    out = ROOT / str(Path(src_rel).parent / out_name)
    header = sl(lines, 1, header_end)
    for name, start, end in sections:
        extra = "" if name == sections[0][0] else header.replace("use super::", "use crate::") + "\n"
        if out_name in src_rel:
            extra = header if name == sections[0][0] else header + f"use super::{sections[0][0]}::*;\n\n"
        write(out / f"{name}.rs", header + extra + promote_pub_super(sl(lines, start, end)))
    write(out / "mod.rs", exports)
    if src.exists() and src.is_file():
        src.unlink()


def split_datasets() -> None:
    print("=== datasets ===")
    split_file_module(
        "crates/datasets/src/result_artifact.rs",
        "result_artifact",
        38,
        [("core", 39, 258), ("index_a", 259, 490), ("index_b", 491, 732), ("store", 733, 959)],
        """mod core; mod index_a; mod index_b; mod store;
pub use core::{
    default_result_artifact_scope, load_metric_response_result_artifact,
    metric_dataframe_result_artifact_exists, metric_response_result_artifact_exists,
    store_metric_dataframe_result_artifact, store_metric_response_result_artifact,
    take_metric_response_index_stats,
};
pub use index_a::{invalidate_prebuild_metric_response_index, prebuild_metric_response_index_covers_key};
pub use index_b::{
    load_prebuild_metric_response_artifact_dataset_fallback, preload_prebuild_metric_response_index,
    rebuild_and_install_prebuild_metric_response_index,
};
pub use store::{load_metric_dataframe_result_artifact};
""",
    )
    split_file_module(
        "crates/datasets/src/metric_dataframe.rs",
        "metric_dataframe",
        87,
        [("cache", 88, 317), ("query", 318, 797), ("materialize", 798, 914)],
        """mod cache; mod materialize; mod query;
pub use cache::metric_dataframe_result_cache_key;
pub use query::query_metric_dataframe;
""",
    )
    split_file_module(
        "crates/datasets/src/paginate.rs",
        "paginate",
        10,
        [("core", 11, 165), ("sort", 166, 287), ("filter", 288, 504), ("helpers", 505, 660)],
        """mod core; mod filter; mod helpers; mod sort;
pub(crate) use core::{paginate_rows, paginate_rows_iter};
pub(crate) use helpers::{apply_normalize, infer_columns, output_columns, row_matches, QueryWindow};
pub(crate) use sort::normalize_search;
""",
    )
    src = ROOT / "crates/datasets/src/metric_cache_key/cache_key.rs"
    if src.exists() and len(read_lines(src)) > 501:
        lines = read_lines(src)
        header = sl(lines, 1, 57)
        out = ROOT / "crates/datasets/src/metric_cache_key/cache_key"
        write(out / "identity.rs", header + promote_pub_super(sl(lines, 58, 260)))
        write(out / "scope.rs", header + "use super::identity::*;\n\n" + promote_pub_super(sl(lines, 261, 457)))
        write(out / "lookup.rs", header + "use super::identity::*;\nuse super::scope::*;\n\n" + promote_pub_super(sl(lines, 458, len(lines))))
        write(
            out / "mod.rs",
            """mod identity; mod lookup; mod scope;
pub(crate) use identity::{
    dataset_metric_identity_key, dataset_resource_lookup_aliases, effective_compile_revision_for_slot,
    equivalent_dataset_resource_ids, eval_node_cache_key, metric_request_revision_fingerprint,
    metric_request_revision_fingerprint_for_compiled, metric_scope_cache_key, serialize_cache_value,
    stable_slot_hash,
};
pub(crate) use lookup::{
    equivalent_dataframe_metric_scope_tokens, lookup_compiled_dataset_view,
    metric_dataframe_artifact_lookup_cache_keys, metric_response_artifact_lookup_cache_keys,
};
pub(crate) use scope::runtime_metric_eval_scope;
""",
        )
        src.unlink()
        mod_path = ROOT / "crates/datasets/src/metric_cache_key/mod.rs"
        text = mod_path.read_text(encoding="utf-8")
        mod_path.write_text(text.replace("mod cache_key;", "mod cache_key;\n").replace("mod cache_key;\n", "mod cache_key;\n", 1))
        if "cache_key/" not in text:
            mod_path.write_text(mod_path.read_text().replace("mod cache_key;", "mod cache_key;"), encoding="utf-8")

    mod_path = ROOT / "crates/datasets/src/metric_cache_key/mod.rs"
    text = mod_path.read_text(encoding="utf-8")
    if "#[cfg(test)]" in text and "mod tests;" not in text.split("#[cfg(test)]")[0]:
        idx = text.index("#[cfg(test)]")
        mod_path.write_text(text[:idx].rstrip() + "\n\n#[cfg(test)]\nmod tests;\n", encoding="utf-8")
        body_lines = text[idx:].splitlines(keepends=True)
        # strip outer #[cfg(test)] mod tests { ... }
        inner = "".join(body_lines)
        inner = re.sub(r"#\[cfg\(test\)\]\s*\nmod tests \{\n", "", inner, count=1)
        if inner.endswith("}\n"):
            inner = inner[:-2]
        chunks: list[str] = []
        cur: list[str] = []
        for line in inner.splitlines(keepends=True):
            if line.lstrip().startswith("#[test]") and cur:
                chunks.append("".join(cur))
                cur = []
            cur.append(line)
        if cur:
            chunks.append("".join(cur))
        out = ROOT / "crates/datasets/src/metric_cache_key/tests"
        mid = len(chunks) // 2
        write(out / "a.rs", sl(body_lines, 1, 1).replace("mod tests {", "").strip() + "\n" + chunks[0] if False else "    use super::*;\n\n" + chunks[0])
        # simpler: split lines in half at test boundary
        all_inner = inner
        test_lines = all_inner.splitlines(keepends=True)
        mid_i = len(test_lines) // 2
        for i in range(mid_i, len(test_lines)):
            if test_lines[i].lstrip().startswith("#[test]"):
                mid_i = i
                break
        write(out / "a.rs", "    use super::*;\n\n" + "".join(test_lines[:mid_i]))
        write(out / "b.rs", "    use super::*;\n\n" + "".join(test_lines[mid_i:]))
        write(out / "mod.rs", "mod a;\nmod b;\n")


def split_pages_tests() -> None:
    src = ROOT / "server/src/http/pages/tests.rs"
    if not src.exists() or len(read_lines(src)) <= 501:
        return
    print("=== pages/tests ===")
    lines = read_lines(src)
    header_end = next(i for i, l in enumerate(lines) if l.startswith("#[tokio::test]"))
    header = "".join(lines[:header_end])
    body = "".join(lines[header_end:])
    chunks: list[str] = []
    cur: list[str] = []
    for line in body.splitlines(keepends=True):
        if (line.lstrip().startswith("#[tokio::test]") or line.lstrip().startswith("#[test]")) and cur:
            chunks.append("".join(cur))
            cur = []
        cur.append(line)
    if cur:
        chunks.append("".join(cur))
    out = ROOT / "server/src/http/pages/tests"
    per = 4
    mods = []
    for i in range(0, len(chunks), per):
        name = f"c{i // per + 1}"
        write(out / f"{name}.rs", header + "".join(chunks[i : i + per]))
        mods.append(f"mod {name};")
    write(out / "mod.rs", header + "\n#[cfg(test)]\nmod cases;\n")
    write(out / "cases.rs", "\n".join(mods) + "\n")
    src.unlink()


def split_preview_oversized() -> None:
    out = ROOT / "app/src/ui/preview/tests"
    for name in ("g11", "g12"):
        path = out / f"{name}.rs"
        if not path.exists() or len(read_lines(path)) <= 501:
            continue
        print(f"=== split {name} ===")
        lines = read_lines(path)
        header_end = next(i for i, l in enumerate(lines) if l.lstrip().startswith("#[test]"))
        header = "".join(lines[:header_end])
        body = "".join(lines[header_end:])
        chunks: list[str] = []
        cur: list[str] = []
        for line in body.splitlines(keepends=True):
            if line.lstrip().startswith("#[test]") and cur:
                chunks.append("".join(cur))
                cur = []
            cur.append(line)
        if cur:
            chunks.append("".join(cur))
        mid = len(chunks) // 2
        write(out / f"{name}a.rs", header + chunks[0])
        write(out / f"{name}b.rs", header + "".join(chunks[mid:]))
        path.unlink()
        mod = out / "mod.rs"
        t = mod.read_text()
        t = t.replace(f"mod {name};", f"mod {name}a;\nmod {name}b;")
        mod.write_text(t)


def split_spbjw_regression() -> None:
    src = ROOT / "crates/ws-spbjw-integration-tests/tests/spbjw_regression.rs"
    if not src.exists() or len(read_lines(src)) <= 501:
        return
    print("=== spbjw_regression ===")
    lines = read_lines(src)
    header_end = next(i for i, l in enumerate(lines) if l.lstrip().startswith("#[test]"))
    header = "".join(lines[:header_end])
    body = "".join(lines[header_end:])
    chunks: list[str] = []
    cur: list[str] = []
    for line in body.splitlines(keepends=True):
        if line.lstrip().startswith("#[test]") and cur:
            chunks.append("".join(cur))
            cur = []
        cur.append(line)
    if cur:
        chunks.append("".join(cur))
    out = ROOT / "crates/ws-spbjw-integration-tests/tests/spbjw_regression"
    per = 3
    mods = []
    for i in range(0, len(chunks), per):
        name = f"t{i // per + 1}"
        write(out / f"{name}.rs", header + "".join(chunks[i : i + per]))
        mods.append(f"mod {name};")
    write(out / "mod.rs", header + "\n" + "\n".join(mods) + "\n")
    src.unlink()


def trim_execute_imports() -> None:
    path = ROOT / "server/src/http/pages/metric_api/handlers/execute.rs"
    if not path.exists():
        return
    write(
        path,
        """use std::time::Instant;

use crate::http::pages::metric_api::assembly::{
    hash_metric_response_cache_key, metric_eval_diagnostic_code, write_dag_perf,
    MetricQueryGroupRequest, MetricQueryGroupResponse,
};
use crate::http::observation::EvalObservation;
use crate::AppError;
use mei_lang_datasets::{
    collect_all_query_options, default_result_artifact_scope, evaluate_runtime_metrics_from_plan,
    load_metric_response_result_artifact, load_prebuild_metric_response_artifact_dataset_fallback,
    metric_response_artifact_lookup_cache_keys, metric_response_cache_scope_key,
    plan_access_metric_eval_for_ids, populate_l1_from_loaded_metric_artifact,
    project_requested_metrics, run_metric_response_artifact_load_singleflight,
    store_cached_metric_response_aliases, store_metric_response_result_artifact,
    take_cached_metric_response, take_metric_response_index_stats, runtime_metric_workset,
    RuntimeMetricEvalMode,
};
use super::helpers::write_runtime_policy_perf;
use super::types::*;

"""
        + "\n".join(read_lines(path)[40:]),
    )


def main() -> None:
    split_datasets()
    split_pages_tests()
    split_preview_oversized()
    split_spbjw_regression()
    trim_execute_imports()
    print("done")


if __name__ == "__main__":
    main()
