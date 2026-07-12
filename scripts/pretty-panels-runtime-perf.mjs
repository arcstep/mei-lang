#!/usr/bin/env node
import fs from "node:fs/promises";
import { chromium } from "@playwright/test";

const argv = process.argv.slice(2);
const base = (argv.find((arg) => !arg.startsWith("-")) || "http://127.0.0.1:9527").replace(
  /\/+$/,
  "",
);
const valueFor = (flag, fallback) => {
  const index = argv.indexOf(flag);
  return index >= 0 && argv[index + 1] ? argv[index + 1] : fallback;
};
const repeat = Math.max(1, Number.parseInt(valueFor("--repeat", "10"), 10) || 10);
const jsonPath = valueFor("--json", "");
const targetUrl = `${base}/apps/pretty-panels/home`;
const budgets = {
  warmP50Ms: 500,
  warmP95Ms: 800,
  documentP50Ms: 100,
  htmlBytes: 32 * 1024,
  layerRestoreP50Ms: 50,
  composeP50Ms: 120,
};

function percentile(values, percentileValue) {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!sorted.length) return null;
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil((percentileValue / 100) * sorted.length) - 1),
  );
  return sorted[index];
}

function summarize(values) {
  return {
    count: values.filter(Number.isFinite).length,
    p50: percentile(values, 50),
    p95: percentile(values, 95),
    min: Math.min(...values.filter(Number.isFinite)),
    max: Math.max(...values.filter(Number.isFinite)),
  };
}

function classify(url) {
  const pathname = new URL(url).pathname;
  if (pathname === "/api/host/view-revision") return "view-revision";
  if (pathname === "/api/host/layer-batch") return "layer-batch";
  if (pathname === "/api/host/scene-drilldown-context") return "drilldown";
  if (pathname === "/apps/pretty-panels/home") return "document";
  return "";
}

async function waitVisibleReady(page) {
  await page.waitForFunction(
    () =>
      window.__meiRenderPipeline?.last?.endedAt === "user_visible_ready" ||
      window.__meiRenderPipeline?.last?.marks?.some(
        (mark) => mark?.name === "user_visible_ready",
      ),
    { timeout: 120000 },
  );
}

async function captureClient(page) {
  return page.evaluate(() => {
    const summary = window.__meiRenderPipeline?.last || {};
    const diagnostics =
      window.__meiLangBoot?.layerArtifactCache?.readDiagnostics?.() || {};
    const compose =
      Number(summary.phases?.compose_structure?.durationMs || 0) +
      Number(summary.phases?.bind_eval_slots?.durationMs || 0) +
      Number(summary.phases?.apply_chrome?.durationMs || 0);
    return {
      wallMs: Number(summary.wallMs) || null,
      documentMs: Number(summary.documentMs) || null,
      htmlBytes:
        Number(summary.navigation?.decodedBodySize) ||
        Number(summary.bodyPerf?.htmlBytes) ||
        null,
      layerRestoreMs: Number(summary.phases?.layer_restore?.durationMs) || 0,
      composeMs: compose,
      surfaceReadyMs: Number(summary.surfaceReadyMs) || null,
      diagnostics,
      fetchByKind: summary.fetchByKind || {},
      marks: summary.marks || [],
    };
  });
}

async function deleteOnePersistedLayer(page) {
  return page.evaluate(async () => {
    const boot = window.__meiLangBoot || {};
    const ctx = boot.parseViewContext?.(location.href) || {};
    const stored = boot.readViewRevision?.(ctx);
    const entry = Object.entries(stored?.manifest_snapshot?.layers || {}).find(
      ([, value]) => value?.artifact_id,
    );
    if (!entry) throw new Error("no persisted manifest layer available");
    const [name, ref] = entry;
    const deleted = await boot.layerArtifactCache.deleteLayer(ref.artifact_id);
    return { name, artifactId: ref.artifact_id, deleted };
  });
}

async function staleStoredRevision(page) {
  await page.evaluate(() => {
    for (const key of ["mei-view-revisions", "mei:view-revisions:v1"]) {
      const storage = key === "mei-view-revisions" ? sessionStorage : localStorage;
      const raw = storage.getItem(key);
      if (!raw) continue;
      const store = JSON.parse(raw);
      for (const value of Object.values(store)) {
        if (!value || typeof value !== "object") continue;
        value.manifest_revision_digest = "perf-stale-manifest";
        value.surface_revision_digest = "perf-stale-surface";
      }
      storage.setItem(key, JSON.stringify(store));
    }
  });
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();
  let activeRequests = [];
  page.on("request", (request) => {
    const kind = classify(request.url());
    if (!kind) return;
    let body = null;
    try {
      body = request.postDataJSON();
    } catch {}
    activeRequests.push({
      kind,
      method: request.method(),
      url: request.url(),
      body,
    });
  });

  async function measure(label, action) {
    activeRequests = [];
    const startedAt = Date.now();
    await action();
    await waitVisibleReady(page);
    const client = await captureClient(page);
    return {
      label,
      observedWallMs: Date.now() - startedAt,
      ...client,
      requests: activeRequests.slice(),
    };
  }

  const cold = await measure("cold_start", () =>
    page.goto(targetUrl, { waitUntil: "domcontentloaded", timeout: 120000 }),
  );
  await page.waitForTimeout(150);

  const warm = [];
  for (let index = 0; index < repeat; index += 1) {
    warm.push(
      await measure(`warm_f5_${index + 1}`, () =>
        page.reload({ waitUntil: "domcontentloaded", timeout: 120000 }),
      ),
    );
  }

  await staleStoredRevision(page);
  const revisionChanged = await measure("revision_changed", () =>
    page.reload({ waitUntil: "domcontentloaded", timeout: 120000 }),
  );
  await page.waitForTimeout(150);

  const deletedLayer = await deleteOnePersistedLayer(page);
  const partialMissing = await measure("partial_missing_layer", () =>
    page.reload({ waitUntil: "domcontentloaded", timeout: 120000 }),
  );

  const aggregate = {
    warmVisibleReady: summarize(warm.map((sample) => sample.wallMs)),
    warmDocument: summarize(warm.map((sample) => sample.documentMs)),
    warmLayerRestore: summarize(warm.map((sample) => sample.layerRestoreMs)),
    warmCompose: summarize(warm.map((sample) => sample.composeMs)),
    warmHtmlBytes: summarize(warm.map((sample) => sample.htmlBytes)),
  };
  const partialBatch = partialMissing.requests.filter(
    (request) => request.kind === "layer-batch",
  );
  const failures = [];
  if (aggregate.warmVisibleReady.p50 > budgets.warmP50Ms) {
    failures.push(`warm p50 ${aggregate.warmVisibleReady.p50}ms > ${budgets.warmP50Ms}ms`);
  }
  if (aggregate.warmVisibleReady.p95 > budgets.warmP95Ms) {
    failures.push(`warm p95 ${aggregate.warmVisibleReady.p95}ms > ${budgets.warmP95Ms}ms`);
  }
  if (aggregate.warmDocument.p50 > budgets.documentP50Ms) {
    failures.push(`document p50 ${aggregate.warmDocument.p50}ms > ${budgets.documentP50Ms}ms`);
  }
  if (aggregate.warmHtmlBytes.max > budgets.htmlBytes) {
    failures.push(`HTML ${aggregate.warmHtmlBytes.max}B > ${budgets.htmlBytes}B`);
  }
  if (aggregate.warmLayerRestore.p50 > budgets.layerRestoreP50Ms) {
    failures.push(
      `layer restore p50 ${aggregate.warmLayerRestore.p50}ms > ${budgets.layerRestoreP50Ms}ms`,
    );
  }
  if (aggregate.warmCompose.p50 > budgets.composeP50Ms) {
    failures.push(`compose p50 ${aggregate.warmCompose.p50}ms > ${budgets.composeP50Ms}ms`);
  }
  for (const sample of warm) {
    if (sample.requests.some((request) => request.kind === "layer-batch")) {
      failures.push(`${sample.label}: unchanged revision requested layer-batch`);
    }
    if (Number(sample.diagnostics?.readwriteTransactions || 0) !== 0) {
      failures.push(`${sample.label}: warm restore wrote IndexedDB`);
    }
  }
  if (
    partialBatch.length !== 1 ||
    partialBatch[0].body?.layers?.length !== 1 ||
    partialBatch[0].body.layers[0] !== deletedLayer.name
  ) {
    failures.push("partial missing layer did not fetch exactly the deleted ref");
  }

  const report = {
    schemaVersion: 1,
    measuredAt: new Date().toISOString(),
    targetUrl,
    repeat,
    budgets,
    aggregate,
    scenarios: { cold, warm, revisionChanged, partialMissing, deletedLayer },
    failures,
    ok: failures.length === 0,
  };
  await browser.close();
  if (jsonPath) await fs.writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  console.log(JSON.stringify(report, null, 2));
  if (failures.length) process.exitCode = 1;
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
