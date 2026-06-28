#!/usr/bin/env bash
# Compile .mei sources and import into running host registry.
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-data-demo}"
parse_common_args "$@"

ensure_build_generation_aligned "${WORKSPACE_ROOT}" "${APP}"

echo "==> compile"
run_mei_compiler "${WORKSPACE_ROOT}" \
  compile --workspace "${WORKSPACE_ROOT}" --app "${APP}"

echo "==> reload (import)"
run_mei_host_shell "${WORKSPACE_ROOT}" \
  reload --workspace "${WORKSPACE_ROOT}" --app "${APP}" "${DEPLOY_CLI_ARGS[@]}"
