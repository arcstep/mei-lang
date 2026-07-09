/**
 * 0517 收敛手测清单（layout draft compose + app 演说无 run 路由）
 */
import { test, expect } from "@playwright/test";

const APP = process.env.MEI_HAND_APP || "data-demo";

test.describe("0517 convergence hand checklist", () => {
  test("legacy build/run/copilot routes return 404", async ({ request }) => {
    for (const path of [
      `/apps/build/${APP}`,
      `/apps/manage/${APP}`,
      `/apps/run/${APP}`,
      `/apps/copilot/${APP}/presentation/home`,
    ]) {
      const res = await request.get(path);
      expect(res.status(), path).toBe(404);
    }
  });

  test("legacy layout-tuning routes are gone", async ({ request }) => {
    for (const path of [
      `/api/ops/layout-tuning/draft/${APP}`,
      `/api/ops/layout-tuning/overlay/${APP}`,
      `/api/ops/layout-tuning/apply/${APP}`,
    ]) {
      const res = await request.put(path, { data: {} }).catch(async () =>
        request.get(path),
      );
      // Deleted stack: no live dual-path handlers (404/405/410 all acceptable).
      expect([404, 405, 410]).toContain(res.status());
    }
    const themeOverlay = await request.get(`/api/ops/themes/layout/overlay/${APP}`);
    expect([200, 401, 403, 404]).toContain(themeOverlay.status());
  });

  test("layout surface loads and scene-manifest accepts surface query", async ({
    page,
    request,
  }) => {
    const manifestRes = await request.get(
      `/api/host/scene-manifest?app_id=${APP}&scene=home&surface=layout`,
    );
    if (manifestRes.status() === 404) {
      test.skip(true, "scene-manifest API requires mei-host-shell stack or ACCESS READY workspace");
    }
    expect(manifestRes.ok()).toBeTruthy();
    const payload = await manifestRes.json();
    expect(payload.manifest?.compose_defaults?.route_mode).toBe("layout");

    await page.goto(`/apps/${APP}/view?surface=layout&scene=home&tab=preview`);
    await expect(page.locator(".shell, #mei-compose-root, .preview-pane-scroll").first()).toBeVisible({
      timeout: 60000,
    });
    const hasDraftStore = await page.evaluate(
      () => typeof window.__meiLangBoot?.sceneManifestLoader?.resolveWorkspaceSurface === "function",
    );
    expect(hasDraftStore).toBe(true);
  });

  test("run/copilot routes 404 and app URL never navigates to run", async ({
    page,
    request,
  }) => {
    for (const path of [`/apps/run/${APP}`, `/apps/copilot/${APP}/presentation/home`]) {
      const res = await request.get(path);
      expect(res.status(), path).toBe(404);
    }
    const response = await page.goto(`/apps/${APP}/view?surface=app&scene=home`, {
      waitUntil: "domcontentloaded",
      timeout: 60000,
    });
    if (!response || response.status() >= 500) {
      test.skip(true, "app surface requires ACCESS READY workspace prebuild");
    }
    expect(page.url()).not.toMatch(/\/apps\/(?:run|copilot)\//);
    await page.keyboard.press("ArrowRight");
    await page.waitForTimeout(500);
    expect(page.url()).not.toMatch(/\/apps\/(?:run|copilot)\//);
  });

  test("legacy layout URL 301 redirects to unified view", async ({ request }) => {
    const res = await request.get(`/apps/${APP}/layout?scene=home`, {
      maxRedirects: 0,
    });
    expect(res.status()).toBe(301);
    const location = res.headers().location || "";
    expect(location).toContain(`/apps/${APP}/view`);
    expect(location).toContain("surface=layout");
  });

  test("surface switch avoids full document navigation", async ({ page, request }) => {
    const manifestRes = await request.get(
      `/api/host/scene-manifest?app_id=${APP}&scene=home&surface=app`,
    );
    if (manifestRes.status() === 404) {
      test.skip(true, "requires mei-host-shell stack");
    }
    let documentLoads = 0;
    page.on("framenavigated", (frame) => {
      if (frame === page.mainFrame()) documentLoads += 1;
    });
    await page.goto(`/apps/${APP}/view?surface=app&scene=home`, {
      waitUntil: "domcontentloaded",
      timeout: 60000,
    });
    const loadsAfterColdStart = documentLoads;
    const layoutTab = page.locator('sl-button[data-mei-app-view="布局"]');
    if ((await layoutTab.count()) === 0) {
      test.skip(true, "topbar mode tabs not rendered for this app");
    }
    await layoutTab.first().click();
    await page.waitForTimeout(2000);
    expect(page.url()).toContain("surface=layout");
    expect(documentLoads).toBe(loadsAfterColdStart);
  });
});
