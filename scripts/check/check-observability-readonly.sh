#!/usr/bin/env bash
# 只读检查台：串行检查 projection/world context/metric perf/host meta。
set -euo pipefail

BASE_URL="${MEI_BASE_URL:-http://127.0.0.1:9527}"
APP_ID="${MEI_APP_ID:-examples/ds/04-data-table-features}"
SCENE_ID="${MEI_SCENE_ID:-manage_server_paging}"
TARGET_FILE="${MEI_TARGET_FILE:-main.mei}"
DATASET_ID="${MEI_DATASET_ID:-orders}"
MODE="${MEI_MODE:-manage}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

fetch_json() {
  local method="$1"
  local url="$2"
  local output="$3"
  local body="${4:-}"
  if [[ "${method}" == "GET" ]]; then
    curl -fsS "${url}" -H 'accept: application/json' >"${output}"
  else
    curl -fsS -X "${method}" "${url}" \
      -H 'accept: application/json' \
      -H 'content-type: application/json' \
      -d "${body}" >"${output}"
  fi
}

echo "==> [1/5] Projection 编译真值"
PROJECTION_JSON="${TMP_DIR}/projection.json"
fetch_json "GET" "${BASE_URL}/api/projection/${APP_ID}" "${PROJECTION_JSON}"
python3 - "${PROJECTION_JSON}" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    data = json.load(f)

app_id = data.get("app_id")
active_scene = data.get("active_scene")
active_target = data.get("active_target_file")
scene_routes = data.get("scene_routes") or []
diagnostics = data.get("diagnostics") or []
resources = data.get("resources") or []

print(f"app_id={app_id}")
print(f"active_scene={active_scene} active_target_file={active_target}")
print(f"scene_routes={len(scene_routes)} resources={len(resources)} diagnostics={len(diagnostics)}")

severity_counts = {}
for item in diagnostics:
    severity = (item or {}).get("severity", "unknown")
    severity_counts[severity] = severity_counts.get(severity, 0) + 1

if severity_counts:
    print("diagnostic_severity_counts=", severity_counts)

codes = []
for item in diagnostics:
    code = (item or {}).get("code")
    if code:
        codes.append(code)

if codes:
    print("diagnostic_codes_preview=", codes[:12])
PY
echo ""

echo "==> [2/5] World Context 资源与工具暴露"
WORLD_CONTEXT_JSON="${TMP_DIR}/world_context.json"
fetch_json \
  "GET" \
  "${BASE_URL}/api/world/context/${APP_ID}?scene_id=${SCENE_ID}&target_file=${TARGET_FILE}" \
  "${WORLD_CONTEXT_JSON}"
python3 - "${WORLD_CONTEXT_JSON}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as f:
    data = json.load(f)

snapshot = data.get("world_snapshot") or {}
inventory = data.get("resource_inventory") or {}
runtime_summary = data.get("runtime_summary") or {}
tools = data.get("query_tools") or []
tool_ids = [item.get("id") for item in tools if isinstance(item, dict) and item.get("id")]

print(f"scene_id={snapshot.get('scene_id')} world_id={snapshot.get('world_id')}")
print(f"world_resource_count={snapshot.get('world_resource_count')} inventory_total={inventory.get('total_items')}")
print(f"runtime_phase={runtime_summary.get('phase')} runtime_result={runtime_summary.get('result')}")
print("query_tool_ids=", tool_ids)

required = {"dataset_query", "dataset_metric", "resource_list", "resource_get", "resource_runtime_peek"}
missing = sorted(required - set(tool_ids))
if missing:
    raise SystemExit(f"missing world query tools: {missing}")
PY
echo ""

echo "==> [3/5] Host metric API 求值与缓存指标"
METRIC_JSON="${TMP_DIR}/metric.json"
fetch_json \
  "POST" \
  "${BASE_URL}/api/datasets/metrics/${APP_ID}" \
  "${METRIC_JSON}" \
  "$(cat <<EOF
{
  "scene_id": "${SCENE_ID}",
  "dataset_id": "${DATASET_ID}"
}
EOF
)"
python3 - "${METRIC_JSON}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as f:
    data = json.load(f)

perf = data.get("perf") or {}
metrics = data.get("metrics") or []
required_keys = [
    "compile_ms",
    "compile_cache_hit",
    "response_cache_hit",
    "total_ms",
]
missing = [key for key in required_keys if key not in perf]
if missing:
    raise SystemExit(f"metric perf missing keys: {missing}")

print(f"scene_id={data.get('scene_id')} dataset_id={data.get('dataset_id')} total_rows={data.get('total_rows')}")
print(f"metrics_returned={len(metrics)}")
print("perf_core=", {k: perf.get(k) for k in required_keys})

extra_keys = [
    "request_dag_nodes",
    "request_dag_hits",
    "eval_memo_hits",
    "eval_scope_key_hash",
]
present_extra = {k: perf.get(k) for k in extra_keys if k in perf}
if present_extra:
    print("perf_eval=", present_extra)
PY
echo ""

echo "==> [4/5] Agent 视角工具清单（Exposure）"
AGENT_CONTEXT_JSON="${TMP_DIR}/agent_context_preview.json"
fetch_json \
  "GET" \
  "${BASE_URL}/api/agent/context/preview?app_id=${APP_ID}&scene_id=${SCENE_ID}&target_file=${TARGET_FILE}&mode=build" \
  "${AGENT_CONTEXT_JSON}"
python3 - "${AGENT_CONTEXT_JSON}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as f:
    data = json.load(f)

native_tool_names = data.get("native_tool_names") or []
schema_version = data.get("query_schema_version")
resource_count = data.get("resource_inventory", {}).get("total_items")

print(f"query_schema_version={schema_version} resource_inventory_total={resource_count}")
print("native_tool_names_preview=", native_tool_names[:16])

required = {"dataset_query", "dataset_metric"}
missing = sorted(required - set(native_tool_names))
if missing:
    raise SystemExit(f"missing native tools for access flow: {missing}")
PY
echo ""

echo "==> [5/5] Host _mei.runtime_capabilities 注入检查"
HOST_HTML="${TMP_DIR}/host.html"
curl -fsS "${BASE_URL}/apps/${MODE}/${APP_ID}?scene=${SCENE_ID}&target_file=${TARGET_FILE}" >"${HOST_HTML}"
python3 - "${HOST_HTML}" "${APP_ID}" <<'PY'
import sys

path = sys.argv[1]
app_id = sys.argv[2]
html = open(path, "r", encoding="utf-8").read()

required_tokens = [
    '"runtime_capabilities"',
    f'"/api/datasets/query/{app_id}"',
    f'"/api/datasets/metrics/{app_id}"',
    '"scene_qualified":true',
]
missing = [token for token in required_tokens if token not in html]
if missing:
    raise SystemExit(f"host runtime capability tokens missing in html: {missing}")

print("host_runtime_capabilities=ok")
PY
echo ""

echo "Readonly checkbench OK"
