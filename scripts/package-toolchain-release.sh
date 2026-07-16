#!/usr/bin/env bash
# Build release toolchain binaries and pack a versioned archive for GitHub Releases.
#
# Outputs under dist/toolchain/ (gitignored-friendly; created on demand):
#   mei-toolchain-<ver>-<target-triple>.{tar.gz|zip}
#   MANIFEST.json
#
# Binaries included:
#   mei-host-shell, mei-compiler, mei-app-runtime, mei-plug-ds, mei-snapshot,
#   mei-lsp, mei-toolchain
#
# Usage:
#   ./scripts/package-toolchain-release.sh
#   ./scripts/package-toolchain-release.sh --skip-build
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SKIP_BUILD=0
OUT_ROOT="${MEI_LANG_ROOT}/dist/toolchain"

usage() {
  cat <<'EOF'
Usage: package-toolchain-release.sh [--skip-build] [--out DIR]

Builds release binaries and packs a portable archive for GitHub Releases.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --out) OUT_ROOT="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

read_workspace_version() {
  local ver
  ver="$(
    sed -n '/^\[workspace\.package\]/,/^\[/{s/^version *= *"\([^"]*\)".*/\1/p;}' \
      "${MEI_LANG_ROOT}/Cargo.toml" | head -1
  )"
  if [[ -z "${ver}" ]]; then
    echo "error: could not read [workspace.package].version from Cargo.toml" >&2
    exit 1
  fi
  printf '%s' "${ver}"
}

detect_target_triple() {
  if command -v rustc >/dev/null 2>&1; then
    rustc -vV | sed -n 's/^host: //p' | head -1
    return 0
  fi
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "${os}" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT*)
      echo "x86_64-pc-windows-msvc"
      return 0
      ;;
  esac
  case "${os}-${arch}" in
    Linux-x86_64) echo "x86_64-unknown-linux-gnu" ;;
    Linux-aarch64) echo "aarch64-unknown-linux-gnu" ;;
    Darwin-arm64) echo "aarch64-apple-darwin" ;;
    Darwin-x86_64) echo "x86_64-apple-darwin" ;;
    *)
      echo "error: unsupported host ${os}-${arch}" >&2
      exit 1
      ;;
  esac
}

VERSION="$(read_workspace_version)"
TARGET="$(detect_target_triple)"
EXT=""
case "${TARGET}" in
  *windows*) EXT=".exe" ;;
esac

BINS=(
  mei-host-shell
  mei-compiler
  mei-app-runtime
  mei-plug-ds
  mei-snapshot
  mei-lsp
  mei-toolchain
)

TARGET_DIR="${CARGO_TARGET_DIR:-${MEI_LANG_ROOT}/target}"
if [[ -n "${MEI_CARGO_TARGET_DIR:-}" ]]; then
  TARGET_DIR="${MEI_CARGO_TARGET_DIR}"
elif [[ "${TARGET_DIR}" != "${MEI_LANG_ROOT}/target" ]]; then
  unset CARGO_TARGET_DIR || true
  TARGET_DIR="${MEI_LANG_ROOT}/target"
fi
BIN_DIR="${TARGET_DIR}/release"

if [[ "${SKIP_BUILD}" != "1" ]]; then
  echo "==> cargo build --release (toolchain packages)"
  CARGO_TARGET_DIR="${TARGET_DIR}" cargo build --release \
    --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
    -p mei-host-shell \
    -p mei-compiler \
    -p mei-app-runtime \
    -p mei-plug-ds \
    -p mei-snapshot \
    -p mei-lang-lsp \
    -p mei-lang-server
fi

STAGE="${OUT_ROOT}/stage/mei-toolchain-${VERSION}-${TARGET}"
rm -rf "${STAGE}"
mkdir -p "${STAGE}/bin"

for name in "${BINS[@]}"; do
  src="${BIN_DIR}/${name}${EXT}"
  if [[ ! -f "${src}" ]]; then
    echo "error: missing binary after build: ${src}" >&2
    exit 1
  fi
  cp -f "${src}" "${STAGE}/bin/${name}${EXT}"
  if [[ "$(uname -s)" == "Darwin" ]] && command -v codesign >/dev/null 2>&1; then
    codesign --force --sign - "${STAGE}/bin/${name}${EXT}" >/dev/null 2>&1 || true
  fi
  echo "  + bin/${name}${EXT}"
done

EXT="${EXT}" VERSION="${VERSION}" TARGET="${TARGET}" \
  python3 - "${STAGE}/MANIFEST.json" "${BINS[@]}" <<'PY'
import json, os, sys
out = sys.argv[1]
ext = os.environ.get("EXT", "")
bins = [f"{name}{ext}" for name in sys.argv[2:]]
doc = {
    "format": "mei-toolchain",
    "formatVersion": 1,
    "version": os.environ["VERSION"],
    "target": os.environ["TARGET"],
    "bins": bins,
}
open(out, "w", encoding="utf-8").write(json.dumps(doc, indent=2) + "\n")
PY

mkdir -p "${OUT_ROOT}"
ARCHIVE_BASENAME="mei-toolchain-${VERSION}-${TARGET}"
if [[ "${EXT}" == ".exe" ]]; then
  ARCHIVE_PATH="${OUT_ROOT}/${ARCHIVE_BASENAME}.zip"
  rm -f "${ARCHIVE_PATH}"
  (
    cd "${OUT_ROOT}/stage"
    if command -v zip >/dev/null 2>&1; then
      zip -r "${ARCHIVE_PATH}" "$(basename "${STAGE}")"
    else
      python3 - "${ARCHIVE_PATH}" "$(basename "${STAGE}")" <<'PY'
import sys, zipfile, os
from pathlib import Path
out, root = sys.argv[1], Path(sys.argv[2])
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
    for p in root.rglob("*"):
        if p.is_file():
            zf.write(p, p.as_posix())
PY
    fi
  )
else
  ARCHIVE_PATH="${OUT_ROOT}/${ARCHIVE_BASENAME}.tar.gz"
  rm -f "${ARCHIVE_PATH}"
  tar -C "${OUT_ROOT}/stage" -czf "${ARCHIVE_PATH}" "$(basename "${STAGE}")"
fi

# Top-level manifest next to the archive (for CI upload convenience).
python3 - "${OUT_ROOT}/MANIFEST.json" "${VERSION}" "${TARGET}" "${ARCHIVE_PATH}" <<'PY'
import json, hashlib, os, sys, pathlib
out, version, target, archive = sys.argv[1:5]
p = pathlib.Path(archive)
doc = {
    "format": "mei-toolchain-dist",
    "formatVersion": 1,
    "version": version,
    "target": target,
    "archive": p.name,
    "sha256": hashlib.sha256(p.read_bytes()).hexdigest(),
    "bytes": p.stat().st_size,
}
pathlib.Path(out).write_text(json.dumps(doc, indent=2) + "\n")
print("wrote", out)
print("archive", p)
PY

echo "==> done"
echo "TOOLCHAIN_ARCHIVE=${ARCHIVE_PATH}"
echo "TOOLCHAIN_VERSION=${VERSION}"
echo "TOOLCHAIN_TARGET=${TARGET}"
