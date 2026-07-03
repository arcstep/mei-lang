(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  const PANEL_ID = "mei-presentation-script-panel";
  const DIAGNOSTICS_ID = "mei-presentation-compile-diagnostics";

  const uiState = {
    open: false,
    busy: false,
    source: "",
    lastDiagnostics: [],
    lastWarnings: [],
    lastError: "",
  };

  function engine() {
    return boot.presentationStepEngine || null;
  }

  function toolbar() {
    return boot.copilotToolbar || null;
  }

  function parseAppIdFromPath() {
    const match = String(window.location.pathname || "").match(
      /^\/apps\/(?:app|access|access-only|access_only|copilot|speaker|run)\/([^/]+)/,
    );
    return match ? String(match[1] || "").trim() : "";
  }

  function parseSceneIdFromPath() {
    const match = String(window.location.pathname || "").match(/\/scene\/([^/?#]+)/);
    return match ? String(match[1] || "").trim() : "home";
  }

  function escapeHtml(value) {
    return String(value || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function formatDiagnostics(items) {
    if (!Array.isArray(items) || !items.length) return "";
    return items
      .map((item) => {
        const level = String(item?.level || "error").trim() || "error";
        const code = String(item?.code || "").trim();
        const message = String(item?.message || "").trim();
        const refBits = [item?.stepId, item?.refKind, item?.refId]
          .map((value) => String(value || "").trim())
          .filter(Boolean);
        const suffix = refBits.length ? ` (${refBits.join(" / ")})` : "";
        return `<li class="mei-presentation-diagnostic mei-presentation-diagnostic--${escapeHtml(level)}"><code>${escapeHtml(code || level)}</code> ${escapeHtml(message)}${escapeHtml(suffix)}</li>`;
      })
      .join("");
  }

  function ensureDiagnostics() {
    let node = document.getElementById(DIAGNOSTICS_ID);
    if (node) return node;
    node = document.createElement("div");
    node.id = DIAGNOSTICS_ID;
    node.className = "mei-presentation-compile-diagnostics";
    node.setAttribute("hidden", "hidden");
    if (typeof boot.mountCopilotInViewport === "function") {
      boot.mountCopilotInViewport(node);
    } else {
      document.body.appendChild(node);
    }
    return node;
  }

  function renderDiagnostics() {
    const node = ensureDiagnostics();
    const errors = uiState.lastDiagnostics;
    const warnings = uiState.lastWarnings;
    const errorMessage = String(uiState.lastError || "").trim();
    if (!errors.length && !warnings.length && !errorMessage) {
      node.setAttribute("hidden", "hidden");
      node.innerHTML = "";
      return;
    }
    node.removeAttribute("hidden");
    const parts = [];
    if (errorMessage) {
      parts.push(`<p class="mei-presentation-diagnostic-summary">${escapeHtml(errorMessage)}</p>`);
    }
    if (errors.length) {
      parts.push(`<ul class="mei-presentation-diagnostic-list">${formatDiagnostics(errors)}</ul>`);
    }
    if (warnings.length) {
      parts.push(
        `<ul class="mei-presentation-diagnostic-list mei-presentation-diagnostic-list--warnings">${formatDiagnostics(warnings)}</ul>`,
      );
    }
    node.innerHTML = parts.join("");
  }

  function setCompileResult(result, error) {
    uiState.lastDiagnostics = Array.isArray(result?.diagnostics) ? result.diagnostics : [];
    uiState.lastWarnings = Array.isArray(result?.warnings) ? result.warnings : [];
    uiState.lastError = error ? String(error?.message || error || "") : "";
    renderDiagnostics();
  }

  function ensurePanel() {
    let panel = document.getElementById(PANEL_ID);
    if (panel) return panel;
    panel = document.createElement("aside");
    panel.id = PANEL_ID;
    panel.className = "mei-presentation-script-panel";
    panel.setAttribute("hidden", "hidden");
    panel.innerHTML =
      '<header class="mei-presentation-script-panel-head">' +
      '<strong>临时讲稿</strong>' +
      '<button type="button" data-presentation-script-close="true" aria-label="关闭讲稿面板">×</button>' +
      "</header>" +
      '<textarea class="mei-presentation-script-panel-editor" data-presentation-script-editor="true" spellcheck="false" placeholder="粘贴 .presentation.mdx 临时脚本"></textarea>' +
      '<div class="mei-presentation-script-panel-actions">' +
      '<button type="button" data-presentation-script-run="true">运行</button>' +
      '<button type="button" data-presentation-script-replace="true">替换</button>' +
      '<button type="button" data-presentation-script-clear="true">清空</button>' +
      "</div>";
    panel.addEventListener("click", onPanelClick);
    if (typeof boot.mountCopilotInViewport === "function") {
      boot.mountCopilotInViewport(panel);
    } else {
      document.body.appendChild(panel);
    }
    return panel;
  }

  async function compileSource(source, options = {}) {
    const compile = boot.compileAndRunPresentation;
    const compileOnly = boot.compileEphemeralPresentation;
    if (typeof compileOnly !== "function" && typeof compile !== "function") {
      throw new Error("presentation compile API is not ready");
    }
    const appId = String(options.appId || parseAppIdFromPath()).trim();
    if (!appId) {
      throw new Error("compile requires appId");
    }
    const payload = {
      appId,
      sceneId: String(options.sceneId || parseSceneIdFromPath()).trim() || "home",
      presentationId: String(options.presentationId || "ephemeral").trim() || "ephemeral",
    };
    if (typeof compileOnly === "function") {
      return compileOnly(source, payload);
    }
    return compile(source, { ...payload, dryRun: true });
  }

  async function runCompiledManifest(result, options = {}) {
    const eng = engine();
    if (!eng || typeof eng.runManifest !== "function") {
      throw new Error("presentation step engine is not ready");
    }
    const tb = toolbar();
    if (tb && typeof tb.mount === "function") {
      tb.mount({ autoStart: false, apply: false, toolbarOpen: true });
    }
    eng.runManifest(result.manifest, {
      source: "ephemeral",
      stepIndex: options.stepIndex,
      apply: options.apply !== false,
    });
    if (tb && typeof tb.renderAll === "function") {
      tb.renderAll();
    }
    return result;
  }

  async function compileAndRun(source, options = {}) {
    uiState.busy = true;
    renderPanel();
    try {
      const result = await compileSource(source, options);
      setCompileResult(result);
      return runCompiledManifest(result, options);
    } catch (error) {
      setCompileResult(error?.payload || null, error);
      throw error;
    } finally {
      uiState.busy = false;
      renderPanel();
    }
  }

  async function replacePresentation(source, options = {}) {
    uiState.busy = true;
    renderPanel();
    try {
      const result = await compileSource(source, options);
      setCompileResult(result);
      const eng = engine();
      if (!eng || typeof eng.replaceManifest !== "function") {
        throw new Error("presentation step engine is not ready");
      }
      const tb = toolbar();
      if (tb && typeof tb.mount === "function") {
        tb.mount({ autoStart: false, apply: false, toolbarOpen: true });
      }
      eng.replaceManifest(result.manifest, { source: "ephemeral" });
      if (options.apply !== false && typeof eng.start === "function") {
        eng.start({ apply: true, stepIndex: options.stepIndex });
      }
      if (tb && typeof tb.renderAll === "function") {
        tb.renderAll();
      }
      return result;
    } catch (error) {
      setCompileResult(error?.payload || null, error);
      throw error;
    } finally {
      uiState.busy = false;
      renderPanel();
    }
  }

  function clearPresentation() {
    const eng = engine();
    if (!eng || typeof eng.clearEphemeralManifest !== "function") {
      return false;
    }
    const cleared = eng.clearEphemeralManifest();
    if (typeof eng.stop === "function") {
      eng.stop();
    }
    uiState.source = "";
    setCompileResult(null);
    renderPanel();
    const tb = toolbar();
    if (tb && typeof tb.renderAll === "function") {
      tb.renderAll();
    }
    return cleared;
  }

  function currentEditorSource() {
    const panel = document.getElementById(PANEL_ID);
    const editor = panel?.querySelector("[data-presentation-script-editor]");
    if (!(editor instanceof HTMLTextAreaElement)) return uiState.source;
    return String(editor.value || "");
  }

  async function onPanelClick(event) {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    if (target.dataset.presentationScriptClose === "true") {
      uiState.open = false;
      renderPanel();
      return;
    }
    const source = currentEditorSource();
    uiState.source = source;
    if (target.dataset.presentationScriptRun === "true") {
      await compileAndRun(source, { apply: true });
      return;
    }
    if (target.dataset.presentationScriptReplace === "true") {
      await replacePresentation(source, { apply: true });
      return;
    }
    if (target.dataset.presentationScriptClear === "true") {
      clearPresentation();
    }
  }

  function renderPanel() {
    const panel = ensurePanel();
    const editor = panel.querySelector("[data-presentation-script-editor]");
    if (editor instanceof HTMLTextAreaElement && editor.value !== uiState.source) {
      editor.value = uiState.source;
    }
    panel.querySelectorAll("button").forEach((button) => {
      if (!(button instanceof HTMLButtonElement)) return;
      button.disabled = uiState.busy;
    });
    if (uiState.open) {
      panel.removeAttribute("hidden");
    } else {
      panel.setAttribute("hidden", "hidden");
    }
  }

  function togglePanel(next) {
    uiState.open = typeof next === "boolean" ? next : !uiState.open;
    renderPanel();
    return uiState.open;
  }

  const scriptPanel = {
    togglePanel,
    renderPanel,
    compileAndRun,
    replacePresentation,
    clearPresentation,
    setCompileResult,
    renderDiagnostics,
    uiState,
  };

  boot.presentationScriptPanel = scriptPanel;
})();
