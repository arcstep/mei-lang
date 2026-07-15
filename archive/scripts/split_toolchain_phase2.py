#!/usr/bin/env python3
"""One-shot split for toolchain Phase 2 modularization."""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOOLCHAIN = os.path.join(ROOT, "crates/toolchain/src")


def write(path: str, content: str) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(content)


def extract_lines(src: str, start: int, end: int) -> str:
    if os.path.isfile(src):
        with open(src) as f:
            lines = f.readlines()
        return "".join(lines[start - 1:end])
    rel = os.path.relpath(src, ROOT)
    text = subprocess.check_output(["git", "show", f"HEAD:{rel}"], cwd=ROOT, text=True)
    lines = text.splitlines(keepends=True)
    return "".join(lines[start - 1:end])


def submodule_header() -> str:
    return "use super::prelude::*;\nuse super::*;\n\n"


def pubcrate_helpers(body: str) -> str:
    names = [
        "compiled_app_artifact_enabled",
        "compiled_app_artifact_scope",
        "artifact_matches_compile_scene_request",
        "compiled_app_artifact_lookup_scopes",
        "compiled_app_artifact_root",
        "store_compile_cache_entry",
        "maybe_write_compiled_app_artifact",
        "ensure_compiled_app_artifact_alias",
        "maybe_load_compiled_app_artifact",
        "load_compiled_app_artifact_at_scope",
        "validate_cached_entry",
        "count_files_recursively",
        "hydrate_compiled_app_runtime_payloads",
        "extract_dataset_runtime_payloads",
        "build_assembly_inputs",
        "write_compiled_app_artifact_value",
        "compiled_app_artifact_context",
        "list_compiled_app_scopes_for_target",
        "normalized_scope_target",
        "stable_assembly_hash",
    ]
    for fn in names:
        body = body.replace(f"fn {fn}(", f"pub(crate) fn {fn}(")
    return body


def split_editor_runtime() -> None:
    src = os.path.join(TOOLCHAIN, "editor_runtime.rs")
    ed = os.path.join(TOOLCHAIN, "editor_runtime")
    write(
        f"{ed}/prelude.rs",
        """//! Shared imports for editor_runtime submodules.

pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::process::Command;

pub(crate) use anyhow::{Context, Result};
pub(crate) use chrono::{SecondsFormat, Utc};
pub(crate) use serde::Serialize;
pub(crate) use serde_json::Value;
pub(crate) use walkdir::WalkDir;

pub(crate) use mei_lang_kernel::{
    apply_toolchain_store_symlinks, build_runtime_warmup_manifest, record_toolchain_install_links,
    resolve_toolchain_root, resolve_workspace_runtime_root, toolchain_store_dir,
    RuntimeWarmupManifest, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};

pub(crate) use crate::capability_catalog::CAPABILITY_CATALOG_SCHEMA_VERSION;
pub(crate) use crate::{knowledge_bundle::package_root_hint, knowledge_bundle_descriptor_for_package_root};

pub(crate) use super::types::*;
""",
    )
    write(f"{ed}/types.rs", "use serde::Serialize;\n\n" + extract_lines(src, 19, 191))
    for name, start, end in [
        ("paths", 193, 224),
        ("layout", 225, 390),
        ("binaries", 392, 547),
        ("descriptor", 549, 678),
        ("doctor", 680, 989),
        ("io", 991, 1075),
        ("render", 1077, 1231),
        ("install", 1233, 1509),
        ("scaffold", 1511, 1730),
    ]:
        write(f"{ed}/{name}.rs", submodule_header() + extract_lines(src, start, end))
    write(
        f"{ed}/tests.rs",
        extract_lines(src, 1732, 1848).replace(
            "use super::*;", "use super::prelude::*;\nuse super::*;"
        ),
    )
    write(
        f"{ed}/mod.rs",
        """mod prelude;
mod types;
mod paths;
mod layout;
mod binaries;
mod descriptor;
mod doctor;
mod io;
mod render;
mod install;
mod scaffold;

#[cfg(test)]
mod tests;

pub(crate) use paths::*;
pub(crate) use layout::*;
pub(crate) use binaries::*;
pub(crate) use descriptor::*;
pub(crate) use doctor::*;
pub(crate) use io::*;
pub(crate) use render::*;
pub(crate) use install::*;
pub(crate) use scaffold::*;

pub use types::*;
pub use descriptor::{
    doctor_editor_runtime_for_package_root, editor_runtime_descriptor_for_package_root,
    workspace_runtime_manifest_for_package_root, workspace_runtime_version_descriptor,
};
pub use doctor::{
    doctor_editor_runtime_for_workspace_root, workspace_runtime_status_for_workspace_root,
};
pub use install::{
    ensure_workspace_author_skill_package, install_editor_runtime_support_files,
};
pub use scaffold::scaffold_editor_runtime_tooling;
""",
    )
    if os.path.isfile(src):
        os.remove(src)


def split_capability_catalog() -> None:
    src = os.path.join(TOOLCHAIN, "capability_catalog.rs")
    cat = os.path.join(TOOLCHAIN, "capability_catalog")
    write(
        f"{cat}/prelude.rs",
        """//! Shared imports for capability_catalog submodules.

pub(crate) use std::path::Path;

pub(crate) use mei_lang_kernel::{
    host_extension_registry_descriptor, host_requirements_descriptor,
    host_runtime_capabilities_catalog, host_runtime_contract_descriptor,
};
pub(crate) use serde::Serialize;
pub(crate) use serde_json::{json, Value};

pub(crate) use crate::knowledge_bundle::knowledge_bundle_descriptor_for_package_root;
pub(crate) use crate::platform_assets::{
    platform_asset_catalog_descriptor_for_package_root,
    platform_asset_catalog_descriptor_for_workspace_root,
};
pub(crate) use crate::types::ResourceQueryToolSpec;

pub(crate) use super::types::*;
""",
    )
    write(f"{cat}/types.rs", "use serde::Serialize;\n\n" + extract_lines(src, 17, 49))
    write(f"{cat}/profiles.rs", submodule_header() + extract_lines(src, 51, 228))
    write(f"{cat}/catalog.rs", submodule_header() + extract_lines(src, 230, 296))
    mcp = extract_lines(src, 298, 831).replace(
        "fn mcp_surface_descriptor_for_roots", "pub(crate) fn mcp_surface_descriptor_for_roots"
    )
    write(f"{cat}/mcp_surface.rs", submodule_header() + mcp)
    write(f"{cat}/access_host.rs", submodule_header() + extract_lines(src, 832, 1015))
    write(
        f"{cat}/mod.rs",
        """mod prelude;
mod types;
mod profiles;
mod catalog;
mod mcp_surface;
mod access_host;

pub(crate) use profiles::*;
pub(crate) use catalog::*;
pub(crate) use mcp_surface::*;
pub(crate) use access_host::*;

pub use types::*;
pub use profiles::{
    access_profile_descriptor, ai_profile_descriptor, ai_profile_policy_lines,
    author_profile_descriptor, meilang_access_skill_package, meilang_author_skill_package,
};
pub use catalog::{
    capability_catalog_descriptor, capability_catalog_descriptor_for_package_root,
    capability_catalog_descriptor_for_workspace_root,
};
pub use mcp_surface::{
    mcp_surface_descriptor, mcp_surface_descriptor_for_workspace_root,
};
pub use access_host::{
    access_host_bound_query_tools, access_host_bound_tool_descriptors,
    access_host_bound_tool_names,
};
""",
    )
    if os.path.isfile(src):
        os.remove(src)


def split_workspace_stock() -> None:
    src = os.path.join(TOOLCHAIN, "workspace_stock.rs")
    ws = os.path.join(TOOLCHAIN, "workspace_stock")
    write(
        f"{ws}/prelude.rs",
        """//! Shared imports for workspace_stock submodules.

pub(crate) use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

pub(crate) use anyhow::{Context, Result};
pub(crate) use chrono::Utc;
pub(crate) use mei_lang_kernel::{
    resolve_authoring_root, resolve_components_root, resolve_stock_root, resolve_templates_root,
    resolve_toolchain_root, resolve_workspace_runtime_root, stock_authoring_source,
    stock_components_source, stock_templates_source, workspace_config_path, write_workspace_config,
    APP_CONFIG_FILENAME, WorkspaceConfig, WorkspacePathsConfig, WorkspaceProfile,
    WorkspaceStockBootstrapConfig, WorkspaceStockCatalogAppConfig, WorkspaceStockCatalogConfig,
    WorkspaceStockCatalogKindConfig, WorkspaceStockConfig, WorkspaceStockPreviewConfig,
    DEFAULT_APPS_REL, DEFAULT_STOCK_AUTHORING_REL, DEFAULT_STOCK_COMPONENTS_REL,
    DEFAULT_STOCK_TEMPLATES_REL, WORKSPACE_HOSTS_DIR_REL,
};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use walkdir::WalkDir;

pub(crate) use super::types::*;
""",
    )
    write(f"{ws}/types.rs", "use serde::{Deserialize, Serialize};\n\n" + extract_lines(src, 22, 84))
    write(
        f"{ws}/materialize.rs",
        submodule_header() + extract_lines(src, 86, 149) + extract_lines(src, 397, 458),
    )
    write(
        f"{ws}/doctor.rs",
        submodule_header() + extract_lines(src, 151, 159) + extract_lines(src, 161, 336),
    )
    write(f"{ws}/migrate.rs", submodule_header() + extract_lines(src, 338, 395))
    write(f"{ws}/manifest.rs", submodule_header() + extract_lines(src, 460, 533))
    write(f"{ws}/profile.rs", submodule_header() + extract_lines(src, 535, 672))
    write(f"{ws}/tests.rs", extract_lines(src, 674, 756))
    write(
        f"{ws}/mod.rs",
        """mod prelude;
mod types;
mod materialize;
mod doctor;
mod migrate;
mod manifest;
mod profile;

#[cfg(test)]
mod tests;

pub(crate) use materialize::*;
pub(crate) use doctor::*;
pub(crate) use migrate::*;
pub(crate) use manifest::*;
pub(crate) use profile::*;

pub use types::*;
pub use materialize::{
    ensure_workspace_stock_materialized, materialize_workspace_stock, sync_workspace_stock,
};
pub use doctor::{doctor_workspace_stock, ensure_stock_catalog_app_synced};
pub use migrate::migrate_workspace_stock_paths;
pub use manifest::workspace_stock_revision;
pub use profile::{create_app_skeleton, init_workspace_profile};
""",
    )
    if os.path.isfile(src):
        os.remove(src)


def split_compile_cache() -> None:
    cache = os.path.join(TOOLCHAIN, "compile_service/cache")
    orig = subprocess.check_output(
        ["git", "show", "HEAD:crates/toolchain/src/compile_service/cache/mod.rs"],
        cwd=ROOT,
        text=True,
    )
    lines = orig.splitlines(keepends=True)

    write(
        f"{cache}/prelude.rs",
        """//! Shared imports for compile cache submodules.

pub(crate) use std::collections::{BTreeMap, HashMap};
pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::{Arc, Mutex as StdMutex, OnceLock, RwLock};
pub(crate) use std::time::{Duration, Instant, UNIX_EPOCH};

pub(crate) use anyhow;
pub(crate) use mei_lang_kernel::{
    compile_app_with_options, compile_app_with_options_and_revision, resolve_app_root,
    AnalysisGraph, CompileOptions, CompileWatchedFile, CompiledApp, COMPILE_SEMANTICS_GENERATION,
};
pub(crate) use mei_lang_kernel::resolve_components_root as kernel_resolve_components_root;
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use serde_json::Value;

pub(crate) use crate::artifact_store::{
    compiled_app_manifest_identity, read_artifact_manifest, read_json_artifact,
    write_json_artifact, ArtifactStoreManifest, ArtifactWatchedFile, ArtifactWriteContext,
};
pub(crate) use crate::types::WorldScope;

pub(crate) use super::access_slim::{
    access_slim_artifacts_enabled, canonical_artifact_persist_enabled, content_store_preferred,
    should_persist_compiled_app_artifact, slim_compiled_app_for_access,
    strip_loaded_compiled_app_for_access,
};

pub(crate) use super::types::*;
""",
    )

    types_body = extract_lines(
        os.path.join(cache, "mod.rs") if os.path.exists(os.path.join(cache, "mod.rs")) else "",
        39,
        185,
    )
    # types from git original
    types_body = "".join(lines[38:185])
    types_body = types_body.replace("pub(super) struct CachedCompiledApp", "pub(crate) struct CachedCompiledApp")
    types_body = re.sub(
        r"^struct (DatasetRuntimePayload|AssemblyInputDiskRecord|CompiledAppDiskArtifact)",
        r"pub(crate) struct \1",
        types_body,
        flags=re.M,
    )
    types_body = re.sub(r"^const COMPILED_APP", "pub(crate) const COMPILED_APP", types_body, flags=re.M)
    types_body = types_body.replace("pub(super) fn compile_cache", "pub(crate) fn compile_cache")
    types_body = types_body.replace("pub(super) fn compile_failure_latch", "pub(crate) fn compile_failure_latch")
    types_body = types_body.replace("const COMPILE_FAILURE_LATCH_TTL", "pub(crate) const COMPILE_FAILURE_LATCH_TTL")
    types_body = types_body.replace("fn compile_cache_max_entries", "pub(crate) fn compile_cache_max_entries")
    types_body = types_body.replace("    fn into_owned(self)", "    pub(crate) fn into_owned(self)")
    write(
        f"{cache}/types.rs",
        """use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex as StdMutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use anyhow;
use mei_lang_kernel::{
    AnalysisGraph, CompileOptions, CompileWatchedFile, CompiledApp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

"""
        + types_body,
    )

    store_body = pubcrate_helpers("".join(lines[186:594]))  # write path through maybe_write
    lookup_body = pubcrate_helpers("".join(lines[594:736]))  # ensure + lookup scopes
    write(f"{cache}/store.rs", submodule_header() + store_body)
    write(f"{cache}/lookup.rs", submodule_header() + lookup_body)

    load_body = pubcrate_helpers("".join(lines[405:437]))  # hydrate
    load_body += pubcrate_helpers("".join(lines[736:854]))  # probe + load paths
    write(f"{cache}/load.rs", submodule_header() + load_body)

    compile_body = "".join(lines[856:1198]).replace("pub(super) ", "pub(crate) ")
    invalidate_body = "".join(lines[1200:1463]).replace("pub(super) ", "pub(crate) ")
    write(f"{cache}/compile.rs", submodule_header() + compile_body)
    write(f"{cache}/invalidate.rs", submodule_header() + invalidate_body)
    write(f"{cache}/tests.rs", "".join(lines[1465:1579]))

    for fn in ["revision.rs", "singleflight.rs"]:
        path = os.path.join(cache, fn)
        with open(path) as f:
            content = f.read()
        content = content.replace("pub(super) ", "pub(crate) ")
        write(path, content)

    write(
        f"{cache}/mod.rs",
        """mod revision;
mod singleflight;
mod access_slim;

mod prelude;
mod types;
mod store;
mod lookup;
mod load;
mod compile;
mod invalidate;

#[cfg(test)]
mod tests;

pub(crate) use revision::{compile_revision, components_revision, normalize_path, CompileRevisionStamp};
pub(crate) use singleflight::{
    compile_singleflight_enabled, env_flag_enabled, finish_compile_inflight,
    register_compile_inflight, wait_for_compile_inflight,
};

pub(crate) use types::*;
pub(crate) use store::*;
pub(crate) use lookup::*;
pub(crate) use load::*;
pub(crate) use compile::*;
pub(crate) use invalidate::*;

pub use access_slim::{
    access_slim_artifacts_enabled, canonical_artifact_persist_enabled, content_store_preferred,
    should_persist_compiled_app_artifact, slim_compiled_app_for_access,
    strip_loaded_compiled_app_for_access,
};
pub use singleflight::env_flag_enabled;

pub use types::{
    CompileWithCacheFailure, CompileWithCacheOutcome, CompileWithCacheOutcomeShared,
    PeekCompileCacheHit, PeekCompileCacheHitShared,
};
pub use load::{
    hydrate_compiled_app_from_disk_artifacts, probe_compiled_app_manifest_identity,
};
pub use compile::{
    apply_compile_options_scope, compile_app_with_cache, compile_app_with_cache_shared,
    load_compile_artifact_only, load_compile_artifact_only_shared, recent_compile_failure,
};
pub use invalidate::{
    clear_compile_cache_for_app, clear_compiled_app_artifacts_for_app, compile_cache_key,
    is_compile_inflight, peek_compile_cache, peek_compile_cache_hit, peek_compile_cache_hit_shared,
    peek_compile_cache_shared, resolve_components_root,
};
""",
    )


def report_line_counts() -> None:
    dirs = [
        "editor_runtime",
        "capability_catalog",
        "workspace_stock",
        "compile_service/cache",
    ]
    for rel in dirs:
        base = os.path.join(TOOLCHAIN, rel)
        for fn in sorted(os.listdir(base)):
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(base, fn)
            with open(path) as f:
                n = sum(1 for _ in f)
            print(f"{path}: {n}")


def main() -> int:
    split_editor_runtime()
    split_capability_catalog()
    split_workspace_stock()
    split_compile_cache()
    report_line_counts()
    return 0


if __name__ == "__main__":
    sys.exit(main())
