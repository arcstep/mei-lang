#!/usr/bin/env bash
# Probe all zhifa T2 page_instance typical queries (local / temp env; not public CI).
# Usage:
#   BASE_URL=http://127.0.0.1:19531 \
#   AUDIT_DIR=/path/to/instance/var/query-audit \
#   OUT_DIR=/tmp/zhifa-t2-df-audit-probe \
#   ./scripts/probe_zhifa_t2_df_audit.sh
#
# Each T2: warm once + measure once. Writes t2_summary.tsv / t2_summary.md with
# sql_chars alongside cold/hot wall_ms and (if AUDIT_DIR set) engine exec/total/lower_ms.
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:19531}"
OUT_DIR="${OUT_DIR:-/tmp/zhifa-t2-df-audit-probe}"
AUDIT_DIR="${AUDIT_DIR:-}"
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

echo "=== wait home ==="
for i in $(seq 1 60); do
  code=$(curl -sS -o /dev/null -w "%{http_code}" "$BASE_URL/apps/zhifa/home" || true)
  echo "try=$i home=$code"
  [[ "$code" == "200" ]] && break
  sleep 1
done

# --- Z1 regression (materialize target) ---
Z1_BODY='{"scene_id":"effect_analytics_page","target":"src/scene/home/t1/region-right-rail/section-effect/plane-effect-analytics.mei","dataset_id":"effectiveness_analytics_list","metric_id":"effectiveness_analytics::__scalar_rowset__","page":1,"page_size":20,"filters":{},"query_state":{"filters":{}},"full":false,"summary":false}'
echo "=== Z1 warm ==="
post_query z1_warm "$Z1_BODY"
echo "=== Z1 measure ==="
post_query z1 "$Z1_BODY"

# --- Full T2 matrix (34 page_instance) + timing summary ---
# shellcheck disable=SC2016
python3 - "$BASE_URL" "$OUT_DIR" "${AUDIT_DIR}" <<'PY'
import json, os, re, sys, time, urllib.request, urllib.error
from pathlib import Path

base_url, out_dir_s, audit_dir_s = sys.argv[1], sys.argv[2], sys.argv[3]
out_dir = Path(out_dir_s)
audit_dir = Path(audit_dir_s) if audit_dir_s else None
root = Path(os.environ.get(
    "ZHIFA_APP_ROOT",
    "/Users/xuehongwei/codeup/mei-projects/workspaces/ws-spbjw/apps/zhifa",
))

INDICATOR = {
    "plane-indicator-inspection-frequency.mei": (
        "indicator_inspection_frequency_analytics_page",
        ["inspection_frequency_reduction_rate"],
        "__world_metrics__::metrics/indicator-system.bundle.mei",
    ),
    "plane-indicator-warnings-verification.mei": (
        "indicator_warnings_verification_analytics_page",
        ["warnings_verification_rate"],
        "__world_metrics__::metrics/indicator-system.bundle.mei",
    ),
    "plane-indicator-rectification.mei": (
        "indicator_rectification_analytics_page",
        ["effectiveness_verified_rectification_rate"],
        "__world_metrics__::metrics/indicator-system.bundle.mei",
    ),
    "plane-indicator-penalty-revenue.mei": (
        "indicator_penalty_revenue_analytics_page",
        ["penalty_revenue_growth_rate"],
        "__world_metrics__::metrics/indicator-system.bundle.mei",
    ),
}


def post(path, body, name, retries=4):
    data = json.dumps(body).encode()
    last_err = None
    t0 = time.perf_counter()
    for attempt in range(retries):
        req = urllib.request.Request(
            f"{base_url}{path}",
            data=data,
            headers={"content-type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=180) as resp:
                raw = resp.read()
                code = resp.status
            wall_ms = int((time.perf_counter() - t0) * 1000)
            (out_dir / f"{name}.json").write_bytes(raw)
            (out_dir / f"{name}.http").write_text(str(code))
            (out_dir / f"{name}.wall_ms").write_text(str(wall_ms))
            print(f"{'METRICS' if 'metrics' in path else 'QUERY'} {name} http={code} wall_ms={wall_ms}")
            return code, wall_ms
        except urllib.error.HTTPError as e:
            raw = e.read()
            code = e.code
            wall_ms = int((time.perf_counter() - t0) * 1000)
            (out_dir / f"{name}.json").write_bytes(raw)
            (out_dir / f"{name}.http").write_text(str(code))
            (out_dir / f"{name}.wall_ms").write_text(str(wall_ms))
            print(f"{'METRICS' if 'metrics' in path else 'QUERY'} {name} http={code} wall_ms={wall_ms}")
            return code, wall_ms
        except Exception as e:
            last_err = e
            time.sleep(1.5 * (attempt + 1))
            try:
                urllib.request.urlopen(f"{base_url}/apps/zhifa/home", timeout=10)
            except Exception:
                time.sleep(2)
    wall_ms = int((time.perf_counter() - t0) * 1000)
    (out_dir / f"{name}.err").write_text(str(last_err))
    (out_dir / f"{name}.http").write_text("fail")
    (out_dir / f"{name}.wall_ms").write_text(str(wall_ms))
    print(f"FAIL {name} {last_err}")
    return "fail", wall_ms


def load_audit_index():
    """metric_id -> latest entry fields from AUDIT_DIR/*.jsonl."""
    idx = {}
    if not audit_dir or not audit_dir.is_dir():
        return idx
    files = sorted(audit_dir.glob("*.jsonl"))
    for f in files:
        try:
            for line in f.read_text(encoding="utf-8").splitlines():
                line = line.strip()
                if not line:
                    continue
                try:
                    e = json.loads(line)
                except json.JSONDecodeError:
                    continue
                mid = e.get("metric_id") or ""
                if not mid:
                    continue
                shape = e.get("shape") or {}
                timing = e.get("timing_ms") or {}
                ts = e.get("ts_ms") or 0
                row = {
                    "ts_ms": ts,
                    "sql_chars": shape.get("sql_chars"),
                    "union_all": shape.get("union_all"),
                    "has_arm": shape.get("has_arm"),
                    "exec_ms": timing.get("exec"),
                    "total_ms": timing.get("total"),
                    "lower_ms": timing.get("lower"),
                    "scene_id": e.get("scene_id"),
                    "path": e.get("path"),
                }
                prev = idx.get(mid)
                if prev is None or ts >= prev.get("ts_ms", 0):
                    idx[mid] = row
                # also key by scene+metric for disambiguation
                sc = e.get("scene_id") or ""
                if sc:
                    key = f"{sc}::{mid}"
                    prev2 = idx.get(key)
                    if prev2 is None or ts >= prev2.get("ts_ms", 0):
                        idx[key] = row
        except OSError:
            continue
    return idx


def lookup_audit(idx, body, kind):
    if kind == "metrics":
        mids = body.get("metric_ids") or []
        mid = mids[0] if mids else ""
        # metrics path often logs without ::__scalar_rowset__
        candidates = [mid, f"{mid}::__scalar_rowset__"] if mid else []
    else:
        mid = body.get("metric_id") or ""
        candidates = [mid]
        if mid.endswith("::__scalar_rowset__"):
            candidates.append(mid[: -len("::__scalar_rowset__")])
    scene = body.get("scene_id") or ""
    for c in candidates:
        if not c:
            continue
        if scene:
            hit = idx.get(f"{scene}::{c}")
            if hit:
                return hit
        hit = idx.get(c)
        if hit:
            return hit
    return {}


entries = []
for p in sorted(root.glob("src/scene/**/plane-*.mei")):
    text = p.read_text(encoding="utf-8")
    if "page_instance(" not in text:
        continue
    scene = re.search(r'\bscene\s*=\s*"([^"]+)"', text)
    if not scene:
        continue
    target = "src/" + str(p.relative_to(root / "src")).replace("\\", "/")
    name = "t2_" + p.stem.replace("plane-", "").replace("-", "_")
    fname = p.name
    if fname in INDICATOR:
        sc, mids, ds = INDICATOR[fname]
        entries.append(("metrics", name, {
            "scene_id": sc,
            "target": target,
            "dataset_id": ds,
            "metric_ids": mids,
            "filters": {},
            "query_state": {"filters": {}},
        }))
        continue
    metric = re.search(r'metric_ref\(\s*"([^"]+)"', text)
    dataset = re.search(r'"rowset_dataset_id"\s*:\s*"([^"]+)"', text)
    if metric and dataset:
        entries.append(("query", name, {
            "scene_id": scene.group(1),
            "target": target,
            "dataset_id": dataset.group(1),
            "metric_id": f"{metric.group(1)}::__scalar_rowset__",
            "page": 1,
            "page_size": 20,
            "filters": {},
            "query_state": {"filters": {}},
            "full": False,
            "summary": False,
        }))
    elif metric:
        entries.append(("query", name, {
            "scene_id": scene.group(1),
            "target": target,
            "dataset_id": dataset.group(1) if dataset else metric.group(1).rsplit("_", 1)[0],
            "metric_id": f"{metric.group(1)}::__scalar_rowset__",
            "page": 1,
            "page_size": 20,
            "filters": {},
            "query_state": {"filters": {}},
            "full": False,
            "summary": False,
        }))
    else:
        print(f"SKIP {name} no metric/examples")

# Also include Z1 in summary rows (already probed via bash)
z1_body = json.loads(
    '{"scene_id":"effect_analytics_page","target":"src/scene/home/t1/region-right-rail/section-effect/plane-effect-analytics.mei","dataset_id":"effectiveness_analytics_list","metric_id":"effectiveness_analytics::__scalar_rowset__","page":1,"page_size":20,"filters":{},"query_state":{"filters":{}},"full":false,"summary":false}'
)

(out_dir / "t2_probe_plan.json").write_text(
    json.dumps([{"kind": k, "name": n, "body": b} for k, n, b in entries], ensure_ascii=False, indent=2),
    encoding="utf-8",
)
print(f"=== T2 probes planned={len(entries)} (warm+measure each) ===")

rows = []
for kind, name, body in entries:
    path = "/api/datasets/metrics/zhifa" if kind == "metrics" else "/api/datasets/query/zhifa"
    cold_code, cold_ms = post(path, body, f"{name}_warm")
    time.sleep(0.05)
    hot_code, hot_ms = post(path, body, name)
    time.sleep(0.15)
    rows.append({
        "name": name,
        "kind": kind,
        "scene": body.get("scene_id", ""),
        "metric": (body.get("metric_id") or (body.get("metric_ids") or [""])[0]),
        "http": hot_code,
        "cold_wall_ms": cold_ms,
        "hot_wall_ms": hot_ms,
        "body": body,
    })

# refresh audit index after all probes
idx = load_audit_index()

# Z1 summary from bash artifacts
z1_http = (out_dir / "z1.http").read_text().strip() if (out_dir / "z1.http").exists() else ""
z1_warm_ms = ""
z1_hot_ms = ""
# bash curl posts don't write wall_ms — leave blank or estimate from audit only
z1_row = {
    "name": "z1",
    "kind": "query",
    "scene": "effect_analytics_page",
    "metric": "effectiveness_analytics::__scalar_rowset__",
    "http": z1_http,
    "cold_wall_ms": z1_warm_ms,
    "hot_wall_ms": z1_hot_ms,
    "body": z1_body,
}
summary_rows = [z1_row] + rows

def enrich(row):
    a = lookup_audit(idx, row["body"], row["kind"])
    return {
        "name": row["name"],
        "http": row["http"],
        "sql_chars": a.get("sql_chars", ""),
        "union_all": a.get("union_all", ""),
        "has_arm": a.get("has_arm", ""),
        "cold_wall_ms": row["cold_wall_ms"],
        "hot_wall_ms": row["hot_wall_ms"],
        "exec_ms": a.get("exec_ms", ""),
        "total_ms": a.get("total_ms", ""),
        "lower_ms": a.get("lower_ms", ""),
        "scene": row["scene"],
        "metric": row["metric"],
    }

enriched = [enrich(r) for r in summary_rows]

cols = [
    "name", "http", "sql_chars", "union_all", "has_arm",
    "cold_wall_ms", "hot_wall_ms", "exec_ms", "total_ms", "lower_ms",
    "scene", "metric",
]
tsv_lines = ["\t".join(cols)]
for e in enriched:
    tsv_lines.append("\t".join(str(e.get(c, "")) for c in cols))
(out_dir / "t2_summary.tsv").write_text("\n".join(tsv_lines) + "\n", encoding="utf-8")

md = [
    "# zhifa T2 probe summary",
    "",
    f"- BASE_URL: `{base_url}`",
    f"- AUDIT_DIR: `{audit_dir or '(none)'}`",
    "",
    "| name | http | sql_chars | union_all | cold_wall_ms | hot_wall_ms | exec_ms | total_ms | lower_ms |",
    "|---|---:|---:|---:|---:|---:|---:|---:|---:|",
]
for e in enriched:
    md.append(
        "| {name} | {http} | {sql_chars} | {union_all} | {cold_wall_ms} | {hot_wall_ms} | {exec_ms} | {total_ms} | {lower_ms} |".format(**e)
    )
(out_dir / "t2_summary.md").write_text("\n".join(md) + "\n", encoding="utf-8")

print("=== summary http ===")
for f in sorted(out_dir.glob("*.http")):
    if f.stem.endswith("_warm") or f.stem.endswith("_cold"):
        continue
    print(f.stem, f.read_text().strip())
print(f"=== wrote {out_dir / 't2_summary.tsv'} ===")
print(f"=== wrote {out_dir / 't2_summary.md'} ===")
PY

echo "bodies in $OUT_DIR"
