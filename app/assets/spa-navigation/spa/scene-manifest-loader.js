/**
 * Fetch SceneViewManifest + layer batch for host artifact pipeline.
 */
(function initSceneManifestLoader(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const MANIFEST_API = "/api/host/scene-manifest";
  const LAYER_BATCH_API = "/api/host/layer-batch";

  function readShellAxes() {
    const shell = global.document?.querySelector?.(".shell");
    if (!(shell instanceof HTMLElement)) {
      return { data_mode: "", review_projection: "", tab: "", chrome: "" };
    }
    return {
      data_mode: String(shell.getAttribute("data-data-mode") || "").trim(),
      review_projection: String(shell.getAttribute("data-review-projection") || "").trim(),
      tab: String(shell.getAttribute("data-tab") || "").trim(),
      chrome: String(shell.getAttribute("data-chrome") || "").trim(),
    };
  }

  function layerRefFromManifest(layerName, manifest) {
    const value = manifest?.layers?.[layerName];
    if (!value) return null;
    const artifactId = String(value.artifact_id || "").trim();
    const contentHash = String(value.content_hash || "").trim();
    if (!artifactId || !contentHash) return null;
    return { name: layerName, artifact_id: artifactId, content_hash: contentHash };
  }

  async function fetchManifest(appId, sceneId, axes, surface) {
    const params = new URLSearchParams({
      app_id: appId,
      scene: sceneId || "home",
    });
    if (axes?.data_mode) params.set("data_mode", axes.data_mode);
    if (axes?.review_projection) params.set("review_projection", axes.review_projection);
    if (axes?.tab) params.set("tab", axes.tab);
    if (axes?.chrome) params.set("chrome", axes.chrome);
    const response = await global.fetch(`${MANIFEST_API}?${params.toString()}`, {
      credentials: "same-origin",
      headers: boot.clientCommandHeaders ? boot.clientCommandHeaders("MANIFEST", "scene-manifest") : {},
    });
    if (!response.ok) throw new Error(`scene-manifest ${response.status}`);
    const payload = await response.json();
    const hits = {
      structure: response.headers.get("x-mei-structure-hit") === "1",
      eval: response.headers.get("x-mei-eval-hit") === "1",
      theme: response.headers.get("x-mei-theme-hit") === "1",
      overlay: response.headers.get("x-mei-overlay-hit") === "1",
      shell: response.headers.get("x-mei-shell-hit") === "1",
    };
    boot.lastArtifactHits = hits;
    if (payload.manifest && boot.layerStore?.syncHoldingsFromManifest) {
      boot.layerStore.syncHoldingsFromManifest(payload.manifest);
    }
    return { manifest: payload.manifest, hits };
  }

  async function fetchLayerBatch(appId, sceneId, layerNames, axes, options) {
    const opts = options || {};
    const response = await global.fetch(LAYER_BATCH_API, {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "content-type": "application/json",
        ...(boot.clientCommandHeaders ? boot.clientCommandHeaders("LAYER", "layer-batch") : {}),
      },
      body: JSON.stringify({
        app_id: appId,
        scene: sceneId || "home",
        layers: layerNames,
        data_mode: axes?.data_mode || "",
        review_projection: axes?.review_projection || "",
        tab: axes?.tab || "",
        chrome: axes?.chrome || "",
        surface: opts.surface || "",
        local_miss: !!opts.local_miss,
        client_layers: opts.client_layers || [],
      }),
    });
    if (!response.ok) throw new Error(`layer-batch ${response.status}`);
    const payload = await response.json();
    boot.lastArtifactHits = payload.hits || boot.lastArtifactHits;
    return payload;
  }

  async function storeLayerDocuments(appId, sceneId, manifest, batchLayers) {
    if (!batchLayers || !boot.layerStore?.putLayerByRef) return;
    for (const [name, bytes] of Object.entries(batchLayers)) {
      const ref = layerRefFromManifest(name, manifest);
      if (!ref || bytes == null) continue;
      await boot.layerStore.putLayerByRef(appId, sceneId, ref, bytes, manifest);
    }
  }

  async function ensureLayers(layerNames, appId, sceneId, ctx, manifest) {
    const axes = readShellAxes();
    let activeManifest = manifest;
    if (!activeManifest) {
      const fetched = await fetchManifest(appId, sceneId, axes, ctx?.surface);
      activeManifest = fetched.manifest;
    }
    const missing = [];
    for (const name of layerNames) {
      const ref = layerRefFromManifest(name, activeManifest);
      if (!ref) {
        missing.push(name);
        continue;
      }
      const cached = boot.layerStore?.takeLayerByRef?.(ref);
      if (!cached) missing.push(name);
    }
    if (!missing.length) {
      return { manifest: activeManifest, layers: {}, hits: boot.lastArtifactHits };
    }
    const batch = await fetchLayerBatch(appId, sceneId, missing, axes, {
      surface: ctx?.surface || ctx?.mode || "build",
      local_miss: !!ctx?.local_miss,
      client_layers: boot.holdingsFromLayerCache
        ? boot.holdingsFromLayerCache(await boot.layerStore?.listHoldings?.(appId, sceneId))
        : [],
    });
    await storeLayerDocuments(appId, sceneId, activeManifest, batch.layers);
    return { manifest: activeManifest, layers: batch.layers || {}, hits: batch.hits };
  }

  async function ensureStructureFull(appId, sceneId) {
    const result = await ensureLayers(["structure.full"], appId, sceneId, { surface: "build" });
    const ref = layerRefFromManifest("structure.full", result.manifest);
    const document = ref ? boot.layerStore?.takeLayerByRef?.(ref) : result.layers?.["structure.full"];
    return { document, hits: result.hits, manifest: result.manifest };
  }

  function syncHoldingsFromManifest(manifest) {
    return boot.layerStore?.syncHoldingsFromManifest?.(manifest) || [];
  }

  boot.sceneManifestLoader = {
    fetchManifest,
    fetchLayerBatch,
    ensureStructureFull,
    ensureLayers,
    syncHoldingsFromManifest,
    readShellAxes,
  };
})(typeof window !== "undefined" ? window : globalThis);
