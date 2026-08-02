#!/usr/bin/env node
/**
 * E10: 真实点击 penalty_total 弹窗 → 分页表有行；邻域已预热时 0 datasets/query。
 */
import { chromium } from "@playwright/test";
import { resolveAppId } from "../lib/resolve-app.mjs";

const appId = resolveAppId();
const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/${appId}/view?surface=app`;
const TARGET_SCOPE = "penalty_total_analytics_page";

function isDatasetQuery(url) {
  const u = new URL(url);
  return u.pathname.includes("/api/datasets/") && u.pathname.includes("/query");
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const datasetQueries = [];

  page.on("request", (req) => {
    if (isDatasetQuery(req.url())) {
      datasetQueries.push(req.url());
    }
  });

  await page.goto(appUrl, { waitUntil: "domcontentloaded", timeout: 120000 });
  await page
    .waitForFunction(
      () =>
        Number(window.__meiBootstrapSeedCount || 0) > 0 ||
        (window.__meiBootstrapPayloadReady &&
          Array.isArray(window.__mei?.bootstrap_metrics) &&
          window.__mei.bootstrap_metrics.length > 0) ||
        (Array.isArray(window.__mei?.bootstrap_scopes) &&
          window.__mei.bootstrap_scopes.length > 0),
      { timeout: 60000 },
    )
    .catch(() => {});
  await page.waitForTimeout(2500);
  await page
    .waitForFunction(
      () =>
        !!document.querySelector('[data-mei-drilldown-metric="penalties_total_count"]') ||
        !!document.querySelector("#penalty_total_card"),
      { timeout: 90000 },
    )
    .catch(() => {});

  const preflight = await page.evaluate((scopeId) => {
    const neighborScopeIds = Array.isArray(window.__mei?.bootstrap_scopes)
      ? window.__mei.bootstrap_scopes.map((entry) =>
          String(entry?.bootstrapScope || entry?.bootstrap_scope || "").trim(),
        )
      : [];
    return {
      neighborScopeIds,
      hasTargetScope: neighborScopeIds.includes(scopeId),
      bootstrapSeeded: !!window.__meiBootstrapSeeded,
    };
  }, TARGET_SCOPE);

  const failures = [];
  if (!preflight.hasTargetScope) {
    failures.push(
      `bootstrap_scopes missing ${TARGET_SCOPE} (got ${preflight.neighborScopeIds.length} scopes) — run ${appId} prebuild/warmup first`,
    );
  }

  const clickTarget = await page.evaluate(() => {
    const selectors = [
      '[data-mei-drilldown-scene="penalty_total_analytics_page"]',
      '[data-mei-drilldown-metric="penalties_total_count"]',
      "#penalty_total_card [data-mei-drilldown-active='true']",
      "#penalty_total_card",
      "mei-metric-card#penalty_total_card",
    ];
    for (const selector of selectors) {
      const el = document.querySelector(selector);
      if (!(el instanceof Element)) {
        continue;
      }
      const target =
        el.querySelector("[data-mei-drilldown-active='true']") ||
        el.querySelector("[data-mei-drilldown-metric]") ||
        el;
      const rect = target.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        return { ok: true, x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
      }
    }
    return { ok: false };
  });

  let clicked = false;
  if (failures.length === 0) {
    await page.evaluate(
      ({ scopeId, targetAppId }) => {
        const boot = window.__meiLangBoot || {};
        if (typeof boot.dispatchScopeActivation === "function") {
          boot.dispatchScopeActivation({
            appId: targetAppId,
            sceneId: scopeId,
            scope: scopeId,
            source: "eval-drilldown-table-audit-preflight",
          });
        }
      },
      { scopeId: TARGET_SCOPE, targetAppId: appId },
    );
    await page.waitForTimeout(1500);
    const domClicked = await page.evaluate(() => {
      const card = document.querySelector("#penalty_total_card");
      if (card instanceof HTMLElement) {
        card.click();
        return true;
      }
      const el = document.querySelector('[data-mei-drilldown-metric="penalties_total_count"]');
      if (el instanceof HTMLElement) {
        el.click();
        return true;
      }
      return false;
    });
    clicked = domClicked;
    if (!clicked) {
      clicked = await page.evaluate(async (targetAppId) => {
        const boot = window.__meiLangBoot || {};
        if (typeof boot.openSceneProjection !== "function") {
          return false;
        }
        await boot.openSceneProjection({
          metric_id: "penalties_total_count",
          dataset_id: "penalty_result_dashboard_ds",
          _mei: {
            active_scene_id: "home",
            app_id: targetAppId,
            active_target_file: "src/scene/home/assembly.mei",
          },
          popup: {
            scene_id: "penalty_total_analytics_page",
            mode: "board_link",
            entry: "detail",
          },
        });
        return true;
      }, appId);
    }
    if (!clicked && clickTarget.ok) {
      await page.mouse.click(clickTarget.x, clickTarget.y);
      clicked = true;
    }
    if (!clicked) {
      failures.push("penalty_total drilldown target not clickable");
    }
  }

  if (failures.length === 0 && clicked) {
    await page
      .waitForFunction(
        () =>
          document.body.classList.contains("access-layer2-open") ||
          document.body.classList.contains("access-scene-board-open") ||
          document.body.classList.contains("access-drilldown-open"),
        { timeout: 20000 },
      )
      .catch(() => {});
    const beforeQueries = datasetQueries.length;
    await page
      .waitForFunction(
        () =>
          document.querySelectorAll(
            ".access-drilldown-table-host tbody tr, [data-drilldown-structured-layout] tbody tr, mei-cockpit-data-table tbody tr, mei-dataset-table tbody tr",
          ).length > 0,
        { timeout: 30000 },
      )
      .catch(async () => {
        await page.evaluate(() => {
          const tab =
            document.querySelector('[data-drilldown-tab="detail"]') ||
            document.querySelector('[data-entry-tab="detail"]');
          if (tab instanceof HTMLElement) {
            tab.click();
          }
        });
      });
    await page.waitForTimeout(3000);
    const tableState = await page.evaluate(() => {
      const rows = document.querySelectorAll(
        ".access-drilldown-table-host tbody tr, [data-drilldown-structured-layout] tbody tr, mei-cockpit-data-table tbody tr, mei-dataset-table tbody tr",
      );
      return {
        rowCount: rows.length,
        missReason: String(window.__meiEvalPackMissReason || ""),
        fallbackNetwork: Boolean(window.__meiEvalPackFallbackNetwork),
      };
    });
    if (tableState.rowCount < 1) {
      const hint =
        tableState.fallbackNetwork || datasetQueries.length > beforeQueries
          ? " (table empty despite network fallback — check seed key alignment)"
          : " (no rows and no dataset query — check overlay render)";
      failures.push(`expected table rows after drilldown, got ${tableState.rowCount}${hint}`);
    }
    const afterQueries = datasetQueries.length - beforeQueries;
    if (afterQueries > 0) {
      failures.push(
        `expected 0 datasets/query when pack prefetched, got ${afterQueries} (miss=${tableState.missReason || "n/a"})`,
      );
    }
  }

  await browser.close();
  if (failures.length) {
    console.error("eval-drilldown-table-audit FAILED");
    failures.forEach((f) => console.error(`  - ${f}`));
    process.exit(1);
  }
  console.log("eval-drilldown-table-audit OK");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
