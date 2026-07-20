#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-zhifa}"
HOST="${MEI_HOST:-127.0.0.1}"
PORT="${MEI_PORT:-9527}"
parse_common_args "$@"
apply_runtime_env_from_flags

echo "workspace=${WORKSPACE_ROOT}"
echo "profile=${PROFILE}"
echo "source=${SOURCE}"
echo "runtime=${RUNTIME}"
echo "app=${APP}"
if [[ -f "$(runtime_json_path "${WORKSPACE_ROOT}")" ]]; then
  echo "runtime.json=$(runtime_json_path "${WORKSPACE_ROOT}")"
fi

if [[ -x "$(resolve_bin_path "${WORKSPACE_ROOT}" "mei-host-shell")" ]]; then
  "$(resolve_bin_path "${WORKSPACE_ROOT}" "mei-host-shell")" -V 2>/dev/null | head -3 || true
fi

run_mei_host_shell "${WORKSPACE_ROOT}" build status --workspace "${WORKSPACE_ROOT}" 2>/dev/null || true

if [[ -f "${WORKSPACE_ROOT}/deploy/state/host.pid" ]]; then
  echo "host.pid=$(cat "${WORKSPACE_ROOT}/deploy/state/host.pid")"
else
  echo "host.pid=-"
fi
if [[ -n "${MEI_PLUG_DS_URL:-}" ]]; then
  echo "plug-ds=external:${MEI_PLUG_DS_URL}"
else
  echo "plug-ds=managed-by-host-shell"
fi
report_app_runtime_process_status "${WORKSPACE_ROOT}"
echo "url=http://${HOST}:${PORT}/apps/app/${APP}/scene/home"
