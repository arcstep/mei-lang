#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/split_toolchain_phase2.py
rm -f crates/toolchain/src/editor_runtime.rs \
      crates/toolchain/src/capability_catalog.rs \
      crates/toolchain/src/workspace_stock.rs
python3 scripts/split_toolchain_phase2_fixes.py
# fix compile_revision on CompiledAppDiskArtifact if regex missed
python3 - <<'PY'
from pathlib import Path
p = Path("crates/toolchain/src/compile_service/cache/types.rs")
t = p.read_text()
if "pub(crate) compile_revision: String," not in t:
    t = t.replace(
        "pub(crate) struct CompiledAppDiskArtifact {\n    pub(crate) schema_version:",
        "pub(crate) struct CompiledAppDiskArtifact {\n    pub(crate) schema_version:",
    )
    t = t.replace(
        "    pub(crate) schema_version: String,\n    compile_revision:",
        "    pub(crate) schema_version: String,\n    pub(crate) compile_revision:",
    )
    p.write_text(t)
PY
cargo build -p mei-lang-server 2>&1 | tail -5
