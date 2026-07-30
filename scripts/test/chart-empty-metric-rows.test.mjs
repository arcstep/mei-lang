/**
 * Guard: empty dataframe rows + overlapping refresh race must not blank charts.
 *
 * Run: node scripts/test/chart-empty-metric-rows.test.mjs
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(__dirname, "../..");

const {
  hasAuthoritativeDataframeRows,
  shouldCommitDatasetMetricRows,
  isAuthoritativeBootstrapDatasetPage,
  metricContractHasRenderableRows,
  runtimePropsHaveRenderableRows,
  shouldApplyDatasetMetricRowsResult,
  shouldApplyMetricFallbackResult,
} = await import(
  path.join(root, "stock/components/dataset/metric-dataframe-authority.js")
);

assert.equal(hasAuthoritativeDataframeRows(null), false);
assert.equal(hasAuthoritativeDataframeRows([]), false);
assert.equal(hasAuthoritativeDataframeRows([{ label: "a", value: 1 }]), true);

assert.equal(shouldCommitDatasetMetricRows({ rows: [] }), false);
assert.equal(
  shouldCommitDatasetMetricRows({ rows: [{ label: "对损毁园林植物的处罚", value: 9 }] }),
  true,
);

assert.equal(
  isAuthoritativeBootstrapDatasetPage({ rows: [], columns: ["label", "value"], total: 0 }),
  false,
);
assert.equal(
  isAuthoritativeBootstrapDatasetPage({
    rows: [{ label: "x", value: 1 }],
    columns: ["label", "value"],
    total: 1,
  }),
  true,
);

assert.equal(metricContractHasRenderableRows({ shape: "dataframe", value: [] }), false);
assert.equal(
  metricContractHasRenderableRows({
    shape: "dataframe",
    value: [{ label: "对损毁园林植物的处罚", value: 9 }],
  }),
  true,
);
assert.equal(
  runtimePropsHaveRenderableRows({ data: { rows: [{ label: "a", value: 1 }] } }),
  true,
);

const goodRows = { rows: [{ label: "a", value: 1 }, { label: "b", value: 2 }, { label: "c", value: 3 }] };

// Current gen + good rows → apply
assert.equal(
  shouldApplyDatasetMetricRowsResult({
    refreshGen: 1,
    currentGen: 1,
    rowsResult: goodRows,
    runtimeProps: null,
  }),
  true,
);

// Current gen + empty → never apply
assert.equal(
  shouldApplyDatasetMetricRowsResult({
    refreshGen: 1,
    currentGen: 1,
    rowsResult: { rows: [] },
    runtimeProps: null,
  }),
  false,
);

// ROOT CAUSE: stale gen with good rows while paint still empty → MUST apply
assert.equal(
  shouldApplyDatasetMetricRowsResult({
    refreshGen: 1,
    currentGen: 2,
    rowsResult: goodRows,
    runtimeProps: null,
  }),
  true,
  "stale successful fetch must upgrade empty paint (overlapping refresh race)",
);

// Stale gen with good rows but paint already full → skip (newer gen owns the surface)
assert.equal(
  shouldApplyDatasetMetricRowsResult({
    refreshGen: 1,
    currentGen: 2,
    rowsResult: goodRows,
    runtimeProps: { data: { rows: [{ label: "keep", value: 1 }] } },
  }),
  false,
);

// Stale empty → never apply
assert.equal(
  shouldApplyDatasetMetricRowsResult({
    refreshGen: 1,
    currentGen: 2,
    rowsResult: { rows: [] },
    runtimeProps: null,
  }),
  false,
);

assert.equal(
  shouldApplyMetricFallbackResult({
    refreshGen: 1,
    currentGen: 2,
    metric: { shape: "dataframe", value: [{ label: "a", value: 1 }] },
    runtimeProps: null,
  }),
  true,
  "stale metrics fallback with rows must upgrade empty paint",
);
assert.equal(
  shouldApplyMetricFallbackResult({
    refreshGen: 1,
    currentGen: 1,
    metric: { shape: "dataframe", value: [] },
    runtimeProps: { data: { rows: goodRows.rows } },
  }),
  false,
  "empty metrics fallback must not wipe good paint",
);

const engineSrc = await readFile(
  path.join(root, "stock/components/chart/echarts/engine.js"),
  "utf8",
);
assert.match(engineSrc, /shouldApplyDatasetMetricRowsResult/);
assert.match(engineSrc, /shouldApplyMetricFallbackResult/);
assert.match(
  engineSrc,
  /空 items 不得清掉已画好的榜/,
  "empty ranking items must not wipe painted DOM",
);
assert.match(
  engineSrc,
  /禁止在 await 前清空画布/,
  "echarts path must not clear canvas before ensureECharts await",
);
assert.doesNotMatch(
  engineSrc,
  /readBootstrapMetricContract/,
  "chart must not rely on bootstrap last-resort兜底; fix overlapping refresh race instead",
);
assert.doesNotMatch(
  engineSrc,
  /if \(Array\.isArray\(rowsResult\?\.rows\)\) \{\s*\n\s*const dataset = resolveDatasetSource\(props\);/,
);

const runtimeQuerySrc = await readFile(
  path.join(root, "stock/components/dataset/runtime-query.js"),
  "utf8",
);
assert.match(runtimeQuerySrc, /isAuthoritativeBootstrapDatasetPage/);
assert.match(runtimeQuerySrc, /metricPageAuthoritative/);

const fetchDatasetRowsIdx = runtimeQuerySrc.indexOf("export async function fetchDatasetRows(");
assert.ok(fetchDatasetRowsIdx >= 0);
const earlyCacheBlock = runtimeQuerySrc.slice(
  fetchDatasetRowsIdx,
  runtimeQuerySrc.indexOf("packFirstAppliesToDatasetFetch", fetchDatasetRowsIdx),
);
assert.match(earlyCacheBlock, /metricPageAuthoritative\(cached\.data\)/);
assert.match(earlyCacheBlock, /metricPageAuthoritative\(sessionCached\.data\)/);

console.log("chart-empty-metric-rows.test.mjs: ok");
