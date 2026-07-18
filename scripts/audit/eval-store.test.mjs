#!/usr/bin/env node
import assert from "node:assert/strict";

const REVISION_LS_PREFIX = "mei:eval-store-revision:v1:";

function revisionStorageKey(appId, scope) {
  return `${String(appId || "").trim()}:${String(scope || "home").trim()}`;
}

function readStoredRevision(appId, scope, storage) {
  return (storage.get(`${REVISION_LS_PREFIX}${revisionStorageKey(appId, scope)}`) || "").trim();
}

function writeStoredRevision(appId, scope, revision, storage) {
  storage.set(`${REVISION_LS_PREFIX}${revisionStorageKey(appId, scope)}`, String(revision || ""));
}

function ensureRevisionAligned(coords, storage, cleared, caches) {
  const prev = readStoredRevision(coords.appId, coords.scope, storage);
  if (prev && prev !== coords.clientRevision) {
    cleared.count += 1;
    caches.metricResults.clear();
    caches.datasetResults.clear();
  }
  writeStoredRevision(coords.appId, coords.scope, coords.clientRevision, storage);
  return coords;
}

function datasetQueryCacheKey(api, payload, fingerprint = "") {
  const page = payload?.page ?? 1;
  const pageSize = payload?.page_size ?? payload?.pageSize ?? 20;
  return `${api}|${payload?.dataset_id || ""}|${payload?.metric_id || ""}|p${page}|s${pageSize}|${fingerprint}`;
}

function bootstrapDatasetCacheKeyForTest(api, pageCtx, datasetId, metricId, pageSize = 20) {
  const fingerprint = `${pageCtx.compileEpoch || ""}|${pageCtx.dataGeneration || ""}`;
  return datasetQueryCacheKey(
    api,
    { dataset_id: datasetId, metric_id: metricId, page: 1, page_size: pageSize },
    fingerprint,
  );
}

const storage = new Map();
let cleared = { count: 0 };
const caches = {
  metricResults: new Map(),
  datasetResults: new Map(),
};

const coordsA = {
  appId: "data-demo",
  scope: "home",
  clientRevision: "rev-a",
};

ensureRevisionAligned(coordsA, storage, cleared, caches);
assert.equal(readStoredRevision("data-demo", "home", storage), "rev-a");
assert.equal(cleared.count, 0);

ensureRevisionAligned({ ...coordsA, clientRevision: "rev-b" }, storage, cleared, caches);
assert.equal(readStoredRevision("data-demo", "home", storage), "rev-b");
assert.equal(cleared.count, 1);
assert.equal(caches.metricResults.size, 0);

ensureRevisionAligned({ ...coordsA, clientRevision: "rev-b" }, storage, cleared, caches);
assert.equal(cleared.count, 1);

caches.datasetResults.set(
  bootstrapDatasetCacheKeyForTest(
    "/api/datasets/demo/query",
    { compileEpoch: "ce-1", dataGeneration: "dg-1" },
    "ds-1",
    "m-1",
  ),
  { data: { rows: [] }, expiresAt: Date.now() + 60000 },
);
assert.equal(caches.datasetResults.size, 1);

const sameKey = bootstrapDatasetCacheKeyForTest(
  "/api/datasets/demo/query",
  { compileEpoch: "ce-1", dataGeneration: "dg-1" },
  "ds-1",
  "m-1",
);
assert.ok(caches.datasetResults.has(sameKey), "bootstrap seed key must align with runtime-query cache key");

console.log("eval-store.test.mjs: ok");
