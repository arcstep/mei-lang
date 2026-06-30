#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-data-demo}"
POLICY="${MEI_WARMUP_POLICY:-home}"
parse_common_args "$@"
if [[ "${SOURCE}" == "lang" ]]; then
  ensure_runtime_binaries "${WORKSPACE_ROOT}"
fi

mapfile -t APP_IDS < <(discovered_app_ids "${WORKSPACE_ROOT}" || true)
if [[ ${#APP_IDS[@]} -eq 0 ]]; then
  APP_IDS=("${APP}")
fi

ensure_build_generation_aligned "${WORKSPACE_ROOT}" "${APP_IDS[0]}"

BUILD_ID="${MEI_ENV_GENERATION:-}"
if [[ -z "${BUILD_ID}" ]]; then
  PREPARE_ARGS=(build prepare --workspace "${WORKSPACE_ROOT}")
  for app_id in "${APP_IDS[@]}"; do
    PREPARE_ARGS+=(--app "${app_id}")
  done
  BUILD_ID="$(run_mei_host_shell "${WORKSPACE_ROOT}" "${PREPARE_ARGS[@]}")"
fi
echo "envVersion=${BUILD_ID}"

for app_id in "${APP_IDS[@]}"; do
  echo "==> prebuild app=${app_id}"
  echo "==> compile (${app_id})"
  run_mei_compiler "${WORKSPACE_ROOT}" \
    compile --workspace "${WORKSPACE_ROOT}" --app "${app_id}" "${DEPLOY_CLI_ARGS[@]}"

  echo "==> import (${app_id})"
  run_mei_host_shell "${WORKSPACE_ROOT}" \
    import --workspace "${WORKSPACE_ROOT}" --app "${app_id}" "${DEPLOY_CLI_ARGS[@]}"

  echo "==> prebuild-data (${app_id})"
  run_mei_host_shell "${WORKSPACE_ROOT}" \
    prebuild-data --workspace "${WORKSPACE_ROOT}" --app "${app_id}" "${DEPLOY_CLI_ARGS[@]}"

  echo "==> invalidate eval-cache (${app_id})"
  INVALIDATE_ARGS=(eval-cache invalidate --workspace "${WORKSPACE_ROOT}" --app "${app_id}")
  if [[ "${MEI_FORCE_EVAL_CACHE_CLEAR:-0}" == "1" ]]; then
    INVALIDATE_ARGS+=(--force)
  fi
  run_mei_host_shell "${WORKSPACE_ROOT}" \
    "${INVALIDATE_ARGS[@]}" "${DEPLOY_CLI_ARGS[@]}"

  APP_POLICY="${POLICY}"
  if command -v jq >/dev/null 2>&1; then
    CONFIG_FILE="${MEI_WORKSPACE_CONFIG:-${WORKSPACE_ROOT}/workspace.json}"
    if [[ -f "${CONFIG_FILE}" ]]; then
      HOT_SCENE="$(jq -r --arg app "${app_id}" '.warmup.apps[$app].hotScenes[0] // empty' "${CONFIG_FILE}" 2>/dev/null || true)"
      if [[ -n "${HOT_SCENE}" ]]; then
        APP_POLICY="${HOT_SCENE}"
      elif [[ "${app_id}" != "${APP_IDS[0]}" ]]; then
        APP_POLICY="home"
      fi
    fi
  fi

  echo "==> warmup app=${app_id} policy=${APP_POLICY}"
  run_mei_plug_ds "${WORKSPACE_ROOT}" \
    warmup --workspace "${WORKSPACE_ROOT}" --app "${app_id}" \
    --policy "${APP_POLICY}" --tier all "${DEPLOY_CLI_ARGS[@]}"

  echo "==> build finalize (${app_id})"
  run_mei_host_shell "${WORKSPACE_ROOT}" \
    build finalize --workspace "${WORKSPACE_ROOT}" --app "${app_id}" \
    --build-id "${BUILD_ID}" "${DEPLOY_CLI_ARGS[@]}"
done

echo "Prebuild complete for ${#APP_IDS[@]} app(s) (envVersion=${BUILD_ID})."
