#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-data-demo}"
HOST="${MEI_HOST:-127.0.0.1}"
PORT="${MEI_PORT:-9527}"
RUNTIME="${MEI_RUNTIME:-local}"
SKIP_PREBUILD=0
BACKGROUND=0
WARMUP_POLICY="${MEI_WARMUP_POLICY:-home}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-prebuild) SKIP_PREBUILD=1; shift ;;
    --background) BACKGROUND=1; shift ;;
    --port) PORT="$2"; shift 2 ;;
    --port=*) PORT="${1#*=}"; shift ;;
    --host) HOST="$2"; shift 2 ;;
    --host=*) HOST="${1#*=}"; shift ;;
    --policy) WARMUP_POLICY="$2"; shift 2 ;;
    --policy=*) WARMUP_POLICY="${1#*=}"; shift ;;
    --runtime) RUNTIME="$2"; shift 2 ;;
    --runtime=*) RUNTIME="${1#*=}"; shift ;;
    --cargo) RUNTIME="cargo"; shift ;;
    *) break ;;
  esac
done

if [[ "${RUNTIME}" == "local" ]]; then
  ensure_local_bins "${WORKSPACE_ROOT}"
fi

if [[ "${SKIP_PREBUILD}" -eq 0 ]]; then
  echo "==> prebuild (policy=${WARMUP_POLICY})"
  MEI_WARMUP_POLICY="${WARMUP_POLICY}" MEI_RUNTIME="${RUNTIME}" \
    "${DEPLOY_DIR}/prebuild.sh" --runtime "${RUNTIME}"
  echo ""
fi

URL="http://${HOST}:${PORT}/apps/app/${APP}/scene/home"
echo "Workspace: ${WORKSPACE_ROOT}"
echo "Runtime:   ${RUNTIME}"
echo "Listen:    http://${HOST}:${PORT}"
echo "Open:      ${URL}"
echo ""

PID_FILE="${WORKSPACE_ROOT}/deploy/state/host.pid"
mkdir -p "${WORKSPACE_ROOT}/deploy/state"

if [[ "${BACKGROUND}" -eq 1 ]]; then
  nohup bash -c "
    source '${DEPLOY_DIR}/lib.sh'
    RUNTIME='${RUNTIME}'
    run_mei_host_shell '${WORKSPACE_ROOT}' \
      serve --workspace '${WORKSPACE_ROOT}' --app '${APP}' \
      --host '${HOST}' --port '${PORT}' $*
  " >"${WORKSPACE_ROOT}/deploy/state/host.log" 2>&1 &
  echo $! >"${PID_FILE}"
  echo "background pid=$(cat "${PID_FILE}") log=deploy/state/host.log"
  exit 0
fi

run_mei_host_shell "${WORKSPACE_ROOT}" \
  serve --workspace "${WORKSPACE_ROOT}" --app "${APP}" \
  --host "${HOST}" --port "${PORT}" "$@"
