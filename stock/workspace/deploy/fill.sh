#!/usr/bin/env bash
# 平台四步之「灌入」：把当前 mei-lang 编进 mei-env（场景 monorepo / lang-source）。
# 场景 installed：明确报错，应使用 fill-from-bundle 或下载包。
#
# 用法：
#   ./deploy/fill.sh
#   ./deploy/fill.sh --release
#   ./deploy/fill.sh --tag darwin-arm64-local
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

PROFILE="debug"
RELEASE_TAG="${MEI_RELEASE_TAG:-darwin-arm64-local}"
SCENARIO=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
    --profile) PROFILE="$2"; shift 2 ;;
    --tag) RELEASE_TAG="$2"; shift 2 ;;
    --tag=*) RELEASE_TAG="${1#*=}"; shift ;;
    --scenario) SCENARIO="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "${SCENARIO}" ]] && command -v jq >/dev/null 2>&1 && [[ -f "${WORKSPACE_ROOT}/workspace.json" ]]; then
  SCENARIO="$(jq -r '.meiEnv.scenario // empty' "${WORKSPACE_ROOT}/workspace.json" 2>/dev/null || true)"
fi
SCENARIO="${SCENARIO:-monorepo}"

if [[ "${SCENARIO}" == "installed" ]]; then
  echo "error: scenario=installed has no source compile; use mei-env/release/collect/fill-from-bundle.sh" >&2
  exit 1
fi

mei_env_root="$(resolve_mei_env_root "${WORKSPACE_ROOT}")"
fill_script="${mei_env_root}/release/collect/fill-from-lang.sh"
if [[ ! -x "${fill_script}" ]]; then
  echo "error: fill-from-lang missing at ${fill_script}" >&2
  exit 1
fi

MEI_LANG_ROOT="$(resolve_mei_lang_root "${WORKSPACE_ROOT}")" \
  MEI_ENV_ROOT="${mei_env_root}" \
  "${fill_script}" --tag "${RELEASE_TAG}" --profile "${PROFILE}"
