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

  test("data-demo app surface materializes preview without html fragment", async ({ page }) => {
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
    htmlFragmentRequests.length = 0;
    await page.reload({ waitUntil: "networkidle" });
    expect(htmlFragmentRequests).toEqual([]);
  });

  test("data-demo cold start uses compose preview without scene-revision", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const sceneRevisionCalls = [];
    page.on("request", (request) => {
      if (request.url().includes("/api/host/scene-revision")) {
        sceneRevisionCalls.push(request.url());
      }
    });
    page.on("console", () => {});
    await page.goto(`${base}/apps/data-demo/view?surface=app&scene=home`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(
      () =>
        document.querySelector("[data-mei-frame-viewport]") &&
        (document.querySelector("[data-props]") ||
          document.querySelector("[data-mei-use-key] .component-host *")),
      { timeout: 60000 },
    );
    const pipeline = await page.evaluate(() => {
      const marks = window.__meiRenderPipeline?.last?.marks || [];
      const previewEnd = marks.find((m) => m?.name === "preview_compose:end");
      return {
        previewEndSource: String(previewEnd?.detail?.source || ""),
        materialized:
          document
            .querySelector("#mei-compose-root, .preview-pane-scroll, .shell")
            ?.getAttribute("data-mei-compose-materialized") === "1",
      };
    });
    expect(sceneRevisionCalls).toEqual([]);
    expect(pipeline.materialized).toBeTruthy();
    if (pipeline.previewEndSource) {
      expect(["compose", "assemble_local"]).toContain(pipeline.previewEndSource);
      expect(pipeline.previewEndSource).not.toBe("ssr_preview");
    }
  });

  test("cold start does not fetch whole-page scene html", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const wholePageSceneFetches = [];
    page.on("request", (request) => {
      if (request.method() !== "GET") return;
      try {
        const path = new URL(request.url()).pathname;
        if (
          /\/apps\/(?:app|access|run|presentation|slides|copilot|data-demo|mini-park)\/[^/]+\/scene\//.test(
            path,
          )
        ) {
          wholePageSceneFetches.push(request.url());
        }
      } catch (_) {}
    });
    await page.goto(`${base}/apps/mini-park/view?surface=app&scene=home`, {
      waitUntil: "networkidle",
    });
    await page.waitForFunction(
      () =>
        document.querySelector("[data-mei-frame-viewport]") ||
        document.querySelector("[data-mei-use-key]"),
      { timeout: 60000 },
    );
    expect(wholePageSceneFetches).toEqual([]);
  });

  test("compose applies runtime.plans layer_plan on thin shell", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/data-demo/view?surface=app&scene=home`, {
      waitUntil: "networkidle",
    });
    await page.waitForFunction(
      () => document.querySelector("[data-mei-frame-viewport], [data-mei-use-key]"),
      { timeout: 60000 },
    );
    const plans = await page.evaluate(() => ({
      layerPlan: !!window.__mei?.layer_plan,
      composeProjection: String(
        document
          .querySelector("#mei-compose-root, .preview-pane-scroll")
          ?.getAttribute("data-compose-projection") || "",
      ),
      materialized:
        document
          .querySelector("#mei-compose-root, .preview-pane-scroll")
          ?.getAttribute("data-mei-compose-materialized") === "1",
    }));
    expect(plans.layerPlan).toBeTruthy();
    expect(plans.materialized).toBeTruthy();
    expect(plans.composeProjection.length).toBeGreaterThan(0);
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

  test("scene-eval-pack returns pack_hit with metrics parity to scene-bootstrap", async ({
    page,
  }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const bootstrap = await page.request.get(
      `${base}/api/host/scene-bootstrap?app=data-demo&scene=home`,
    );
    const evalPack = await page.request.get(
      `${base}/api/host/scene-eval-pack?app=data-demo&scene=home&pack=unified`,
    );
    expect(bootstrap.ok()).toBeTruthy();
    expect(evalPack.ok()).toBeTruthy();
    const bootJson = await bootstrap.json();
    const packJson = await evalPack.json();
    expect(packJson.status).toBe("pack_hit");
    expect(packJson.clientRevision).toBe(bootJson.clientRevision);
    expect(packJson.metrics?.length || 0).toBe(bootJson.metrics?.length || 0);
  });

  test("board drilldown path avoids whole-page scene fetch and mrg activate", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const wholePageSceneFetches = [];
    const activateCalls = [];
    page.on("request", (request) => {
      const url = request.url();
      if (url.includes("/api/host/mrg/activate")) activateCalls.push(url);
      if (request.method() !== "GET") return;
      try {
        const path = new URL(url).pathname;
        if (/\/apps\/[^/]+\/scene\//.test(path) && !path.includes("/api/")) {
          wholePageSceneFetches.push(url);
        }
      } catch (_) {}
    });
    await page.goto(`${base}/apps/data-demo/view?surface=app&scene=home`, {
      waitUntil: "networkidle",
    });
    await page.waitForFunction(
      () => document.querySelector("[data-mei-frame-viewport], [data-mei-use-key]"),
      { timeout: 60000 },
    );
    expect(wholePageSceneFetches).toEqual([]);
    expect(activateCalls).toEqual([]);
  });
});
