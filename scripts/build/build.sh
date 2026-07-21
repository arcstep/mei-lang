#!/usr/bin/env bash
# Build mei-compiler + mei-host-shell + mei-app-runtime from mei-lang source.
# (mei-plug-ds is a library embedded by mei-app-runtime; no standalone product bin.)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROFILE="debug"

# shellcheck source=build-env.sh
source "${SCRIPT_DIR}/build-env.sh"
TARGET_DIR="$(mei_cargo_target_dir "${MEI_LANG_ROOT}")"
export CARGO_TARGET_DIR="${TARGET_DIR}"
mei_export_build_identity "${MEI_LANG_ROOT}"
# DataFusion is linked into mei bins; no libduckdb fetch.
mei_export_duckdb_prebuilt "${MEI_LANG_ROOT}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

CARGO_ARGS=(build --message-format=json-render-diagnostics \
  --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
  -p mei-compiler -p mei-host-shell -p mei-app-runtime)
if [[ "${PROFILE}" == "release" ]]; then
  CARGO_ARGS=(build --release --message-format=json-render-diagnostics \
    --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
    -p mei-compiler -p mei-host-shell -p mei-app-runtime)
fi

export MEI_CARGO_BUILD_PROFILE="${PROFILE}"
export MEI_CARGO_SWEEP_KEEP_PKGS="${MEI_CARGO_SWEEP_KEEP_PKGS:-mei-compiler,mei-host-shell,mei-app-runtime}"

# shellcheck source=../ops/cargo-target-gc.sh
source "${SCRIPT_DIR}/../ops/cargo-target-gc.sh"

if [[ "${MEI_CARGO_TARGET_HYGIENE:-1}" != "0" && "${MEI_CARGO_TARGET_HYGIENE_RAN:-0}" != "1" ]]; then
  maybe_cargo_target_hygiene "${MEI_LANG_ROOT}"
fi

if [[ "${MEI_CARGO_RUNTIME_PANEL_EMITTED:-0}" != "1" ]]; then
  cargo_target_emit_startup_panel "${TARGET_DIR}" "${PROFILE}" "compile"
  export MEI_CARGO_RUNTIME_PANEL_EMITTED=1
fi

echo "==> mei-lang build (profile=${PROFILE}, root=${MEI_LANG_ROOT})"
before_kb="$(du -sk "${TARGET_DIR}" 2>/dev/null | awk '{print $1}' || true)"
set +e
CARGO_TARGET_DIR="${TARGET_DIR}" cargo "${CARGO_ARGS[@]}" \
  | python3 "${SCRIPT_DIR}/cargo-build-report.py"
pipeline_status=("${PIPESTATUS[@]}")
set -e
cargo_status="${pipeline_status[0]:-1}"
report_status="${pipeline_status[1]:-1}"
if (( cargo_status != 0 )); then
  exit "${cargo_status}"
fi
if (( report_status != 0 )); then
  exit "${report_status}"
fi
after_kb="$(du -sk "${TARGET_DIR}" 2>/dev/null | awk '{print $1}' || true)"
if [[ "${before_kb}" =~ ^[0-9]+$ && "${after_kb}" =~ ^[0-9]+$ ]]; then
  delta_kb=$((after_kb - before_kb))
  awk -v before="${before_kb}" -v after="${after_kb}" -v delta="${delta_kb}" \
    'BEGIN { printf "==> target size: %.2fGiB -> %.2fGiB (%+.1fMiB)\n", before / 1048576, after / 1048576, delta / 1024 }'
fi
echo "==> binaries at ${TARGET_DIR}/${PROFILE}/mei-{compiler,host-shell,app-runtime}"
