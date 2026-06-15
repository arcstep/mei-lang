const baseUrl = String(process.env.MEI_SERVER_URL || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appId = String(process.env.MEI_APP_ID || "examples/ds/01-dataset-baseline").trim();

const cases = [
  {
    name: "dataset-baseline-metrics",
    coords: {
      scene_id: "home",
      target: "main.mei",
      dataset_id: "__world_metrics__",
    },
    shortMetricIds: ["pack_total_value", "pack_focus_rate"],
    drilldownTables: {},
    namespacedPrefix: "",
  },
];

function extractScalarValue(metric) {
  if (!metric || typeof metric !== "object") return null;
  const value = metric.value;
  if (value && typeof value === "object" && !Array.isArray(value) && "value" in value) {
    return value.value;
  }
  return value;
}

function pickPerf(perf = {}) {
  const keys = [
    "response_cache_hit",
    "compile_cache_hit",
    "compile_ms",
    "query_api_ms",
    "hydrate_datasets_ms",
    "metric_eval_ms",
    "total_ms",
  ];
  const out = {};
  for (const key of keys) {
    if (key in perf) out[key] = perf[key];
  }
  for (const [key, value] of Object.entries(perf)) {
    if (key.startsWith("hydrate_") && key !== "hydrate_datasets_ms") {
      out[key] = value;
    }
  }
  return out;
}

async function postJson(path, body) {
  const response = await fetch(`${baseUrl}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  const text = await response.text();
  let json;
  try {
    json = text ? JSON.parse(text) : null;
  } catch (error) {
    throw new Error(`failed to parse JSON from ${path}: ${error}\n${text}`);
  }
  if (!response.ok) {
    throw new Error(`${path} -> ${response.status} ${response.statusText}\n${text}`);
  }
  return json;
}

async function warmCase(testCase) {
  await postJson(`/api/datasets/recompute/${appId}`, {
    ...testCase.coords,
    mode: "clear_and_warm",
  });
}

async function runMetricQuery(testCase, metricIds) {
  return postJson(`/api/datasets/metrics/${appId}`, {
    ...testCase.coords,
    metric_ids: metricIds,
  });
}

async function runDataframeQuery(testCase, metricId) {
  return postJson(`/api/datasets/query/${appId}`, {
    ...testCase.coords,
    metric_id: metricId,
    page: 1,
    page_size: 1,
  });
}

function assertMetricIds(response, metricIds, label) {
  const actual = (response.metrics || []).map((metric) => metric.id);
  if (JSON.stringify(actual) !== JSON.stringify(metricIds)) {
    throw new Error(
      `${label}: metric ids mismatch\nexpected=${JSON.stringify(metricIds)}\nactual=${JSON.stringify(actual)}`
    );
  }
}

function assertSameMetricValues(left, right, label) {
  const leftValues = (left.metrics || []).map(extractScalarValue);
  const rightValues = (right.metrics || []).map(extractScalarValue);
  if (JSON.stringify(leftValues) !== JSON.stringify(rightValues)) {
    throw new Error(
      `${label}: metric values mismatch\nleft=${JSON.stringify(leftValues)}\nright=${JSON.stringify(rightValues)}`
    );
  }
}

async function assertDrilldownTotals(testCase, metricsResponse) {
  for (const metric of metricsResponse.metrics || []) {
    const scalarValue = Number(extractScalarValue(metric));
    if (!Number.isFinite(scalarValue)) {
      continue;
    }
    const tableMetricId = testCase.drilldownTables?.[metric.id];
    if (!tableMetricId) {
      continue;
    }
    const dataframe = await runDataframeQuery(testCase, tableMetricId);
    if (Number(dataframe.total) !== scalarValue) {
      throw new Error(
        `${testCase.name}: scalar ${metric.id}=${scalarValue} but dataframe ${tableMetricId}.total=${dataframe.total}`
      );
    }
  }
}

console.log(`# runtime metric perf check`);
console.log(`baseUrl=${baseUrl}`);
console.log(`appId=${appId}`);

for (const testCase of cases) {
  const namespacedMetricIds = testCase.shortMetricIds.map(
    (metricId) => `${testCase.namespacedPrefix}${metricId}`
  );

  console.log(`\n## ${testCase.name}`);
  await warmCase(testCase);

  const first = await runMetricQuery(testCase, testCase.shortMetricIds);
  const second = await runMetricQuery(testCase, testCase.shortMetricIds);
  const namespaced = await runMetricQuery(testCase, namespacedMetricIds);

  assertMetricIds(first, testCase.shortMetricIds, `${testCase.name} first`);
  assertMetricIds(second, testCase.shortMetricIds, `${testCase.name} second`);
  assertMetricIds(namespaced, namespacedMetricIds, `${testCase.name} namespaced`);
  assertSameMetricValues(first, namespaced, `${testCase.name} short-vs-namespaced`);
  await assertDrilldownTotals(testCase, first);

  if ((second.perf || {}).response_cache_hit !== 1) {
    throw new Error(`${testCase.name}: second request should hit response cache`);
  }
  if ((namespaced.perf || {}).response_cache_hit !== 1) {
    throw new Error(`${testCase.name}: namespaced request should reuse canonical response cache`);
  }

  console.log(`first.perf=${JSON.stringify(pickPerf(first.perf))}`);
  console.log(`second.perf=${JSON.stringify(pickPerf(second.perf))}`);
  console.log(`namespaced.perf=${JSON.stringify(pickPerf(namespaced.perf))}`);
}

console.log("\nruntime metric perf checks ok");
