#!/usr/bin/env node
/**
 * 0524 §4 filter JIT acceptance + E5 bootstrap writeback observability.
 */
import { chromium } from "@playwright/test";

const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/data-demo/app`;

function isMetricApi(url) {
  return new URL(url).pathname.includes("/api/datasets/metrics/");
}

function isJitPackApi(url) {
  const p = new URL(url).pathname;
  return p.includes("/api/host/scene-bootstrap");
}

async function fetchSnapshot(request) {
  const response = await request.get(
    `${base}/api/runtime/snapshot?appId=${encodeURIComponent("data-demo")}`,
  );
  if (!response.ok()) {
    return null;
  }
  return response.json();
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const metricCalls = [];
  const jitPackCalls = [];

  page.on("request", (req) => {
    const url = req.url();
    if (isMetricApi(url)) {
      metricCalls.push({ method: req.method(), url });
    }
    if (isJitPackApi(url)) {
      jitPackCalls.push({ method: req.method(), url });
    }
  });

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(2000);

  const snapshotBefore = await fetchSnapshot(page.request);
  const revisionBefore = String(
    snapshotBefore?.evalPack?.bootstrapEmbed?.clientRevision ||
      (await page.evaluate(() => String(window.__mei?.client_revision || "").trim())),
  ).trim();

  const baseline = { metric: metricCalls.length, jit: jitPackCalls.length };

  const applied = await page.evaluate(async () => {
    const boot = window.__meiLangBoot || {};
    const sceneId = "home/t2/r-drilldown/s-supervision-warning";
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        appId: "data-demo",
        sceneId,
        scope: sceneId,
        source: "eval-filter-jit-audit",
      });
    }
    await new Promise((resolve) => setTimeout(resolve, 2500));

    const overlay = document.querySelector(
      "#mei-access-drilldown-overlay, #mei-access-scene-board-overlay",
    );
    let queryStateId = "";
    if (overlay) {
      const anchor = overlay.querySelector("[data-props]");
      if (anchor) {
        try {
          const props = JSON.parse(anchor.getAttribute("data-props") || "{}");
          queryStateId = String(props?.query_state || props?.queryState || "").trim();
        } catch (_) {
          /* ignore */
        }
      }
    }
    if (!queryStateId) {
      queryStateId = "home-default";
    }

    if (typeof boot.setQueryState === "function") {
      boot.setQueryState(queryStateId, {
        filters: { unit_name: "测试单位" },
        search: "",
      });
      return `boot.setQueryState:${queryStateId}`;
    }
    if (typeof setQueryState === "function") {
      setQueryState(queryStateId, {
        filters: { unit_name: "测试单位" },
        search: "",
      });
      return `setQueryState:${queryStateId}`;
    }
    document.dispatchEvent(
      new CustomEvent("meilang:query-state-changed", {
        detail: { id: queryStateId, filters: { unit_name: "测试单位" }, search: "" },
      }),
    );
    return "event-only";
  });

  await page.waitForTimeout(4000);

  const snapshotAfter = await fetchSnapshot(page.request);
  const revisionAfter = String(
    snapshotAfter?.evalPack?.bootstrapEmbed?.clientRevision ||
      (await page.evaluate(() => String(window.__mei?.client_revision || "").trim())),
  ).trim();

  const afterFilterMetric = metricCalls.length - baseline.metric;
  const afterFilterJit = jitPackCalls.length - baseline.jit;
  const combinedEvalHttp = afterFilterMetric + afterFilterJit;

  const clientState = await page.evaluate(() => ({
    evalPackSource: String(window.__meiEvalPackSource || ""),
    evalPackMissReason: String(window.__meiEvalPackMissReason || ""),
    hasSetQueryState: typeof (window.__meiLangBoot || {}).setQueryState === "function",
  }));

  await browser.close();

  const failures = [];
  if (applied === "event-only") {
    failures.push("setQueryState not exposed on __meiLangBoot");
  }
  if (afterFilterMetric > 1) {
    failures.push(`expected <=1 metric HTTP after filter change, got ${afterFilterMetric}`);
  }
  if (afterFilterJit > 1) {
    failures.push(`expected <=1 JitEvalPack HTTP after filter change, got ${afterFilterJit}`);
  }
  if (combinedEvalHttp > 1) {
    failures.push(`expected <=1 combined eval HTTP (metric+jit), got ${combinedEvalHttp}`);
  }
  if (!revisionBefore && afterFilterJit === 0 && afterFilterMetric === 0) {
    failures.push("expected bootstrap revision or jit/metric activity after filter");
  }

  const report = {
    ok: failures.length === 0,
    url: appUrl,
    applied,
    metricCallsAfterFilter: afterFilterMetric,
    jitPackCallsAfterFilter: afterFilterJit,
    combinedEvalHttp,
    bootstrapRevision: { before: revisionBefore, after: revisionAfter },
    metricCalls: metricCalls.slice(baseline.metric),
    jitPackCalls: jitPackCalls.slice(baseline.jit),
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
