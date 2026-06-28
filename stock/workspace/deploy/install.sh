#!/usr/bin/env bash
# Build mei-compiler + mei-host-shell from mei-lang source into deploy/bin/.
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-data-demo}"

MEI_LANG_ROOT="$(resolve_mei_lang_root "${WORKSPACE_ROOT}")"
TARGET_DIR="${CARGO_TARGET_DIR:-${MEI_LANG_ROOT}/target}"
BIN_DIR="${WORKSPACE_ROOT}/deploy/bin"
mkdir -p "${BIN_DIR}"

old_version=""
if [[ -x "${BIN_DIR}/mei-host-shell" ]]; then
  old_version="$("${BIN_DIR}/mei-host-shell" -V 2>/dev/null | head -1 || true)"
fi

echo "==> building mei-compiler + mei-host-shell (mei-lang=${MEI_LANG_ROOT})"
CARGO_TARGET_DIR="${TARGET_DIR}" cargo build --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
  -p mei-compiler -p mei-host-shell

ln -sfn "${TARGET_DIR}/debug/mei-compiler" "${BIN_DIR}/mei-compiler"
ln -sfn "${TARGET_DIR}/debug/mei-host-shell" "${BIN_DIR}/mei-host-shell"

new_version="$("${BIN_DIR}/mei-host-shell" -V 2>/dev/null | head -1 || true)"
echo "Installed to ${BIN_DIR}"
if [[ -n "${old_version}" ]]; then
  echo "Previous: ${old_version}"
fi
if [[ -n "${new_version}" ]]; then
  echo "Current:  ${new_version}"
fi
if [[ -n "${old_version}" && -n "${new_version}" && "${old_version}" != "${new_version}" ]]; then
  echo "==> mei-lang version changed; aligning workspace env generation"
  ensure_build_generation_aligned "${WORKSPACE_ROOT}" "${APP}"
fi
