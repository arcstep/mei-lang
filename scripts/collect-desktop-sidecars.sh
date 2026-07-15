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
BIN_DIR="${TARGET_DIR}/${PROFILE}"

if [[ "${SKIP_BUILD}" != "1" ]]; then
  echo "==> building Viewer sidecars (profile=${PROFILE})"
  CARGO_ARGS=(build --manifest-path "${MEI_LANG_ROOT}/Cargo.toml"
    -p mei-host-shell -p mei-app-runtime -p mei-plug-ds -p mei-snapshot -p mei-compiler)
  if [[ "${PROFILE}" == "release" ]]; then
    CARGO_ARGS=(build --release --manifest-path "${MEI_LANG_ROOT}/Cargo.toml"
      -p mei-host-shell -p mei-app-runtime -p mei-plug-ds -p mei-snapshot -p mei-compiler)
  fi
  CARGO_TARGET_DIR="${TARGET_DIR}" cargo "${CARGO_ARGS[@]}"
fi

if [[ "${SKIP_ASSETS}" != "1" ]]; then
  echo "==> building frontend assets"
  (cd "${MEI_LANG_ROOT}" && npm run assets:build)
fi

mkdir -p "${OUT_DIR}/bin" "${OUT_DIR}/app/assets"
EXT=""
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT) EXT=".exe" ;;
esac

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

ASSETS_SRC="${MEI_LANG_ROOT}/app/assets"
if [[ -d "${ASSETS_SRC}/dist" ]]; then
  rsync -a --delete "${ASSETS_SRC}/dist/" "${OUT_DIR}/app/assets/dist/"
elif [[ -d "${ASSETS_SRC}" ]]; then
  rsync -a --delete \
    --exclude 'node_modules' \
    --exclude '.git' \
    "${ASSETS_SRC}/" "${OUT_DIR}/app/assets/"
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
