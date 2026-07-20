#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

PORT="${MEI_PORT:-9527}"
HOST_PID_FILE="${WORKSPACE_ROOT}/deploy/state/host.pid"
PLUG_PID_FILE="${WORKSPACE_ROOT}/deploy/state/plug-ds.pid"

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
    # Prefer SIGTERM so host can run graceful child teardown.
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

# Always sweep workspace-scoped children (covers host kill -9 / IDE Stop orphans).
sweep_stale_app_runtimes "${WORKSPACE_ROOT}"
sweep_stale_managed_martin "${WORKSPACE_ROOT}"

if [[ "${stopped}" -eq 0 ]]; then
  echo "no host-shell process found (pid file or port ${PORT}); runtime/martin sweep still ran"
fi
