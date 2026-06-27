#!/usr/bin/env bash
# PR regression gate: block golden tests + layer verify (fast feedback).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

echo "== block/layer regression gates =="

echo "-- cargo test block_eval_golden --"
cargo test -p ws-spbjw-integration-tests --test block_eval_golden

WS_SSPBJW="${ROOT}/../workspaces/ws-spbjw"
if [[ -d "${WS_SSPBJW}" ]]; then
  echo "-- layer verify qunfu mcg --"
  cargo run -q -p mei-lang-server --bin mei-toolchain -- \
    layer verify --workspace "${WS_SSPBJW}" --app qunfu --layer mcg

  echo "-- prebuild qunfu (hot path) --"
  cargo run -q -p mei-lang-server --bin mei-toolchain -- \
    prebuild --workspace "${WS_SSPBJW}" --app qunfu --hot-only --json \
    | (command -v jq >/dev/null 2>&1 && jq -e '.warning_count == 0' || cat)

  echo "-- prebuild zhifa (hot-only, pending queue guard) --"
  ZHIFA_LOG="$(mktemp)"
  ZHIFA_START=$(date +%s)
  cargo run -q -p mei-lang-server --bin mei-toolchain -- \
    prebuild --workspace "${WS_SSPBJW}" --app zhifa --hot-only --json 2>&1 | tee "${ZHIFA_LOG}" \
    | (command -v jq >/dev/null 2>&1 && jq -e '.warning_count == 0' || cat)
  ZHIFA_ELAPSED=$(( $(date +%s) - ZHIFA_START ))
  if [[ "${ZHIFA_ELAPSED}" -gt 180 ]]; then
    echo "WARN: zhifa hot-only took ${ZHIFA_ELAPSED}s (target <3min)" >&2
  fi
  PENDING_PEAK="$(rg -o '待处理 [0-9]+' "${ZHIFA_LOG}" | rg -o '[0-9]+' | sort -n | tail -1 || true)"
  if [[ -n "${PENDING_PEAK}" && "${PENDING_PEAK}" -gt 50 ]]; then
    echo "FAIL: zhifa hot-only pending queue peak ${PENDING_PEAK} (>50)" >&2
    exit 1
  fi
  rm -f "${ZHIFA_LOG}"

  echo "-- layer verify zhifa mrg --"
  cargo run -q -p mei-lang-server --bin mei-toolchain -- \
    layer verify --workspace "${WS_SSPBJW}" --app zhifa --layer mrg
fi

echo "block/layer gates ok"
