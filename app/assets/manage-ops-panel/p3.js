    const basemapsEl = editorRoot.querySelector('[data-ops-json="basemaps"]');
    const themesEl = editorRoot.querySelector('[data-ops-json="themes"]');
    const rawEl = editorRoot.querySelector('[data-ops-json="raw"]');
    if (basemapsEl) state.basemapsText = String(basemapsEl.value || "");
    if (themesEl) state.themesText = String(themesEl.value || "");
    if (rawEl) state.rawOpsText = String(rawEl.value || "");
  }

  function syncEditorDraftsFromDom() {
    syncSummaryFromDom();
    syncParamRowsFromDom();
    syncJsonDraftsFromDom();
  }

  function markDirty() {
    state.isDirty = true;
    setEditorStatus("有未保存更改", "warn");
  }

  function buildParamsObject() {
    syncParamRowsFromDom();
    return buildParamsObjectFromRows(state.paramRows);
  }

  function parseOptionalInteger(raw, label) {
    const trimmed = String(raw || "").trim();
    if (!trimmed) return undefined;
    const value = Number(trimmed);
    if (!Number.isFinite(value) || value < 0) {
      throw new Error(`${label} 必须是非负数字`);
    }
    return Math.trunc(value);
  }

  function persistCurrentSourceDetailFromDom() {
    if (!editorRoot || !state.selectedSourceId || !state.ops.sources[state.selectedSourceId]) return;
    if (!isSourcePanel(state.selectedPanel)) return;
    const next = {};
    const fields = ["kind", "path", "connection", "sheet", "table", "query"];
    fields.forEach((key) => {
      const value = String(editorRoot.querySelector(`[data-source-field="${key}"]`)?.value || "").trim();
      if (value) next[key] = value;
    });
    const integerFields = [
      ["header_row", "表头行"],
      ["preview_rows", "预览行数"],
      ["page_size", "分页大小"],
      ["max_page_size", "最大分页"],
    ];
    for (const [field, label] of integerFields) {
      const raw = editorRoot.querySelector(`[data-source-field="${field}"]`)?.value;
      const value = parseOptionalInteger(raw, label);
      if (value !== undefined) next[field] = value;
    }
    state.ops.sources[state.selectedSourceId] = next;
    state.rawOpsDirty = false;
  }

  function tryPersistCurrentSourceDetail() {
    try {
      persistCurrentSourceDetailFromDom();
      return true;
    } catch (error) {
      setEditorStatus(String(error?.message || error), "danger");
      return false;
    }
  }

  function validateSources(sources) {
    for (const [id, source] of Object.entries(sources)) {
      if (!isPlainObject(source)) {
        throw new Error(`数据源 ${id} 必须是对象`);
      }
      const kind = String(source.kind || "").trim();
      if (!kind) {
        throw new Error(`数据源 ${id} 缺少 kind`);
      }
      if (kind === "db") {
        if (!String(source.connection || source.path || "").trim()) {
          throw new Error(`数据库数据源 ${id} 需填写 connection 或 path`);
        }
      } else if (!String(source.path || "").trim()) {
        throw new Error(`数据源 ${id} 缺少 path`);
      }
    }
  }

  async function fetchJson(url, options = undefined) {
    const response = await fetch(url, {
      credentials: "same-origin",
      ...options,
    });
    const payload = await response.json().catch(() => ({}));
    if (!response.ok) {
      throw new Error(payload.error || payload.message || `HTTP ${response.status}`);
    }
    return payload;
  }

  const SHELL_THEME_VAR_PREFIX = /^--mei-(shell|chrome)-/;

  function isSceneThemeCssVar(name) {
    return String(name || "").startsWith("--mei-") && !SHELL_THEME_VAR_PREFIX.test(name);
  }

  function parseInlineStyleMap(styleText) {
    const map = new Map();
    for (const chunk of String(styleText || "").split(";")) {
      const part = chunk.trim();
      if (!part) continue;
      const idx = part.indexOf(":");
      if (idx <= 0) continue;
      const key = part.slice(0, idx).trim();
      const value = part.slice(idx + 1).trim();
      if (key) map.set(key, value);
    }
    return map;
  }

  function mergeSceneThemeInlineStyle(existingStyle, sceneVarStyle) {
    const merged = parseInlineStyleMap(existingStyle);
    for (const key of [...merged.keys()]) {
      if (isSceneThemeCssVar(key)) {
        merged.delete(key);
      }
    }
    for (const [key, value] of parseInlineStyleMap(sceneVarStyle)) {
      if (isSceneThemeCssVar(key)) {
        merged.set(key, value);
      }
    }
    return Array.from(merged.entries())
      .map(([key, value]) => `${key}: ${value}`)
      .join("; ");
  }

  function patchSceneThemeStyleInDocument(doc, cssVarsStyle) {
    if (!doc || !cssVarsStyle?.trim()) return false;
    const style = cssVarsStyle.trim();
    let patched = false;
    doc.querySelectorAll(".preview-viewport").forEach((node) => {
      node.setAttribute(
        "style",
        mergeSceneThemeInlineStyle(node.getAttribute("style") || "", style),
      );
      patched = true;
    });
    const body = doc.body;
    if (body) {
      body.setAttribute(
        "style",
        mergeSceneThemeInlineStyle(body.getAttribute("style") || "", style),
      );
      patched = true;
    }
    return patched;
  }

  function notifyPreviewThemeUpdated(targetWindow) {
    const view = targetWindow || window;
    try {
      view.dispatchEvent(
        new CustomEvent("meilang:preview-updated", {
          bubbles: true,
          detail: { reason: "ops-theme-overlay", resetRuntimeQueryCache: false },
        }),
      );
    } catch {
      /* ignore cross-origin opener */
    }
  }

  function resolveSceneQueryFromLocation(loc) {
    try {
      const params = new URLSearchParams(loc?.search || "");
      const scene = params.get("scene") || params.get("sceneId") || "";
      return scene.trim();
    } catch {
      return "";
    }
  }

  async function applySceneThemeOverlayHot(targetWindow) {
    if (!state.appId) return;
    const scene = resolveSceneQueryFromLocation(targetWindow?.location || window.location);
    const query = scene ? `?scene=${encodeURIComponent(scene)}` : "";
    const payload = await fetchJson(
      `/api/ops/theme/style/${encodeURIComponent(state.appId)}${query}`,
    );
    if (patchSceneThemeStyleInDocument(targetWindow.document, payload.css_vars_style || "")) {
      notifyPreviewThemeUpdated(targetWindow);
    }
  }

  async function broadcastSceneThemeOverlayHot() {
    const targets = [window];
    if (window.opener && !window.opener.closed) {
      targets.push(window.opener);
    }
    const seen = new Set();
    for (const target of targets) {
      if (!target || seen.has(target)) continue;
      seen.add(target);
      try {
        await applySceneThemeOverlayHot(target);
      } catch {
        /* non-fatal: preview may be closed or artifact missing */
      }
    }
  }

  async function loadOpsConfig() {
    setBusy(true);
    try {
      const [configPayload, boundaryPayload, journalPayload] = await Promise.all([
        fetchJson(`/api/ops/config/${encodeURIComponent(state.appId)}`),
        fetchJson("/api/ops/boundary"),
        fetchJson(`/api/ops/journal/${encodeURIComponent(state.appId)}`),
      ]);
      state.journalRevision = configPayload.journal_revision || 0;
      state.boundary = boundaryPayload;
      state.ops = normalizeOps(configPayload.config?.ops || {});
      state.journal = journalPayload.journal || { entries: [] };
      state.paramRows = hydrateParamRows(state.ops.params);
      state.basemapsText = stringifyJson(state.ops.basemaps);
      state.themesText = stringifyJson(state.ops.themes);
      state.rawOpsText = stringifyJson(state.ops);
      state.rawOpsDirty = false;
      state.isDirty = false;
      ensureSelectedPanel();
      renderAll();
      setEditorStatus(`rev ${state.journalRevision || 0}`, "good");
    } catch (error) {
      renderSummary();
      if (editorRoot) {
        editorRoot.innerHTML = `<div class="manage-config-editor"><div class="manage-ops-empty-state">${escapeHtml(
          String(error?.message || error),
        )}</div></div>`;
      }
    } finally {
      setBusy(false);
    }
  }

  function handleAddParamRow() {
    syncEditorDraftsFromDom();
    if (!tryPersistCurrentSourceDetail()) return;
    state.paramRows.push({
      id: nextParamRowId(),
      key: "",
      valueText: "",
    });
    state.selectedPanel = "params";
    state.rawOpsDirty = false;
    markDirty();
    renderEditor();
  }

  function handleAddSource() {
    syncEditorDraftsFromDom();
    if (!tryPersistCurrentSourceDetail()) return;
    let nextId = "new_source";
    let index = 1;
    while (state.ops.sources[nextId]) {
      nextId = `new_source_${index}`;
      index += 1;
    }
    state.ops.sources[nextId] = {
      kind: "xlsx",
      path: "",
    };
    state.selectedSourceId = nextId;
    state.selectedPanel = sourcePanelKey(nextId);
    state.rawOpsDirty = false;
    markDirty();
    renderAll();
  }

  function applyRawOpsDraft() {
    const nextOps = normalizeOps(parseJsonObject(state.rawOpsText, "JSON（ops）"));
    state.ops = nextOps;
    state.paramRows = hydrateParamRows(nextOps.params);
    state.basemapsText = stringifyJson(nextOps.basemaps);
    state.themesText = stringifyJson(nextOps.themes);
    state.rawOpsText = stringifyJson(nextOps);
    state.rawOpsDirty = false;
    ensureSelectedPanel();
  }

  async function saveOpsConfig() {
    try {
      syncEditorDraftsFromDom();
      if (state.rawOpsDirty) {
        applyRawOpsDraft();
      }
      if (!tryPersistCurrentSourceDetail()) return;
      const params = buildParamsObject();
      const basemaps = parseJsonObject(state.basemapsText, "底图配置");
      const themes = parseJsonObject(state.themesText, "主题配置");
      validateSources(state.ops.sources);
      setBusy(true);
      setEditorStatus("保存中…");
      syncSummaryFromDom();
      const summary = state.summaryText.trim();
      const payload = await fetchJson(`/api/ops/config/${encodeURIComponent(state.appId)}`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          actor: "manage-ui",
          summary,
          patch: {
            params,
            basemaps,
            themes,
            sources: state.ops.sources,
          },
        }),
      });
      state.summaryText = "";
      state.journalRevision = payload.revision || state.journalRevision;
      await loadOpsConfig();
      await broadcastSceneThemeOverlayHot();
      const usesUploadPath = Object.values(state.ops.sources || {}).some((source) => {
        const path = String(source?.path || "").trim();
        return path.startsWith("upload/") && /\.xlsx$/i.test(path);
      });
      if (usesUploadPath) {
        setEditorStatus(
          "已保存。若引用 upload/*.xlsx，请在 Runtime 执行 prebuild 刷新数据链。",
          "good",
        );
      }
    } catch (error) {
      setEditorStatus(`保存失败：${String(error?.message || error)}`, "danger");
      setBusy(false);
    }
  }

  function readConfigDeepLink() {
    try {
      const params = new URL(window.location.href).searchParams;
      const section = String(params.get("section") || "").trim();
      const scope = String(params.get("scope") || "").trim();
      const validPanels = new Set(["params", "basemaps", "themes", "raw", "journal"]);
      if (validPanels.has(section) || section.startsWith("source:")) {
        state.selectedPanel = section;
      }
      if (scope) state.deepLinkScope = scope;
    } catch (_error) {
      /* ignore malformed URL */
    }
  }

  function mountIfPresent() {
    if (!resolveRoots()) return;
    readConfigDeepLink();
    renderSummary();
    if (editorRoot) {
      editorRoot.innerHTML = '<div class="manage-config-editor"><div class="manage-ops-empty-state">加载中…</div></div>';
    }
    loadOpsConfig();
  }

  document.addEventListener("mei:manage-context-change", () => {
    window.setTimeout(mountIfPresent, 0);
  });

  mountIfPresent();
})();
