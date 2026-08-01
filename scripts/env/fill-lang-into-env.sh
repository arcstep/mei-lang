#!/usr/bin/env bash
# Fill mei-env/targets from local mei-lang cargo build (no sibling mei-env repo required).
# Layout matches collect-v2-runtime: targets/<tag>/mei-lang-<ver>/v2/bin/{mei-host-shell,...}
#
# Usage:
#   MEI_LANG_ROOT=... MEI_ENV_ROOT=... ./fill-lang-into-env.sh [--tag t] [--profile debug|release]
set -euo pipefail

ENV_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="${MEI_LANG_ROOT:-$(cd "${ENV_DIR}/../.." && pwd)}"
MEI_ENV_ROOT="${MEI_ENV_ROOT:-${HOME}/.mei-env}"

TARGET_TAG=""
PROFILE="debug"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag) TARGET_TAG="${2:?}"; shift 2 ;;
    --tag=*) TARGET_TAG="${1#*=}"; shift ;;
    --profile) PROFILE="${2:?}"; shift 2 ;;
    --profile=*) PROFILE="${1#*=}"; shift ;;
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
    -h|--help)
      sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 1
      ;;
  esac
done

case "${PROFILE}" in
  debug|release) ;;
  *)
    echo "error: profile must be debug or release" >&2
    exit 1
    ;;
esac

if [[ -z "${TARGET_TAG}" ]]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) TARGET_TAG="darwin-arm64-local" ;;
    Darwin-x86_64) TARGET_TAG="darwin-x86_64-local" ;;
    Linux-x86_64) TARGET_TAG="linux-x86_64-local" ;;
    Linux-aarch64) TARGET_TAG="linux-aarch64-local" ;;
    MINGW*|MSYS*|CYGWIN*) TARGET_TAG="windows-x86_64-local" ;;
    *) TARGET_TAG="local" ;;
  esac
fi

if [[ ! -f "${MEI_LANG_ROOT}/Cargo.toml" ]]; then
  echo "error: mei-lang not found at ${MEI_LANG_ROOT}" >&2
  exit 1
fi

# Prefer monorepo fill-from-lang when mei-env release tree is present
FILL_FROM_LANG="${MEI_ENV_ROOT}/release/collect/fill-from-lang.sh"
if [[ -x "${FILL_FROM_LANG}" ]]; then
  echo "==> delegating to ${FILL_FROM_LANG}"
  exec env MEI_LANG_ROOT="${MEI_LANG_ROOT}" MEI_ENV_ROOT="${MEI_ENV_ROOT}" \
    "${FILL_FROM_LANG}" --tag "${TARGET_TAG}" --profile "${PROFILE}" --skip-assets
fi

"${ENV_DIR}/ensure-mei-env-root.sh" --root "${MEI_ENV_ROOT}" >/dev/null

command -v cargo >/dev/null 2>&1 || {
  echo "error: cargo not found" >&2
  exit 1
}
command -v rsync >/dev/null 2>&1 || {
  echo "error: rsync not found" >&2
  exit 1
}

TARGET_DIR="${MEI_CARGO_TARGET_DIR:-${MEI_LANG_ROOT}/target}"
SUBDIR="debug"
[[ "${PROFILE}" == "release" ]] && SUBDIR="release"
BUILD_SCRIPT="${MEI_LANG_ROOT}/scripts/build/build.sh"

echo "==> fill-lang-into-env profile=${PROFILE} tag=${TARGET_TAG}"
echo "    mei-lang=${MEI_LANG_ROOT}"
echo "    mei-env=${MEI_ENV_ROOT}"

if [[ -f "${BUILD_SCRIPT}" ]]; then
  if [[ "${PROFILE}" == "release" ]]; then
    MEI_CARGO_TARGET_HYGIENE=1 CARGO_TARGET_DIR="${TARGET_DIR}" "${BUILD_SCRIPT}" --release
  else
    MEI_CARGO_TARGET_HYGIENE=1 CARGO_TARGET_DIR="${TARGET_DIR}" "${BUILD_SCRIPT}" --debug
  fi
else
  cargo_args=(build --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
    -p mei-compiler -p mei-host-shell -p mei-app-runtime)
  if [[ "${PROFILE}" == "release" ]]; then
    cargo_args=(build --release --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
      -p mei-compiler -p mei-host-shell -p mei-app-runtime)
  fi
  CARGO_TARGET_DIR="${TARGET_DIR}" cargo "${cargo_args[@]}"
fi

BIN_SRC="${TARGET_DIR}/${SUBDIR}"
for name in mei-compiler mei-host-shell mei-app-runtime; do
  if [[ ! -x "${BIN_SRC}/${name}" ]]; then
    echo "error: missing binary after build: ${BIN_SRC}/${name}" >&2
    exit 1
  fi
done

cargo_ver="$(sed -n 's/^version = "\(.*\)"/\1/p' "${MEI_LANG_ROOT}/Cargo.toml" | head -1)"
if [[ -z "${cargo_ver}" ]]; then
  cargo_ver="$(sed -n 's/^version = "\(.*\)"/\1/p' "${MEI_LANG_ROOT}/host-shell/Cargo.toml" 2>/dev/null | head -1 || true)"
fi
cargo_ver="${cargo_ver:-0.0.0}"
short="$(git -C "${MEI_LANG_ROOT}" rev-parse --short HEAD 2>/dev/null || echo nogit)"
dirty=""
if [[ -n "$(git -C "${MEI_LANG_ROOT}" status --porcelain 2>/dev/null || true)" ]]; then
  dirty="-dirty"
fi
BUILD_VERSION="${cargo_ver}+${short}${dirty}"

OUT_DIR="${MEI_ENV_ROOT}/targets/${TARGET_TAG}/mei-lang-${BUILD_VERSION}"
V2_BIN="${OUT_DIR}/v2/bin"
mkdir -p "${V2_BIN}"

for name in mei-compiler mei-host-shell mei-app-runtime; do
  rsync -a "${BIN_SRC}/${name}" "${V2_BIN}/${name}"
  chmod +x "${V2_BIN}/${name}"
done

echo "==> fill complete: ${V2_BIN}"
echo "${OUT_DIR}"
