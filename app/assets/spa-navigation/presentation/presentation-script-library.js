(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

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
      /^\/apps\/(?:app|access|access-only|access_only|copilot|speaker|run|presentation)\/([^/]+)/,
    );
    return match ? String(match[1] || "").trim() : "";
  }

  function parseSceneIdFromPath() {
    const match = String(window.location.pathname || "").match(/\/scene\/([^/?#]+)/);
    if (match) return String(match[1] || "").trim();
    const mei = window.__mei;
    return String(mei?.active_scene_id || mei?.activeSceneId || "home").trim() || "home";
  }

  function resolveAppId(appId) {
    return String(appId || state.appId || parseAppIdFromPath() || "").trim();
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
      state.appId = resolvedAppId;
      state.scripts = Array.isArray(payload?.scripts) ? payload.scripts : [];
      state.defaultScriptId = String(payload?.defaultScriptId || "").trim();
      state.loaded = true;
      return payload;
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
    return fetchJson(scriptApi(resolvedAppId, resolvedScriptId));
  }

  async function saveScript(scriptId, source, options = {}) {
    const resolvedAppId = resolveAppId(options.appId);
    const resolvedScriptId = String(scriptId || options.scriptId || state.activeScriptId || "").trim();
    if (!resolvedAppId || !resolvedScriptId) {
      throw new Error("saveScript requires appId and scriptId");
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
