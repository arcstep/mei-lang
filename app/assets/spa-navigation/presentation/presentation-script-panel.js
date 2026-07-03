(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  const PANEL_ID = "mei-presentation-script-panel";
  const DIAGNOSTICS_ID = "mei-presentation-compile-diagnostics";

  const uiState = {
    open: false,
    pickerOpen: false,
    busy: false,
    source: "",
    activeScriptId: "",
    scripts: [],
    defaultScriptId: "",
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

  function library() {
    return boot.presentationScriptLibrary || null;
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

  function panelInnerHtml() {
    return (
      '<header class="mei-presentation-script-panel-head">' +
      '<div class="mei-presentation-script-panel-headline">' +
      '<strong class="mei-presentation-script-panel-title">演说稿目录</strong>' +
      '<span class="mei-presentation-script-panel-active" data-presentation-script-active-label="true"></span>' +
      "</div>" +
      '<button type="button" data-presentation-script-close="true" aria-label="关闭讲稿面板">×</button>' +
      "</header>" +
      '<div class="mei-presentation-script-panel-picker" data-presentation-script-picker="true" hidden>' +
      '<p class="mei-presentation-script-panel-picker-hint">从应用演说稿目录选择；默认稿会在点「演」时自动载入。</p>' +
      '<ul class="mei-presentation-script-panel-list" data-presentation-script-list="true"></ul>' +
      "</div>" +
      '<textarea class="mei-presentation-script-panel-editor" data-presentation-script-editor="true" spellcheck="false" placeholder="编辑 .presentation.mdx"></textarea>' +
      '<div class="mei-presentation-script-panel-actions">' +
      '<button type="button" data-presentation-script-load="true">载入</button>' +
      '<button type="button" data-presentation-script-run="true">运行</button>' +
      '<button type="button" data-presentation-script-save="true">保存</button>' +
      '<button type="button" data-presentation-script-default="true">设为默认</button>' +
      '<button type="button" data-presentation-script-clear="true">清空会话</button>' +
      "</div>"
    );
  }

  function ensurePanel() {
    let panel = document.getElementById(PANEL_ID);
    if (panel) return panel;
    panel = document.createElement("aside");
    panel.id = PANEL_ID;
    panel.className = "mei-presentation-script-panel";
    panel.setAttribute("hidden", "hidden");
    panel.innerHTML = panelInnerHtml();
    panel.addEventListener("click", onPanelClick);
    if (typeof boot.mountCopilotInViewport === "function") {
      boot.mountCopilotInViewport(panel);
    } else {
      document.body.appendChild(panel);
    }
    return panel;
  }

  function renderScriptList() {
    const panel = ensurePanel();
    const list = panel.querySelector("[data-presentation-script-list]");
    const picker = panel.querySelector("[data-presentation-script-picker]");
    if (!(list instanceof HTMLElement) || !(picker instanceof HTMLElement)) return;
    if (!uiState.pickerOpen) {
      picker.setAttribute("hidden", "hidden");
      list.innerHTML = "";
      return;
    }
    picker.removeAttribute("hidden");
    if (!uiState.scripts.length) {
      list.innerHTML = '<li class="mei-presentation-script-panel-empty">演说稿目录为空</li>';
      return;
    }
    list.innerHTML = uiState.scripts
      .map((script) => {
        const id = escapeHtml(script.id);
        const title = escapeHtml(script.title || script.id);
        const badges = [
          script.isDefault || uiState.defaultScriptId === script.id ? "默认" : "",
          uiState.activeScriptId === script.id ? "当前" : "",
        ]
          .filter(Boolean)
          .map((label) => `<span class="mei-presentation-script-badge">${escapeHtml(label)}</span>`)
          .join("");
        return (
          `<li class="mei-presentation-script-panel-item">` +
          `<button type="button" data-presentation-script-select="${id}">` +
          `<strong>${title}</strong>` +
          `<span class="mei-presentation-script-panel-item-id">${id}</span>` +
          `${badges}` +
          `</button></li>`
        );
      })
      .join("");
  }

  function renderPanel() {
    const panel = ensurePanel();
    const editor = panel.querySelector("[data-presentation-script-editor]");
    const label = panel.querySelector("[data-presentation-script-active-label]");
    if (editor instanceof HTMLTextAreaElement && editor.value !== uiState.source) {
      editor.value = uiState.source;
    }
    if (label) {
      label.textContent = uiState.activeScriptId
        ? `当前：${uiState.activeScriptId}`
        : uiState.defaultScriptId
          ? `默认：${uiState.defaultScriptId}`
          : "";
    }
    panel.querySelectorAll("button").forEach((button) => {
      if (!(button instanceof HTMLButtonElement)) return;
      if (button.dataset.presentationScriptClose === "true") return;
      button.disabled = uiState.busy;
    });
    renderScriptList();
    if (uiState.open) {
      panel.removeAttribute("hidden");
    } else {
      panel.setAttribute("hidden", "hidden");
    }
  }

  async function refreshLibraryState(appId) {
    const lib = library();
    if (!lib || typeof lib.listScripts !== "function") return;
    const payload = await lib.listScripts(appId);
    uiState.scripts = Array.isArray(payload?.scripts) ? payload.scripts : [];
    uiState.defaultScriptId = String(payload?.defaultScriptId || "").trim();
    if (!uiState.activeScriptId) {
      uiState.activeScriptId =
        uiState.defaultScriptId ||
        (typeof lib.resolveDefaultScriptId === "function" ? lib.resolveDefaultScriptId() : "");
    }
  }

  async function loadScriptIntoEditor(scriptId, options = {}) {
    const lib = library();
    if (!lib || typeof lib.getScript !== "function") {
      throw new Error("presentation script library is not ready");
    }
    const script = await lib.getScript(scriptId, options.appId);
    uiState.activeScriptId = String(script.id || scriptId || "").trim();
    uiState.source = String(script.source || "");
    renderPanel();
    return script;
  }

  async function compileSource(source, options = {}) {
    const compileOnly = boot.compileEphemeralPresentation;
    if (typeof compileOnly !== "function") {
      throw new Error("presentation compile API is not ready");
    }
    const appId = String(options.appId || parseAppIdFromPath()).trim();
    if (!appId) {
      throw new Error("compile requires appId");
    }
    return compileOnly(source, {
      appId,
      sceneId: String(options.sceneId || parseSceneIdFromPath()).trim() || "home",
      presentationId: String(options.presentationId || uiState.activeScriptId || "library").trim(),
    });
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
      source: "library",
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

  function clearPresentation() {
    const eng = engine();
    if (!eng) return false;
    if (typeof eng.clearSessionManifest === "function") {
      eng.clearSessionManifest();
    } else if (typeof eng.clearEphemeralManifest === "function") {
      eng.clearEphemeralManifest();
    }
    if (typeof eng.stop === "function") {
      eng.stop();
    }
    setCompileResult(null);
    renderPanel();
    const tb = toolbar();
    if (tb && typeof tb.renderAll === "function") {
      tb.renderAll();
    }
    return true;
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
    const selectId = target.closest("[data-presentation-script-select]")?.getAttribute(
      "data-presentation-script-select",
    );
    if (selectId) {
      uiState.busy = true;
      try {
        await loadScriptIntoEditor(selectId);
        uiState.pickerOpen = false;
        uiState.open = true;
      } catch (error) {
        setCompileResult(null, error);
      } finally {
        uiState.busy = false;
        renderPanel();
      }
      return;
    }
    if (target.dataset.presentationScriptClose === "true") {
      uiState.open = false;
      uiState.pickerOpen = false;
      renderPanel();
      return;
    }
    const source = currentEditorSource();
    uiState.source = source;
    if (target.dataset.presentationScriptLoad === "true") {
      const scriptId = uiState.activeScriptId || uiState.defaultScriptId;
      if (!scriptId) return;
      uiState.busy = true;
      try {
        await loadScriptIntoEditor(scriptId);
      } catch (error) {
        setCompileResult(null, error);
      } finally {
        uiState.busy = false;
        renderPanel();
      }
      return;
    }
    if (target.dataset.presentationScriptRun === "true") {
      await compileAndRun(source, { apply: true });
      return;
    }
    if (target.dataset.presentationScriptSave === "true") {
      const lib = library();
      const scriptId = uiState.activeScriptId || uiState.defaultScriptId || "draft";
      if (!lib || typeof lib.saveScript !== "function") return;
      uiState.busy = true;
      try {
        await lib.saveScript(scriptId, source, { appId: parseAppIdFromPath() });
        uiState.activeScriptId = scriptId;
        await refreshLibraryState(parseAppIdFromPath());
        setCompileResult(null);
      } catch (error) {
        setCompileResult(error?.payload || null, error);
      } finally {
        uiState.busy = false;
        renderPanel();
      }
      return;
    }
    if (target.dataset.presentationScriptDefault === "true") {
      const lib = library();
      const scriptId = uiState.activeScriptId || uiState.defaultScriptId;
      if (!lib || !scriptId || typeof lib.setDefaultScript !== "function") return;
      uiState.busy = true;
      try {
        await lib.setDefaultScript(scriptId, parseAppIdFromPath());
        await refreshLibraryState(parseAppIdFromPath());
      } catch (error) {
        setCompileResult(error?.payload || null, error);
      } finally {
        uiState.busy = false;
        renderPanel();
      }
      return;
    }
    if (target.dataset.presentationScriptClear === "true") {
      clearPresentation();
    }
  }

  async function openPicker() {
    uiState.open = true;
    uiState.pickerOpen = true;
    uiState.busy = true;
    renderPanel();
    try {
      await refreshLibraryState(parseAppIdFromPath());
      if (!uiState.source) {
        const scriptId = uiState.activeScriptId || uiState.defaultScriptId;
        if (scriptId) {
          await loadScriptIntoEditor(scriptId);
        }
      }
    } catch (error) {
      setCompileResult(null, error);
      throw error;
    } finally {
      uiState.busy = false;
      renderPanel();
    }
  }

  function togglePanel(next) {
    const nextOpen = typeof next === "boolean" ? next : !uiState.open;
    uiState.open = nextOpen;
    if (nextOpen) {
      void openPicker().catch((error) => {
        setCompileResult(null, error);
        renderPanel();
      });
    } else {
      uiState.pickerOpen = false;
      renderPanel();
    }
    return uiState.open;
  }

  function syncFromLibrary(script) {
    if (!script || typeof script !== "object") return;
    uiState.activeScriptId = String(script.id || "").trim();
    uiState.source = String(script.source || "");
    renderPanel();
  }

  const scriptPanel = {
    togglePanel,
    openPicker,
    renderPanel,
    compileAndRun,
    clearPresentation,
    setCompileResult,
    renderDiagnostics,
    syncFromLibrary,
    refreshLibraryState,
    uiState,
  };

  boot.presentationScriptPanel = scriptPanel;
})();
