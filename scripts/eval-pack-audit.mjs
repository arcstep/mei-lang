#!/usr/bin/env node
/**
 * Phase A acceptance: data-demo home default query should not hit metrics/dataset APIs.
 * Usage: node scripts/eval-pack-audit.mjs [baseUrl]
 */
import { chromium } from "@playwright/test";

const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/data-demo/app`;

function isEvalRuntimeApi(url) {
  const u = new URL(url);
  const p = u.pathname;
  if (p.includes("/api/datasets/metrics/")) return true;
  if (p.includes("/api/datasets/") && p.includes("/query")) return true;
  if (p.includes("/api/plug-ds/") && (p.includes("/metrics") || p.includes("/query"))) return true;
  return false;
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const evalApiCalls = [];

  page.on("request", (req) => {
    if (isEvalRuntimeApi(req.url())) {
      evalApiCalls.push({ method: req.method(), url: req.url() });
    }
  });

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(2000);

  const clientState = await page.evaluate(() => ({
    bootstrapSeedCount: Number(window.__meiBootstrapSeedCount || 0),
    bootstrapSeeded: !!window.__meiBootstrapSeeded,
    evalPackSource: String(window.__meiEvalPackSource || ""),
    evalPackMissReason: String(window.__meiEvalPackMissReason || ""),
    bootstrapInlined:
      !!document.querySelector('meta[name="mei-bootstrap-inlined"][content="1"]') ||
      !!document.getElementById("mei-client-bootstrap"),
    metricCount: Array.isArray(window.__mei?.bootstrap_metrics)
      ? window.__mei.bootstrap_metrics.length
      : 0,
  }));

  await browser.close();

  const failures = [];
  if (evalApiCalls.length > 0) {
    failures.push(`expected 0 eval runtime API calls, got ${evalApiCalls.length}`);
  }
  if (clientState.bootstrapSeedCount <= 0) {
    failures.push(`expected __meiBootstrapSeedCount > 0, got ${clientState.bootstrapSeedCount}`);
  }
  if (clientState.evalPackMissReason) {
    failures.push(`unexpected __meiEvalPackMissReason: ${clientState.evalPackMissReason}`);
  }

  const report = {
    ok: failures.length === 0,
    url: appUrl,
    evalApiCalls,
    clientState,
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
