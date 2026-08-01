#!/usr/bin/env bash
# 停止本工作区 Host / plug-ds，并清扫遗留 app-runtime / Martin。
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

# CLI --port > MEI_PORT > workspace.json#workspace.port > 9527
PORT="$(default_workspace_serve_port "${WORKSPACE_ROOT}")"
HOST_PID_FILE="${WORKSPACE_ROOT}/deploy/state/host.pid"
PLUG_PID_FILE="${WORKSPACE_ROOT}/deploy/state/plug-ds.pid"

print_runtime_banner "${WORKSPACE_ROOT}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="$2"; shift 2 ;;
    --port=*) PORT="${1#*=}"; shift ;;
    *) shift ;;
  esac
done

stop_pid_file() {
  local label="$1"
  local pid_file="$2"
  if [[ ! -f "${pid_file}" ]]; then
    return 1
  fi
  local pid
  pid="$(cat "${pid_file}")"
  if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
    kill -TERM "${pid}" 2>/dev/null || true
    local i
    for i in 1 2 3 4 5 6 7 8; do
      if ! kill -0 "${pid}" 2>/dev/null; then
        echo "stopped ${label} pid=${pid} (SIGTERM)"
        rm -f "${pid_file}"
        return 0
      fi
      sleep 0.25
    done
    kill -KILL "${pid}" 2>/dev/null || true
    echo "stopped ${label} pid=${pid} (SIGKILL after grace)"
    rm -f "${pid_file}"
    return 0
  fi
  rm -f "${pid_file}"
  return 1
}

stop_port() {
  local label="$1"
  local port="$2"
  local pids
  pids="$(lsof -ti ":${port}" 2>/dev/null || true)"
  if [[ -n "${pids}" ]]; then
    # shellcheck disable=SC2086
    kill -TERM ${pids} 2>/dev/null || true
    sleep 1
    # shellcheck disable=SC2086
    kill -KILL ${pids} 2>/dev/null || true
    echo "stopped ${label} process(es) on port ${port}"
    return 0
  fi
  return 1
}

stopped=0
if stop_pid_file "legacy plug-ds" "${PLUG_PID_FILE}"; then
  stopped=1
fi
if stop_pid_file "host-shell" "${HOST_PID_FILE}"; then
  stopped=1
fi
if stop_port "host-shell" "${PORT}"; then
  stopped=1
fi

sweep_stale_app_runtimes "${WORKSPACE_ROOT}"
sweep_stale_managed_martin "${WORKSPACE_ROOT}"

if [[ "${stopped}" -eq 0 ]]; then
  echo "no host-shell process found (pid file or port ${PORT}); runtime/martin sweep still ran"
fi
