import assert from "node:assert/strict";

function stableSerialize(value) {
  if (Array.isArray(value)) {
    return `[${value.map((item) => stableSerialize(item)).join(",")}]`;
  }
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableSerialize(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value ?? null);
}

function datasetQueryCacheKey(api, payload, fingerprint = "") {
  return `${String(api || "").trim()}|${String(fingerprint || "").trim()}|${stableSerialize(payload)}`;
}

function buildBootstrapDatasetRowsData(contract) {
  const value = contract?.value;
  let rows = null;
  if (Array.isArray(value)) {
    rows = value;
  } else if (value && typeof value === "object" && Array.isArray(value.rows)) {
    rows = value.rows;
  } else if (Array.isArray(contract?.rows)) {
    rows = contract.rows;
  }
  if (!Array.isArray(rows)) {
    return null;
  }
  return { rows, total: rows.length };
}

function main() {
  const arrayContract = {
    shape: "dataframe",
    value: [{ x: 1 }],
  };
  const nestedContract = {
    shape: "dataframe",
    value: { rows: [{ x: 2 }] },
  };
  assert.deepEqual(buildBootstrapDatasetRowsData(arrayContract), {
    rows: [{ x: 1 }],
    total: 1,
  });
  assert.deepEqual(buildBootstrapDatasetRowsData(nestedContract), {
    rows: [{ x: 2 }],
    total: 1,
  });

  const pageSizes = [16, 20, 64];
  const pageCtx = {
    scene_id: "home",
    target: "src/scene/home/assembly.mei",
    compile_epoch: "epoch-a",
    data_generation: "gen-1",
  };
  const api = "/api/datasets/rows/demo";
  const fingerprint = `${pageCtx.compile_epoch}|${pageCtx.data_generation}`;
  const keys = pageSizes.map((pageSize) =>
    datasetQueryCacheKey(
      api,
      {
        scene_id: pageCtx.scene_id,
        target: pageCtx.target,
        dataset_id: "ds-1",
        metric_id: "chart_metric",
        page: 1,
        page_size: pageSize,
        full: false,
        summary: false,
        filters: {},
        query_state: { filters: {} },
      },
      fingerprint,
    ),
  );
  assert.equal(new Set(keys).size, pageSizes.length, "each page size must produce a distinct cache key");

  console.log("runtime-query-session-cache.test: ok");
}

main();
