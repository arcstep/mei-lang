/**
 * Fetch or restore scene drilldown context when SSR uses revision-only meta injection.
 */
(function initDrilldownContextLoader(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const SS_PREFIX = "mei:drilldown:v1:";

  function readDrilldownMeta(name) {
    const el = document.querySelector(`meta[name="${name}"]`);
    return el ? String(el.content || "").trim() : "";
  }

  function isDrilldownRevisionOnly() {
    return readDrilldownMeta("mei-drilldown-inlined") === "0";
  }

  function drilldownStorageKey(appId, sceneId, revision) {
    return `${SS_PREFIX}${appId}:${sceneId}:${revision || "default"}`;
  }

  function resolveDrilldownAppId(ctx) {
    const fromCtx = String(ctx?.appId || ctx?.app_id || "").trim();
    if (fromCtx) return fromCtx;
    const fromMeta = readDrilldownMeta("mei-drilldown-app-id");
    if (fromMeta) return fromMeta;
    const host =
      document.querySelector("[data-mei-app-id]") ||
      document.querySelector("[data-app-id]") ||
      document.querySelector("[data-app]");
    if (!(host instanceof HTMLElement)) return "";
    return String(
      host.dataset.meiAppId || host.dataset.appId || host.dataset.app || "",
    ).trim();
  }

  function resolveDrilldownSceneId(ctx) {
    const fromCtx = String(ctx?.sceneId || ctx?.scene_id || "").trim();
    if (fromCtx) return fromCtx;
    const fromMeta = readDrilldownMeta("mei-drilldown-scope");
    if (fromMeta) return fromMeta;
    const host = document.querySelector("[data-scene-id], [data-scene]");
    if (!(host instanceof HTMLElement)) return "home";
    return String(host.dataset.sceneId || host.dataset.scene || "home").trim() || "home";
  }

  function resolveDrilldownRevision() {
    // Prefer content-sensitive revisions so same-workset rebuilds bust sessionStorage
    // (compile_epoch alone can stay stable while projection_assembly gap/padding changes).
    const mei = global.__mei || {};
    const clientRev = String(mei.client_revision || mei.clientRevision || "").trim();
    if (clientRev) return clientRev;
    const refs = mei.scene_manifest_refs;
    const fromRefs = String(
      refs?.registry_revision || refs?.registryRevision || refs?.revision || "",
    ).trim();
    if (fromRefs) return fromRefs;
    return String(mei.compile_epoch || mei.bootstrap_compile_epoch || "").trim();
  }

  function ensureDrilldownScriptElement() {
    let el = document.getElementById("mei-scene-drilldown-context");
    if (!el) {
      el = document.createElement("script");
      el.id = "mei-scene-drilldown-context";
      el.type = "application/json";
      document.head.appendChild(el);
    }
    return el;
  }

  function injectDrilldownPayload(payloadText) {
    const text = String(payloadText || "").trim();
    if (!text) return false;
    const el = ensureDrilldownScriptElement();
    el.textContent = text;
    try {
      delete global.__meiSceneDrilldownContext;
    } catch (_) {}
    try {
      document.dispatchEvent(new CustomEvent("mei-drilldown-context-ready"));
    } catch (_) {}
    return true;
  }

  function readSessionDrilldown(appId, sceneId, revision) {
    try {
      return sessionStorage.getItem(drilldownStorageKey(appId, sceneId, revision));
    } catch (_) {
      return null;
    }
  }

  function writeSessionDrilldown(appId, sceneId, revision, payloadText) {
    try {
      sessionStorage.setItem(drilldownStorageKey(appId, sceneId, revision), payloadText);
      return true;
    } catch (_) {
      return false;
    }
  }

  function copyDrilldownMetaFromDoc(doc) {
    if (!doc) return;
    const names = [
      "mei-drilldown-inlined",
      "mei-drilldown-scope",
      "mei-drilldown-app-id",
      "mei-drilldown-artifact-url",
    ];
    names.forEach((name) => {
      const next = doc.querySelector(`meta[name="${name}"]`);
      if (!next) return;
      let current = document.querySelector(`meta[name="${name}"]`);
      if (!current) {
        current = document.createElement("meta");
        current.setAttribute("name", name);
        document.head.appendChild(current);
      }
      current.setAttribute("content", next.getAttribute("content") || "");
    });
  }

  async function ensureSceneDrilldownContext(ctx) {
    const inline = document.getElementById("mei-scene-drilldown-context");
    if (inline && inline.textContent && !isDrilldownRevisionOnly()) {
      return JSON.parse(inline.textContent || "{}");
    }
    const appId = resolveDrilldownAppId(ctx);
    const sceneId = resolveDrilldownSceneId(ctx);
    if (!appId) return null;
    const revision = resolveDrilldownRevision();
    const artifactUrl =
      readDrilldownMeta("mei-drilldown-artifact-url") ||
      `/api/host/scene-drilldown-context?app=${encodeURIComponent(appId)}&scene=${encodeURIComponent(sceneId)}`;
    // Network-first: same-workset rebuilds can keep compile_epoch stable while
    // projection_assembly shell gap/padding changes; sessionStorage must not win.
    try {
      const response = await fetch(artifactUrl, {
        credentials: "same-origin",
        cache: "no-store",
        headers: { Accept: "application/json" },
      });
      if (response.ok) {
        const payload = await response.json();
        const payloadText = JSON.stringify(payload);
        writeSessionDrilldown(appId, sceneId, revision, payloadText);
        injectDrilldownPayload(payloadText);
        global.__meiDrilldownSource = "scene_drilldown_api";
        return payload;
      }
    } catch (_) {
      /* fall through to sessionStorage */
    }
    const cached = readSessionDrilldown(appId, sceneId, revision);
    if (cached) {
      injectDrilldownPayload(cached);
      global.__meiDrilldownSource = "session_storage";
      return JSON.parse(cached);
    }
    throw new Error(`scene drilldown context failed for ${appId}/${sceneId}`);
  }

  boot.isDrilldownRevisionOnly = isDrilldownRevisionOnly;
  boot.ensureSceneDrilldownContext = ensureSceneDrilldownContext;
  boot.copyDrilldownMetaFromDoc = copyDrilldownMetaFromDoc;
  boot.injectDrilldownPayload = injectDrilldownPayload;
})(typeof window !== "undefined" ? window : globalThis);
