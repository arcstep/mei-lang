#!/usr/bin/env python3
"""Split server/src/runtime_entry.rs into runtime_entry/ directory."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "server" / "src" / "runtime_entry.rs"
OUT = ROOT / "server" / "src" / "runtime_entry"

PRELUDE = """//! Shared imports for runtime_entry submodules.
pub(crate) use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
pub(crate) use anyhow::{Context, Result};
pub(crate) use axum::{
    body::Body,
    http::StatusCode,
    http::{HeaderName, HeaderValue, Method, Request, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
pub(crate) use clap::Parser;
pub(crate) use http_body_util::BodyExt;
pub(crate) use mei_lang_kernel::{set_mei_package_root, HostSurface};
pub(crate) use tracing::Instrument;
pub(crate) use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
pub(crate) use crate::cli::args::{AgentRuntimeArgs, Cli, Command, HostCommand, ServeArgs};
pub(crate) use crate::cli::util::{
    print_cli_version_if_requested, resolve_cargo_package_root, resolve_cli_source_root,
    resolve_package_root, resolve_source_root_arg,
};
pub(crate) use crate::cli::{
    agent_command, compile_or_check_command, diagnostics_command, editor_runtime_command,
    export_command, host_command, inspect_command, knowledge_command, mcp_command,
    prebuild_command, query_command, readiness_command, runtime_command, warmup_command,
    workspace_command,
};
"""

MOD_RS = """mod prelude;
mod types;
mod cli_dispatch;
mod startup;
mod request_logging;

pub use cli_dispatch::run_cli_for_flavor;
pub use types::BinaryFlavor;
pub(crate) use types::{AppState, SessionContextSnapshot};
pub(crate) use cli_dispatch::ensure_command_allowed;
pub(crate) use request_logging::{AppError, log_request};
pub(crate) use startup::serve;
pub(crate) use request_logging::test_support;
"""

USE = "use super::prelude::*;\n\n"


def sl(lines: list[str], start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


def promote_fields(content: str) -> str:
    """Promote struct fields to pub(crate); leave impl/trait methods unchanged."""
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
            if stripped.startswith("struct ") and "{" in stripped and "}" not in stripped.rstrip():
                in_struct = True
                field_indent = None
                if not stripped.startswith("pub"):
                    out.append(f"pub(crate) {stripped}")
                else:
                    out.append(line)
                continue
            if not stripped.startswith("pub") and stripped.startswith(
                ("fn ", "async fn ", "static ")
            ):
                out.append(f"pub(crate) {stripped}")
                continue
        out.append(line)
    return "".join(out)


def main() -> None:
    if not SRC.is_file():
        raise SystemExit(f"missing {SRC}")
    lines = SRC.read_text(encoding="utf-8").splitlines(keepends=True)
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "prelude.rs").write_text(PRELUDE, encoding="utf-8")
    (OUT / "types.rs").write_text(
        USE + promote_fields(sl(lines, 42, 76)), encoding="utf-8"
    )
    cli = sl(lines, 78, 228).replace(
        "Command::Serve(args) => serve(args).await,",
        "Command::Serve(args) => super::startup::serve(args).await,",
    )
    (OUT / "cli_dispatch.rs").write_text(
        USE + "use super::types::BinaryFlavor;\n\n" + promote_fields(cli),
        encoding="utf-8",
    )
    startup = sl(lines, 230, 411).replace(
        ".layer(middleware::from_fn(log_request));",
        ".layer(middleware::from_fn(super::request_logging::log_request));",
    )
    (OUT / "startup.rs").write_text(
        USE + "use super::types::AppState;\n\n" + promote_fields(startup),
        encoding="utf-8",
    )
    req = sl(lines, 40, 40) + sl(lines, 413, len(lines))
    req = req.replace(
        "fn test_app_state() -> anyhow::Result<super::AppState>",
        "fn test_app_state() -> anyhow::Result<super::super::types::AppState>",
    )
    req = req.replace("Ok(super::AppState {", "Ok(super::super::types::AppState {")
    req = req.replace(
        "use super::{ensure_command_allowed, BinaryFlavor};",
        "use super::super::cli_dispatch::ensure_command_allowed;\n    use super::super::types::BinaryFlavor;",
    )
    (OUT / "request_logging.rs").write_text(USE + promote_fields(req), encoding="utf-8")
    (OUT / "mod.rs").write_text(MOD_RS, encoding="utf-8")
    for name in ("mod.rs", "prelude.rs", "types.rs", "cli_dispatch.rs", "startup.rs", "request_logging.rs"):
        n = (OUT / name).read_text(encoding="utf-8").count("\n") + 1
        print(f"  {name}: {n}")
    SRC.unlink()


if __name__ == "__main__":
    main()
