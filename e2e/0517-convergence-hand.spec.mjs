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

  test("layout-tuning draft PUT returns 410 Gone", async ({ request }) => {
    const res = await request.put(`/api/ops/layout-tuning/draft/${APP}`, {
      data: { tuning: {} },
    });
    expect(res.status()).toBe(410);
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

    await page.goto(`/apps/${APP}/layout?scene=home&tab=preview`);
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
    const response = await page.goto(`/apps/${APP}/app?scene=home`, {
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
});
