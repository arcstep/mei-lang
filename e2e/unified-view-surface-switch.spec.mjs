import { test, expect } from "@playwright/test";

/**
 * LEGACY: three-surface (app/layout/prototype) switch e2e.
 * Access is stage-only navigation now — see docs/mei-lang-v2/03-ui/0334-stage-scene-presentation-and-presenter-freeze.md
 * and 0523 (2.2.9). Prefer stage-switch coverage under /apps/{id}/{stage} when rewritten.
 */
const APP_ID = process.env.MEI_UNIFIED_VIEW_APP || "zhifa";
const COLD_START_TREE_TIMEOUT_MS = Number(process.env.MEI_E2E_TREE_TIMEOUT_MS || 15000);
const SKIP_REASON =
  "skipped: Access is stage-only (0334); layout/prototype surfaces removed on host-shell";

test.describe("unified view surface switch", () => {
  test("F5 app surface keeps host topbar chrome", async ({ page }) => {
    test.skip(true, SKIP_REASON);
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run unified view e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/${APP_ID}/view?surface=app`);
    await page.reload({ waitUntil: "domcontentloaded" });
    await expect
      .poll(
        async () =>
          page.evaluate(() =>
            document.querySelectorAll(
              "#mei-host-topbar-slot .topbar-shell, #mei-host-topbar-slot sl-button[data-mei-app-view]",
            ).length,
          ),
        { timeout: COLD_START_TREE_TIMEOUT_MS },
      )
      .toBeGreaterThan(0);
    const chrome = await page.evaluate(() => ({
      topbarButtons: document.querySelectorAll(
        "#mei-host-topbar-slot .topbar-shell, #mei-host-topbar-slot sl-button[data-mei-app-view]",
      ).length,
      hostChromeReady:
        typeof window.__meiLangBoot?.hostChromeReady === "function"
          ? window.__meiLangBoot.hostChromeReady()
          : false,
    }));
    expect(chrome.hostChromeReady).toBeTruthy();
  });

  test("F5 layout surface shows structure tree within 3s", async ({ page }) => {
    test.skip(true, SKIP_REASON);
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run unified view e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/${APP_ID}/view?surface=layout`);
    await expect
      .poll(
        async () =>
          page.evaluate(
            () => document.querySelectorAll("aside .build-tree-node, .build-tree-shell .build-tree-node").length,
          ),
        { timeout: COLD_START_TREE_TIMEOUT_MS },
      )
      .toBeGreaterThan(0);
    const scopes = await page.evaluate(
      () => document.querySelectorAll("#mei-surface-workspace [data-preview-scope]").length,
    );
    expect(scopes).toBeGreaterThan(0);
  });

  test("app layout prototype round-trip without document fetch", async ({ page }) => {
    test.skip(true, SKIP_REASON);
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run unified view e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const docFetches = [];
    page.on("request", (req) => {
      if (req.resourceType() === "document") docFetches.push(req.url());
    });
    await page.goto(`${base}/apps/${APP_ID}/view?surface=app`);
    await page.waitForLoadState("networkidle");
    docFetches.length = 0;

    const surfaces = ["layout", "prototype", "app"];
    for (const surface of surfaces) {
      await page.evaluate((slug) => {
        const url = new URL(window.location.href);
        url.searchParams.set("surface", slug);
        window.history.pushState({}, "", url.toString());
        return window.__meiLangBoot?.viewAssembly?.assemble
          ? window.__meiLangBoot.viewAssembly.assemble(
              { kind: "cold_start", surfaceSwitch: true, url: url.toString() },
              { debounce: false, surfaceSwitch: true, omit_digests: true },
            )
          : window.__meiLangBoot?.navigateSurface?.(url.toString(), true);
      }, surface);
      await page.waitForTimeout(20000);
    }

    expect(docFetches.length).toBe(0);
    const state = await page.evaluate(() => ({
      surface: new URL(window.location.href).searchParams.get("surface"),
      treeNodes: document.querySelectorAll("aside .build-tree-node, .build-tree-shell .build-tree-node").length,
      appScopes: document.querySelectorAll("#mei-compose-root [data-preview-scope]").length,
      workspaceScopes: document.querySelectorAll(
        "#mei-surface-workspace .preview-pane-scroll [data-preview-scope]",
      ).length,
      previewScopes: document.querySelectorAll("[data-preview-scope]").length,
      surfaceReady:
        typeof window.__meiLangBoot?.isSurfaceMaterialized === "function"
          ? window.__meiLangBoot.isSurfaceMaterialized(
              window.__meiLangBoot.parseViewContext?.(window.location.href) || {},
            )
          : null,
      coordinator: typeof window.__meiLangBoot?.viewAssembly?.assemble === "function",
    }));
    expect(state.surface).toBe("app");
    if (state.coordinator) {
      expect(state.previewScopes).toBeGreaterThan(0);
      expect(state.appScopes).toBeGreaterThan(0);
      if (state.surfaceReady != null) {
        expect(state.surfaceReady).toBeTruthy();
      }
    }
  });

  test("client bundles expose view assembly modules", async ({ page }) => {
    test.skip(true, SKIP_REASON);
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run unified view e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/${APP_ID}/view?surface=layout`);
    const exposed = await page.evaluate(() => ({
      coordinator: typeof window.__meiLangBoot?.viewAssembly?.assemble === "function",
      materializer: typeof window.__meiLangBoot?.renderStructureTree === "function",
      capabilities: typeof window.__meiLangBoot?.hostCapabilitiesReady === "function",
      workspaceUrl: typeof window.isWorkspaceSurfaceUrl === "function",
    }));
    expect(exposed.coordinator).toBeTruthy();
    expect(exposed.materializer).toBeTruthy();
    expect(exposed.capabilities).toBeTruthy();
    expect(exposed.workspaceUrl).toBeTruthy();
  });
});
