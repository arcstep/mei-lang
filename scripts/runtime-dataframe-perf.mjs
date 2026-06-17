#!/usr/bin/env node
/**
 * 大表明细（metric dataframe / __scalar_rowset__）冷→热探测。
 * 用于边观测 perf 字段、边验证 dataset_rows / materialized / file 缓存是否生效。
 *
 * 用法（需已启动 mei-host-web；带 --auth 时从浏览器复制 cookie 或 bearer）：
 *
 *   MEI_COOKIE='mei_auth_token=...' \
 *   MEI_APP_ID=zhifa \
 *   node scripts/runtime-dataframe-perf.mjs
 *
 * 可选环境变量：
 *   MEI_SERVER_URL, MEI_SCENE_ID, MEI_TARGET, MEI_DATASET_ID, MEI_METRIC_ID,
 *   MEI_PAGE_SIZE, MEI_CLEAR_MODE (clear_only|clear_and_warm), MEI_SKIP_CLEAR
 */

const baseUrl = String(process.env.MEI_SERVER_URL || "http://127.0.0.1:9527").replace(
  /\/+$/,
  ""
);
const appId = String(process.env.MEI_APP_ID || "zhifa").trim();
const sceneId = String(process.env.MEI_SCENE_ID || "inspection_total_analytics_board").trim();
const target = String(process.env.MEI_TARGET || "scenes/02-行政检查.board.mei").trim();
const datasetId = String(
  process.env.MEI_DATASET_ID || "administrative_inspection_dashboard_ds"
).trim();
const metricId = String(
  process.env.MEI_METRIC_ID ||
    "scenes/02-行政检查.mei::inspections_total_count::__scalar_rowset__"
).trim();
const pageSize = Number(process.env.MEI_PAGE_SIZE || 20);
const clearMode = String(process.env.MEI_CLEAR_MODE || "clear_only").trim();
const skipClear = process.env.MEI_SKIP_CLEAR === "1";

const authBearer = String(process.env.MEI_AUTH_BEARER || "").trim();
const cookieHeader = String(process.env.MEI_COOKIE || "").trim();

const PERF_KEYS = [
  "total_ms",
  "compile_ms",
  "compile_cache_hit",
  "query_api_ms",
  "base_query_ms",
  "hydrate_datasets_ms",
  "metric_eval_ms",
  "file_cache_hit",
  "file_cache_load_ms",
  "file_cache_paginate_ms",
  "dataset_rows_cache_hit",
  "dataset_rows_cache_lookup_ms",
  "dataset_rows_cache_rows",
  "materialized_cache_hit",
  "response_cache_hit",
  "request_dag_nodes",
  "request_dag_hits",
  "request_dag_misses",
];

function buildHeaders() {
  const headers = { "content-type": "application/json" };
  if (authBearer) headers.authorization = `Bearer ${authBearer}`;
  if (cookieHeader) headers.cookie = cookieHeader;
  return headers;
}

async function postJson(path, body) {
  const response = await fetch(`${baseUrl}${path}`, {
    method: "POST",
    headers: buildHeaders(),
    body: JSON.stringify(body),
  });
  const text = await response.text();
  let json;
  try {
    json = text ? JSON.parse(text) : null;
  } catch (error) {
    throw new Error(`JSON parse failed ${path}: ${error}\n${text.slice(0, 400)}`);
  }
  if (!response.ok) {
    const err = json?.error || response.statusText;
    throw new Error(`${path} -> ${response.status} ${err}`);
  }
  return json;
}

function pickPerf(perf = {}) {
  const out = {};
  for (const key of PERF_KEYS) {
    if (key in perf) out[key] = perf[key];
  }
  for (const [key, value] of Object.entries(perf)) {
    if (key.startsWith("hydrate_") && !(key in out)) {
      out[key] = value;
    }
  }
  return out;
}

function formatRow(label, payload, wallMs) {
  const perf = pickPerf(payload?.perf || {});
  return {
    step: label,
    wall_ms: wallMs,
    total_rows: payload?.total ?? null,
    page: payload?.page ?? null,
    rows: Array.isArray(payload?.rows) ? payload.rows.length : null,
    ...perf,
  };
}

function printTable(rows) {
  const cols = ["step", "wall_ms", "total_ms", "total_rows", "compile_ms", "compile_cache_hit"];
  const cacheCols = [
    "file_cache_hit",
    "dataset_rows_cache_hit",
    "materialized_cache_hit",
    "metric_eval_ms",
    "hydrate_datasets_ms",
  ];
  const allCols = [...cols, ...cacheCols];
  console.log("\n" + allCols.join("\t"));
  for (const row of rows) {
    console.log(allCols.map((c) => String(row[c] ?? "-")).join("\t"));
  }
}

async function clearCaches() {
  const payload = await postJson(`/api/datasets/recompute/${encodeURI(appId)}`, {
    scene_id: sceneId,
    target,
    dataset_id: datasetId,
    metric_id: metricId,
    mode: clearMode,
  });
  const perf = payload?.perf || {};
  console.log(
    `[clear] mode=${clearMode} total_ms=${perf.total_ms ?? "-"} compile_cleared=${payload?.compile_cache_cleared ?? "-"} file_cleared=${payload?.file_cache_cleared ?? "-"}`
  );
}

async function queryPage(label, page) {
  const started = Date.now();
  const payload = await postJson(`/api/datasets/query/${encodeURI(appId)}`, {
    scene_id: sceneId,
    target,
    dataset_id: datasetId,
    metric_id: metricId,
    page,
    page_size: pageSize,
  });
  const wallMs = Date.now() - started;
  const row = formatRow(label, payload, wallMs);
  console.log(`[${label}] wall=${wallMs}ms total_rows=${row.total_rows} perf=${JSON.stringify(pickPerf(payload?.perf))}`);
  return row;
}

async function main() {
  console.log(
    `probe app=${appId} scene=${sceneId} dataset=${datasetId} metric=${metricId} page_size=${pageSize}`
  );
  console.log(`server=${baseUrl} auth=${authBearer || cookieHeader ? "yes" : "no (may 401 with --auth)"}`);

  if (!skipClear) {
    await clearCaches();
  }

  const rows = [];
  rows.push(await queryPage("cold_page1", 1));
  rows.push(await queryPage("warm_page2", 2));
  rows.push(await queryPage("warm_page1", 1));

  printTable(rows);

  const cold = rows[0];
  const warmPage2 = rows[1];
  const warmPage1 = rows[2];

  console.log("\n--- 判读提示 ---");
  if (Number(cold.dataset_rows_cache_hit) !== 1 && Number(warmPage2.dataset_rows_cache_hit) === 1) {
    console.log("✓ 翻页命中 dataset_rows_cache（首次全表物化后复用）");
  } else if (Number(warmPage2.dataset_rows_cache_hit) !== 1) {
    console.log("✗ warm 翻页未命中 dataset_rows_cache — 检查 server 是否已 cargo build 并重启");
  }
  if (Number(cold.materialized_cache_hit) === 1) {
    console.log("✓ cold 已命中 materialized_cache（同 scope 曾物化过）");
  }
  if (Number(cold.file_cache_hit) === 1) {
    console.log("✓ xlsx/file 层 L3 缓存命中");
  }
  if (Number(warmPage1.total_ms) < Number(cold.total_ms) / 2) {
    console.log(`✓ 同页二次请求明显加速 cold=${cold.total_ms}ms warm=${warmPage1.total_ms}ms`);
  }
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
