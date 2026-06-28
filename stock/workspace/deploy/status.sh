#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-data-demo}"
HOST="${MEI_HOST:-127.0.0.1}"
PORT="${MEI_PORT:-9527}"
parse_common_args "$@"

echo "workspace=${WORKSPACE_ROOT}"
echo "runtime=${RUNTIME}"
echo "app=${APP}"

if [[ "${RUNTIME}" == "local" && -x "${WORKSPACE_ROOT}/deploy/bin/mei-host-shell" ]]; then
  "${WORKSPACE_ROOT}/deploy/bin/mei-host-shell" -V 2>/dev/null | head -3 || true
elif [[ "${RUNTIME}" == "cargo" ]]; then
  run_mei_host_shell "${WORKSPACE_ROOT}" -V 2>/dev/null | head -3 || true
fi

run_mei_host_shell "${WORKSPACE_ROOT}" build status --workspace "${WORKSPACE_ROOT}" 2>/dev/null || true

if [[ -f "${WORKSPACE_ROOT}/deploy/state/host.pid" ]]; then
  echo "host.pid=$(cat "${WORKSPACE_ROOT}/deploy/state/host.pid")"
else
  echo "host.pid=-"
fi
echo "url=http://${HOST}:${PORT}/apps/app/${APP}/scene/home"
