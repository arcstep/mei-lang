#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-data-demo}"
POLICY="${MEI_WARMUP_POLICY:-home}"
parse_common_args "$@"

ensure_build_generation_aligned "${WORKSPACE_ROOT}" "${APP}"

BUILD_ID="$(run_mei_host_shell "${WORKSPACE_ROOT}" \
  build prepare --workspace "${WORKSPACE_ROOT}" --app "${APP}" "${DEPLOY_CLI_ARGS[@]}")"
echo "envVersion=${BUILD_ID}"

echo "==> compile"
run_mei_compiler "${WORKSPACE_ROOT}" \
  compile --workspace "${WORKSPACE_ROOT}" --app "${APP}" "${DEPLOY_CLI_ARGS[@]}"

echo "==> import"
run_mei_host_shell "${WORKSPACE_ROOT}" \
  import --workspace "${WORKSPACE_ROOT}" --app "${APP}" "${DEPLOY_CLI_ARGS[@]}"

echo "==> prebuild-data"
run_mei_host_shell "${WORKSPACE_ROOT}" \
  prebuild-data --workspace "${WORKSPACE_ROOT}" --app "${APP}" "${DEPLOY_CLI_ARGS[@]}"

echo "==> clear eval-cache"
EVAL_CACHE="${WORKSPACE_ROOT}/apps/${APP}/var/active/eval-cache"
rm -rf "${EVAL_CACHE}"

echo "==> warmup policy=${POLICY}"
run_mei_plug_ds "${WORKSPACE_ROOT}" \
  warmup --workspace "${WORKSPACE_ROOT}" --app "${APP}" \
  --policy "${POLICY}" --tier all "${DEPLOY_CLI_ARGS[@]}"

echo "==> build finalize"
run_mei_host_shell "${WORKSPACE_ROOT}" \
  build finalize --workspace "${WORKSPACE_ROOT}" --app "${APP}" \
  --build-id "${BUILD_ID}" "${DEPLOY_CLI_ARGS[@]}"

echo "Prebuild complete (envVersion=${BUILD_ID})."
