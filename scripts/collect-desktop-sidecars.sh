#!/usr/bin/env bash
# Collect Viewer sidecars from cargo build into a staging directory for Tauri / portable packs.
# See docs/mei-lang-v2/05-host/0541-desktop-viewer-implementation-plan.md
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROFILE="release"
OUT_DIR="${MEI_LANG_ROOT}/desktop/sidecars"
SKIP_BUILD=0
SKIP_ASSETS=0

usage() {
  cat <<'EOF'
Usage: collect-desktop-sidecars.sh [--debug|--release] [--out DIR] [--skip-build] [--skip-assets]

Copies:
  mei-host-shell, mei-app-runtime, mei-plug-ds, mei-snapshot, mei-compiler
  app/assets/ (via npm run assets:build unless --skip-assets)
into OUT (default: mei-lang/desktop/sidecars/).

Also runs cargo target hygiene (same as scripts/build.sh) before compile.
Skips cargo when binaries are newer than Cargo.lock unless MEI_DESKTOP_FORCE_BUILD=1.

mei-compiler enables /runtime reload & local .mei recompile without
reshipping a full snapshot/bundle (still not a full Studio).
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
    --out) OUT_DIR="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --skip-assets) SKIP_ASSETS=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

TARGET_DIR="${CARGO_TARGET_DIR:-${MEI_LANG_ROOT}/target}"
# Cursor/agent shells may inject a sandbox CARGO_TARGET_DIR outside the repo.
# Viewer sidecars must come from mei-lang/target unless explicitly overridden.
if [[ -n "${MEI_CARGO_TARGET_DIR:-}" ]]; then
  TARGET_DIR="${MEI_CARGO_TARGET_DIR}"
elif [[ "${TARGET_DIR}" != "${MEI_LANG_ROOT}/target" ]]; then
  echo "==> ignoring inherited CARGO_TARGET_DIR=${TARGET_DIR}"
  echo "    using ${MEI_LANG_ROOT}/target (set MEI_CARGO_TARGET_DIR to override)"
  unset CARGO_TARGET_DIR || true
  TARGET_DIR="${MEI_LANG_ROOT}/target"
fi
BIN_DIR="${TARGET_DIR}/${PROFILE}"

EXT=""
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT) EXT=".exe" ;;
esac

# Same target hygiene as mei-lang/scripts/build.sh (sweep stale → budget → optional clean).
export MEI_CARGO_BUILD_PROFILE="${PROFILE}"
export MEI_CARGO_SWEEP_KEEP_PKGS="${MEI_CARGO_SWEEP_KEEP_PKGS:-mei-host-shell,mei-app-runtime,mei-plug-ds,mei-snapshot,mei-compiler}"
# shellcheck source=cargo-target-gc.sh
source "${SCRIPT_DIR}/cargo-target-gc.sh"
if [[ "${MEI_CARGO_TARGET_HYGIENE:-1}" != "0" && "${MEI_CARGO_TARGET_HYGIENE_RAN:-0}" != "1" ]]; then
  maybe_cargo_target_hygiene "${MEI_LANG_ROOT}"
  export MEI_CARGO_TARGET_HYGIENE_RAN=1
fi

sidecar_bins_fresh() {
  local name src
  for name in mei-host-shell mei-app-runtime mei-plug-ds mei-snapshot mei-compiler; do
    src="${BIN_DIR}/${name}${EXT}"
    if [[ ! -f "${src}" ]]; then
      return 1
    fi
    # Rebuild if Cargo.lock is newer than any required binary.
    if [[ -f "${MEI_LANG_ROOT}/Cargo.lock" && "${MEI_LANG_ROOT}/Cargo.lock" -nt "${src}" ]]; then
      return 1
    fi
  done
  return 0
}

if [[ "${SKIP_BUILD}" != "1" ]]; then
  if [[ "${MEI_DESKTOP_FORCE_BUILD:-0}" != "1" ]] \
    && [[ "${MEI_CARGO_SKIP_BUILD_IF_FRESH:-1}" == "1" ]] \
    && sidecar_bins_fresh; then
    echo "==> skip cargo build (sidecars fresh vs Cargo.lock; set MEI_DESKTOP_FORCE_BUILD=1 to rebuild)"
  else
    echo "==> building Viewer sidecars (profile=${PROFILE})"
    CARGO_ARGS=(build --manifest-path "${MEI_LANG_ROOT}/Cargo.toml"
      -p mei-host-shell -p mei-app-runtime -p mei-plug-ds -p mei-snapshot -p mei-compiler)
    if [[ "${PROFILE}" == "release" ]]; then
      CARGO_ARGS=(build --release --manifest-path "${MEI_LANG_ROOT}/Cargo.toml"
        -p mei-host-shell -p mei-app-runtime -p mei-plug-ds -p mei-snapshot -p mei-compiler)
    fi
    CARGO_TARGET_DIR="${TARGET_DIR}" cargo "${CARGO_ARGS[@]}"
  fi
fi

if [[ "${SKIP_ASSETS}" != "1" ]]; then
  echo "==> building frontend assets"
  (
    cd "${MEI_LANG_ROOT}"
    if [[ ! -d node_modules/esbuild ]]; then
      echo "==> installing root npm deps (esbuild required for assets:build)"
      if [[ -f package-lock.json ]]; then
        npm ci || npm install
      else
        npm install
      fi
    fi
    npm run assets:build
  )
fi

mkdir -p "${OUT_DIR}/bin" "${OUT_DIR}/app/assets"

copy_bin() {
  local name="$1"
  local src="${BIN_DIR}/${name}${EXT}"
  local dest="${OUT_DIR}/bin/${name}${EXT}"
  if [[ ! -f "${src}" ]]; then
    echo "missing binary: ${src}" >&2
    exit 1
  fi
  cp -f "${src}" "${dest}"
  # macOS: `cp` can invalidate linker-signed adhoc signatures → runtime SIGKILL
  # (Code Signature Invalid). Re-sign adhoc after every copy.
  if [[ "$(uname -s)" == "Darwin" ]]; then
    if command -v codesign >/dev/null 2>&1; then
      codesign --force --sign - "${dest}" >/dev/null
    fi
  fi
  echo "  + bin/${name}${EXT}"
}

echo "==> collecting into ${OUT_DIR}"
copy_bin mei-host-shell
copy_bin mei-app-runtime
copy_bin mei-plug-ds
copy_bin mei-snapshot
copy_bin mei-compiler

# Mirror a directory tree. Prefer rsync; fall back for Windows runners without it.
sync_tree() {
  local src="$1"
  local dest="$2"
  mkdir -p "${dest}"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete \
      --exclude 'node_modules' \
      --exclude '.git' \
      "${src}/" "${dest}/"
    return 0
  fi
  # Portable fallback (Git Bash / Windows): wipe dest then copy, skipping bulky dirs.
  python3 - "$src" "$dest" <<'PY'
import os, shutil, sys
src, dest = sys.argv[1], sys.argv[2]
skip = {"node_modules", ".git"}

def ignore(_dir, names):
    return [n for n in names if n in skip]

if os.path.isdir(dest):
    shutil.rmtree(dest)
shutil.copytree(src, dest, ignore=ignore)
PY
}

ASSETS_SRC="${MEI_LANG_ROOT}/host-shell/app/assets"
if [[ -d "${ASSETS_SRC}" ]]; then
  # Always sync full app/assets (host-shell.css, favicon, page-load-progress-shell, dist/, …).
  # Copying dist/ alone breaks shell chrome styles in Viewer sidecar mode.
  sync_tree "${ASSETS_SRC}" "${OUT_DIR}/app/assets"
fi

# Stock components + templates are resolved via MEI_PACKAGE_ROOT/stock when workspace has no override.
STOCK_COMPONENTS="${MEI_LANG_ROOT}/stock/components"
if [[ -d "${STOCK_COMPONENTS}" ]]; then
  mkdir -p "${OUT_DIR}/stock"
  sync_tree "${STOCK_COMPONENTS}" "${OUT_DIR}/stock/components"
  echo "  + stock/components"
fi
STOCK_TEMPLATES="${MEI_LANG_ROOT}/stock/templates"
if [[ -d "${STOCK_TEMPLATES}" ]]; then
  mkdir -p "${OUT_DIR}/stock"
  sync_tree "${STOCK_TEMPLATES}" "${OUT_DIR}/stock/templates"
  echo "  + stock/templates"
fi

export OUT_DIR PROFILE
MANIFEST="${OUT_DIR}/MANIFEST.json"
python3 - <<'PY'
import json, os, hashlib, pathlib
root = pathlib.Path(os.environ["OUT_DIR"])
files = []
for p in sorted(root.rglob("*")):
    if not p.is_file() or p.name == "MANIFEST.json":
        continue
    rel = p.relative_to(root).as_posix()
    h = hashlib.sha256(p.read_bytes()).hexdigest()
    files.append({"path": rel, "sha256": h, "bytes": p.stat().st_size})
doc = {
    "format": "mei-desktop-sidecars",
    "formatVersion": 1,
    "profile": os.environ["PROFILE"],
    "bins": ["mei-host-shell", "mei-app-runtime", "mei-plug-ds", "mei-snapshot", "mei-compiler"],
    "files": files,
}
(root / "MANIFEST.json").write_text(json.dumps(doc, indent=2) + "\n")
print("wrote", root / "MANIFEST.json", "entries=", len(files))
PY

echo "==> done"
echo "SIDECARS_DIR=${OUT_DIR}"
