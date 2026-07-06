#!/usr/bin/env node
/**
 * Route A preview surfaceHtml IDB cache acceptance.
 * Usage: MEI_E2E_BASE_URL=http://127.0.0.1:9527 node scripts/preview-surface-cache-audit.mjs [path]
 */
import { chromium } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.argv[2] || "http://127.0.0.1:9527").replace(
  /\/+$/,
  "",
);
const viewPath = process.argv.find((a) => a.startsWith("/")) || "/apps/data-demo/view?surface=app";
const appUrl = `${base}${viewPath.startsWith("/") ? viewPath : `/${viewPath}`}`;

function isHtmlFragment(url) {
  try {
    const u = new URL(url);
    return u.pathname.includes("/api/host/scene-fragment") && u.searchParams.get("format") === "html";
  } catch {
    return false;
  }
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const fragmentCalls = { first: [], second: [] };
  let pass = "first";

  page.on("request", (req) => {
    if (!isHtmlFragment(req.url())) return;
    fragmentCalls[pass].push(req.url());
  });

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page
    .waitForFunction(
      () =>
        document.querySelector("[data-preview-scope], [data-mei-frame-viewport], [data-mei-use-key]") ||
        document.querySelector(".preview-pane-scroll:not([data-mei-compose-placeholder='1'])"),
      { timeout: 60000 },
    )
    .catch(() => {});

  pass = "second";
  await page.reload({ waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(1500);

  const clientState = await page.evaluate(() => {
    const pipeline = window.__meiRenderPipeline?.last || null;
    const marks = Array.isArray(pipeline?.marks) ? pipeline.marks : [];
    const previewEnd = marks.find((m) => m?.name === "preview_fragment:end");
    return {
      evalPackSource: String(window.__meiEvalPackSource || ""),
      bootstrapFromLocal: !!window.__meiBootstrapFromLocalStorage,
      previewEndSource: String(previewEnd?.detail?.source || ""),
      pipelineMarks: marks.map((m) => m.name),
    };
  });

  await browser.close();

  const failures = [];
  if (fragmentCalls.second.length > 0) {
    failures.push(
      `expected 0 scene-fragment requests on second F5, got ${fragmentCalls.second.length}`,
    );
  }
  if (
    clientState.previewEndSource &&
    clientState.previewEndSource !== "idb" &&
    clientState.previewEndSource !== "compose"
  ) {
    failures.push(
      `expected preview_fragment:end source idb|compose|absent, got ${clientState.previewEndSource}`,
    );
  }

  const report = {
    ok: failures.length === 0,
    url: appUrl,
    fragmentCalls,
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
