#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PROJECTION="$ROOT/crates/kernel/src/compile/projection_assembly"
FAIL=0

check_absent_in() {
  local file="$1"
  local pattern="$2"
  local label="$3"
  if rg -n "$pattern" "$file" >/dev/null 2>&1; then
    echo "kernel business boundary violation: $label still present in ${file#$ROOT/}" >&2
    FAIL=1
  fi
}

# projection_assembly — hardcoded analytics filter columns
METRIC_RS="$PROJECTION/metric.rs"
for item in \
  'ANALYTICS_FILTER_COLUMNS' \
  '预警等级' \
  'warningLevel'; do
  check_absent_in "$METRIC_RS" "$item" "$item"
done

# Platform defaults — should not hardcode customer workspace ids
for file in README.md .env.example server/src/gis_config.rs stock/components/gis/layer-spec.js; do
  check_absent_in "$ROOT/$file" 'shapingba-z10-16' 'shapingba-z10-16 default'
  check_absent_in "$ROOT/$file" 'ws-spbjw' 'ws-spbjw in platform default'
done

# Frontend drilldown — no hardcoded customer scene paths or metric ids
DRILLDOWN="$ROOT/app/assets/spa-navigation/drilldown"
for item in \
  '05-监督预警' \
  'warnings_count' \
  '预警条数' \
  '承办部门|主责单位'; do
  check_absent_in "$DRILLDOWN" "$item" "$item in drilldown runtime"
done

# Stock table heuristics — no discipline-specific column names
FORMAT_JS="$ROOT/stock/components/dataset/table-runtime/format.js"
for item in \
  '预警条数' \
  '监督预警' \
  '预警类型' \
  '监督规则'; do
  check_absent_in "$FORMAT_JS" "$item" "$item in table format heuristics"
done

# Default perf script should not embed customer metric coordinates
check_absent_in "$ROOT/scripts/runtime-metric-perf.mjs" '问题办理' 'customer scene in default runtime-metric-perf'
check_absent_in "$ROOT/scripts/runtime-metric-perf.mjs" 'warnings_pending_count' 'customer metric in default runtime-metric-perf'

if [[ "$FAIL" -ne 0 ]]; then
  exit 1
fi

echo "kernel business boundary check passed"
