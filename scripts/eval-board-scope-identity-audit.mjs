#!/usr/bin/env node
/**
 * E11: penalty_total_analytics_page manifest 存在；seed 后 dataset cache 命中。
 */
import { chromium } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/data-demo/view?surface=app`;
const scope = "penalty_total_analytics_page";

function resolveManifestPath(scopeId) {
  const roots = [
    path.join(
      process.cwd(),
      "../workspaces/ws-demo-v2/apps/data-demo/env/current/var/client-bootstrap",
      `${scopeId}.json`,
    ),
    path.join(
      process.cwd(),
      "../workspaces/ws-demo-v2/apps/data-demo/var/active/client-bootstrap",
      `${scopeId}.json`,
    ),
    path.join(
      process.cwd(),
      "../workspaces/ws-demo-v2/apps/data-demo/var/client-bootstrap",
      `${scopeId}.json`,
    ),
  ];
  for (const candidate of roots) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return roots[0];
}

async function main() {
  const failures = [];
  const manifestPath = resolveManifestPath(scope);
  if (!fs.existsSync(manifestPath)) {
    failures.push(
      `missing client-bootstrap manifest for ${scope} (checked env/current/var and var/active)`,
    );
  }

  let bootstrapScopesFromApi = [];
  try {
    const response = await fetch(
      `${base}/api/host/scene-bootstrap?app=data-demo&scene=home`,
      { headers: { Accept: "application/json" } },
    );
    if (response.ok) {
      const payload = await response.json();
      bootstrapScopesFromApi = Array.isArray(payload?.bootstrapScopes)
        ? payload.bootstrapScopes.map((entry) =>
            String(entry?.bootstrapScope || entry?.bootstrap_scope || "").trim(),
          )
        : [];
    }
  } catch (_) {
    /* host may be down; manifest path check still applies */
  }
  if (
    bootstrapScopesFromApi.length > 0 &&
    !bootstrapScopesFromApi.includes(scope)
  ) {
    failures.push(
      `scene-bootstrap missing ${scope} in bootstrapScopes: ${bootstrapScopesFromApi.join(", ")}`,
    );
  }

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
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
  await page.waitForTimeout(2000);

  const activated = await page.evaluate(
    ({ scopeId }) => {
      const boot = window.__meiLangBoot || {};
      if (typeof boot.dispatchScopeActivation === "function") {
        return boot.dispatchScopeActivation({
          appId: "data-demo",
          sceneId: scopeId,
          scope: scopeId,
          source: "eval-board-scope-identity",
        });
      }
      window.dispatchEvent(
        new CustomEvent("meilang:scope-activation", {
          detail: { appId: "data-demo", sceneId: scopeId, scope: scopeId },
        }),
      );
      return true;
    },
    { scopeId: scope },
  );
  if (!activated) {
    failures.push("scope activation dispatch failed");
  }
  await page.waitForTimeout(3500);

  const state = await page.evaluate(() => ({
    bootstrapSeeded: !!window.__meiBootstrapSeeded,
    datasetCacheSize: Number(
      window.__meiEvalStoreReaders?.datasetCacheSize?.() ??
        window.__meiDatasetRuntime?.datasetCache?.size ??
        0,
    ),
    missReason: String(window.__meiEvalPackMissReason || ""),
    fallbackNetwork: Boolean(window.__meiEvalPackFallbackNetwork),
    neighborScopeIds: Array.isArray(window.__mei?.bootstrap_scopes)
      ? window.__mei.bootstrap_scopes.map((entry) =>
          String(entry?.bootstrapScope || entry?.bootstrap_scope || "").trim(),
        )
      : [],
  }));

  if (!state.bootstrapSeeded) {
    failures.push("bootstrap not seeded after scope activation");
  }
  if (state.datasetCacheSize < 1) {
    failures.push(`expected datasetCacheSize > 0, got ${state.datasetCacheSize}`);
  }
  if (state.missReason === "dataset_cache_miss_after_seed" && !state.fallbackNetwork) {
    failures.push("dataset_cache_miss_after_seed without network fallback");
  }

  await browser.close();
  if (failures.length) {
    console.error("eval-board-scope-identity-audit FAILED");
    failures.forEach((f) => console.error(`  - ${f}`));
    process.exit(1);
  }
  console.log("eval-board-scope-identity-audit OK");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
