#!/usr/bin/env node
/**
 * Pack-first strict audit: no legacy revision API, no whole-page scene fetch, compose-only preview.
 * Usage: MEI_E2E_BASE_URL=http://127.0.0.1:9527 node scripts/pack-first-strict-audit.mjs [path]
 */
import { chromium } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.argv[2] || "http://127.0.0.1:9527").replace(
  /\/+$/,
  "",
);
const viewPath =
  process.argv.find((a) => a.startsWith("/") && !a.includes("://")) ||
  "/apps/zhifa/view?surface=app&scene=home";
const appUrl = `${base}${viewPath.startsWith("/") ? viewPath : `/${viewPath}`}`;

function isLegacyHostApi(url) {
  try {
    const path = new URL(url).pathname;
    return (
      path.includes("/api/host/scene-revision") ||
      path.includes("/api/host/scene-fragment")
    );
  } catch {
    return false;
  }
}

function isUnauthorizedPackFirstEvalApi(url, method) {
  if (method !== "POST" && method !== "GET") return false;
  try {
    const path = new URL(url).pathname;
    if (!path.includes("/api/datasets/metrics/") && !path.includes("/api/datasets/query")) {
      return false;
    }
    const u = new URL(url);
    const deliveryClass = u.searchParams.get("delivery_class") || "";
    const allowed = ["dataframe_page_n", "media_blob", "map_tile", "mesh_asset"];
    if (allowed.includes(deliveryClass)) return false;
    return true;
  } catch {
    return false;
  }
}

function isWholePageSceneFetch(url, method) {
  if (method !== "GET") return false;
  try {
    const u = new URL(url);
    return (
      /\/apps\/(?:app|access|run|presentation|slides|copilot)\/[^/]+\/scene\//.test(u.pathname) &&
      !u.pathname.includes("/api/")
    );
  } catch {
    return false;
  }
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const legacyCalls = [];
  const wholePageSceneFetches = [];
  const unauthorizedEvalCalls = [];
  const activateCalls = [];
  let bootstrapReadyAt = null;
  let firstActivateAt = null;

  page.on("request", (req) => {
    const url = req.url();
    if (isLegacyHostApi(url)) legacyCalls.push(url);
    if (isWholePageSceneFetch(url, req.method())) wholePageSceneFetches.push(url);
    if (isUnauthorizedPackFirstEvalApi(url, req.method())) {
      unauthorizedEvalCalls.push(url);
    }
    if (url.includes("/api/host/mrg/activate")) {
      activateCalls.push({ at: Date.now(), url });
      if (!firstActivateAt) firstActivateAt = Date.now();
    }
    if (url.includes("/api/host/scene-bootstrap") && !bootstrapReadyAt) {
      bootstrapReadyAt = Date.now();
    }
  });

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page
    .waitForFunction(
      () =>
        document.querySelector("[data-preview-scope], [data-mei-frame-viewport], [data-mei-use-key]") ||
        document.querySelector(".preview-pane-scroll[data-mei-compose-materialized='1']"),
      { timeout: 60000 },
    )
    .catch(() => {});

  const clientState = await page.evaluate(() => {
    const marks = window.__meiRenderPipeline?.last?.marks || [];
    const previewEnd = marks.find((m) => m?.name === "preview_compose:end");
    return {
      bootstrapPayloadReady: !!window.__meiBootstrapPayloadReady,
      bootstrapSeeded: !!window.__meiBootstrapSeeded,
      previewEndSource: String(previewEnd?.detail?.source || ""),
      materialized:
        document
          .querySelector("#mei-compose-root, .preview-pane-scroll, .shell")
          ?.getAttribute("data-mei-compose-materialized") === "1",
    };
  });

  await browser.close();

  const failures = [];
  if (legacyCalls.length > 0) {
    failures.push(`expected 0 scene-revision/fragment calls, got ${legacyCalls.length}`);
  }
  if (wholePageSceneFetches.length > 0) {
    failures.push(`expected 0 whole-page scene HTML fetches, got ${wholePageSceneFetches.length}`);
  }
  if (
    clientState.previewEndSource &&
    !["compose", "assemble_local"].includes(clientState.previewEndSource)
  ) {
    failures.push(`preview_compose:end source=${clientState.previewEndSource}`);
  }
  if (clientState.previewEndSource === "ssr_preview") {
    failures.push("thin shell must not report ssr_preview");
  }
  if (firstActivateAt && !clientState.bootstrapPayloadReady && !clientState.bootstrapSeeded) {
    failures.push("mrg/activate observed before bootstrap seed on cold load");
  }
  if (unauthorizedEvalCalls.length > 0) {
    failures.push(
      `expected 0 default metrics/dataset fetches without delivery_class whitelist, got ${unauthorizedEvalCalls.length}`,
    );
  }
  if (!clientState.materialized && clientState.previewEndSource !== "assemble_local") {
    failures.push("preview surface not materialized after cold load");
  }

  const report = {
    ok: failures.length === 0,
    url: appUrl,
    legacyCalls,
    wholePageSceneFetches,
    unauthorizedEvalCalls: unauthorizedEvalCalls.length,
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
