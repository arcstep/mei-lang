#!/usr/bin/env bash
# ws-hello tier-1/tier-2 gate: fast MCG + block path before spbjw regression.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

echo "== block/layer hello gates =="

echo "-- cargo build (prebuild/block SSOT) --"
cargo build -p mei-lang-server

WS_HELLO="${ROOT}/../workspaces/ws-hello"
if [[ ! -d "${WS_HELLO}" ]]; then
  echo "skip workspace gates: ${WS_HELLO} not found"
  exit 0
fi

HELLO_START=$(date +%s)
echo "-- prebuild hello (hot-only) --"
cargo run -q -p mei-lang-server --bin mei-toolchain -- \
  prebuild --workspace "${WS_HELLO}" --app hello --hot-only --json \
  | (command -v jq >/dev/null 2>&1 && jq -e '.warning_count == 0' || cat)
HELLO_ELAPSED=$(( $(date +%s) - HELLO_START ))
if [[ "${HELLO_ELAPSED}" -gt 30 ]]; then
  echo "WARN: hello hot-only took ${HELLO_ELAPSED}s (target <30s)" >&2
fi

echo "-- layer verify hello mcg --"
cargo run -q -p mei-lang-server --bin mei-toolchain -- \
  layer verify --workspace "${WS_HELLO}" --app hello --layer mcg

echo "-- block compile home (assemble-only) --"
cargo run -q -p mei-lang-server --bin mei-toolchain -- \
  block compile --workspace "${WS_HELLO}" --app hello \
  --node scene_payload:src/scenes/home.mei --assemble-only

CATALOG_START=$(date +%s)
echo "-- prebuild _stock-catalog (hot-only) --"
CATALOG_JSON="$(mktemp)"
cargo run -q -p mei-lang-server --bin mei-toolchain -- \
  prebuild --workspace "${WS_HELLO}" --app _stock-catalog --hot-only --json-full \
  > "${CATALOG_JSON}"
if command -v jq >/dev/null 2>&1; then
  jq -e '[.apps[].warnings | length] | add // 0 | . == 0' "${CATALOG_JSON}" >/dev/null
  EXPANSION="$(jq -r '.apps[0].diagnostics.expansion_ratio // 0' "${CATALOG_JSON}")"
  if awk "BEGIN { exit !(${EXPANSION} < 2) }"; then
    echo "_stock-catalog expansion_ratio=${EXPANSION} ok"
  else
    echo "WARN: _stock-catalog expansion_ratio=${EXPANSION} (target <2)" >&2
  fi
fi
rm -f "${CATALOG_JSON}"
CATALOG_ELAPSED=$(( $(date +%s) - CATALOG_START ))
if [[ "${CATALOG_ELAPSED}" -gt 60 ]]; then
  echo "WARN: _stock-catalog hot-only took ${CATALOG_ELAPSED}s (target <60s)" >&2
fi

PERF_LAB_START=$(date +%s)
echo "-- prebuild _perf-lab (Tier1.5 hot-only + diagnostics) --"
PERF_LAB_JSON="$(mktemp)"
cargo run -q -p mei-lang-server --bin mei-toolchain -- \
  prebuild --workspace "${WS_HELLO}" --app _perf-lab --hot-only --json-full \
  > "${PERF_LAB_JSON}"
if command -v jq >/dev/null 2>&1; then
  jq -e '[.apps[].warnings | length] | add // 0 | . == 0' "${PERF_LAB_JSON}" >/dev/null
  EXPANSION="$(jq -r '.apps[0].diagnostics.expansion_ratio // 0' "${PERF_LAB_JSON}")"
  SCOPES="$(jq -r '.apps[0].diagnostics.plan_nodes.manifest_compile_scope_nodes // 0' "${PERF_LAB_JSON}")"
  if awk "BEGIN { exit !(${EXPANSION} < 3) }"; then
    echo "_perf-lab expansion_ratio=${EXPANSION} ok (target <3; P1 goal <2)"
  else
    echo "WARN: _perf-lab expansion_ratio=${EXPANSION} (target <3)" >&2
  fi
  if [[ "${SCOPES}" -le 100 ]]; then
    echo "_perf-lab manifest_compile_scope_nodes=${SCOPES} ok"
  else
    echo "WARN: _perf-lab scope explosion scopes=${SCOPES} (target <=100)" >&2
  fi
fi
rm -f "${PERF_LAB_JSON}"
PERF_LAB_ELAPSED=$(( $(date +%s) - PERF_LAB_START ))
if [[ "${PERF_LAB_ELAPSED}" -gt 30 ]]; then
  echo "WARN: _perf-lab hot-only took ${PERF_LAB_ELAPSED}s (target <30s)" >&2
fi

if [[ "${MEI_HELLO_PERF_LAB_FULL:-0}" == "1" ]]; then
  PERF_FULL_START=$(date +%s)
  echo "-- prebuild _perf-lab (full, nightly opt-in) --"
  PERF_FULL_JSON="$(mktemp)"
  cargo run -q -p mei-lang-server --bin mei-toolchain -- \
    prebuild --workspace "${WS_HELLO}" --app _perf-lab --json-full \
    > "${PERF_FULL_JSON}"
  if command -v jq >/dev/null 2>&1; then
    jq -e '[.apps[].warnings | length] | add // 0 | . == 0' "${PERF_FULL_JSON}" >/dev/null
  fi
  rm -f "${PERF_FULL_JSON}"
  PERF_FULL_ELAPSED=$(( $(date +%s) - PERF_FULL_START ))
  if [[ "${PERF_FULL_ELAPSED}" -gt 120 ]]; then
    echo "WARN: _perf-lab full took ${PERF_FULL_ELAPSED}s (target <120s)" >&2
  fi
fi

echo "block/layer hello gates ok"
