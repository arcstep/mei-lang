#!/usr/bin/env bash
# Build MapLibre Martin from source (pure Rust) into DEST.
# Preferred for TARGET matrices that must not depend on opaque prebuilt bins.
# Fetch cache (fetch-martin-sidecar.sh) remains an optional accelerator.
#
# Usage:
#   ./scripts/desktop/build-martin-from-source.sh --dest DIR
#   MEI_MARTIN_VERSION=1.10.1 ./scripts/desktop/build-martin-from-source.sh --dest DIR
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
MARTIN_VERSION="${MEI_MARTIN_VERSION:-1.10.1}"
DEST=""
PROFILE="release"

usage() {
  cat <<'EOF'
Usage: build-martin-from-source.sh --dest DIR [--version VER] [--debug|--release]

Clones maplibre/martin @ tag vVER (shallow) into a cache dir and cargo-installs
the `martin` binary into DEST.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dest) DEST="${2:?}"; shift 2 ;;
    --version) MARTIN_VERSION="${2:?}"; shift 2 ;;
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

if [[ -z "${DEST}" ]]; then
  echo "error: --dest required" >&2
  usage >&2
  exit 1
fi

mkdir -p "${DEST}"
CACHE_ROOT="${MEI_LANG_ROOT}/.cache/martin-src/v${MARTIN_VERSION}"
SRC_DIR="${CACHE_ROOT}/src"
mkdir -p "${CACHE_ROOT}"

if [[ ! -d "${SRC_DIR}/.git" ]]; then
  rm -rf "${SRC_DIR}"
  echo "==> cloning maplibre/martin v${MARTIN_VERSION}"
  git clone --depth 1 --branch "v${MARTIN_VERSION}" \
    https://github.com/maplibre/martin.git "${SRC_DIR}"
else
  echo "==> reusing martin source cache ${SRC_DIR}"
fi

EXT=""
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT) EXT=".exe" ;;
esac

CARGO_ARGS=(install --path "${SRC_DIR}/martin" --root "${CACHE_ROOT}/install" --locked --force)
if [[ "${PROFILE}" != "release" ]]; then
  CARGO_ARGS+=(--debug)
fi

echo "==> cargo install martin (${PROFILE})"
cargo "${CARGO_ARGS[@]}"

SRC_BIN="${CACHE_ROOT}/install/bin/martin${EXT}"
if [[ ! -f "${SRC_BIN}" ]]; then
  # Some cargo versions place bins under install/bin without forcing path crate name.
  SRC_BIN="$(find "${CACHE_ROOT}/install" -type f \( -name "martin" -o -name "martin.exe" \) | head -1 || true)"
fi
if [[ -z "${SRC_BIN}" || ! -f "${SRC_BIN}" ]]; then
  echo "error: martin binary missing after cargo install" >&2
  exit 1
fi

cp -f "${SRC_BIN}" "${DEST}/martin${EXT}"
chmod a+x "${DEST}/martin${EXT}" || true
echo "MARTIN_BIN=${DEST}/martin${EXT}"
