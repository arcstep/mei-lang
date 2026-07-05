import { test, expect } from "@playwright/test";

test.describe("artifact navigation", () => {
  test("thin_shell flag when explicitly requested", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run artifact e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/app/data-demo/scene/home?thin_shell=1`);
    const thinShell = await page.evaluate(() => window.__mei?.thin_shell === true);
    expect(thinShell).toBeTruthy();
  });

  test("workspace-fragment includes scene_manifest", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run artifact e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    let sawManifest = false;
    page.on("response", async (response) => {
      if (!response.url().includes("/api/build/workspace-fragment")) return;
      try {
        const json = await response.json();
        if (json?.scene_manifest?.schema_version) {
          sawManifest = true;
        }
      } catch (_) {}
    });
    await page.goto(`${base}/apps/build/data-demo/scene/home?tab=preview&node=scene-panel:home`);
    await page.waitForTimeout(3000);
    expect(sawManifest).toBeTruthy();
  });
});
