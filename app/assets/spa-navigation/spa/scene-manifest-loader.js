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

  async function fetchManifest(appId, sceneId, axes) {
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
    return { manifest: payload.manifest, hits };
  }

  async function fetchLayerBatch(appId, sceneId, layerNames, axes) {
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
      }),
    });
    if (!response.ok) throw new Error(`layer-batch ${response.status}`);
    const payload = await response.json();
    boot.lastArtifactHits = payload.hits || boot.lastArtifactHits;
    return payload;
  }

  async function ensureStructureFull(appId, sceneId) {
    const axes = readShellAxes();
    const { manifest, hits } = await fetchManifest(appId, sceneId, axes);
    const layerRef = manifest?.layers?.["structure.full"];
    const key = boot.layerStore?.layerKey("structure", appId, sceneId, "structure.full", axes);
    if (boot.layerStore?.hasLayer(key)) {
      return { document: boot.layerStore.takeLayer(key), hits, manifest };
    }
    const batch = await fetchLayerBatch(appId, sceneId, ["structure.full"], axes);
    const document = batch.layers?.["structure.full"];
    if (document && boot.layerStore) {
      boot.layerStore.putLayer(key, document, manifest?.revision_digest || "");
    }
    return { document, hits: batch.hits || hits, manifest };
  }

  boot.sceneManifestLoader = {
    fetchManifest,
    fetchLayerBatch,
    ensureStructureFull,
    readShellAxes,
  };
})(typeof window !== "undefined" ? window : globalThis);
