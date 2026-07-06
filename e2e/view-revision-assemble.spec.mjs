import { test, expect } from "@playwright/test";

const compose = encodeURIComponent(
  JSON.stringify({ route_mode: "app", review_projection: "live_full" }),
);

test.describe("view-revision assemble", () => {
  test("view-revision API returns status field without client_layers", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const response = await page.request.get(
      `${base}/api/host/view-revision?app_id=data-demo&scene=home&surface=app&compose=${compose}`,
    );
    expect(response.ok()).toBeTruthy();
    const json = await response.json();
    expect(["refetch", "assemble_local"]).toContain(json.status);
    expect(json.manifest_revision_digest).toBeTruthy();
    expect(json.surface_revision_digest).toBeTruthy();
  });

  test("view-revision assemble_local when digests match", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const first = await page.request.get(
      `${base}/api/host/view-revision?app_id=data-demo&scene=home&surface=app&compose=${compose}`,
    );
    expect(first.ok()).toBeTruthy();
    const baseline = await first.json();
    const manifestDigest = encodeURIComponent(baseline.manifest_revision_digest);
    const surfaceDigest = encodeURIComponent(baseline.surface_revision_digest);
    const second = await page.request.get(
      `${base}/api/host/view-revision?app_id=data-demo&scene=home&surface=app&compose=${compose}&manifest_revision_digest=${manifestDigest}&surface_revision_digest=${surfaceDigest}`,
    );
    expect(second.ok()).toBeTruthy();
    const matched = await second.json();
    expect(matched.status).toBe("assemble_local");
    expect(matched.assembly_plan).toBeTruthy();
  });

  test("view-revision recover refetches all layers", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const response = await page.request.get(
      `${base}/api/host/view-revision?app_id=data-demo&scene=home&surface=app&compose=${compose}&recover=1`,
    );
    expect(response.ok()).toBeTruthy();
    const json = await response.json();
    expect(json.status).toBe("refetch");
    expect(json.changed_layers?.length).toBeGreaterThan(0);
    expect(json.manifest?.layers).toBeTruthy();
    const manifestLayerCount = Object.keys(json.manifest.layers).length;
    expect(json.changed_layers.length).toBe(manifestLayerCount);
  });

  test("client bundles expose view-revision modules", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/app/data-demo/scene/home`);
    const exposed = await page.evaluate(() => ({
      viewRevision: typeof window.__meiLangBoot?.viewRevisionClient?.fetchViewRevision === "function",
      readClientDigests: typeof window.__meiLangBoot?.readClientDigests === "function",
      layerCache: typeof window.__meiLangBoot?.layerArtifactCache?.listHoldings === "function",
      composeFromLayers: typeof window.__meiLangBoot?.viewCompositor?.composeFromLayers === "function",
      previewMaterializer:
        typeof window.__meiLangBoot?.previewMaterializer?.materializePreview === "function",
    }));
    expect(exposed.viewRevision).toBeTruthy();
    expect(exposed.readClientDigests).toBeTruthy();
    expect(exposed.layerCache).toBeTruthy();
    expect(exposed.composeFromLayers).toBeTruthy();
    expect(exposed.previewMaterializer).toBeTruthy();
  });

  test("mini-park app surface materializes preview without html fragment", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const htmlFragmentRequests = [];
    page.on("request", (request) => {
      const url = request.url();
      if (url.includes("/api/host/scene-fragment") && url.includes("format=html")) {
        htmlFragmentRequests.push(url);
      }
    });
    await page.goto(`${base}/apps/mini-park/view?surface=app&scene=home`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(
      () =>
        document.querySelector("[data-mei-frame-viewport]") &&
        (document.querySelector("[data-props]") ||
          document.querySelector("[data-mei-use-key] .component-host *")),
      { timeout: 60000 },
    );
    expect(htmlFragmentRequests).toEqual([]);
    const materialized = await page.evaluate(() => ({
      viewport: !!document.querySelector("[data-mei-frame-viewport]"),
      propsOrHost:
        !!document.querySelector("[data-props]") ||
        !!document.querySelector("[data-mei-use-key]"),
      materializedFlag:
        document
          .querySelector("#mei-compose-root, .preview-pane-scroll, .shell")
          ?.getAttribute("data-mei-compose-materialized") === "1",
    }));
    expect(materialized.viewport).toBeTruthy();
    expect(materialized.propsOrHost).toBeTruthy();
  });

  test("data-demo second F5 avoids html fragment network", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const htmlFragmentRequests = [];
    page.on("request", (request) => {
      const url = request.url();
      if (url.includes("/api/host/scene-fragment") && url.includes("format=html")) {
        htmlFragmentRequests.push(url);
      }
    });
    await page.goto(`${base}/apps/data-demo/view?surface=app&scene=home`, {
      waitUntil: "networkidle",
    });
    htmlFragmentRequests.length = 0;
    await page.reload({ waitUntil: "networkidle" });
    expect(htmlFragmentRequests).toEqual([]);
  });

  test("app to layout switch reuses semantic layers via assemble_local", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const viewRevisionCalls = [];
    page.on("request", (request) => {
      if (request.url().includes("/api/host/view-revision")) {
        viewRevisionCalls.push(request.url());
      }
    });
    await page.goto(`${base}/apps/mini-park/view?surface=app&scene=home`, {
      waitUntil: "networkidle",
    });
    const callsBefore = viewRevisionCalls.length;
    await page.goto(`${base}/apps/mini-park/view?surface=layout&scene=home`, {
      waitUntil: "networkidle",
    });
    const layoutCalls = viewRevisionCalls.slice(callsBefore);
    const assembleLocal = await page.evaluate(() => {
      const outcome = window.__meiLangBoot?.lastViewRevisionOutcome;
      return outcome === "assemble_local";
    });
    expect(
      assembleLocal ||
        layoutCalls.some((url) => url.includes("manifest_revision_digest")),
    ).toBeTruthy();
  });
});
