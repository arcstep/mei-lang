#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-data-demo}"
HOST="${MEI_HOST:-127.0.0.1}"
PORT="${MEI_PORT:-9527}"
PLUG_PORT="${MEI_PLUG_DS_PORT:-9528}"
RUNTIME="${MEI_RUNTIME:-local}"
SKIP_PREBUILD=0
BACKGROUND=0
WARMUP_POLICY="${MEI_WARMUP_POLICY:-home}"
MEI_AUTH_FLAG=""
if [[ "${MEI_AUTH:-0}" == "1" ]]; then
  MEI_AUTH_FLAG="--auth"
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-prebuild) SKIP_PREBUILD=1; shift ;;
    --background) BACKGROUND=1; shift ;;
    --auth) MEI_AUTH_FLAG="--auth"; shift ;;
    --port) PORT="$2"; shift 2 ;;
    --port=*) PORT="${1#*=}"; shift ;;
    --plug-port) PLUG_PORT="$2"; shift 2 ;;
    --plug-port=*) PLUG_PORT="${1#*=}"; shift ;;
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
echo "Plug-ds:   http://${HOST}:${PLUG_PORT}"
echo "Listen:    http://${HOST}:${PORT}"
echo "Open:      ${URL}"
echo ""

STATE_DIR="${WORKSPACE_ROOT}/deploy/state"
mkdir -p "${STATE_DIR}"
PLUG_PID_FILE="${STATE_DIR}/plug-ds.pid"
HOST_PID_FILE="${STATE_DIR}/host.pid"

start_plug_ds() {
  if [[ "${BACKGROUND}" -eq 1 ]]; then
    nohup bash -c "
      source '${DEPLOY_DIR}/lib.sh'
      RUNTIME='${RUNTIME}'
      run_mei_plug_ds '${WORKSPACE_ROOT}' \
        serve --workspace '${WORKSPACE_ROOT}' --app '${APP}' \
        --host '${HOST}' --port '${PLUG_PORT}'
    " >"${STATE_DIR}/plug-ds.log" 2>&1 &
    echo $! >"${PLUG_PID_FILE}"
  else
    run_mei_plug_ds "${WORKSPACE_ROOT}" \
      serve --workspace "${WORKSPACE_ROOT}" --app "${APP}" \
      --host "${HOST}" --port "${PLUG_PORT}" &
    echo $! >"${PLUG_PID_FILE}"
  fi
  wait_for_plug_ds_health "${HOST}" "${PLUG_PORT}"
}

start_plug_ds

if [[ "${BACKGROUND}" -eq 1 ]]; then
  nohup bash -c "
    source '${DEPLOY_DIR}/lib.sh'
    RUNTIME='${RUNTIME}'
    run_mei_host_shell '${WORKSPACE_ROOT}' \
      serve --workspace '${WORKSPACE_ROOT}' --app '${APP}' \
      --host '${HOST}' --port '${PORT}' ${MEI_AUTH_FLAG} $*
  " >"${STATE_DIR}/host.log" 2>&1 &
  echo $! >"${HOST_PID_FILE}"
  echo "plug-ds pid=$(cat "${PLUG_PID_FILE}") log=deploy/state/plug-ds.log"
  echo "host-shell pid=$(cat "${HOST_PID_FILE}") log=deploy/state/host.log"
  exit 0
fi

run_mei_host_shell "${WORKSPACE_ROOT}" \
  serve --workspace "${WORKSPACE_ROOT}" --app "${APP}" \
  --host "${HOST}" --port "${PORT}" ${MEI_AUTH_FLAG} "$@"
