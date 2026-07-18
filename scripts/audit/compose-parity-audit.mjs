#!/usr/bin/env node
/**
 * 0526 compose parity gate: structure / eval / runtime.plans after cold load.
 * Usage: MEI_E2E_BASE_URL=http://127.0.0.1:9527 node scripts/audit/compose-parity-audit.mjs [path]
 */
import { chromium } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.argv[2] || "http://127.0.0.1:9527").replace(
  /\/+$/,
  "",
);
const viewPath =
  process.argv.find(
    (a) => a.startsWith("/apps/") || a.startsWith("/manage/") || a.startsWith("/run/"),
  ) || "/apps/zhifa/view?surface=app&scene=home";
const appUrl = `${base}${viewPath.startsWith("/") ? viewPath : `/${viewPath}`}`;

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page
    .waitForFunction(
      () =>
        document.querySelector("[data-mei-frame-viewport], [data-preview-scope], [data-mei-use-key]"),
      { timeout: 60000 },
    )
    .catch(() => {});

  const state = await page.evaluate(() => {
    const root =
      document.querySelector("#mei-compose-root, .preview-pane-scroll, .shell") ||
      document.body;
    const marks = window.__meiRenderPipeline?.last?.marks || [];
    const previewEnd = marks.find((m) => m?.name === "preview_compose:end");
    return {
      thinShell: !!window.__mei?.thin_shell,
      layerPlan: !!window.__mei?.layer_plan,
      presentationMap: !!window.__mei?.presentation_map,
      componentAssets: Array.isArray(window.__mei?.component_assets)
        ? window.__mei.component_assets.length
        : 0,
      structureViewport: !!document.querySelector("[data-mei-frame-viewport]"),
      evalProps: !!document.querySelector("[data-props]"),
      evalHost: !!document.querySelector("[data-mei-use-key] .component-host *"),
      materialized: root?.getAttribute("data-mei-compose-materialized") === "1",
      composeProjection: String(root?.getAttribute("data-compose-projection") || ""),
      previewEndSource: String(previewEnd?.detail?.source || ""),
      themeVar: getComputedStyle(document.documentElement).getPropertyValue("--mei-theme-id").trim(),
      warningHeadCarets: !!document.querySelector(
        '[data-preview-scope$="/warning/head"] [data-mei-head-carets]',
      ),
    };
  });

  await browser.close();

  const failures = [];
  const isSupervisionMini = appUrl.includes("mini-data");
  if (!state.structureViewport && !state.evalProps && !state.evalHost) {
    failures.push("missing structure viewport or eval mounts");
  }
  if (!state.layerPlan) {
    failures.push("runtime.plans not applied (__mei.layer_plan missing)");
  }
  if (!state.materialized && state.previewEndSource !== "assemble_local") {
    failures.push("preview root not materialized");
  }
  if (state.previewEndSource === "ssr_preview") {
    failures.push("thin shell reported ssr_preview");
  }
  if (state.thinShell && !state.composeProjection) {
    failures.push("data-compose-projection missing on thin shell root");
  }
  if (isSupervisionMini && !state.warningHeadCarets) {
    failures.push("mini-data warning head missing data-mei-head-carets chrome");
  }

  const report = { ok: failures.length === 0, url: appUrl, state, failures };
  console.log(JSON.stringify(report, null, 2));
  if (failures.length > 0) process.exit(1);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
