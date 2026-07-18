#!/usr/bin/env node
/**
 * F5 可测收敛：SPA 同 scene 内切换不应重复拉 document（对照 F5 reload = 1 document）。
 */
import { chromium } from "@playwright/test";

const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/zhifa/app`;

function isDocument(url) {
  const u = new URL(url);
  if (!u.pathname.includes("/apps/zhifa/")) return false;
  if (u.pathname.includes("/api/")) return false;
  return u.pathname.includes("/view") || u.pathname.endsWith("/app");
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const docs = [];

  page.on("request", (req) => {
    if (isDocument(req.url())) {
      docs.push(req.url());
    }
  });

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(1500);
  const afterFirstLoad = docs.length;

  await page.evaluate(() => {
    const boot = window.__meiLangBoot || {};
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        appId: "zhifa",
        sceneId: "home/t2/r-drilldown/s-inspection-dashboard",
        scope: "home/t2/r-drilldown/s-inspection-dashboard",
        source: "spa-same-scene-audit",
      });
    }
  });
  await page.waitForTimeout(2500);
  const afterSpaActivate = docs.length - afterFirstLoad;

  docs.length = 0;
  await page.reload({ waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(1500);
  const afterHardReload = docs.length;

  await browser.close();

  const failures = [];
  if (afterSpaActivate > 0) {
    failures.push(`expected 0 document fetch on in-page scope activation, got ${afterSpaActivate}`);
  }
  if (afterFirstLoad < 1 && afterHardReload < 1) {
    failures.push(
      `expected >=1 document on first load or hard reload (F5 parity baseline), got initial=${afterFirstLoad} reload=${afterHardReload}`,
    );
  }

  const report = {
    ok: failures.length === 0,
    url: appUrl,
    documentFetches: {
      initial: afterFirstLoad,
      spaScopeActivation: afterSpaActivate,
      hardReload: afterHardReload,
    },
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
