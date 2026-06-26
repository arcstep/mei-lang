#!/usr/bin/env python3
"""Split remaining oversized server modules (support-module pattern)."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

DATASET_IMPORTS = """use std::time::Instant;

use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::{resolve_app_root, FilterIntent, QueryState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::http::observation::CompileObservation;
use crate::{AppError, AppState};

use crate::http::compile_cache::{RuntimeAccessPolicies, RuntimeArtifactPolicy, access_import_required};
use crate::http::datasets::{
    map_dataset_query_filters, query_dataset_rows, query_metric_dataframe,
    query_state_from_request, serde_lenient,
    table_contract::{
        apply_table_request_fields, enrich_table_result, TableColumnState, TableSortSpec,
    },
    DatasetQueryOptions,
};
use crate::http::runtime_cache::{
    invalidate_after_data_reload, invalidate_app_runtime_caches, invalidate_report_perf,
};
use crate::http::pages::components::resolve_components_root;
use crate::http::pages::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context,
    strict_dataset_query_mode_contract, strict_runtime_query_contract, strict_scene_query_coords,
};
use crate::http::pages::util::elapsed_ms;
"""

CONTEXT_IMPORTS = """use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use mei_lang_kernel::{
    build_experience_path, build_overview_backing, build_reachability_tree, experience_layout_hint,
    experience_mount_chain, format_experience_path, resolve_build_node_context,
    resolve_build_view_query, tab_visible_for_node, BuildViewTab, LegacyBuildQuery,
    ProvenanceAnchor,
};
use mei_lang_toolchain::{format_semantic_graph_markdown, load_world_runtime_bundle};

use serde::Deserialize;

use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::http::host_api::artifact_gate_status;
use crate::AppState;

use super::super::graph_markdown;
"""


def read(p: Path) -> list[str]:
    return p.read_text(encoding="utf-8").splitlines(keepends=True)


def write(p: Path, text: str) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    n = text.count("\n") + (0 if text.endswith("\n") else 1)
    print(f"  {p.relative_to(ROOT)}: {n}")


def sl(lines, a, b):
    return "".join(lines[a - 1 : b])


def split_dataset_api() -> None:
    src = ROOT / "server/src/http/pages/dataset_api.rs"
    if not src.is_file():
        return
    lines = read(src)
    out = ROOT / "server/src/http/pages/dataset_api"
    imp = DATASET_IMPORTS
    write(out / "types.rs", imp + sl(lines, 35, 124))
    write(out / "support.rs", imp + "use super::types::*;\n\n" + sl(lines, 125, 187))
    write(
        out / "query.rs",
        imp + "use super::support::*;\nuse super::types::*;\n\n" + sl(lines, 188, 463),
    )
    # recompute + tests split
    end_body = len(lines)
    test_start = next(
        (i + 1 for i, l in enumerate(lines) if l.strip() == "#[cfg(test)]" and i > 400),
        end_body + 1,
    )
    write(
        out / "recompute.rs",
        imp + "use super::support::*;\nuse super::types::*;\n\n" + sl(lines, 464, test_start - 1),
    )
    if test_start <= len(lines):
        write(out / "tests.rs", imp + sl(lines, test_start, len(lines)))
        mod_rs = (
            "mod types;\nmod support;\nmod query;\nmod recompute;\n#[cfg(test)]\nmod tests;\n\n"
            "pub use types::*;\npub use query::*;\npub use recompute::*;\n"
        )
    else:
        mod_rs = (
            "mod types;\nmod support;\nmod query;\nmod recompute;\n\n"
            "pub use types::*;\npub use query::*;\npub use recompute::*;\n"
        )
    write(out / "mod.rs", mod_rs)
    src.unlink()


def split_context_export() -> None:
    src = ROOT / "server/src/http/build_api/context_export.rs"
    if not src.is_file():
        return
    lines = read(src)
    out = ROOT / "server/src/http/build_api/context_export"
    imp = CONTEXT_IMPORTS
    test_start = next(
        (i + 1 for i, l in enumerate(lines) if l.strip().startswith("mod tests")),
        len(lines) + 1,
    )
    write(out / "support.rs", imp + sl(lines, 494, test_start - 1))
    write(
        out / "api.rs",
        imp + "use super::support::*;\nuse super::append::*;\n\n" + sl(lines, 24, 188),
    )
    write(
        out / "append.rs",
        imp + "use super::support::*;\n\n" + sl(lines, 189, 493),
    )
    if test_start <= len(lines):
        write(out / "tests.rs", imp + sl(lines, test_start, len(lines)))
        mod_rs = (
            "mod support;\nmod append;\nmod api;\n#[cfg(test)]\nmod tests;\n\n"
            "pub use api::api_build_context_export;\n"
        )
    else:
        mod_rs = "mod support;\nmod append;\nmod api;\n\npub use api::api_build_context_export;\n"
    write(out / "mod.rs", mod_rs)
    src.unlink()


def main() -> None:
    split_dataset_api()
    split_context_export()
    print("done")


if __name__ == "__main__":
    main()
