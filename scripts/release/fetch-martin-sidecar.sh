#!/usr/bin/env bash
# Download official MapLibre Martin binary into a destination directory (no tiles).
# Used by collect-desktop-sidecars.sh / package-release-bundles.sh.
#
# Usage:
#   ./scripts/fetch-martin-sidecar.sh --dest DIR
#   ./scripts/fetch-martin-sidecar.sh --dest DIR --target macos-arm64
#   MEI_MARTIN_VERSION=1.10.1 ./scripts/fetch-martin-sidecar.sh --dest DIR
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
MARTIN_VERSION="${MEI_MARTIN_VERSION:-1.10.1}"
DEST=""
TARGET=""

usage() {
  cat <<'EOF'
Usage: fetch-martin-sidecar.sh --dest DIR [--target NAME] [--version VER]

Downloads martin from GitHub releases into DEST/martin (or martin.exe on Windows).

Targets:
  macos-arm64 | macos-x64 | linux-x64 | linux-arm64 | windows-x64
  (default: detect host)
EOF
}

detect_host_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "${os}" in
    Darwin)
      case "${arch}" in
        arm64|aarch64) echo "macos-arm64" ;;
        x86_64) echo "macos-x64" ;;
        *) echo "unsupported Darwin arch: ${arch}" >&2; return 1 ;;
      esac
      ;;
    Linux)
      case "${arch}" in
        x86_64) echo "linux-x64" ;;
        aarch64|arm64) echo "linux-arm64" ;;
        *) echo "unsupported Linux arch: ${arch}" >&2; return 1 ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      echo "windows-x64"
      ;;
    *)
      echo "unsupported OS: ${os}" >&2
      return 1
      ;;
  esac
}

asset_for_target() {
  case "$1" in
    macos-arm64) echo "martin-aarch64-apple-darwin.tar.gz" ;;
    macos-x64) echo "martin-x86_64-apple-darwin.tar.gz" ;;
    linux-x64) echo "martin-x86_64-unknown-linux-musl.tar.gz" ;;
    linux-arm64) echo "martin-aarch64-unknown-linux-musl.tar.gz" ;;
    windows-x64) echo "martin-x86_64-pc-windows-msvc.zip" ;;
    *) echo "unknown target: $1" >&2; return 1 ;;
  esac
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dest)
      DEST="${2:?}"
      shift 2
      ;;
    --target)
      TARGET="${2:?}"
      shift 2
      ;;
    --version)
      MARTIN_VERSION="${2:?}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "${DEST}" ]]; then
  echo "error: --dest DIR is required" >&2
  usage >&2
  exit 1
fi

if [[ -z "${TARGET}" ]]; then
  TARGET="$(detect_host_target)"
fi

# Martin v1.10.x ships no x86_64-apple-darwin prebuilt; fail fast so callers
# (collect-desktop-sidecars) can fall back to build-martin-from-source.sh.
if [[ "${TARGET}" == "macos-x64" ]]; then
  echo "error: no official Martin prebuilt for macos-x64 (martin-v${MARTIN_VERSION}); use MEI_MARTIN_FROM_SOURCE=1 / build-martin-from-source.sh" >&2
  exit 1
fi

ASSET="$(asset_for_target "${TARGET}")"
URL="https://github.com/maplibre/martin/releases/download/martin-v${MARTIN_VERSION}/${ASSET}"
CACHE_DIR="${MEI_LANG_ROOT}/dist/martin-sidecar/.cache"
mkdir -p "${CACHE_DIR}" "${DEST}"
CACHE_FILE="${CACHE_DIR}/${MARTIN_VERSION}-${ASSET}"
EXTRACT_DIR="${CACHE_DIR}/extract-host-${TARGET}-${MARTIN_VERSION}"

if [[ ! -f "${CACHE_FILE}" ]]; then
  echo "==> downloading Martin ${MARTIN_VERSION} (${ASSET})"
  curl -fL --retry 3 -o "${CACHE_FILE}.partial" "${URL}"
  mv "${CACHE_FILE}.partial" "${CACHE_FILE}"
else
  echo "==> using cached Martin ${CACHE_FILE}"
fi

rm -rf "${EXTRACT_DIR}"
mkdir -p "${EXTRACT_DIR}"
case "${ASSET}" in
  *.tar.gz) tar -xzf "${CACHE_FILE}" -C "${EXTRACT_DIR}" ;;
  *.zip) unzip -qo "${CACHE_FILE}" -d "${EXTRACT_DIR}" ;;
esac

BIN_SRC=""
if [[ -f "${EXTRACT_DIR}/martin.exe" ]]; then
  BIN_SRC="${EXTRACT_DIR}/martin.exe"
elif [[ -f "${EXTRACT_DIR}/martin" ]]; then
  BIN_SRC="${EXTRACT_DIR}/martin"
else
  BIN_SRC="$(find "${EXTRACT_DIR}" -type f \( -name 'martin' -o -name 'martin.exe' \) | head -1 || true)"
fi
if [[ -z "${BIN_SRC}" || ! -f "${BIN_SRC}" ]]; then
  echo "error: martin binary not found after extract (${ASSET})" >&2
  find "${EXTRACT_DIR}" -maxdepth 2 -type f >&2 || true
  exit 1
fi

BIN_NAME="$(basename "${BIN_SRC}")"
DEST_BIN="${DEST}/${BIN_NAME}"
cp -f "${BIN_SRC}" "${DEST_BIN}"
chmod +x "${DEST_BIN}" || true
if [[ "$(uname -s)" == "Darwin" ]] && command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "${DEST_BIN}" >/dev/null || true
fi
echo "  + ${DEST_BIN} (martin-v${MARTIN_VERSION})"
