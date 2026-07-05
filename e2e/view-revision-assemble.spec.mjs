import { test, expect } from "@playwright/test";

test.describe("view-revision assemble", () => {
  test("view-revision API returns status field", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const response = await page.request.get(
      `${base}/api/host/view-revision?app_id=data-demo&scene=home&surface=app&compose=${encodeURIComponent(
        JSON.stringify({ route_mode: "app", review_projection: "live_full" }),
      )}`,
    );
    expect(response.ok()).toBeTruthy();
    const json = await response.json();
    expect(["refetch", "assemble_local"]).toContain(json.status);
    expect(json.manifest_revision_digest).toBeTruthy();
  });

  test("client bundles expose view-revision modules", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/app/data-demo/scene/home`);
    const exposed = await page.evaluate(() => ({
      viewRevision: typeof window.__meiLangBoot?.viewRevisionClient?.fetchViewRevision === "function",
      layerCache: typeof window.__meiLangBoot?.layerArtifactCache?.listHoldings === "function",
      composeFromLayers: typeof window.__meiLangBoot?.viewCompositor?.composeFromLayers === "function",
    }));
    expect(exposed.viewRevision).toBeTruthy();
    expect(exposed.layerCache).toBeTruthy();
    expect(exposed.composeFromLayers).toBeTruthy();
  });
});
