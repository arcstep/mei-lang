#!/usr/bin/env bash
# 工作区 app 预构建（compile / import / prebuild-data / warmup）。
# 二进制来自 mei-env（见 lib.sh resolve_bin_path），不经 deploy/bin。
set -euo pipefail

# Workspace identity must come from thin deploy/*.sh entry.
if [[ -z "${MEI_WORKSPACE_ROOT:-}" || -z "${MEI_WORKSPACE_DEPLOY_DIR:-}" ]]; then
  echo "error: run via workspace ./deploy/<entry>.sh (thin shell), not stock/impl directly" >&2
  exit 1
fi
DEPLOY_DIR="${MEI_WORKSPACE_DEPLOY_DIR}"
WORKSPACE_ROOT="${MEI_WORKSPACE_ROOT}"
STOCK_DEPLOY="${MEI_STOCK_DEPLOY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
# shellcheck source=../lib.sh
source "${STOCK_DEPLOY}/lib.sh"
export MEI_DEPLOY_LIB_PATH="${STOCK_DEPLOY}/lib.sh"

# When caller pinned an app (`start-host.sh --app` → MEI_APP), only prebuild that app.
# Capture before apply_workspace_deploy_env, which may default MEI_APP for other tools.
PINNED_APP="${MEI_APP:-}"
POLICY="${MEI_WARMUP_POLICY:-home}"
parse_common_args "$@"

if declare -F apply_workspace_deploy_env >/dev/null 2>&1; then
  apply_workspace_deploy_env "${WORKSPACE_ROOT}"
fi

print_runtime_banner "${WORKSPACE_ROOT}"

if [[ -n "${PINNED_APP}" ]]; then
  APP_IDS=("${PINNED_APP}")
else
  mapfile -t APP_IDS < <(discovered_app_ids "${WORKSPACE_ROOT}" || true)
  if [[ ${#APP_IDS[@]} -eq 0 ]]; then
    echo "error: no apps discovered and MEI_APP unset; pass MEI_APP=<id> or start-host --app" >&2
    exit 1
  fi
fi

ensure_build_generation_aligned "${WORKSPACE_ROOT}" "${APP_IDS[@]}"

BUILD_ID="${MEI_ENV_GENERATION:-}"
if [[ -z "${BUILD_ID}" ]]; then
  PREPARE_ARGS=(build prepare --workspace "${WORKSPACE_ROOT}")
  for app_id in "${APP_IDS[@]}"; do
    PREPARE_ARGS+=(--app "${app_id}")
  done
  BUILD_ID="$(capture_build_prepare_generation "${WORKSPACE_ROOT}" "${PREPARE_ARGS[@]}")"
elif ! BUILD_ID="$(extract_ws_generation_id "${BUILD_ID}")"; then
  echo "error: MEI_ENV_GENERATION is not a valid WS-* id" >&2
  printf '%s\n' "${MEI_ENV_GENERATION}" >&2
  exit 1
fi
echo "envVersion=${BUILD_ID}"

for app_id in "${APP_IDS[@]}"; do
  echo "==> prebuild app=${app_id}"
  echo "==> compile (${app_id})"
  # Bash 4.2 + set -u: empty "${arr[@]}" is unbound; expand only when set.
  run_mei_compiler "${WORKSPACE_ROOT}" \
    compile --workspace "${WORKSPACE_ROOT}" --app "${app_id}" ${DEPLOY_CLI_ARGS[@]+"${DEPLOY_CLI_ARGS[@]}"}

  echo "==> import (${app_id})"
  run_mei_host_shell "${WORKSPACE_ROOT}" \
    import --workspace "${WORKSPACE_ROOT}" --app "${app_id}" ${DEPLOY_CLI_ARGS[@]+"${DEPLOY_CLI_ARGS[@]}"}

  echo "==> prebuild-data (${app_id})"
  run_mei_host_shell "${WORKSPACE_ROOT}" \
    prebuild-data --workspace "${WORKSPACE_ROOT}" --app "${app_id}" ${DEPLOY_CLI_ARGS[@]+"${DEPLOY_CLI_ARGS[@]}"}

  echo "==> invalidate eval-cache (${app_id})"
  INVALIDATE_ARGS=(eval-cache invalidate --workspace "${WORKSPACE_ROOT}" --app "${app_id}")
  if [[ "${MEI_FORCE_EVAL_CACHE_CLEAR:-0}" == "1" ]]; then
    INVALIDATE_ARGS+=(--force)
  fi
  run_mei_host_shell "${WORKSPACE_ROOT}" \
    "${INVALIDATE_ARGS[@]}" ${DEPLOY_CLI_ARGS[@]+"${DEPLOY_CLI_ARGS[@]}"}

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
  run_mei_app_runtime "${WORKSPACE_ROOT}" \
    warmup --workspace "${WORKSPACE_ROOT}" --app "${app_id}" \
    --policy "${APP_POLICY}" --tier all ${DEPLOY_CLI_ARGS[@]+"${DEPLOY_CLI_ARGS[@]}"}

  echo "==> build finalize (${app_id})"
  run_mei_host_shell "${WORKSPACE_ROOT}" \
    build finalize --workspace "${WORKSPACE_ROOT}" --app "${app_id}" \
    --build-id "${BUILD_ID}" ${DEPLOY_CLI_ARGS[@]+"${DEPLOY_CLI_ARGS[@]}"}
done

clean_retired_build_generations "${WORKSPACE_ROOT}" "${APP_IDS[@]}"
emit_prebuild_pipeline_complete_banner "${BUILD_ID}" "${APP_IDS[@]}"
