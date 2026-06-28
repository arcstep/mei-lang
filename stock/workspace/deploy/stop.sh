#!/usr/bin/env bash
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"

PORT="${MEI_PORT:-9527}"
PID_FILE="${WORKSPACE_ROOT}/deploy/state/host.pid"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="$2"; shift 2 ;;
    --port=*) PORT="${1#*=}"; shift ;;
    *) shift ;;
  esac
done

stopped=0
if [[ -f "${PID_FILE}" ]]; then
  pid="$(cat "${PID_FILE}")"
  if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
    kill "${pid}" 2>/dev/null || true
    sleep 1
    kill -9 "${pid}" 2>/dev/null || true
    echo "stopped pid=${pid}"
    stopped=1
  fi
  rm -f "${PID_FILE}"
fi

if [[ "${stopped}" -eq 0 ]]; then
  pids="$(lsof -ti ":${PORT}" 2>/dev/null || true)"
  if [[ -n "${pids}" ]]; then
    echo "${pids}" | xargs kill 2>/dev/null || true
    echo "stopped process(es) on port ${PORT}"
    stopped=1
  fi
fi

if [[ "${stopped}" -eq 0 ]]; then
  echo "no host process found (pid file or port ${PORT})"
fi
