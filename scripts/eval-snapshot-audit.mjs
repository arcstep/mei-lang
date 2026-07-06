#!/usr/bin/env node
/**
 * 0524 E7: evalPack fields in runtime snapshot.
 */
import { chromium } from "@playwright/test";

const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const response = await page.request.get(`${base}/api/runtime/snapshot?appId=data-demo`);
  if (!response.ok()) {
    await browser.close();
    console.log(JSON.stringify({ ok: false, status: response.status() }, null, 2));
    process.exit(1);
  }

  const snapshot = await response.json();
  await browser.close();
  const evalPack = snapshot?.evalPack || {};
  const failures = [];

  if (!evalPack.bootstrapEmbed || typeof evalPack.bootstrapEmbed !== "object") {
    failures.push("missing evalPack.bootstrapEmbed");
  }
  if (!evalPack.deliveryClassCounts || typeof evalPack.deliveryClassCounts !== "object") {
    failures.push("missing evalPack.deliveryClassCounts");
  }
  if (!("warmupLastRun" in evalPack)) {
    failures.push("missing evalPack.warmupLastRun key");
  }

  const report = {
    ok: failures.length === 0,
    evalPack,
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
