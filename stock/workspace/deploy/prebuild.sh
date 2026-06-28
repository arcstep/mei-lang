#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-data-demo}"
POLICY="${MEI_WARMUP_POLICY:-home}"
parse_common_args "$@"

run_mei_host_shell "${WORKSPACE_ROOT}" \
  prebuild --workspace "${WORKSPACE_ROOT}" --app "${APP}" --policy "${POLICY}" "${DEPLOY_CLI_ARGS[@]}"
