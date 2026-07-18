#!/usr/bin/env bash
# Gate 0 · Stage 架构基线冻结统一入口
# 协调现有 compiler / host-graph runtime / Playwright / schema 台账检查。
# 用法：
#   ./scripts/check-stage-architecture-baseline.sh
#   MEI_STAGE_BASELINE_SKIP_BROWSER=1 ./scripts/check-stage-architecture-baseline.sh
#   MEI_STAGE_BASELINE_UPDATE=1 ...   # 透传至 Rust fixture 更新（慎用）
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WS_CANDIDATE="${ROOT}/../workspaces/ws-demo-v2"
# Soft-skip when sibling demo workspace is absent (standalone mei-lang clone).
# Do NOT `cd` before this check — `set -e` would otherwise die on missing path.
if [[ ! -d "${WS_CANDIDATE}" ]]; then
  printf 'skip Gate 0: ws-demo-v2 not found at %s\n' "${WS_CANDIDATE}"
  exit 0
fi
WS_ROOT="$(cd "${WS_CANDIDATE}" && pwd)"
DOCS_CANDIDATE="${ROOT}/../docs"
if [[ -d "${DOCS_CANDIDATE}" ]]; then
  DOCS_ROOT="$(cd "${DOCS_CANDIDATE}" && pwd)"
else
  DOCS_ROOT="${ROOT}/tmp"
fi
EVIDENCE_DIR="${MEI_STAGE_BASELINE_EVIDENCE_DIR:-${DOCS_ROOT}/mei-lang-v2/assets/phase-0-golden}"
BASE_URL="${MEI_BASE_URL:-http://127.0.0.1:9527}"
SKIP_BROWSER="${MEI_STAGE_BASELINE_SKIP_BROWSER:-0}"
SKIP_BUILD="${MEI_STAGE_BASELINE_SKIP_BUILD:-0}"
SKIP_RUST="${MEI_STAGE_BASELINE_SKIP_RUST:-0}"
REPEAT="${MEI_STAGE_BASELINE_REPEAT:-2}"
HOST_STARTED_BY_US=0
CREATED_LAUNCH_FILES=()

CONFIGS=(${MEI_STAGE_BASELINE_CONFIGS:-grid-demo mei-tutorial-only panels-dev mini-park})
# shellcheck disable=SC2206
CONFIGS=(${CONFIGS[@]})

log() { printf '\n==> %s\n' "$*"; }
die() { printf 'Gate 0 FAIL: %s\n' "$*" >&2; exit 1; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing command: $1"
}

run_rust_baseline() {
  local label="$1"
  log "[${label}] compiler Graph baseline"
  (
    cd "${ROOT}"
    if [[ "${MEI_STAGE_BASELINE_UPDATE:-0}" == "1" ]]; then
      MEI_UPDATE_STAGE_BASELINE=1 cargo test -p mei-compiler-tests stage_architecture_baseline -- --nocapture
    else
      cargo test -p mei-compiler-tests stage_architecture_baseline -- --nocapture
    fi
  ) || die "compiler baseline failed (${label}). Fix app/fixture or update with MEI_UPDATE_STAGE_BASELINE=1"

  log "[${label}] runtime assemble baseline"
  (
    cd "${ROOT}"
    if [[ "${MEI_STAGE_BASELINE_UPDATE:-0}" == "1" ]]; then
      MEI_UPDATE_STAGE_BASELINE=1 cargo test -p mei-host-graph stage_architecture_runtime_baseline -- --nocapture
    else
      cargo test -p mei-host-graph stage_architecture_runtime_baseline -- --nocapture
    fi
  ) || die "runtime baseline failed (${label}). Fixture 路径: crates/mei-host-graph/tests/fixtures/stage_architecture/{app}__{scene}.runtime.json"
}

check_schema_inventory() {
  log "schema / legacy inventory vs 0106"
  node "${ROOT}/scripts/check/check-stage-architecture-schema-inventory.mjs" \
    || die "schema inventory mismatch — see 0106 §4"
}

  check_gate_c_tests() {
  log "Gate C format / schema / generation tests"
  (
    cd "${ROOT}"
    cargo test -p mei-host-graph --lib schema_gate -- --nocapture
    cargo test -p mei-host-graph --lib gate_c_tests -- --nocapture
    cargo test -p mei-host-graph --lib semantic_revision_digest_changes_when_scene_id -- --nocapture
    cargo test -p mei-host-graph --lib discovers_stage -- --nocapture
    cargo test -p mei-host-graph --lib eval_slot_group_cache_key_includes_schema -- --nocapture
  ) || die "Gate C host-graph tests failed"
  (
    cd "${ROOT}"
    cargo test -p mei-lang-kernel --lib read_build_manifest_ -- --nocapture
    cargo test -p mei-lang-kernel --lib rollback_build_requires_previous -- --nocapture
  ) || die "Gate C kernel tests failed"
}

wait_host_ready() {
  local url="$1"
  local tries="${2:-90}"
  local i
  for ((i = 1; i <= tries; i++)); do
    if curl -fsS "${url}/api/host/ready" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  return 1
}

wait_access_ready() {
  local url="$1"
  local app="$2"
  local scene="$3"
  local tries="${4:-120}"
  local i body
  for ((i = 1; i <= tries; i++)); do
    body="$(curl -fsS "${url}/api/host/access-readiness?app=${app}&scene=${scene}" 2>/dev/null || true)"
    if printf '%s' "${body}" | grep -q '"ready"[[:space:]]*:[[:space:]]*true'; then
      return 0
    fi
    sleep 3
  done
  printf 'last readiness body for %s/%s: %s\n' "${app}" "${scene}" "${body}" >&2
  return 1
}

stop_host() {
  if [[ -x "${WS_ROOT}/deploy/stop.sh" ]]; then
    (cd "${WS_ROOT}" && ./deploy/stop.sh) || true
  fi
  HOST_STARTED_BY_US=0
}

start_host_config() {
  local config="$1"
  log "start host config=${config}"
  stop_host
  sleep 1
  nohup bash -c "cd \"${WS_ROOT}\" && ./deploy/dev.sh --cargo --config \"${config}\"" \
    >/tmp/mei-gate0-${config}.log 2>&1 &
  HOST_STARTED_BY_US=1
  wait_host_ready "${BASE_URL}" 120 || {
    tail -n 80 "/tmp/mei-gate0-${config}.log" >&2 || true
    die "host not ready for config=${config} (log: /tmp/mei-gate0-${config}.log)"
  }
}

apps_for_config() {
  case "$1" in
    grid-demo) echo "mini-grid:home metric-grid:home mini-data:home" ;;
    mei-tutorial-only) echo "mei-tutorial:intro" ;;
    panels-dev) echo "zhifa:home mini-data:home mini-data:supervision" ;;
    mini-park) echo "mini-park:home mini-park:home_2d" ;; # scene_id=home_2d；URL=/apps/mini-park/home-2d
    *) die "unknown config $1" ;;
  esac
}

# 0537：控制面启动后须 POST /api/host/apps/{id}/start。
# 无 launch 文件的 Golden 会临时 ensure-default；结束后删除，保持 workspaces 源码洁净。

ensure_and_start_app() {
  local app="$1"
  local scene_hint="${2:-home}"
  local launch_dir="${WS_ROOT}/apps/${app}/launch"
  local launch_path="${launch_dir}/default.json"
  if [[ ! -f "${launch_path}" ]]; then
    log "write temp default launch for ${app} (hotScenes=${scene_hint})"
    mkdir -p "${launch_dir}"
    cat >"${launch_path}" <<JSON
{
  "schemaVersion": "mei-app-launch-v1",
  "appId": "${app}",
  "generation": "current",
  "runtimePlan": { "defaultMode": "lazy", "apps": {} },
  "warmup": {
    "enabled": true,
    "apps": {
      "${app}": { "hotScenes": ["${scene_hint}"] }
    }
  }
}
JSON
    CREATED_LAUNCH_FILES+=("${launch_path}")
  fi
  log "start app ${app} (launch=default)"
  curl -fsS -X POST "${BASE_URL}/api/host/apps/${app}/start" \
    -H 'content-type: application/json' \
    -d '{"config":"default"}' >/dev/null \
    || die "app start failed for ${app}"
}

cleanup_temp_launches() {
  local f
  for f in "${CREATED_LAUNCH_FILES[@]:-}"; do
    [[ -z "${f}" ]] && continue
    if [[ -f "${f}" ]]; then
      rm -f "${f}"
      rmdir "$(dirname "${f}")" 2>/dev/null || true
      echo "removed temp launch ${f}"
    fi
  done
  CREATED_LAUNCH_FILES=()
}

run_browser_for_config() {
  local config="$1"
  local pair app scene
  local -a started_apps=()
  start_host_config "${config}"
  for pair in $(apps_for_config "${config}"); do
    app="${pair%%:*}"
    scene="${pair##*:}"
    if [[ " ${started_apps[*]} " != *" ${app} "* ]]; then
      ensure_and_start_app "${app}" "${scene}"
      started_apps+=("${app}")
    fi
    log "wait access-readiness ${app}/${scene}"
    wait_access_ready "${BASE_URL}" "${app}" "${scene}" 180 \
      || die "access not ready app=${app} scene=${scene} config=${config}"
  done

  log "playwright config=${config}"
  (
    cd "${ROOT}"
    MEI_TEST_SKIP_SERVER=1 \
    MEI_TEST_BASE_URL="${BASE_URL}" \
    MEI_E2E_BASE_URL="${BASE_URL}" \
    MEI_STAGE_BASELINE_CONFIG="${config}" \
    MEI_STAGE_BASELINE_CAPTURE=1 \
    MEI_STAGE_BASELINE_EVIDENCE_DIR="${EVIDENCE_DIR}" \
    npx playwright test e2e/stage-architecture-baseline.spec.mjs --reporter=list
  ) || die "browser baseline failed for config=${config}"

  cleanup_temp_launches
}

check_workspaces_source_clean() {
  log "workspaces 源码洁净（忽略 env/build/var）"
  (
    cd "${WS_ROOT}"
    # 仅关注 apps/**/src 与 configs；运行产物应被 ignore
    dirty="$(git status --porcelain -- apps configs 2>/dev/null || true)"
    if [[ -n "${dirty}" ]]; then
      printf '%s\n' "${dirty}" >&2
      die "workspaces apps/configs 有未提交改动；Phase 0 不得迁移源码"
    fi
    echo "workspaces apps/configs clean"
  )
}

check_zero_warning_build() {
  [[ "${SKIP_BUILD}" == "1" ]] && { log "skip cargo/assets build"; return 0; }
  log "cargo build -p mei-lang-server (0 warning)"
  local out
  out="$(cd "${ROOT}" && cargo build -p mei-lang-server 2>&1)" || {
    printf '%s\n' "${out}" >&2
    die "cargo build -p mei-lang-server failed"
  }
  if printf '%s\n' "${out}" | grep -E -q '^warning:'; then
    printf '%s\n' "${out}" >&2
    die "cargo build produced warnings"
  fi
  echo "cargo build OK (0 warning)"

  log "related Rust baseline tests already exercised; spot-check host-graph + compiler-tests filters"
}

write_gate_report() {
  mkdir -p "${EVIDENCE_DIR}"
  local report="${EVIDENCE_DIR}/gate0-report.json"
  python3 - "${report}" "${BASE_URL}" <<'PY'
import json, sys, os, glob, datetime
report_path, base = sys.argv[1], sys.argv[2]
evidence = os.path.dirname(report_path)
shots = sorted(glob.glob(os.path.join(evidence, "*.png")))
metas = sorted(glob.glob(os.path.join(evidence, "*.meta.json")))
payload = {
  "gate": "0",
  "generatedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
  "baseUrl": base,
  "screenshotCount": len(shots),
  "metaCount": len(metas),
  "shots": [os.path.basename(p) for p in shots],
}
with open(report_path, "w", encoding="utf-8") as f:
    json.dump(payload, f, ensure_ascii=False, indent=2)
    f.write("\n")
print("wrote", report_path)
PY
}

cleanup() {
  cleanup_temp_launches || true
  if [[ "${HOST_STARTED_BY_US}" -eq 1 ]]; then
    stop_host
  fi
}
trap cleanup EXIT

main() {
  require_cmd cargo
  require_cmd node
  require_cmd curl
  require_cmd python3

  if [[ ! -d "${WS_ROOT}/apps/mini-grid" ]]; then
    printf 'skip Gate 0: ws-demo-v2 incomplete (no apps/mini-grid) at %s\n' "${WS_ROOT}"
    exit 0
  fi

  log "Gate 0 start (repeat=${REPEAT}, skip_browser=${SKIP_BROWSER}, skip_rust=${SKIP_RUST})"

  if [[ "${SKIP_RUST}" != "1" ]]; then
    local i
    for ((i = 1; i <= REPEAT; i++)); do
      run_rust_baseline "pass-${i}/${REPEAT}"
    done
    check_schema_inventory
    check_gate_c_tests
  else
    log "rust baselines skipped (MEI_STAGE_BASELINE_SKIP_RUST=1)"
    check_schema_inventory
    check_gate_c_tests
  fi

  if [[ "${SKIP_BROWSER}" != "1" ]]; then
    require_cmd npx
    mkdir -p "${EVIDENCE_DIR}"
    local cfg
    for cfg in "${CONFIGS[@]}"; do
      run_browser_for_config "${cfg}"
    done
    write_gate_report
    stop_host
  else
    log "browser probes skipped (MEI_STAGE_BASELINE_SKIP_BROWSER=1)"
  fi

  check_workspaces_source_clean
  check_zero_warning_build

  log "Gate 0 PASS"
  echo "evidence: ${EVIDENCE_DIR}"
  echo "update fixtures: MEI_UPDATE_STAGE_BASELINE=1 cargo test -p mei-compiler-tests stage_architecture_baseline -- --nocapture"
  echo "update fixtures: MEI_UPDATE_STAGE_BASELINE=1 cargo test -p mei-host-graph stage_architecture_runtime_baseline -- --nocapture"
}

main "$@"
