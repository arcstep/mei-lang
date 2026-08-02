#!/usr/bin/env node
/**
 * Evidence collector: surface switch + hard refresh network/cache behavior.
 * Usage: node scripts/audit/cache-behavior-audit.mjs http://127.0.0.1:9527
 */
import { chromium } from "@playwright/test";
import { resolveAppId } from "../lib/resolve-app.mjs";

const appId = resolveAppId();
const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/${appId}/view`;

function classify(url) {
  const u = new URL(url);
  const p = u.pathname;
  if (p.includes("/view") && !p.includes("/api/")) return "document";
  if (p.includes("/api/host/view-revision")) {
    const st = u.searchParams.get("surface") || "?";
    const md = u.searchParams.has("manifest_revision_digest");
    const sd = u.searchParams.has("surface_revision_digest");
    return `view-revision:${st}:digests=${md && sd}`;
  }
  if (p.includes("/api/host/scene-manifest")) return "scene-manifest";
  if (p.includes("/api/host/scene-bootstrap")) return "scene-bootstrap";
  if (p.includes("/api/datasets/metrics/")) return "plug-ds-metrics";
  if (p.includes("/api/datasets/") && p.includes("/query")) return "plug-ds-dataset-query";
  if (p.includes("/api/plug-ds/")) return "plug-ds";
  return "other";
}

async function snapshotClient(page, surface, targetAppId) {
  return page.evaluate(
    async ({ surface: surfaceName, targetAppId: resolvedAppId }) => {
    const boot = window.__meiLangBoot || {};
    const ctx = boot.parseViewContext?.(window.location.href) || {};
    const vrCtx = {
      ...ctx,
      surface: surfaceName,
      app_id: ctx.app_id || ctx.appId,
      scene_id: ctx.scene_id || ctx.sceneId,
    };
    const digests = boot.readClientDigests?.(vrCtx) || {};
    const stored = boot.readViewRevision?.(vrCtx) || null;
    const holdings =
      (await boot.layerArtifactCache?.listHoldings?.(
        ctx.app_id || ctx.appId || resolvedAppId,
        ctx.scene_id || ctx.sceneId || "home",
      )) || [];
    const refs = window.__mei?.scene_manifest_refs || {};
    return {
      surface: surfaceName,
      url: location.href,
      digests,
      store_key: boot.viewRevisionStoreKey?.(vrCtx) || null,
      stored_surface_compose: stored?.surface_compose || null,
      stored_has_manifest: !!stored?.manifest_snapshot?.layers,
      refs_route_mode: refs.compose_defaults?.route_mode || null,
      refs_surface_digest: refs.surface_revision_digest || null,
      idb_holdings: holdings.length,
      lastOutcome: boot.lastViewRevisionOutcome || null,
    };
  },
    { surface, targetAppId },
  );
}

async function clickSurface(page, surface) {
  const label = { app: "应用", layout: "布局", prototype: "原型" }[surface];
  const btn = page.locator(`sl-button[data-mei-app-view]:has-text("${label}")`).first();
  await btn.click({ timeout: 30000 });
  await page.waitForURL(new RegExp(`surface=${surface}`), { timeout: 30000 });
}

async function runPhase(page, label, action) {
  const events = [];
  const onReq = (req) => {
    const kind = classify(req.url());
    if (kind === "other") return;
    events.push({ kind, method: req.method(), url: req.url().slice(0, 200) });
  };
  const onRes = async (res) => {
    const kind = classify(res.url());
    if (!kind.startsWith("view-revision")) return;
    let status = "";
    try {
      status = res.headers()["x-mei-view-revision-status"] || "";
    } catch (_) {}
    events.push({
      kind: `${kind}:status=${status || "?"}`,
      method: "RES",
      url: String(res.headers()["content-length"] || "?") + "B",
    });
  };
  page.on("request", onReq);
  page.on("response", onRes);
  const t0 = Date.now();
  await action();
  await page.waitForTimeout(2000);
  page.off("request", onReq);
  page.off("response", onRes);
  const client = await snapshotClient(
    page,
    new URL(page.url()).searchParams.get("surface") || "app",
    appId,
  );
  return { label, ms: Date.now() - t0, requests: events, client };
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  const report = { base, phases: [] };

  report.phases.push(
    await runPhase(page, "A_cold_app", async () => {
      await page.goto(`${appUrl}?surface=app`, { waitUntil: "networkidle", timeout: 120000 });
    }),
  );

  for (const surface of ["prototype", "layout", "app", "prototype"]) {
    report.phases.push(
      await runPhase(page, `B_switch_${surface}`, async () => {
        await clickSurface(page, surface);
      }),
    );
  }

  report.phases.push(
    await runPhase(page, "C_hard_refresh_prototype", async () => {
      await page.reload({ waitUntil: "networkidle", timeout: 120000 });
    }),
  );

  report.phases.push(
    await runPhase(page, "D_switch_layout_after_refresh", async () => {
      await clickSurface(page, "layout");
    }),
  );

  await browser.close();

  for (const phase of report.phases) {
    console.log(`\n=== ${phase.label} (${phase.ms}ms) ===`);
    const byKind = {};
    for (const r of phase.requests) {
      byKind[r.kind] = (byKind[r.kind] || 0) + 1;
    }
    console.log("requests:", JSON.stringify(byKind));
    for (const r of phase.requests) {
      console.log(`  ${r.kind}`);
    }
    console.log("client:", JSON.stringify(phase.client, null, 2));
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
