#!/usr/bin/env python3
"""Split infer.rs by extracting match-arm bodies into grouped modules."""
from __future__ import annotations

import re
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = Path("/tmp/infer_orig.rs")
OUT = ROOT / "src/compile/analysis/rowset/infer"

HEADER = textwrap.dedent(
    """
    use std::collections::BTreeMap;

    use anyhow::{anyhow, Result};
    use serde_json::Value;

    use crate::model::DatasetView;

    use crate::compile::analysis::{
        dates::{filter_rows_in_latest_days, filter_rows_in_latest_months},
        eval_context::EvalContext,
        predicate::predicate_matches_with_ctx,
        transforms::{
            aggregate_group_rows, aggregate_group_rows_pivot, bucket_rows_by_month,
            distinct_rows_by_fields, first_rows_by_field, mutate_row, party_year_aggregate_rows,
            pivot_long_rows, rename_fields, reorder_fields, select_fields, sort_rows_by_field,
            summarize_rows, trend_rows_by_month, trend_year_compare_rows, unpivot_columns_rows,
        },
    };
    use super::super::build::{
        apply_universe, eval_lookup_value_rowset, eval_rowset_with_ctx, eval_split_text_rowset,
        eval_universe_labels, lookup_dataset_view, unknown_dataset_error,
    };
    """
).lstrip("\n")

# (group, analysis_type, start_line, end_line) — inclusive, from infer.rs match arms
ARMS: list[tuple[str, str, int, int]] = [
    ("basic", "rows", 32, 40),
    ("basic", "where", 41, 64),
    ("basic", "select", 65, 81),
    ("basic", "rename", 82, 94),
    ("basic", "mutate", 95, 107),
    ("basic", "sort_by", 108, 120),
    ("basic", "reorder", 121, 137),
    ("basic", "stage", 138, 143),
    ("basic", "first_by", 144, 154),
    ("basic", "distinct_by", 155, 169),
    ("aggregate", "group_by", 170, 256),
    ("aggregate", "agg", 257, 280),
    ("trend", "trend", 281, 319),
    ("trend", "trend_year_compare", 320, 374),
    ("aggregate", "party_year_aggregate", 375, 414),
    ("pivot", "unpivot_columns", 415, 456),
    ("pivot", "pivot_long", 457, 507),
    ("basic", "table_rows", 508, 513),
    ("basic", "latest_window", 516, 531),
    ("basic", "bucket_date", 532, 547),
    ("basic", "limit", 548, 556),
    ("basic", "concat_rowsets", 557, 567),
]

INLINE_ARMS = {
    "split_text": "eval_split_text_rowset(map, datasets, ctx)",
    "lookup_value": "eval_lookup_value_rowset(map, datasets, ctx)",
}


def arm_fn_name(analysis_type: str) -> str:
    return "eval_rowset_" + re.sub(r"[^a-z0-9_]+", "_", analysis_type)


def dedent_body(raw: str) -> str:
    lines = raw.splitlines()
    while lines and not lines[0].strip():
        lines.pop(0)
    while lines and lines[-1].strip() in ("", "}"):
        if lines[-1].strip() == "}":
            lines.pop()
        elif not lines[-1].strip():
            lines.pop()
        else:
            break
    if lines and lines[0].strip() == "{":
        lines = lines[1:]
    if lines and lines[-1].strip() == "}":
        lines = lines[:-1]
    non_empty = [ln for ln in lines if ln.strip()]
    if not non_empty:
        return ""
    indent = min(len(ln) - len(ln.lstrip()) for ln in non_empty)
    return "\n".join(ln[indent:] if len(ln) >= indent else ln for ln in lines)


def extract_body(all_lines: list[str], start: int, end: int) -> str:
    chunk = "".join(all_lines[start - 1 : end])
    # drop `"type" => {` header line if present
    chunk_lines = chunk.splitlines(keepends=True)
    if chunk_lines and "=>" in chunk_lines[0]:
        chunk = "".join(chunk_lines[1:])
    return dedent_body(chunk)


def make_fn(name: str, body: str, inline: bool = False) -> str:
    sig = (
        f"pub(super) fn {name}(\n"
        f"    map: &serde_json::Map<String, Value>,\n"
        f"    datasets: &BTreeMap<String, DatasetView>,\n"
        f"    ctx: &mut EvalContext,\n"
        f") -> Result<Vec<Value>> "
    )
    if inline:
        return f"{sig}{{\n    {body}\n}}\n\n"
    return f"{sig}{{\n{body}\n}}\n\n"


def main() -> None:
    lines = SRC.read_text(encoding="utf-8").splitlines(keepends=True)
    OUT.mkdir(parents=True, exist_ok=True)
    groups: dict[str, list[str]] = {g: [] for g in ("basic", "aggregate", "trend", "pivot")}

    for group, analysis_type, start, end in ARMS:
        body = extract_body(lines, start, end)
        if analysis_type == "latest_window":
            fn = make_fn(
                "eval_rowset_latest_window",
                'let analysis_type = map\n'
                '    .get("type")\n'
                '    .and_then(Value::as_str)\n'
                '    .unwrap_or("latest_days");\n'
                + body,
            )
            groups[group].append(fn)
            continue
        fn = make_fn(arm_fn_name(analysis_type), body)
        groups[group].append(fn)

    for analysis_type, expr in INLINE_ARMS.items():
        groups["basic"].append(make_fn(arm_fn_name(analysis_type), expr, inline=True))

    for group, chunks in groups.items():
        path = OUT / f"{group}.rs"
        path.write_text(HEADER + "\n" + "".join(chunks), encoding="utf-8")
        n = path.read_text(encoding="utf-8").count("\n")
        print(f"  {'WARN' if n > 501 else 'OK'} {path.relative_to(ROOT.parents[1])}: {n}")

    dispatch = [
        ('"rows"', "eval_rowset_rows"),
        ('"where"', "eval_rowset_where"),
        ('"select"', "eval_rowset_select"),
        ('"rename"', "eval_rowset_rename"),
        ('"mutate"', "eval_rowset_mutate"),
        ('"sort_by"', "eval_rowset_sort_by"),
        ('"reorder"', "eval_rowset_reorder"),
        ('"stage"', "eval_rowset_stage"),
        ('"first_by"', "eval_rowset_first_by"),
        ('"distinct_by"', "eval_rowset_distinct_by"),
        ('"table_rows"', "eval_rowset_table_rows"),
        ('"split_text"', "eval_rowset_split_text"),
        ('"lookup_value"', "eval_rowset_lookup_value"),
        ('"latest_days" | "latest_months"', "eval_rowset_latest_window"),
        ('"bucket_date"', "eval_rowset_bucket_date"),
        ('"limit"', "eval_rowset_limit"),
        ('"concat_rowsets"', "eval_rowset_concat_rowsets"),
        ('"group_by"', "eval_rowset_group_by"),
        ('"agg"', "eval_rowset_agg"),
        ('"party_year_aggregate"', "eval_rowset_party_year_aggregate"),
        ('"trend"', "eval_rowset_trend"),
        ('"trend_year_compare"', "eval_rowset_trend_year_compare"),
        ('"unpivot_columns"', "eval_rowset_unpivot_columns"),
        ('"pivot_long"', "eval_rowset_pivot_long"),
    ]
    mod = textwrap.dedent(
        """
        mod basic;
        mod aggregate;
        mod trend;
        mod pivot;

        use std::collections::BTreeMap;

        use anyhow::{anyhow, Result};
        use serde_json::Value;

        use crate::model::DatasetView;
        use crate::compile::analysis::eval_context::EvalContext;

        use basic::*;
        use aggregate::*;
        use trend::*;
        use pivot::*;

        pub(super) fn eval_analysis_rowset(
            map: &serde_json::Map<String, Value>,
            datasets: &BTreeMap<String, DatasetView>,
            ctx: &mut EvalContext,
        ) -> Result<Vec<Value>> {
            let analysis_type = map
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("analysis expression missing type"))?;
            match analysis_type {
        """
    ).lstrip("\n")
    for pat, fn in dispatch:
        mod += f"        {pat} => {fn}(map, datasets, ctx),\n"
    mod += '        other => Err(anyhow!("unsupported rowset analysis `{other}`")),\n    }\n}\n'
    (OUT / "mod.rs").write_text(mod, encoding="utf-8")
    print("infer split done")


if __name__ == "__main__":
    main()
