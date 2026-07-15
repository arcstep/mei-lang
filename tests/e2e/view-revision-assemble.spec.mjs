import { test, expect } from "@playwright/test";

const compose = encodeURIComponent(
  JSON.stringify({ route_mode: "app", review_projection: "live_full" }),
);

test.describe("view-revision assemble", () => {
  test("view-revision API returns status field without client_layers", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const response = await page.request.get(
      `${base}/api/host/view-revision?app_id=zhifa&scene=home&surface=app&compose=${compose}`,
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
      `${base}/api/host/view-revision?app_id=zhifa&scene=home&surface=app&compose=${compose}`,
    );
    expect(first.ok()).toBeTruthy();
    const baseline = await first.json();
    const manifestDigest = encodeURIComponent(baseline.manifest_revision_digest);
    const surfaceDigest = encodeURIComponent(baseline.surface_revision_digest);
    const second = await page.request.get(
      `${base}/api/host/view-revision?app_id=zhifa&scene=home&surface=app&compose=${compose}&manifest_revision_digest=${manifestDigest}&surface_revision_digest=${surfaceDigest}`,
    );
    expect(second.ok()).toBeTruthy();
    const matched = await second.json();
    expect(matched.status).toBe("assemble_local");
    expect(matched.assembly_plan).toBeFalsy();
    expect(matched.manifest).toBeFalsy();
    expect(matched.inline_layers).toBeFalsy();
    expect((await second.body()).byteLength).toBeLessThanOrEqual(4096);
  });

  test("view-revision recover refetches all layers", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const response = await page.request.get(
      `${base}/api/host/view-revision?app_id=zhifa&scene=home&surface=app&compose=${compose}&recover=1`,
    );
    expect(response.ok()).toBeTruthy();
    const json = await response.json();
    expect(json.status).toBe("refetch");
    expect(json.changed_layers?.length).toBeGreaterThan(0);
    expect(json.manifest).toBeFalsy();
    expect(json.inline_layers).toBeFalsy();
    expect(Object.keys(json.assembly_plan?.layer_refs || {})).toHaveLength(
      json.changed_layers.length,
    );
  });

  test("client bundles expose view-revision modules", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/app/zhifa/scene/home`);
    const exposed = await page.evaluate(() => ({
      viewRevision: typeof window.__meiLangBoot?.viewRevisionClient?.fetchViewRevision === "function",
      readClientDigests: typeof window.__meiLangBoot?.readClientDigests === "function",
      layerCache: typeof window.__meiLangBoot?.layerArtifactCache?.listHoldings === "function",
      batchRead: typeof window.__meiLangBoot?.layerArtifactCache?.getLayers === "function",
      batchWrite: typeof window.__meiLangBoot?.layerArtifactCache?.putLayers === "function",
      composeFromLayers: typeof window.__meiLangBoot?.viewCompositor?.composeFromLayers === "function",
      previewMaterializer:
        typeof window.__meiLangBoot?.previewMaterializer?.materializePreview === "function",
    }));
    expect(exposed.viewRevision).toBeTruthy();
    expect(exposed.readClientDigests).toBeTruthy();
    expect(exposed.layerCache).toBeTruthy();
    expect(exposed.batchRead).toBeTruthy();
    expect(exposed.batchWrite).toBeTruthy();
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
    await page.waitForFunction(
      () => {
        const opinion = document.querySelector("mei-cockpit-opinion-panel");
        const actionBar = document.querySelector("mei-cockpit-scene-action-bar");
        return (
          opinion?.props &&
          Object.keys(opinion.props).length > 0 &&
          opinion.shadowRoot?.querySelector(".body")?.textContent?.trim() &&
          actionBar?.props &&
          Array.isArray(actionBar.props.actions) &&
          actionBar.props.actions.length > 0
        );
      },
      { timeout: 60000 },
    );
    const stageState = await page.evaluate(() => {
      const mapStage = document
        .querySelector("mei-map-maplibre")
        ?.closest("[data-mei-stage-kind]");
      const worldStage = document
        .querySelector("mei-world-stage")
        ?.closest("[data-mei-stage-kind]");
      const opinion = document.querySelector("mei-cockpit-opinion-panel");
      const actionBar = document.querySelector("mei-cockpit-scene-action-bar");
      return {
        mapKind: mapStage?.getAttribute("data-mei-stage-kind") || "",
        mapDisplay: mapStage ? getComputedStyle(mapStage).display : "",
        worldKind: worldStage?.getAttribute("data-mei-stage-kind") || "",
        worldVisibility: worldStage ? getComputedStyle(worldStage).visibility : "",
        opinionBody: opinion?.shadowRoot?.querySelector(".body")?.textContent?.trim() || "",
        actionCount: actionBar?.shadowRoot?.querySelectorAll(".action-btn").length || 0,
      };
    });
    expect(stageState.mapKind).toBe("map-stage");
    expect(stageState.mapDisplay).not.toBe("none");
    expect(stageState.worldKind).toBe("world-stage");
    expect(stageState.worldVisibility).toBe("hidden");
    expect(stageState.opinionBody.length).toBeGreaterThan(0);
    expect(stageState.actionCount).toBeGreaterThan(0);

    await page.evaluate(() => {
      document
        .querySelector("mei-cockpit-opinion-panel")
        ?.shadowRoot?.querySelector(".action")
        ?.click();
    });
    await page.waitForFunction(
      () => {
        const mount = document.querySelector(
          "[data-layer2-tab-panel].is-active .access-drilldown-frame-board-mount",
        );
        const narrative = mount?.querySelector("mei-cockpit-opinion-panel");
        return (
          mount &&
          narrative?.shadowRoot?.querySelector(".body")?.textContent?.trim()?.length > 40
        );
      },
      { timeout: 60000 },
    );
    const t2Detail = await page.evaluate(() => {
      const panel = document.querySelector("[data-layer2-tab-panel].is-active");
      const mount = panel?.querySelector(".access-drilldown-frame-board-mount");
      const narrative = mount?.querySelector("mei-cockpit-opinion-panel");
      const visibleError = Array.from(
        panel?.querySelectorAll('[data-drilldown-status="error"]') || [],
      ).some((node) => !node.hidden && getComputedStyle(node).display !== "none");
      return {
        sceneId:
          mount?.querySelector("[data-scene-id]")?.getAttribute("data-scene-id") || "",
        narrative:
          narrative?.shadowRoot?.querySelector(".body")?.textContent?.trim() || "",
        visibleError,
      };
    });
    expect(t2Detail.sceneId).toMatch(/^park_point_[1-4]_page$/);
    expect(t2Detail.narrative.length).toBeGreaterThan(40);
    expect(t2Detail.visibleError).toBe(false);

    await page.evaluate(() => {
      window.__meiLangBoot?.closeLayer2Stack?.();
      const actionBar = document.querySelector("mei-cockpit-scene-action-bar");
      Array.from(actionBar?.shadowRoot?.querySelectorAll(".action-btn") || [])
        .find((button) => button.textContent?.includes("3D"))
        ?.click();
    });
    await page.waitForFunction(
      () => {
        const world = document.querySelector("mei-world-stage");
        return (
          document.documentElement.classList.contains("mei-world-stage-active") &&
          world?.shadowRoot?.querySelector("canvas")
        );
      },
      { timeout: 60000 },
    );
    const worldState = await page.evaluate(() => {
      const world = document.querySelector("mei-world-stage");
      const stage = world?.closest("[data-mei-stage-kind]");
      return {
        active: document.documentElement.classList.contains("mei-world-stage-active"),
        visibility: stage ? getComputedStyle(stage).visibility : "",
        canvasCount: world?.shadowRoot?.querySelectorAll("canvas").length || 0,
        error: world?.shadowRoot?.querySelector('[data-role="error"]')?.textContent || "",
      };
    });
    expect(worldState.active).toBe(true);
    expect(worldState.visibility).toBe("visible");
    expect(worldState.canvasCount).toBeGreaterThan(0);
    expect(worldState.error).toBe("");
  });

  test("mini-park home_2d keeps SVG basemap and T1 content", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    await page.goto(`${base}/apps/mini-park/home_2d`, {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(
      () => {
        const basemap = document.querySelector("mei-cockpit-basemap-stage");
        const opinion = document.querySelector("mei-cockpit-opinion-panel");
        return (
          document.body?.getAttribute("data-scene-id") === "home_2d" &&
          basemap?.shadowRoot?.querySelector("svg") &&
          opinion?.shadowRoot?.querySelector(".body")?.textContent?.trim()?.length > 0
        );
      },
      { timeout: 60000 },
    );
    const state = await page.evaluate(() => {
      const basemap = document.querySelector("mei-cockpit-basemap-stage");
      const opinions = Array.from(document.querySelectorAll("mei-cockpit-opinion-panel"));
      return {
        sceneId: document.body?.getAttribute("data-scene-id") || "",
        svgCount: basemap?.shadowRoot?.querySelectorAll("svg").length || 0,
        opinionBodies: opinions.map(
          (panel) => panel.shadowRoot?.querySelector(".body")?.textContent?.trim() || "",
        ),
        actionCount:
          document
            .querySelector("mei-cockpit-scene-action-bar")
            ?.shadowRoot?.querySelectorAll(".action-btn").length || 0,
      };
    });
    expect(state.sceneId).toBe("home_2d");
    expect(state.svgCount).toBeGreaterThan(0);
    expect(state.opinionBodies).toHaveLength(4);
    expect(state.opinionBodies.every((body) => body.length > 0)).toBe(true);
    expect(state.actionCount).toBeGreaterThan(0);
  });

  test("zhifa app surface materializes preview without html fragment", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const htmlFragmentRequests = [];
    page.on("request", (request) => {
      const url = request.url();
      if (url.includes("/api/host/scene-fragment") && url.includes("format=html")) {
        htmlFragmentRequests.push(url);
      }
    });
    await page.goto(`${base}/apps/zhifa/view?surface=app&scene=home`, {
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

  test("zhifa warm F5 restores in one readonly transaction", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const layerBatchRequests = [];
    const drilldownRequests = [];
    page.on("request", (request) => {
      const url = request.url();
      if (url.includes("/api/host/layer-batch")) layerBatchRequests.push(url);
      if (url.includes("/api/host/scene-drilldown-context")) drilldownRequests.push(url);
    });
    await page.goto(`${base}/apps/zhifa/home`, { waitUntil: "networkidle" });
    await page.waitForFunction(
      () =>
        document
          .querySelector("#mei-compose-root, .preview-pane-scroll")
          ?.getAttribute("data-mei-compose-materialized") === "1",
      { timeout: 60000 },
    );
    await page.waitForFunction(
      () =>
        Number(
          window.__meiLangBoot?.layerArtifactCache?.readDiagnostics?.()
            ?.completedReadwriteTransactions || 0,
        ) >= 1,
      { timeout: 60000 },
    );
    layerBatchRequests.length = 0;
    drilldownRequests.length = 0;
    await page.reload({ waitUntil: "networkidle" });
    await page.waitForFunction(
      () => window.__meiRenderPipeline?.last?.marks?.some((mark) => mark.name === "user_visible_ready"),
      { timeout: 60000 },
    );
    const diagnostics = await page.evaluate(
      () => window.__meiLangBoot?.layerArtifactCache?.readDiagnostics?.() || {},
    );
    expect(layerBatchRequests).toEqual([]);
    expect(drilldownRequests.length).toBeLessThanOrEqual(1);
    expect(diagnostics.opens).toBe(1);
    expect(diagnostics.readonlyTransactions).toBe(1);
    expect(diagnostics.readwriteTransactions).toBe(0);
    expect(diagnostics.writes).toBe(0);
    expect(diagnostics.prunes).toBe(0);
  });

  test("zhifa cold start uses compose preview without scene-revision", async ({ page }) => {
    test.skip(!process.env.MEI_E2E_BASE_URL, "set MEI_E2E_BASE_URL to run view-revision e2e");
    const base = process.env.MEI_E2E_BASE_URL.replace(/\/+$/, "");
    const sceneRevisionCalls = [];
    page.on("request", (request) => {
      if (request.url().includes("/api/host/scene-revision")) {
        sceneRevisionCalls.push(request.url());
      }
    });
    page.on("console", () => {});
    await page.goto(`${base}/apps/zhifa/view?surface=app&scene=home`, {
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
          /\/apps\/(?:app|access|run|presentation|slides|copilot|zhifa|mini-park)\/[^/]+\/scene\//.test(
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
    await page.goto(`${base}/apps/zhifa/view?surface=app&scene=home`, {
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
      `${base}/api/host/scene-bootstrap?app=zhifa&scene=home`,
    );
    const evalPack = await page.request.get(
      `${base}/api/host/scene-eval-pack?app=zhifa&scene=home&pack=unified`,
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
    await page.goto(`${base}/apps/zhifa/view?surface=app&scene=home`, {
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
