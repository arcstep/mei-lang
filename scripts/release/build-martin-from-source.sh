#!/usr/bin/env bash
# Build MapLibre Martin from source (pure Rust) into DEST.
# Preferred for TARGET matrices that must not depend on opaque prebuilt bins.
# Fetch cache (fetch-martin-sidecar.sh) remains an optional accelerator.
#
# Usage:
#   ./scripts/build-martin-from-source.sh --dest DIR
#   MEI_MARTIN_VERSION=1.10.1 ./scripts/build-martin-from-source.sh --dest DIR
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
MARTIN_VERSION="${MEI_MARTIN_VERSION:-1.10.1}"
DEST=""
PROFILE="release"

usage() {
  cat <<'EOF'
Usage: build-martin-from-source.sh --dest DIR [--version VER] [--debug|--release]

Clones maplibre/martin @ tag martin-vVER (shallow) into a cache dir outside
mei-lang (avoids inheriting mei-lang/.cargo vendor overlay) and cargo-installs
the `martin` binary into DEST.

Env:
  MEI_MARTIN_VERSION      default 1.10.1
  MEI_MARTIN_FEATURES     cargo --features for sidecar (default: mbtiles; no webui/npm)
  MEI_MARTIN_CACHE_ROOT   override cache parent (default: $TMPDIR/mei-martin-src)
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
TAG="martin-v${MARTIN_VERSION}"
# Keep cache outside mei-lang so mei-lang/.cargo vendor overlay is not inherited.
CACHE_ROOT="${MEI_MARTIN_CACHE_ROOT:-${TMPDIR:-${TMP:-${TEMP:-/tmp}}}/mei-martin-src}/${TAG}"
CACHE_ROOT="${CACHE_ROOT//\/\//\/}"
SRC_DIR="${CACHE_ROOT}/src"
mkdir -p "${CACHE_ROOT}"

if [[ ! -d "${SRC_DIR}/.git" ]]; then
  rm -rf "${SRC_DIR}"
  echo "==> cloning maplibre/martin ${TAG}"
  git clone --depth 1 --branch "${TAG}" \
    https://github.com/maplibre/martin.git "${SRC_DIR}"
else
  echo "==> reusing martin source cache ${SRC_DIR}"
fi

EXT=""
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT) EXT=".exe" ;;
esac

# Prefer the crate path used by modern martin tags; fall back to repo root.
MARTIN_CRATE="${SRC_DIR}/martin"
if [[ ! -f "${MARTIN_CRATE}/Cargo.toml" ]]; then
  MARTIN_CRATE="${SRC_DIR}"
fi

# Sidecar only serves MBTiles; skip webui (needs npm) and other default features.
MARTIN_FEATURES="${MEI_MARTIN_FEATURES:-mbtiles}"

CARGO_ARGS=(install --path "${MARTIN_CRATE}" --root "${CACHE_ROOT}/install" --locked --force \
  --no-default-features --features "${MARTIN_FEATURES}")
if [[ "${PROFILE}" != "release" ]]; then
  CARGO_ARGS+=(--debug)
fi

echo "==> cargo install martin (${PROFILE}, features=${MARTIN_FEATURES}) from ${MARTIN_CRATE}"
# Build outside mei-lang so local vendor replace-with does not apply.
(
  cd "${SRC_DIR}"
  export CARGO_TARGET_DIR="${CACHE_ROOT}/target"
  cargo "${CARGO_ARGS[@]}"
)
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
