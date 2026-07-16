(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const DECK_DEFAULT_SCRIPT_ID = "deck-default";

  const state = {
    appId: "",
    scripts: [],
    defaultScriptId: "",
    activeScriptId: "",
    loaded: false,
    loading: null,
  };

  function parseAppIdFromPath() {
    const match = String(window.location.pathname || "").match(
      /^\/apps\/([^/]+)(?:\/|$)/,
    );
    return match ? String(match[1] || "").trim() : "";
  }

  function parseSceneIdFromPath() {
    const utils = boot.presentationRouteUtils || globalThis.MeiPresentationRouteUtils;
    if (utils && typeof utils.parsePresentationSceneId === "function") {
      return String(utils.parsePresentationSceneId() || "").trim() || "home";
    }
    const path = String(window.location.pathname || "");
    const stageMatch = path.match(/^\/apps\/[^/]+\/([^/?#]+)/);
    if (stageMatch) {
      const seg = String(stageMatch[1] || "").trim();
      const reserved = new Set([
        "view",
        "layout",
        "prototype",
        "app",
        "access",
        "build",
        "manage",
      ]);
      if (seg && !reserved.has(seg.toLowerCase())) return seg;
    }
    const match = path.match(/\/scene\/([^/?#]+)/);
    if (match) return String(match[1] || "").trim();
    const mei = window.__mei;
    return String(mei?.active_stage_id || mei?.active_stage || mei?.active_scene_id || mei?.activeSceneId || "home").trim() || "home";
  }

  function resolveAppId(appId) {
    return String(appId || state.appId || parseAppIdFromPath() || "").trim();
  }

  function readAotDefaultManifest() {
    // Phase 5: prefer NarrationCatalog via Presenter Session.
    const session = boot.presenterSession;
    if (session && typeof session.getSnapshot === "function") {
      const snap = session.getSnapshot();
      if (snap?.prefs?.track === "off") return null;
      if (snap?.manifest && Array.isArray(snap.manifest.steps) && snap.manifest.steps.length) {
        return snap.manifest;
      }
    }
    if (session && typeof session.catalogToManifest === "function") {
      const stageId =
        (typeof session.parseStageId === "function" && session.parseStageId()) ||
        parseSceneIdFromPath();
      const { catalog } = session.catalogForStage(stageId);
      const fromCatalog = session.catalogToManifest(catalog);
      if (fromCatalog && Array.isArray(fromCatalog.steps) && fromCatalog.steps.length) {
        return fromCatalog;
      }
    }
    const map = window.__mei?.presentation_map;
    if (map && typeof map === "object" && Object.keys(map).length) {
      const ver = String(map.schemaVersion || map.schema_version || "").trim();
      if (ver !== "mei-presentation-map-v1") return null;
    }
    const manifest = map?.defaultScript || map?.default_script || null;
    return manifest && Array.isArray(manifest.steps) && manifest.steps.length ? manifest : null;
  }

  function currentStageTargetKey() {
    const ctx = boot.copilotFabContext;
    if (ctx && typeof ctx.resolveStageTargetKey === "function") {
      return String(ctx.resolveStageTargetKey() || "").trim();
    }
    const sceneId = parseSceneIdFromPath();
    const map = window.__mei?.presentation_map;
    if (map && typeof map === "object" && Object.keys(map).length) {
      const ver = String(map.schemaVersion || map.schema_version || "").trim();
      if (ver !== "mei-presentation-map-v1") {
        return `scene/${sceneId}`;
      }
    }
    const kind = map?.deck ? "presentation" : "scene";
    return `${kind}/${sceneId}`;
  }

  function aotScriptEntry() {
    const manifest = readAotDefaultManifest();
    if (!manifest) return null;
    return {
      id: DECK_DEFAULT_SCRIPT_ID,
      title: String(manifest.title || "Deck 默认讲稿").trim() || "Deck 默认讲稿",
      path: "",
      modifiedMs: null,
      isDefault: true,
      target: currentStageTargetKey(),
      sourceKind: "aot",
      aot: true,
      readOnly: true,
      manifest,
    };
  }

  function resolveDefaultScriptId() {
    const mei = window.__mei;
    return String(
      state.defaultScriptId ||
        state.activeScriptId ||
        mei?.presentation_default_script_id ||
        mei?.presentation_manifest_id ||
        "",
    ).trim();
  }

  function scriptsApi(appId) {
    return `/api/presentation/scripts/${encodeURIComponent(appId)}`;
  }

  function scriptApi(appId, scriptId) {
    return `/api/presentation/scripts/${encodeURIComponent(appId)}/${encodeURIComponent(scriptId)}`;
  }

  async function fetchJson(url, options = {}) {
    const response = await fetch(url, {
      credentials: "same-origin",
      ...options,
      headers: {
        "Content-Type": "application/json",
        ...(options.headers || {}),
      },
    });
    const payload = await response.json().catch(() => ({}));
    if (!response.ok) {
      const error = new Error(payload?.error || `request failed: ${response.status}`);
      error.payload = payload;
      error.status = response.status;
      throw error;
    }
    return payload;
  }

  async function listScripts(appId) {
    const resolvedAppId = resolveAppId(appId);
    if (!resolvedAppId) {
      throw new Error("listScripts requires appId");
    }
    state.loading = resolvedAppId;
    try {
      const payload = await fetchJson(scriptsApi(resolvedAppId));
      const scripts = Array.isArray(payload?.scripts) ? payload.scripts.slice() : [];
      const aotEntry = aotScriptEntry();
      if (aotEntry && !scripts.some((entry) => entry?.id === aotEntry.id)) {
        scripts.push(aotEntry);
      }
      const normalizedPayload = { ...payload, scripts };
      if (aotEntry && !String(normalizedPayload.defaultScriptId || "").trim()) {
        normalizedPayload.defaultScriptId = aotEntry.id;
      }
      if (aotEntry) {
        const defaultByStage = {
          ...(normalizedPayload.defaultByStage &&
          typeof normalizedPayload.defaultByStage === "object"
            ? normalizedPayload.defaultByStage
            : {}),
        };
        if (!Object.prototype.hasOwnProperty.call(defaultByStage, aotEntry.target)) {
          defaultByStage[aotEntry.target] = aotEntry.id;
        }
        normalizedPayload.defaultByStage = defaultByStage;
      }
      state.appId = resolvedAppId;
      state.scripts = scripts;
      state.defaultScriptId = String(normalizedPayload.defaultScriptId || "").trim();
      state.loaded = true;
      const embedRuntime = boot.presentationSlideEmbedRuntime;
      if (normalizedPayload.imageAssets && typeof normalizedPayload.imageAssets === "object") {
        if (embedRuntime && typeof embedRuntime.applyPresentationImageAssets === "function") {
          embedRuntime.applyPresentationImageAssets(normalizedPayload.imageAssets);
        } else {
          boot.presentationImageAssets = normalizedPayload.imageAssets;
        }
      }
      return normalizedPayload;
    } finally {
      state.loading = null;
    }
  }

  async function getScript(scriptId, appId) {
    const resolvedAppId = resolveAppId(appId);
    const resolvedScriptId = String(scriptId || resolveDefaultScriptId()).trim();
    if (!resolvedAppId || !resolvedScriptId) {
      throw new Error("getScript requires appId and scriptId");
    }
    if (resolvedScriptId === DECK_DEFAULT_SCRIPT_ID) {
      const entry = aotScriptEntry();
      if (!entry) throw new Error("当前舞台没有 AOT 默认讲稿");
      return { ...entry, appId: resolvedAppId, source: "" };
    }
    return fetchJson(scriptApi(resolvedAppId, resolvedScriptId));
  }

  async function saveScript(scriptId, source, options = {}) {
    const resolvedAppId = resolveAppId(options.appId);
    const resolvedScriptId = String(scriptId || options.scriptId || state.activeScriptId || "").trim();
    if (!resolvedAppId || !resolvedScriptId) {
      throw new Error("saveScript requires appId and scriptId");
    }
    if (resolvedScriptId === DECK_DEFAULT_SCRIPT_ID) {
      throw new Error("Deck 默认讲稿由编译产物提供，不能保存");
    }
    const payload = await fetchJson(scriptApi(resolvedAppId, resolvedScriptId), {
      method: "PUT",
      body: JSON.stringify({
        source: String(source || ""),
        title: options.title,
      }),
    });
    state.activeScriptId = resolvedScriptId;
    await listScripts(resolvedAppId);
    return payload;
  }

  async function setDefaultScript(scriptId, appId) {
    const resolvedAppId = resolveAppId(appId);
    const resolvedScriptId = String(scriptId || "").trim();
    if (!resolvedAppId || !resolvedScriptId) {
      throw new Error("setDefaultScript requires appId and scriptId");
    }
    if (resolvedScriptId === DECK_DEFAULT_SCRIPT_ID) {
      throw new Error("Deck 默认讲稿是只读 AOT 讲稿");
    }
    const payload = await fetchJson(`${scriptApi(resolvedAppId, resolvedScriptId)}/default`, {
      method: "POST",
      body: "{}",
    });
    state.defaultScriptId = resolvedScriptId;
    const mei = (window.__mei = window.__mei || {});
    mei.presentation_default_script_id = resolvedScriptId;
    mei.presentation_manifest_id = resolvedScriptId;
    await listScripts(resolvedAppId);
    return payload;
  }

  async function compileScriptSource(source, options = {}) {
    const compileOnly = boot.compileEphemeralPresentation;
    if (typeof compileOnly !== "function") {
      throw new Error("compileEphemeralPresentation is not ready");
    }
    return compileOnly(source, {
      appId: resolveAppId(options.appId),
      sceneId: String(options.sceneId || parseSceneIdFromPath()).trim() || "home",
      presentationId: String(options.presentationId || options.scriptId || "library").trim(),
    });
  }

  async function loadAndCompileScript(scriptId, options = {}) {
    const resolvedScriptId = String(scriptId || resolveDefaultScriptId()).trim();
    const script = await getScript(resolvedScriptId, options.appId);
    state.activeScriptId = resolvedScriptId;
    if (script.aot && script.manifest) {
      return {
        script,
        result: {
          manifest: script.manifest,
          diagnostics: [],
          warnings: [],
          sourceKind: "aot",
        },
      };
    }
    const result = await compileScriptSource(script.source, {
      ...options,
      scriptId: resolvedScriptId,
      presentationId: resolvedScriptId,
    });
    return { script, result };
  }

  async function loadDefaultAndCompile(options = {}) {
    await listScripts(options.appId).catch(() => null);
    const scriptId = String(options.scriptId || resolveDefaultScriptId()).trim();
    if (!scriptId) {
      throw new Error("未配置默认演说稿");
    }
    return loadAndCompileScript(scriptId, options);
  }

  async function runScript(scriptId, options = {}) {
    const { script, result } = await loadAndCompileScript(scriptId, options);
    const eng = boot.presentationStepEngine;
    if (!eng || typeof eng.runManifest !== "function") {
      throw new Error("presentation step engine is not ready");
    }
    const toolbar = boot.copilotToolbar;
    if (toolbar && typeof toolbar.mount === "function") {
      toolbar.mount({ autoStart: false, apply: false, toolbarOpen: true });
    }
    eng.runManifest(result.manifest, {
      source: "library",
      stepIndex: options.stepIndex,
      apply: options.apply !== false,
    });
    const fabContext = boot.copilotFabContext;
    if (fabContext && typeof fabContext.revealFabForScript === "function") {
      fabContext.revealFabForScript();
    }
    if (toolbar && typeof toolbar.renderAll === "function") {
      toolbar.renderAll();
    }
    const panel = boot.presentationScriptPanel;
    if (panel && typeof panel.syncFromLibrary === "function") {
      panel.syncFromLibrary(script);
    }
    return { script, result };
  }

  async function runDefaultScript(options = {}) {
    await listScripts(options.appId).catch(() => null);
    const scriptId = String(options.scriptId || resolveDefaultScriptId()).trim();
    return runScript(scriptId, options);
  }

  async function tryAutoStartPresentation(options = {}) {
    // 讲稿改为显式选择；不再进舞台自动挂稿（force=true 时仍可用于测试）
    if (options.force !== true) {
      const fabContext = boot.copilotFabContext;
      if (fabContext && typeof fabContext.syncFabVisibility === "function") {
        fabContext.syncFabVisibility();
      }
      return false;
    }
    const eng = boot.presentationStepEngine;
    if (!eng) return false;
    if (typeof eng.hasManifest === "function" && eng.hasManifest()) {
      if (typeof eng.ensureLoaded === "function" && eng.ensureLoaded()) {
        return typeof eng.start === "function" ? Boolean(eng.start({ apply: true, ...options })) : false;
      }
      if (typeof eng.ensureLoadedAsync === "function" && (await eng.ensureLoadedAsync())) {
        return typeof eng.start === "function" ? Boolean(eng.start({ apply: true, ...options })) : false;
      }
    }
    await runDefaultScript(options);
    return Boolean(eng.isActive && typeof eng.isActive === "function" && eng.isActive());
  }

  const library = {
    listScripts,
    getScript,
    saveScript,
    setDefaultScript,
    loadAndCompileScript,
    loadDefaultAndCompile,
    runScript,
    runDefaultScript,
    tryAutoStartPresentation,
    resolveDefaultScriptId,
    get state() {
      return { ...state };
    },
  };

  boot.presentationScriptLibrary = library;
})();
