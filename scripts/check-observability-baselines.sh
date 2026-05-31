#!/usr/bin/env bash
# 代表性 examples 的观测基线与错误签名回归。
set -euo pipefail

BASE_URL="${MEI_BASE_URL:-http://127.0.0.1:9527}"
APP_ID="${MEI_APP_ID:-examples/ds/04-data-table-features}"
TARGET_FILE="${MEI_TARGET_FILE:-main.mei}"
DATASET_ID="${MEI_DATASET_ID:-orders}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECKBENCH="${SCRIPT_DIR}/check-observability-readonly.sh"

if [[ ! -x "${CHECKBENCH}" ]]; then
  echo "missing executable: ${CHECKBENCH}" >&2
  exit 1
fi

run_readonly_case() {
  local scene_id="$1"
  echo "==> readonly baseline scene=${scene_id}"
  MEI_BASE_URL="${BASE_URL}" \
  MEI_APP_ID="${APP_ID}" \
  MEI_SCENE_ID="${scene_id}" \
  MEI_TARGET_FILE="${TARGET_FILE}" \
  MEI_DATASET_ID="${DATASET_ID}" \
  "${CHECKBENCH}"
  echo ""
}

echo "==> [1/3] 正向基线：Manage 数据路径"
run_readonly_case "manage_server_paging"

echo "==> [2/3] 正向基线：Access explain 合同路径"
run_readonly_case "metric_explain_access"

echo "==> [3/3] 反向基线：错误签名"
RESP_MISSING_SCENE="$(curl -sS -X POST "${BASE_URL}/api/datasets/query/${APP_ID}" \
  -H 'content-type: application/json' \
  -d "{
    \"dataset_id\": \"${DATASET_ID}\"
  }")"

python3 - "${RESP_MISSING_SCENE}" <<'PY'
import json
import sys

payload = json.loads(sys.argv[1])
msg = payload.get("error") or payload.get("message") or str(payload)
assert "scene_id" in msg and "require" in msg.lower(), f"unexpected missing-scene error: {payload}"
print("OK: missing scene_id error signature")
PY

RESP_MISSING_QUERY_STATE="$(curl -sS -X POST "${BASE_URL}/api/datasets/metrics/${APP_ID}" \
  -H 'content-type: application/json' \
  -d "{
    \"scene_id\": \"manage_server_paging\",
    \"dataset_id\": \"${DATASET_ID}\",
    \"filters\": {\"status\": \"进行中\"}
  }")"

python3 - "${RESP_MISSING_QUERY_STATE}" <<'PY'
import json
import sys

payload = json.loads(sys.argv[1])
msg = payload.get("error") or payload.get("message") or str(payload)
assert "query_state" in msg and "require" in msg.lower(), f"unexpected missing-query_state error: {payload}"
print("OK: missing query_state error signature")
PY

echo ""
echo "Observability baselines OK"
