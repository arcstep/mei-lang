#!/usr/bin/env python3
"""Post-split fixes for toolchain Phase 2 modularization."""
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TS = os.path.join(ROOT, "crates/toolchain/src")


def sed_inplace(path: str, replacements: list[tuple[str, str]]) -> None:
    with open(path) as f:
        content = f.read()
    for old, new in replacements:
        content = content.replace(old, new)
    with open(path, "w") as f:
        f.write(content)


def ensure_ends_with(path: str, suffix: str) -> None:
    with open(path) as f:
        content = f.read()
    if not content.rstrip().endswith(suffix.strip()):
        with open(path, "a") as f:
            if not content.endswith("\n"):
                f.write("\n")
            f.write(suffix if suffix.startswith("\n") else f"\n{suffix}\n")


# editor_runtime + workspace_stock helpers
for rel in [
    "editor_runtime/paths.rs",
    "editor_runtime/layout.rs",
    "editor_runtime/binaries.rs",
    "editor_runtime/io.rs",
    "editor_runtime/render.rs",
    "editor_runtime/install.rs",
    "editor_runtime/doctor.rs",
    "workspace_stock/materialize.rs",
    "workspace_stock/doctor.rs",
    "workspace_stock/migrate.rs",
    "workspace_stock/manifest.rs",
    "workspace_stock/profile.rs",
]:
    p = os.path.join(TS, rel)
    with open(p) as f:
        lines = f.readlines()
    with open(p, "w") as f:
        for line in lines:
            if line.startswith("fn "):
                f.write("pub(crate) " + line)
            else:
                f.write(line)

for rel in ["editor_runtime/types.rs", "workspace_stock/types.rs"]:
    p = os.path.join(TS, rel)
    with open(p) as f:
        lines = f.readlines()
    with open(p, "w") as f:
        for line in lines:
            if line.startswith("const "):
                f.write("pub(crate) " + line)
            else:
                f.write(line)

# mcp_surface split from git (fixes script must not break json braces)
import subprocess
orig = subprocess.check_output(
    ["git", "show", "HEAD:crates/toolchain/src/capability_catalog.rs"],
    cwd=ROOT,
    text=True,
).splitlines(keepends=True)
header = "use super::prelude::*;\nuse super::*;\n\n"
helpers = "".join(orig[297:323])
access_inner = "".join(orig[620:808])
access_file = header + helpers + (
    "pub(crate) fn access_mcp_surface_descriptor(\n"
    "    package_root: &Path,\n"
    "    workspace_root: Option<&Path>,\n"
    "    access_adapter_reference: String,\n"
    "    access_adapter_entrypoint: String,\n"
    ") -> Value {\n"
    "    json!({\n"
    + access_inner
    + "    })\n"
    "}\n"
)
with open(os.path.join(TS, "capability_catalog/access_mcp_surface.rs"), "w") as f:
    f.write(access_file)
dispatcher = "".join(orig[323:345])
author_arm = "".join(orig[345:619])
tail = (
    "        \"access\" => Some(access_mcp_surface_descriptor(\n"
    "            package_root,\n"
    "            workspace_root,\n"
    "            access_adapter_reference,\n"
    "            access_adapter_entrypoint,\n"
    "        )),\n"
    "        _ => None,\n"
    "    }\n"
    "}\n\n"
) + "".join(orig[813:830])
mcp_file = header + helpers + dispatcher + author_arm + tail
mcp_path = os.path.join(TS, "capability_catalog/mcp_surface.rs")
mcp_file = mcp_file.replace(
    "fn mcp_surface_descriptor_for_roots",
    "pub(crate) fn mcp_surface_descriptor_for_roots",
)
with open(mcp_path, "w") as f:
    f.write(mcp_file)
mod_path = os.path.join(TS, "capability_catalog/mod.rs")
with open(mod_path) as f:
    mod = f.read()
if "access_mcp_surface" not in mod:
    mod = mod.replace(
        "mod mcp_surface;\n", "mod mcp_surface;\nmod access_mcp_surface;\n"
    ).replace(
        "pub(crate) use mcp_surface::*;\n",
        "pub(crate) use mcp_surface::*;\npub(crate) use access_mcp_surface::*;\n",
    )
    with open(mod_path, "w") as f:
        f.write(mod)

# cache types visibility
types_path = os.path.join(TS, "compile_service/cache/types.rs")
with open(types_path) as f:
    c = f.read()
c = c.replace(
    "pub(crate) struct CachedCompiledApp {\n    compile_revision:",
    "pub(crate) struct CachedCompiledApp {\n    pub(crate) compile_revision:",
)
c = c.replace("    watched_files: Vec", "    pub(crate) watched_files: Vec", 1)
c = c.replace(
    "    components_revision: u128,\n    compiled: Arc",
    "    pub(crate) components_revision: u128,\n    pub(crate) compiled: Arc",
    1,
)
for field in ["runtime_metric_defs", "runtime_analysis_graph", "runtime_analysis_contracts"]:
    c = c.replace(f"    {field}:", f"    pub(crate) {field}:")
c = re.sub(
    r"(pub\(crate\) struct AssemblyInputDiskRecord \{[^}]*?)\n    kind:",
    r"\1\n    pub(crate) kind:",
    c,
    count=1,
)
c = re.sub(
    r"pub\(crate\) kind: String,\n    key:",
    "pub(crate) kind: String,\n    pub(crate) key:",
    c,
)
c = re.sub(
    r"pub\(crate\) key: String,\n    revision:",
    "pub(crate) key: String,\n    pub(crate) revision:",
    c,
)
for field in [
    "schema_version",
    "compile_revision",
    "revision_scope",
    "compiled",
    "dataset_runtime_payloads",
    "assembly_inputs",
    "access_slim",
]:
    c = re.sub(
        f"(pub\\(crate\\) struct CompiledAppDiskArtifact \\{{[^}}]*?)\\n    {field}:",
        f"\\1\n    pub(crate) {field}:",
        c,
        count=1,
    )
with open(types_path, "w") as f:
    f.write(c)

# cache invalidate + mod + prelude
invalidate = os.path.join(TS, "compile_service/cache/invalidate.rs")
sed_inplace(
    invalidate,
    [
        ("fn evict_compile_cache_entries_for_write", "pub(crate) fn evict_compile_cache_entries_for_write"),
        ("fn validate_cached_entry", "pub(crate) fn validate_cached_entry"),
    ],
)
mod_path = os.path.join(TS, "compile_service/cache/mod.rs")
sed_inplace(
    mod_path,
    [
        (
            "compile_singleflight_enabled, env_flag_enabled, finish",
            "compile_singleflight_enabled, finish",
        ),
    ],
)
prelude_path = os.path.join(TS, "compile_service/cache/prelude.rs")
with open(prelude_path) as f:
    prelude = f.read()
if "singleflight::env_flag_enabled" not in prelude:
    prelude = prelude.rstrip() + "\npub(crate) use super::singleflight::env_flag_enabled;\n"
    with open(prelude_path, "w") as f:
        f.write(prelude)

# closing braces
ensure_ends_with(os.path.join(TS, "compile_service/cache/load.rs"), "}")
ensure_ends_with(os.path.join(TS, "compile_service/cache/compile.rs"), "}")
ensure_ends_with(os.path.join(TS, "compile_service/cache/invalidate.rs"), "}")

lookup_path = os.path.join(TS, "compile_service/cache/lookup.rs")
with open(lookup_path) as f:
    lookup = f.read()
if "/// Lightweight manifest probe" in lookup:
    lookup = lookup.split("/// Lightweight manifest probe")[0].rstrip() + "\n"
    with open(lookup_path, "w") as f:
        f.write(lookup)

# revision/singleflight pub(crate)
for fn in ["revision.rs", "singleflight.rs"]:
    p = os.path.join(TS, "compile_service/cache", fn)
    with open(p) as f:
        content = f.read()
    content = content.replace("pub(super) ", "pub(crate) ")
    with open(p, "w") as f:
        f.write(content)

print("fixes applied")
