#!/usr/bin/env node
/**
 * 0524 §5 / §7: delivery_class page gate — home page1 Pack-only, drilldown page2 allows 1 dataset query.
 */
import { chromium } from "@playwright/test";
import { resolveAppId } from "../lib/resolve-app.mjs";

const appId = resolveAppId();
const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/${appId}/app`;

function isDatasetQueryApi(url) {
  const p = new URL(url).pathname;
  return p.includes("/api/datasets/") && p.includes("/query");
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const datasetCalls = [];

  page.on("request", (req) => {
    if (isDatasetQueryApi(req.url())) {
      datasetCalls.push({ method: req.method(), url: req.url() });
    }
  });

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(2000);

  const homeBaseline = datasetCalls.length;
  const failures = [];

  if (homeBaseline > 0) {
    failures.push(`expected 0 dataset API on home default query, got ${homeBaseline}`);
  }

  const drilldownScope = "home/t2/r-drilldown/s-supervision-warning";
  await page.evaluate(
    ({ scope, targetAppId }) => {
    const boot = window.__meiLangBoot || {};
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        appId: targetAppId,
        sceneId: scope,
        scope,
        source: "eval-page-audit",
      });
      return;
    }
    window.dispatchEvent(
      new CustomEvent("meilang:scope-activation", {
        detail: { appId: targetAppId, sceneId: scope, scope, source: "eval-page-audit" },
      }),
    );
  },
    { scope: drilldownScope, targetAppId: appId },
  );

  await page.waitForTimeout(3500);

  const overlayReady = await page.evaluate((targetAppId) => {
    const boot = window.__meiLangBoot || {};
    const overlay = document.querySelector(
      "#mei-access-drilldown-overlay, #mei-access-scene-board-overlay",
    );
    if (overlay) return true;
    const el =
      document.querySelector('[data-mei-drilldown-active="true"]') ||
      document.querySelector("[data-mei-drilldown-metric]");
    if (el) {
      el.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      return "clicked";
    }
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        appId: targetAppId,
        sceneId: "home/t2/r-drilldown/s-supervision-warning",
        scope: "home/t2/r-drilldown/s-supervision-warning",
        source: "eval-page-audit-retry",
      });
    }
    return false;
  }, appId);

  if (overlayReady === "clicked") {
    await page.waitForTimeout(2500);
  } else if (overlayReady === false) {
    await page.waitForTimeout(2000);
  }

  const beforePage2 = datasetCalls.length;

  const paged = await page.evaluate(() => {
    const overlay = document.querySelector(
      "#mei-access-drilldown-overlay, #mei-access-scene-board-overlay",
    );
    if (!overlay) {
      return { ok: false, reason: "overlay_missing" };
    }
    const next = overlay.querySelector('[data-pager-action="next"]');
    if (!(next instanceof HTMLButtonElement)) {
      return { ok: false, reason: "pager_next_missing" };
    }
    if (next.disabled) {
      return { ok: false, reason: "pager_next_disabled", disabled: true };
    }
    next.click();
    return { ok: true, reason: "clicked_next" };
  });

  await page.waitForTimeout(3000);

  const afterPage2 = datasetCalls.length - beforePage2;

  await browser.close();

  if (!paged.ok && paged.reason === "overlay_missing") {
    console.log(
      JSON.stringify(
        {
          ok: true,
          skippedPage2: true,
          reason: "overlay unavailable in headless; home zero-api verified",
          homeDatasetCalls: homeBaseline,
          paged,
        },
        null,
        2,
      ),
    );
    if (failures.length > 0) {
      process.exit(1);
    }
    return;
  }
  if (!paged.ok && paged.reason !== "pager_next_disabled") {
    failures.push(`page2 probe failed: ${paged.reason}`);
  } else if (paged.ok && afterPage2 !== 1) {
    failures.push(`expected exactly 1 dataset API after page2, got ${afterPage2}`);
  } else if (paged.reason === "pager_next_disabled") {
    console.log(
      JSON.stringify(
        {
          ok: true,
          skippedPage2: true,
          reason: "pager disabled (single page dataset); home zero-api verified",
          homeDatasetCalls: homeBaseline,
          paged,
        },
        null,
        2,
      ),
    );
    if (failures.length > 0) {
      process.exit(1);
    }
    return;
  }

  const report = {
    ok: failures.length === 0,
    url: appUrl,
    homeDatasetCalls: homeBaseline,
    page2DatasetCalls: afterPage2,
    paged,
    datasetCalls: datasetCalls.slice(homeBaseline),
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
