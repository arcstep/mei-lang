#!/usr/bin/env bash
# Host 性能回归检查：采样当前结果并与 latest / pinned baseline 比较。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROJECTS_ROOT="$(cd "${MEI_LANG_ROOT}/.." && pwd)"

BASE_URL="${MEI_SERVER_URL:-http://127.0.0.1:9527}"
SCENARIO_FILE="${MEI_SCENARIO_FILE:-${SCRIPT_DIR}/../perf/scenarios/template.app.json}"
LEDGER_PATH="${MEI_LEDGER_PATH:-${PROJECTS_ROOT}/docs/archive/mei-lang-v1/benchmarks/template-app-perf-ledger.jsonl}"
BASELINE_FILE="${MEI_BASELINE_FILE:-}"
ENVIRONMENT_NAME="${MEI_ENV:-local_release_noauth}"
COMPARE_MODE="${MEI_COMPARE_MODE:-auto}"
REPORT_OUTPUT="${MEI_REPORT_OUTPUT:-}"

TMP_SAMPLE="$(mktemp)"
trap 'rm -f "${TMP_SAMPLE}"' EXIT

echo "==> 采样当前 host perf"
node "${SCRIPT_DIR}/../perf/host-perf-sample.mjs" \
  --server-url "${BASE_URL}" \
  --scenario-file "${SCENARIO_FILE}" \
  --output "${TMP_SAMPLE}" \
  --environment "${ENVIRONMENT_NAME}" \
  --no-append

echo ""
echo "==> 与台账基线比较"

REPORT_ARGS=(
  --sample "${TMP_SAMPLE}"
  --scenario-file "${SCENARIO_FILE}"
  --ledger "${LEDGER_PATH}"
  --mode "${COMPARE_MODE}"
)

if [[ -n "${BASELINE_FILE}" ]]; then
  REPORT_ARGS+=(--baseline-file "${BASELINE_FILE}")
fi

if [[ -n "${REPORT_OUTPUT}" ]]; then
  REPORT_ARGS+=(--report-output "${REPORT_OUTPUT}" --format markdown)
fi

node "${SCRIPT_DIR}/../perf/host-perf-report.mjs" "${REPORT_ARGS[@]}"
#!/usr/bin/env bash
# Host 性能回归检查：采样当前结果并与最近基线比较。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROJECTS_ROOT="$(cd "${MEI_LANG_ROOT}/.." && pwd)"

BASE_URL="${MEI_SERVER_URL:-http://127.0.0.1:9527}"
SCENARIO_FILE="${MEI_SCENARIO_FILE:-${SCRIPT_DIR}/../perf/scenarios/template.app.json}"
LEDGER_PATH="${MEI_LEDGER_PATH:-${PROJECTS_ROOT}/docs/archive/mei-lang-v1/benchmarks/template-app-perf-ledger.jsonl}"
ENVIRONMENT_NAME="${MEI_ENV:-local_release_noauth}"

TMP_SAMPLE="$(mktemp)"
trap 'rm -f "${TMP_SAMPLE}"' EXIT

echo "==> 采样当前 host perf"
node "${SCRIPT_DIR}/../perf/host-perf-sample.mjs" \
  --server-url "${BASE_URL}" \
  --scenario-file "${SCENARIO_FILE}" \
  --output "${TMP_SAMPLE}" \
  --environment "${ENVIRONMENT_NAME}" \
  --no-append

echo ""
echo "==> 与台账基线比较"

node --input-type=module - "${LEDGER_PATH}" "${TMP_SAMPLE}" <<'EOF'
import fs from "node:fs/promises";

const ledgerPath = process.argv[2];
const samplePath = process.argv[3];

const RED_THRESHOLDS = {
  html_ready_ratio: 0.15,
  stable_render_ratio: 0.15,
  interactive_ratio: 0.15,
  compile_ratio: 0.2,
  metric_total_ratio: 0.2,
  request_count_ratio: 0.3,
};
const YELLOW_THRESHOLDS = {
  hydrate_ratio: 0.15,
  eval_ratio: 0.15,
  metric_request_start_ratio: 0.25,
  metric_request_total_ratio: 0.25,
  compile_cache_lookup_ratio: 0.25,
  compile_cache_lock_wait_ratio: 0.25,
};

const history = await readJsonlOrEmpty(ledgerPath);
const current = await readJsonlOrEmpty(samplePath);

if (current.length === 0) {
  throw new Error("sample output is empty, cannot perform regression checks");
}

let hasRed = false;
let yellowCount = 0;
let noBaselineCount = 0;

for (const entry of current) {
  const prev = findLastHistory(history, entry);
  if (!prev) {
    noBaselineCount += 1;
    console.log(`- ${entry.scenario_id}: 无历史基线，跳过对比`);
    continue;
  }
  const { red, yellow } = compareEntry(prev, entry);
  if (red.length > 0) {
    hasRed = true;
    console.log(`- ${entry.scenario_id}: RED ${red.join("; ")}`);
  } else if (yellow.length > 0) {
    yellowCount += 1;
    console.log(`- ${entry.scenario_id}: YELLOW ${yellow.join("; ")}`);
  } else {
    console.log(`- ${entry.scenario_id}: OK`);
  }
}

console.log("");
console.log(
  `summary: red=${hasRed ? 1 : 0} yellow=${yellowCount} no_baseline=${noBaselineCount} checked=${current.length}`
);

if (hasRed) {
  process.exit(1);
}

function findLastHistory(list, currentEntry) {
  for (let index = list.length - 1; index >= 0; index -= 1) {
    const row = list[index];
    if (
      row.scenario_id === currentEntry.scenario_id &&
      row.run_kind === currentEntry.run_kind &&
      String(row.environment || "") === String(currentEntry.environment || "")
    ) {
      return row;
    }
  }
  return null;
}

function compareEntry(prev, curr) {
  const red = [];
  const yellow = [];
  const prevPerf = prev.perf || {};
  const currPerf = curr.perf || {};

  pushRatioRegression(
    red,
    "handler_html_ready_ms",
    prevPerf.handler_html_ready_ms,
    currPerf.handler_html_ready_ms,
    RED_THRESHOLDS.html_ready_ratio
  );
  pushRatioRegression(
    red,
    "first_stable_render_ms",
    prevPerf.first_stable_render_ms,
    currPerf.first_stable_render_ms,
    RED_THRESHOLDS.stable_render_ratio
  );
  pushRatioRegression(
    red,
    "first_interactive_ms",
    prevPerf.first_interactive_ms,
    currPerf.first_interactive_ms,
    RED_THRESHOLDS.interactive_ratio
  );
  pushRatioRegression(
    red,
    "compile_ms",
    prevPerf.compile_ms,
    currPerf.compile_ms,
    RED_THRESHOLDS.compile_ratio
  );
  pushRatioRegression(
    red,
    "metric_total_ms",
    prevPerf.metric_total_ms,
    currPerf.metric_total_ms,
    RED_THRESHOLDS.metric_total_ratio
  );
  pushRatioRegression(
    red,
    "metrics_request_count",
    prevPerf.metrics_request_count,
    currPerf.metrics_request_count,
    RED_THRESHOLDS.request_count_ratio
  );

  if (
    toFinite(prevPerf.manage_dataset_resources) <= 9 &&
    toFinite(currPerf.manage_dataset_resources) >= 15
  ) {
    red.push(
      `manage_dataset_resources ${toFinite(prevPerf.manage_dataset_resources)} -> ${toFinite(currPerf.manage_dataset_resources)}`
    );
  }

  if (
    curr.run_kind === "warm" &&
    toFinite(prevPerf.compile_cache_hit) === 0 &&
    toFinite(currPerf.compile_cache_hit) === 0
  ) {
    red.push("warm compile_cache_hit 连续为 0");
  }

  if (toFinite(prevPerf.metric_response_cache_hit) === 1 && toFinite(currPerf.metric_response_cache_hit) === 0) {
    yellow.push("metric_response_cache_hit 1 -> 0");
  }

  if (
    toFinite(prevPerf.stable_render_within_window) === 1 &&
    toFinite(currPerf.stable_render_within_window) === 0
  ) {
    yellow.push("stable_render_within_window 1 -> 0");
  }

  if (
    toFinite(prevPerf.interactive_within_window) === 1 &&
    toFinite(currPerf.interactive_within_window) === 0
  ) {
    yellow.push("interactive_within_window 1 -> 0");
  }

  pushRatioRegression(
    yellow,
    "metric_hydrate_datasets_ms",
    prevPerf.metric_hydrate_datasets_ms,
    currPerf.metric_hydrate_datasets_ms,
    YELLOW_THRESHOLDS.hydrate_ratio
  );
  pushRatioRegression(
    yellow,
    "metric_eval_ms",
    prevPerf.metric_eval_ms,
    currPerf.metric_eval_ms,
    YELLOW_THRESHOLDS.eval_ratio
  );
  pushRatioRegression(
    yellow,
    "first_metric_request_start_ms",
    prevPerf.first_metric_request_start_ms,
    currPerf.first_metric_request_start_ms,
    YELLOW_THRESHOLDS.metric_request_start_ratio
  );
  pushRatioRegression(
    yellow,
    "metric_request_total_ms",
    prevPerf.metric_request_total_ms,
    currPerf.metric_request_total_ms,
    YELLOW_THRESHOLDS.metric_request_total_ratio
  );
  pushRatioRegression(
    yellow,
    "compile_cache_lookup_ms",
    prevPerf.compile_cache_lookup_ms,
    currPerf.compile_cache_lookup_ms,
    YELLOW_THRESHOLDS.compile_cache_lookup_ratio
  );

  const prevLockWait = bestFinite(prevPerf.metric_compile_cache_lock_wait_ms, prevPerf.dataset_compile_cache_lock_wait_ms);
  const currLockWait = bestFinite(currPerf.metric_compile_cache_lock_wait_ms, currPerf.dataset_compile_cache_lock_wait_ms);
  pushRatioRegression(yellow, "compile_cache_lock_wait_ms", prevLockWait, currLockWait, YELLOW_THRESHOLDS.compile_cache_lock_wait_ratio);

  return { red, yellow };
}

function pushRatioRegression(bucket, field, prev, curr, threshold) {
  const base = toFinite(prev);
  const now = toFinite(curr);
  if (!Number.isFinite(base) || !Number.isFinite(now) || base <= 0) {
    return;
  }
  const ratio = (now - base) / base;
  if (ratio >= threshold) {
    bucket.push(`${field} ${base} -> ${now} (+${Math.round(ratio * 100)}%)`);
  }
}

function bestFinite(...values) {
  for (const value of values) {
    const n = toFinite(value);
    if (Number.isFinite(n)) {
      return n;
    }
  }
  return NaN;
}

function toFinite(value) {
  const n = Number(value);
  return Number.isFinite(n) ? n : NaN;
}

async function readJsonlOrEmpty(filePath) {
  try {
    const raw = await fs.readFile(filePath, "utf8");
    return raw
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
      .map((line, index) => {
        try {
          return JSON.parse(line);
        } catch (error) {
          throw new Error(`invalid JSONL at ${filePath}:${index + 1} ${error}`);
        }
      });
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }
}
EOF

echo ""
echo "Host perf regress check passed"
