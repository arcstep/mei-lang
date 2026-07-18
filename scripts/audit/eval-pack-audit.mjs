#!/usr/bin/env node
/**
 * Phase A acceptance: zhifa default query should not hit metrics/dataset APIs.
 * Usage: MEI_E2E_BASE_URL=http://127.0.0.1:9527 node scripts/audit/eval-pack-audit.mjs [baseUrl]
 */
import { chromium } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.argv[2] || "http://127.0.0.1:9527").replace(
  /\/+$/,
  "");
const paths = [
  "/apps/zhifa/app",
  "/apps/zhifa/view?surface=app",
];

function isEvalRuntimeApi(url) {
  const u = new URL(url);
  const p = u.pathname;
  if (p.includes("/api/datasets/metrics/")) return true;
  if (p.includes("/api/datasets/") && p.includes("/query")) return true;
  if (p.includes("/api/plug-ds/") && (p.includes("/metrics") || p.includes("/query"))) return true;
  return false;
}

async function auditPath(browser, appUrl) {
  const page = await browser.newPage();
  const evalApiCalls = [];
  const sceneBootstrapCalls = [];
  const activateCalls = [];

  page.on("request", (req) => {
    const url = req.url();
    if (isEvalRuntimeApi(url)) {
      evalApiCalls.push({ method: req.method(), url });
    }
    if (url.includes("/api/host/scene-bootstrap") || url.includes("/api/host/scene-eval-pack")) {
      sceneBootstrapCalls.push({ method: req.method(), url });
    }
    if (url.includes("/api/host/mrg/activate")) {
      activateCalls.push({ method: req.method(), url, at: Date.now() });
    }
  });

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page
    .waitForFunction(
      () =>
        Number(window.__meiBootstrapSeedCount || 0) > 0 ||
        (window.__meiBootstrapPayloadReady &&
          Array.isArray(window.__mei?.bootstrap_metrics) &&
          window.__mei.bootstrap_metrics.length > 0),
      { timeout: 15000 },
    )
    .catch(() => {});
  await page.waitForTimeout(1000);

  const clientState = await page.evaluate(() => ({
    bootstrapSeedCount: Number(window.__meiBootstrapSeedCount || 0),
    bootstrapSeeded: !!window.__meiBootstrapSeeded,
    evalPackSource: String(window.__meiEvalPackSource || ""),
    evalPackMissReason: String(window.__meiEvalPackMissReason || ""),
    bootstrapRevisionOnly: !!document.querySelector(
      'meta[name="mei-bootstrap-inlined"][content="0"]',
    ),
    bootstrapInlined:
      !!document.querySelector('meta[name="mei-bootstrap-inlined"][content="1"]') ||
      !!document.getElementById("mei-client-bootstrap"),
    metricCount: Array.isArray(window.__mei?.bootstrap_metrics)
      ? window.__mei.bootstrap_metrics.length
      : 0,
    bootstrapFromLocal: !!window.__meiBootstrapFromLocalStorage,
  }));

  await page.close();

  const failures = [];
  if (evalApiCalls.length > 0) {
    failures.push(`expected 0 eval runtime API calls, got ${evalApiCalls.length}`);
  }
  if (clientState.bootstrapSeedCount <= 0 && clientState.metricCount <= 0) {
    failures.push(
      `expected bootstrap seed or metrics, got seed=${clientState.bootstrapSeedCount} metrics=${clientState.metricCount}`,
    );
  }
  if (clientState.evalPackMissReason) {
    failures.push(`unexpected __meiEvalPackMissReason: ${clientState.evalPackMissReason}`);
  }
  if (!clientState.bootstrapRevisionOnly && !clientState.bootstrapInlined) {
    failures.push("expected mei-bootstrap-inlined meta or inline bootstrap script");
  }
  if (sceneBootstrapCalls.length > 1) {
    failures.push(
      `expected <= 1 scene-bootstrap/scene-eval-pack request, got ${sceneBootstrapCalls.length}`,
    );
  }
  if (activateCalls.length > 0) {
    failures.push(
      `expected 0 mrg/activate on cold load (strict ordering), got ${activateCalls.length}`,
    );
  }
  if (
    clientState.bootstrapRevisionOnly &&
    clientState.evalPackSource &&
    !["scene_bootstrap_api", "scene_bootstrap_local", "bootstrap_inline", "eval_store", "eval_pack_api", "eval_pack_local", "eval_pack_inline"].includes(
      clientState.evalPackSource,
    )
  ) {
    failures.push(`unexpected evalPackSource under revision_only: ${clientState.evalPackSource}`);
  }

  return {
    ok: failures.length === 0,
    url: appUrl,
    evalApiCalls,
    sceneBootstrapCalls,
    clientState,
    failures,
  };
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const results = [];
  for (const path of paths) {
    results.push(await auditPath(browser, `${base}${path}`));
  }
  await browser.close();

  const failures = results.flatMap((r) => r.failures.map((f) => `${r.url}: ${f}`));
  const report = {
    ok: failures.length === 0,
    results,
    failures,
  };

  console.log(JSON.stringify(report, null, 2));
  if (failures.length > 0) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
