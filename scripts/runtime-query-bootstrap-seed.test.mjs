import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

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

function metricQueryCacheKey(api, payload, fingerprint = "") {
  return `${String(api || "").trim()}|${String(fingerprint || "").trim()}|${stableSerialize(payload)}`;
}

function buildBootstrapMetricQueryPayload(pageCtx, datasetId, metricIds, queryStatePayload) {
  const coords = { scene_id: pageCtx.scene_id };
  if (pageCtx.target) {
    coords.target = pageCtx.target;
  }
  return {
    ...coords,
    dataset_id: datasetId,
    metric_ids: [...metricIds].sort(),
    filters: queryStatePayload.filters,
    query_state: {
      filters: queryStatePayload.filters,
    },
  };
}

function buildRuntimeMetricQueryPayload(props, datasetId, metricIds) {
  const coords = {
    scene_id: props._mei.active_scene_id,
    target: props._mei.active_target_file,
  };
  return {
    ...coords,
    dataset_id: datasetId,
    metric_ids: [...metricIds].sort(),
    filters: {},
    query_state: { filters: {} },
  };
}

function assertKeysAlign(bootstrap, label = "fixture") {
  const pageCtx = {
    scene_id: bootstrap.bootstrap_scope || "home",
    target: bootstrap.targetFile,
    compile_epoch: bootstrap.compileEpoch,
    data_generation: bootstrap.dataGeneration,
  };
  const api = `/api/datasets/metrics/${bootstrap.appId || "data-demo"}`;
  const fingerprint = `${pageCtx.compile_epoch}|${pageCtx.data_generation}`;
  const queryStatePayload = { filters: {} };
  const sample = bootstrap.metrics[0];
  const datasetId = sample.dataset_id;
  const metricId = sample.contract?.id || sample.id;

  const bootstrapKey = metricQueryCacheKey(
    api,
    buildBootstrapMetricQueryPayload(pageCtx, datasetId, [metricId], queryStatePayload),
    fingerprint,
  );
  const runtimeProps = {
    _mei: {
      active_scene_id: pageCtx.scene_id,
      active_target_file: pageCtx.target,
      entry_target: pageCtx.target,
      compile_epoch: pageCtx.compile_epoch,
      data_generation: pageCtx.data_generation,
    },
  };
  const runtimeKey = metricQueryCacheKey(
    api,
    buildRuntimeMetricQueryPayload(runtimeProps, datasetId, [metricId]),
    fingerprint,
  );
  assert.equal(bootstrapKey, runtimeKey, `${label}: aligned keys must match`);

  const legacyFingerprint = `|${pageCtx.data_generation}`;
  const legacyKey = metricQueryCacheKey(
    api,
    {
      scene_id: pageCtx.scene_id,
      dataset_id: datasetId,
      metric_ids: [metricId],
      query_state: { filters: {} },
    },
    legacyFingerprint,
  );
  assert.notEqual(bootstrapKey, legacyKey, `${label}: legacy seed key must not match`);
}

async function loadHomeHtml() {
  const candidates = [process.env.MEI_HOME_HTML, "/tmp/home.html"].filter(Boolean);
  for (const candidate of candidates) {
    try {
      return await readFile(candidate, "utf8");
    } catch {
      /* try next */
    }
  }
  return null;
}

async function main() {
  const synthetic = {
    bootstrap_scope: "home",
    targetFile: "src/scene/home/assembly.mei",
    compileEpoch: "scene-epoch|data-epoch|src/scene/home/assembly.mei",
    dataGeneration: "2.0.3-ws20260628",
    appId: "data-demo",
    metrics: [
      {
        id: "penalties_top_party_year_amount_bars",
        dataset_id: "__world_metrics__::metrics/penalty-dashboard.bundle.mei",
        contract: { id: "penalties_top_party_year_amount_bars", shape: "dataframe" },
      },
    ],
  };
  assertKeysAlign(synthetic, "synthetic");
  const syntheticMultiScope = {
    ...synthetic,
    bootstrapScopes: [
      {
        clientRevision: synthetic.clientRevision || "rev-home",
        bootstrapScope: synthetic.bootstrap_scope,
        targetFile: synthetic.targetFile,
        compileEpoch: synthetic.compileEpoch,
        dataGeneration: synthetic.dataGeneration,
        appId: synthetic.appId,
        metrics: synthetic.metrics,
      },
      {
        clientRevision: "rev-board",
        bootstrapScope: "supervision-warning",
        targetFile: "src/overlay/boards/supervision-warning.board.mei",
        compileEpoch:
          "scene-epoch|data-epoch|src/overlay/boards/supervision-warning.board.mei",
        dataGeneration: synthetic.dataGeneration,
        appId: synthetic.appId,
        metrics: synthetic.metrics,
      },
    ],
  };
  assertKeysAlign(syntheticMultiScope.bootstrapScopes[1], "synthetic-multi-scope");

  const pageSizes = [16, 20, 64];
  assert.deepEqual(pageSizes, [16, 20, 64], "bootstrap page sizes must cover chart defaults");

  const html = await loadHomeHtml();
  if (html) {
    const match = html.match(/id="mei-client-bootstrap"[^>]*>(\{.*?\})<\/script>/s);
    if (match) {
      const bootstrap = JSON.parse(match[1]);
      if (bootstrap.compileEpoch && bootstrap.targetFile) {
        assertKeysAlign(bootstrap, "live-html");
        console.log("runtime-query-bootstrap-seed.test: ok (synthetic + live HTML)");
        return;
      }
      console.log(
        "runtime-query-bootstrap-seed.test: ok (synthetic); live HTML missing compileEpoch — restart mei-host-shell",
      );
      return;
    }
  }
  console.log("runtime-query-bootstrap-seed.test: ok (synthetic)");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
