#!/usr/bin/env node
/**
 * Client-side surface assembly chain audit: F5 vs topbar switch, phase-by-phase.
 * Usage: node scripts/audit/surface-chain-audit.mjs [baseUrl]
 */
import { chromium } from "@playwright/test";
import { writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveAppId } from "../lib/resolve-app.mjs";

const appId = resolveAppId();
const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const outPath = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "tmp",
  `surface-chain-audit-${Date.now()}.json`,
);

function snap(ctx) {
  return {
    href: location.href,
    surfaceParam: new URL(location.href).searchParams.get("surface"),
    bodySurface: document.body?.getAttribute("data-surface"),
    routeMode: globalThis.__mei?.scene_manifest_refs?.compose_defaults?.route_mode,
    appHidden: document.getElementById("mei-surface-app")?.hidden,
    wsHidden: document.getElementById("mei-surface-workspace")?.hidden,
    appScopes: document.querySelectorAll(
      "#mei-compose-root [data-preview-scope], #mei-compose-root [data-mei-frame-viewport]",
    ).length,
    wsScopes: document.querySelectorAll(
      "#mei-surface-workspace .preview-pane-scroll [data-preview-scope], #mei-surface-workspace .preview-pane-scroll [data-mei-frame-viewport]",
    ).length,
    treeNodes: document.querySelectorAll("aside .build-tree-node, .build-tree-shell .build-tree-node")
      .length,
    topbarButtons: document.querySelectorAll(
      "#mei-host-topbar-slot sl-button[data-mei-app-view]",
    ).length,
    fallbackVisible: document.getElementById("mei-thin-shell-fallback")?.hidden === false,
    loadingVisible: !!document.querySelector(
      ".mei-view-loading-overlay:not([hidden]), #mei-thin-shell-fallback:not([hidden])",
    ),
    coordinator: typeof window.__meiLangBoot?.viewAssembly?.assemble === "function",
    hasNormalize: String(window.__meiLangBoot?.viewAssembly?.assemble || "").includes(
      "normalizeAssemblyOpts",
    ),
    bundleMarker:
      typeof window.__meiLangBoot?.captureSurfacePreviewSnapshot === "function" &&
      String(window.__meiLangBoot?.viewAssembly?.assemble || "").length > 0,
  };
}

async function readSurfaceState(page) {
  return page.evaluate(() => {
    const ctx = window.__meiLangBoot?.parseViewContext?.(location.href) || {};
    const surfaceSnapshot =
      typeof window.__meiLangBoot?.surfaceSnapshot === "function"
        ? window.__meiLangBoot.surfaceSnapshot(ctx)
        : null;
    return {
      dom: {
        href: location.href,
        surfaceParam: new URL(location.href).searchParams.get("surface"),
        bodySurface: document.body?.getAttribute("data-surface"),
        routeMode: globalThis.__mei?.scene_manifest_refs?.compose_defaults?.route_mode,
        appHidden: document.getElementById("mei-surface-app")?.hidden,
        wsHidden: document.getElementById("mei-surface-workspace")?.hidden,
        appScopes: document.querySelectorAll(
          "#mei-compose-root [data-preview-scope], #mei-compose-root [data-mei-frame-viewport]",
        ).length,
        wsScopes: document.querySelectorAll(
          "#mei-surface-workspace .preview-pane-scroll [data-preview-scope], #mei-surface-workspace .preview-pane-scroll [data-mei-frame-viewport]",
        ).length,
        treeNodes: document.querySelectorAll(
          "aside .build-tree-node, .build-tree-shell .build-tree-node",
        ).length,
        topbarButtons: document.querySelectorAll(
          "#mei-host-topbar-slot sl-button[data-mei-app-view]",
        ).length,
        fallbackVisible: document.getElementById("mei-thin-shell-fallback")?.hidden === false,
        loadingVisible: !!document.querySelector(
          ".mei-view-loading-overlay:not([hidden]), .mei-thin-shell-fallback:not([hidden])",
        ),
      },
      ready:
        typeof window.__meiLangBoot?.isSurfaceMaterialized === "function"
          ? window.__meiLangBoot.isSurfaceMaterialized(ctx)
          : null,
      readyRelax:
        typeof window.__meiLangBoot?.isSurfaceMaterialized === "function"
          ? window.__meiLangBoot.isSurfaceMaterialized(ctx, { relaxTree: true })
          : null,
      surfaceSnapshot,
      assemblyState: window.__meiLangBoot?.viewAssembly?.getState?.(),
      buildVersion: globalThis.__mei?.build_version || globalThis.__meiBootstrapBuildVersion || null,
    };
  });
}

async function bundleFingerprint(page) {
  const scripts = await page.evaluate(() =>
    Array.from(document.querySelectorAll("script[src]"))
      .map((s) => s.getAttribute("src") || "")
      .filter((src) => src.includes("access") || src.includes("manage") || src.includes("bundle")),
  );
  let accessHasUnified = false;
  for (const src of scripts) {
    if (!src.includes("access")) continue;
    try {
      const url = new URL(src, base).href;
      const res = await page.request.get(url);
      const body = await res.text();
      accessHasUnified =
        body.includes("normalizeAssemblyOpts") && body.includes("surfaceSwitch");
      break;
    } catch (_) {}
  }
  return { scripts, accessHasUnified };
}

async function main() {
  const report = {
    base,
    appId,
    at: new Date().toISOString(),
    bundle: null,
    phases: [],
    steps: [],
    failures: [],
  };

  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  const phaseLog = [];
  const netLog = [];
  page.on("console", (msg) => {
    const t = msg.text();
    if (t.includes("assembly") || t.includes("surface") || t.includes("navigation")) {
      phaseLog.push({ type: "console", text: t });
    }
  });
  page.on("request", (req) => {
    const u = req.url();
    if (
      u.includes("/api/host/view-revision") ||
      u.includes("/api/host/layer-batch") ||
      (req.isNavigationRequest() && req.frame() === page.mainFrame())
    ) {
      netLog.push({ method: req.method(), url: u, nav: req.isNavigationRequest() });
    }
  });

  await page.addInitScript(() => {
    window.__surfaceAudit = { phases: [], assemblies: [] };
    document.addEventListener("mei:assembly-phase", (e) => {
      window.__surfaceAudit.phases.push({ ...e.detail, t: performance.now() });
    });
    const asm = window.__meiLangBoot?.viewAssembly;
    if (asm?.assemble) {
      const orig = asm.assemble.bind(asm);
      asm.assemble = async (...args) => {
        const t0 = performance.now();
        const r = await orig(...args);
        window.__surfaceAudit.assemblies.push({
          intent: args[0],
          options: args[1],
          result: { ok: r?.ok, reason: r?.preview?.assemble?.reason, generation: r?.generation },
          ms: Math.round(performance.now() - t0),
        });
        return r;
      };
    }
  });

  // Access stage path (canonical). layout/prototype product surfaces are sealed → 301 to stage.
  const stageUrl = `${base}/apps/${appId}/home`;
  netLog.length = 0;
  const t0 = Date.now();
  let navError = null;
  try {
    await page.goto(stageUrl, { waitUntil: "domcontentloaded", timeout: 120000 });
    await page.waitForTimeout(3000);
  } catch (e) {
    navError = String(e.message || e);
  }
  const state = await readSurfaceState(page);
  report.steps.push({
    mode: "F5",
    surface: "app",
    url: stageUrl,
    ms: Date.now() - t0,
    navError,
    network: [...netLog],
    state,
    pass:
      !navError &&
      state.ready === true &&
      !state.dom.fallbackVisible &&
      state.dom.appScopes > 0,
  });
  if (navError) report.failures.push(`F5 stage: ${navError}`);
  else if (state.ready !== true) report.failures.push(`F5 stage: isSurfaceMaterialized=false`);
  else if (state.dom.appScopes === 0) report.failures.push(`F5 stage: preview empty`);

  for (const sealed of ["layout", "prototype"]) {
    const sealedUrl = `${base}/apps/${appId}/view?surface=${sealed}`;
    const resp = await page.goto(sealedUrl, {
      waitUntil: "domcontentloaded",
      timeout: 60000,
    });
    const finalUrl = page.url();
    const redirectedToStage =
      /\/apps\/[^/]+\/[^/?]+/.test(finalUrl) && !finalUrl.includes("/view?");
    report.steps.push({
      mode: "seal-redirect",
      surface: sealed,
      url: sealedUrl,
      finalUrl,
      status: resp?.status?.() ?? null,
      pass: redirectedToStage,
    });
    if (!redirectedToStage) {
      report.failures.push(`seal ${sealed}: expected redirect to /apps/{id}/{stage}, got ${finalUrl}`);
    }
  }

  report.bundle = await bundleFingerprint(page);

  await page.goto(`${base}/apps/${appId}/home`, {
    waitUntil: "domcontentloaded",
    timeout: 120000,
  });
  await page.waitForTimeout(3000);

  const switchSequence = [
    { label: "布局", surface: "layout" },
    { label: "原型", surface: "prototype" },
    { label: "应用", surface: "app" },
    { label: "布局", surface: "layout" },
  ];

  for (const { label, surface } of switchSequence) {
    netLog.length = 0;
    phaseLog.length = 0;
    await page.evaluate(() => {
      window.__surfaceAudit = { phases: [], assemblies: [] };
    });
    const t0 = Date.now();
    const btn = page.locator(`sl-button[data-mei-app-view="${label}"]`).first();
    const btnCount = await btn.count();
    if (btnCount === 0) {
      report.failures.push(`switch ${surface}: topbar button missing`);
      continue;
    }

    const navPromise = page.waitForNavigation({ waitUntil: "domcontentloaded", timeout: 120000 }).catch(() => null);
    await btn.evaluate((el) => el.click());
    await navPromise;
    await page
      .waitForFunction(
        (expected) => {
          const param = new URL(location.href).searchParams.get("surface");
          if (param !== expected) return false;
          const ctx = window.__meiLangBoot?.parseViewContext?.(location.href) || {};
          return window.__meiLangBoot?.isSurfaceMaterialized?.(ctx);
        },
        surface,
        { timeout: 60000 },
      )
      .catch(() => {});
    await page.waitForTimeout(surface === "app" ? 2000 : 3000);

    const audit = await page.evaluate(() => window.__surfaceAudit || {});
    const state = await readSurfaceState(page);
    const f5Baseline = report.steps.find((s) => s.mode === "F5" && s.surface === surface);

    const switchScopes = surface === "app" ? state.dom.appScopes : state.dom.wsScopes;
    const f5Scopes = f5Baseline
      ? f5Baseline.surface === "app"
        ? f5Baseline.state.dom.appScopes
        : f5Baseline.state.dom.wsScopes
      : null;
    const f5Tree = f5Baseline?.state?.surfaceSnapshot?.treeNodes ?? f5Baseline?.state?.dom?.treeNodes;
    const switchTree = state.surfaceSnapshot?.treeNodes ?? state.dom.treeNodes;
    const scopeParity =
      f5Scopes == null ? true : switchScopes > 0 && Math.abs(switchScopes - f5Scopes) <= 2;
    const treeParity =
      surface === "app" || f5Tree == null
        ? true
        : switchTree > 0 && Math.abs(switchTree - f5Tree) <= 2;
    const urlAppOk = state.dom.href.includes(`/apps/${appId}/`);

    const step = {
      mode: "switch",
      label,
      surface,
      ms: Date.now() - t0,
      network: [...netLog],
      documentFetches: netLog.filter((n) => n.nav).length,
      phases: audit.phases || [],
      assemblies: audit.assemblies || [],
      state,
      f5Compare: f5Baseline
        ? {
            f5Scopes,
            switchScopes,
            f5Tree,
            switchTree,
            f5RouteMode: f5Baseline.state.dom.routeMode,
            switchRouteMode: state.dom.routeMode,
            scopeParity,
            treeParity,
          }
        : null,
      pass:
        urlAppOk &&
        state.ready === true &&
        !state.dom.fallbackVisible &&
        !state.dom.loadingVisible &&
        switchScopes > 0 &&
        state.dom.routeMode === surface &&
        scopeParity &&
        treeParity,
    };
    report.steps.push(step);

    if (!report.bundle?.accessHasUnified) {
      report.failures.push("bundle: access.bundle.js missing normalizeAssemblyOpts (stale server binary?)");
    }
    if (state.dom.fallbackVisible) report.failures.push(`switch ${surface}: fallback visible`);
    if (state.dom.loadingVisible) report.failures.push(`switch ${surface}: loading stuck`);
    if (state.dom.routeMode !== surface)
      report.failures.push(`switch ${surface}: routeMode=${state.dom.routeMode}`);
    if (surface === "app" && state.dom.appScopes === 0)
      report.failures.push(`switch app: preview empty`);
    if (surface !== "app" && state.dom.wsScopes === 0)
      report.failures.push(`switch ${surface}: workspace preview empty`);
    if (state.ready !== true) report.failures.push(`switch ${surface}: isSurfaceMaterialized=false`);
    if (!urlAppOk) report.failures.push(`switch ${surface}: url app mismatch ${state.dom.href}`);
    if (!scopeParity)
      report.failures.push(
        `switch ${surface}: scope parity F5=${f5Scopes} switch=${switchScopes}`,
      );
    if (!treeParity)
      report.failures.push(`switch ${surface}: tree parity F5=${f5Tree} switch=${switchTree}`);
  }

  await browser.close();

  report.summary = {
    total: report.steps.length,
    passed: report.steps.filter((s) => s.pass).length,
    failed: report.failures.length,
    bundleUnified: report.bundle?.accessHasUnified,
  };

  try {
    writeFileSync(outPath, JSON.stringify(report, null, 2));
    console.log("Wrote", outPath);
  } catch (e) {
    console.log(JSON.stringify(report, null, 2));
  }

  console.log("\n=== SURFACE CHAIN AUDIT ===");
  console.log("bundle unified:", report.bundle?.accessHasUnified);
  for (const s of report.steps) {
    const scopes = s.surface === "app" ? s.state?.dom?.appScopes : s.state?.dom?.wsScopes;
    console.log(
      `${s.pass ? "PASS" : "FAIL"} [${s.mode}] ${s.surface} scopes=${scopes} route=${s.state?.dom?.routeMode} ready=${s.state?.ready} ms=${s.ms}` +
        (s.assemblies?.length ? ` asm=${JSON.stringify(s.assemblies.map((a) => a.result))}` : "") +
        (s.phases?.length ? ` phases=${s.phases.map((p) => p.phase).join(">")}` : ""),
    );
  }
  if (report.failures.length) {
    console.log("\nFAILURES:");
    for (const f of report.failures) console.log(" -", f);
    process.exitCode = 1;
  } else {
    console.log("\nAll chain audit checks passed.");
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
