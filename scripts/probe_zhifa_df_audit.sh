#!/usr/bin/env bash
# Probe zhifa DF/SQL audit scenarios Z1–Z5 (local / temp env only; not public CI).
# Usage:
#   BASE_URL=http://127.0.0.1:19531 ./scripts/probe_zhifa_df_audit.sh
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:19531}"
OUT_DIR="${OUT_DIR:-/tmp/zhifa-df-audit-probe}"
mkdir -p "$OUT_DIR"

post_query() {
  local name="$1"
  local body="$2"
  local code
  code=$(curl -sS -o "$OUT_DIR/${name}.json" -w "%{http_code}" --max-time 180 \
    -X POST "$BASE_URL/api/datasets/query/zhifa" \
    -H 'content-type: application/json' \
    -d "$body" || echo fail)
  echo "QUERY ${name} http=${code}"
  echo "$code" >"$OUT_DIR/${name}.http"
}

post_metrics() {
  local name="$1"
  local body="$2"
  local code
  code=$(curl -sS -o "$OUT_DIR/${name}.json" -w "%{http_code}" --max-time 180 \
    -X POST "$BASE_URL/api/datasets/metrics/zhifa" \
    -H 'content-type: application/json' \
    -d "$body" || echo fail)
  echo "METRICS ${name} http=${code}"
  echo "$code" >"$OUT_DIR/${name}.http"
}

echo "=== wait home ==="
for i in $(seq 1 60); do
  code=$(curl -sS -o /dev/null -w "%{http_code}" "$BASE_URL/apps/zhifa/home" || true)
  echo "try=$i home=$code"
  [[ "$code" == "200" ]] && break
  sleep 1
done

# Z1 warm + measure
Z1_BODY='{"scene_id":"effect_analytics_page","target":"src/scene/home/t1/region-right-rail/section-effect/plane-effect-analytics.mei","dataset_id":"effectiveness_analytics_list","metric_id":"effectiveness_analytics::__scalar_rowset__","page":1,"page_size":20,"filters":{},"query_state":{"filters":{}},"full":false,"summary":false}'
echo "=== Z1 warm ==="
post_query z1_warm "$Z1_BODY"
echo "=== Z1 measure ==="
post_query z1 "$Z1_BODY"

# Z2 KPIs
Z2_BODY='{"scene_id":"home","target":"src/scenes/home.mei","dataset_id":"effectiveness_analytics_list","metric_ids":["effectiveness_transfer_clue_count","effectiveness_filing_count","effectiveness_party_gov_sanction_count"],"filters":{},"query_state":{"filters":{}}}'
echo "=== Z2 ==="
post_metrics z2 "$Z2_BODY"

# Z3 warning list drilldown
Z3_BODY='{"scene_id":"warnings_analytics_page","target":"src/scene/home/t1/region-right-rail/section-warning/plane-warnings.mei","dataset_id":"warning_list","metric_id":"warnings_count::__scalar_rowset__","page":1,"page_size":20,"filters":{},"query_state":{"filters":{}},"full":false,"summary":false}'
echo "=== Z3 ==="
post_query z3 "$Z3_BODY"

# Z4 issue handling list
Z4_BODY='{"scene_id":"issue_handling_analytics_page","target":"src/scene/home/t1/region-right-rail/section-issue/plane-issue-handling.mei","dataset_id":"issue_handling_list","metric_id":"issue_handling_analytics::__scalar_rowset__","page":1,"page_size":20,"filters":{},"query_state":{"filters":{}},"full":false,"summary":false}'
echo "=== Z4 ==="
post_query z4 "$Z4_BODY"

# Z5 large table page (warning_list page 2)
Z5_BODY='{"scene_id":"warnings_analytics_page","target":"src/scene/home/t1/region-right-rail/section-warning/plane-warnings.mei","dataset_id":"warning_list","metric_id":"warnings_count::__scalar_rowset__","page":2,"page_size":20,"filters":{},"query_state":{"filters":{}},"full":false,"summary":false}'
echo "=== Z5 ==="
post_query z5 "$Z5_BODY"

echo "=== summary ==="
for f in z1_warm z1 z2 z3 z4 z5; do
  printf '%s %s\n' "$f" "$(cat "$OUT_DIR/$f.http" 2>/dev/null || echo missing)"
done
echo "bodies in $OUT_DIR"
