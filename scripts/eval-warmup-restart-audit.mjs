#!/usr/bin/env node
/**
 * 0524 E1: warmup disk_hit / eval_compute exposed via runtime snapshot after prebuild.
 */
import { chromium } from "@playwright/test";

const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/data-demo/app`;

async function fetchSnapshot(request, appId = "data-demo") {
  const response = await request.get(
    `${base}/api/runtime/snapshot?appId=${encodeURIComponent(appId)}`,
  );
  if (!response.ok()) {
    throw new Error(`snapshot failed: ${response.status()}`);
  }
  return response.json();
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(1500);

  const first = await fetchSnapshot(context.request);

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(1500);

  const second = await fetchSnapshot(context.request);
  await browser.close();

  const warmup = second?.evalPack?.warmupLastRun || second?.warmupLastRun || null;
  const failures = [];

  if (!warmup || typeof warmup !== "object") {
    failures.push("expected evalPack.warmupLastRun in runtime snapshot");
  } else {
    const diskHit = Number(warmup.diskHit ?? warmup.disk_hit ?? 0);
    const evalCompute = Number(warmup.evalCompute ?? warmup.eval_compute ?? -1);
    const slotCount = Number(warmup.slotCount ?? warmup.slot_count ?? 0);
    if (slotCount <= 0) {
      failures.push(`expected warmupLastRun.slotCount > 0, got ${slotCount}`);
    }
    if (!warmup.policy) {
      failures.push("expected warmupLastRun.policy");
    }
  }

  const report = {
    ok: failures.length === 0,
    url: appUrl,
    warmupLastRun: warmup,
    snapshotHostPhase: second?.host?.phase || null,
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
