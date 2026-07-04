#!/usr/bin/env bash
# Build mei-compiler + mei-plug-ds + mei-host-shell from mei-lang source.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROFILE="debug"
TARGET_DIR="${CARGO_TARGET_DIR:-${MEI_LANG_ROOT}/target}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

CARGO_ARGS=(build --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
  -p mei-compiler -p mei-plug-ds -p mei-host-shell)
if [[ "${PROFILE}" == "release" ]]; then
  CARGO_ARGS=(build --release --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
    -p mei-compiler -p mei-plug-ds -p mei-host-shell)
fi

export MEI_CARGO_BUILD_PROFILE="${PROFILE}"
export MEI_CARGO_SWEEP_KEEP_PKGS="${MEI_CARGO_SWEEP_KEEP_PKGS:-mei-compiler,mei-plug-ds,mei-host-shell}"

# shellcheck source=cargo-target-gc.sh
source "${SCRIPT_DIR}/cargo-target-gc.sh"
maybe_cargo_target_hygiene "${MEI_LANG_ROOT}"

echo "==> mei-lang build (profile=${PROFILE}, root=${MEI_LANG_ROOT})"
CARGO_TARGET_DIR="${TARGET_DIR}" cargo "${CARGO_ARGS[@]}"
echo "==> binaries at ${TARGET_DIR}/${PROFILE}/mei-{compiler,host-shell,plug-ds}"
