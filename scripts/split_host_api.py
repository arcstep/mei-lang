#!/usr/bin/env python3
"""Split server/src/http/host_api.rs into host_api/ (flat modules, ≤500 lines)."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "server" / "src" / "http" / "host_api.rs"
OUT = ROOT / "server" / "src" / "http" / "host_api"

PRELUDE_RAW = """use std::collections::{BTreeMap, BTreeSet};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use anyhow::{anyhow, Result};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use crate::{
    http::compile_cache::{
        compile_app_with_cache, compile_outcome_from_shared, resolve_runtime_compile_shared,
        CompileWithCacheOutcome, RuntimeAccessPolicies,
    },
    http::startup_run,
    prebuild::{
        app_has_deferred_warmup_work, run_prebuild, PrebuildAppReport, PrebuildDiagnosticsReport,
        PrebuildMode, PrebuildOptions, PrebuildReport, PrebuildScopeProfile, PrebuildWarningReport,
        materialize_scope_after_compile, ScopedMaterializeReport,
    },
    AppState,
};
use mei_lang_datasets::preload_prebuild_metric_response_index;
use mei_lang_kernel::{
    resolve_app_root, resolve_runtime_warmup_manifest, CompileOptions, CompiledApp, Severity,
};
use mei_lang_toolchain::resolve_components_root;
"""


def prelude_text() -> str:
    out = ["//! Shared imports for host_api submodules."]
    for line in PRELUDE_RAW.strip().splitlines():
        out.append(line.replace("use ", "pub(crate) use ", 1) if line.startswith("use ") else line)
    return "\n".join(out) + "\n"

PREFIX = "use super::prelude::*;\nuse super::*;\n\n"

SECTIONS = [
    ("util", 30, 46),
    ("types", 48, 346),
    ("readiness_registry", 348, 547),
    ("readiness_snapshot", 548, 773),
    ("readiness_sync", 774, 1104),
    ("prebuild_lifecycle", 1105, 1254),
    ("prebuild_startup", 1255, 1540),
    ("scoped_build", 1541, 1820),
    ("handlers", 1821, 2135),
]

MOD_RS = """mod prelude;
mod util;
mod types;
mod readiness_registry;
mod readiness_snapshot;
mod readiness_sync;
mod prebuild_lifecycle;
mod prebuild_startup;
mod scoped_build;
mod handlers;

pub(crate) use util::*;
pub(crate) use types::*;
pub(crate) use readiness_registry::*;
pub(crate) use readiness_snapshot::*;
pub(crate) use readiness_sync::*;
pub(crate) use prebuild_lifecycle::*;
pub(crate) use prebuild_startup::*;
pub(crate) use scoped_build::*;
pub(crate) use handlers::*;

pub(crate) use handlers::{
    api_host_build, api_host_diagnostics, api_host_heartbeat, api_host_readiness, api_host_ready,
};
pub(crate) use types::{
    ArtifactGateStatus, HostAppReadinessResponse, HostBuildRequest, HostReadyResponse,
    HostScopeReadinessResponse, ScopedFeedbackStatus,
};
"""


def promote(content: str) -> str:
    out: list[str] = []
    in_struct = False
    field_indent: str | None = None
    for line in content.splitlines(keepends=True):
        stripped = line.lstrip()
        if stripped.startswith("#"):
            out.append(line)
            continue
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
            ):
                out.append(f"{indent}pub(crate) {stripped}")
                continue
            out.append(line)
            continue
        if line and not line[0].isspace():
            if "struct " in stripped or stripped.startswith("enum "):
                in_struct = "{" in stripped and "}" not in stripped.rstrip()
                field_indent = None
                if stripped.startswith("struct ") or stripped.startswith("enum "):
                    out.append(f"pub(crate) {stripped}")
                else:
                    out.append(line)
                continue
            if not stripped.startswith("pub") and stripped.startswith(
                ("fn ", "async fn ", "const ", "type ")
            ):
                out.append(f"pub(crate) {stripped}")
                continue
        if stripped.startswith(("fn ", "async fn ")) and indent == "    " and not stripped.startswith("pub"):
            out.append(f"{indent}pub(crate) {stripped}")
            continue
        out.append(line)
    return "".join(out)


def main() -> None:
    if not SRC.is_file():
        raise SystemExit(f"missing {SRC}")
    lines = SRC.read_text(encoding="utf-8").splitlines(keepends=True)
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "prelude.rs").write_text(prelude_text(), encoding="utf-8")
    (OUT / "mod.rs").write_text(MOD_RS, encoding="utf-8")
    for name, start, end in SECTIONS:
        body = promote("".join(lines[start - 1 : end]))
        path = OUT / f"{name}.rs"
        path.write_text(PREFIX + body, encoding="utf-8")
        print(f"  {name}.rs: {path.read_text(encoding='utf-8').count(chr(10))}")
    SRC.unlink()


if __name__ == "__main__":
    main()
