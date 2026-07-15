      return '<div class="manage-ops-empty-state">暂无运行参数，点击“新增参数”创建。</div>';
    }
    return state.paramRows
      .map(
        (row) => `
          <div class="manage-ops-param-row" data-param-row="${row.id}">
            <input
              class="manage-ops-field-input"
              type="text"
              data-param-key
              placeholder="参数名"
              value="${escapeHtml(row.key)}"
            />
            <input
              class="manage-ops-field-input"
              type="text"
              data-param-value
              placeholder="参数值（字符串或 JSON）"
              value="${escapeHtml(row.valueText)}"
            />
            <button type="button" class="manage-ops-btn" data-remove-param-row="${row.id}">删除</button>
          </div>
        `,
      )
      .join("");
  }

  function renderParamsPanel() {
    return `
      <div class="manage-config-detail-head">
        <div>
          <div class="manage-config-detail-title">运行参数</div>
          <div class="manage-config-detail-desc">编辑 <code>ops.params</code>。这里适合路径、URL、简单样式值和轻量 JSON。</div>
        </div>
        <button type="button" class="manage-ops-btn" data-add-param-row>新增参数</button>
      </div>
      <div class="manage-ops-params-list">
        ${renderParamRows()}
      </div>
    `;
  }

  function renderJsonPanel(title, desc, dataAttr, text) {
    return `
      <div class="manage-config-detail-head">
        <div>
          <div class="manage-config-detail-title">${title}</div>
          <div class="manage-config-detail-desc">${desc}</div>
        </div>
      </div>
      <textarea class="manage-ops-editor-textarea manage-config-code" data-ops-json="${dataAttr}" spellcheck="false">${escapeHtml(
        text,
      )}</textarea>
    `;
  }

  function renderJournalEntries(limit = 20) {
    const entries = Array.isArray(state.journal?.entries)
      ? state.journal.entries.slice().reverse().slice(0, limit)
      : [];
    if (!entries.length) {
      return '<div class="manage-ops-journal-empty">暂无审计记录</div>';
    }
    return entries
      .map((entry) => {
        const summary = escapeHtml(entry.summary || "ops patch");
        const actor = escapeHtml(entry.actor || "manage");
        return `
          <article class="manage-ops-journal-entry">
            <div class="manage-ops-journal-meta">
              <span>#${entry.revision || 0}</span>
              <span>${actor}</span>
              <span>${formatTimestamp(entry.at_ms)}</span>
            </div>
            <div class="manage-ops-journal-summary">${summary}</div>
          </article>
        `;
      })
      .join("");
  }

  function currentOpsSnapshot() {
    const snapshot = normalizeOps(state.ops);
    snapshot.params = buildParamsObjectFromRows(state.paramRows);
    try {
      snapshot.basemaps = parseJsonObject(state.basemapsText, "底图配置");
    } catch (_) {
      snapshot.basemaps = normalizeOps(state.ops).basemaps;
    }
    try {
      snapshot.themes = parseJsonObject(state.themesText, "主题配置");
    } catch (_) {
      snapshot.themes = normalizeOps(state.ops).themes;
    }
return snapshot;
  }

  function renderRawPanel() {
    if (!state.rawOpsDirty) {
      state.rawOpsText = stringifyJson(currentOpsSnapshot());
    }
    return `
      <div class="manage-config-detail-head">
        <div>
          <div class="manage-config-detail-title">JSON（ops）</div>
          <div class="manage-config-detail-desc">必要时直接编辑 <code>.mei-config.json</code> 中可写的 <code>ops</code> 段。保存时会以这里的 JSON 为准。</div>
        </div>
      </div>
      <textarea class="manage-ops-editor-textarea manage-config-code manage-config-code-raw" data-ops-json="raw" spellcheck="false">${escapeHtml(
        state.rawOpsText,
      )}</textarea>
    `;
  }

  function renderJournalPanel() {
    return `
      <div class="manage-config-detail-head">
        <div>
          <div class="manage-config-detail-title">审计记录</div>
          <div class="manage-config-detail-desc">所有保存只写 <code>.mei-config.json</code> 与 <code>ops journal</code>。</div>
        </div>
      </div>
      <div class="manage-ops-panel-journal-list">${renderJournalEntries()}</div>
    `;
  }

  function renderActivePanel() {
    if (isSourcePanel(state.selectedPanel)) {
      state.selectedSourceId = panelSourceId(state.selectedPanel);
      return renderSourceDetail();
    }
    if (state.selectedPanel === "params") {
      return renderParamsPanel();
    }
    if (state.selectedPanel === "basemaps") {
      return renderJsonPanel(
        "底图配置",
        "适合保留为 JSON。对应 <code>ops.basemaps</code>，供 <code>basemap_ref(...)</code> 使用。",
        "basemaps",
        state.basemapsText,
      );
    }
    if (state.selectedPanel === "themes") {
      return renderJsonPanel(
        "主题配置",
        "适合保留为 JSON。对应 <code>ops.themes</code>，供 <code>theme_ref(...)</code> 使用。",
        "themes",
        state.themesText,
      );
    }
    if (state.selectedPanel === "journal") {
      return renderJournalPanel();
    }
    return renderRawPanel();
  }

  function renderEditor() {
    if (!editorRoot) return;
    ensureSelectedPanel();
    editorRoot.innerHTML = `
      <div class="manage-config-editor">
        <div class="manage-config-header-note">配置改动只通过这个页面完成；.mei 和其他资源页统一保持只读查看。</div>
        <div class="manage-config-layout">
          ${renderConfigNav()}
          <section class="manage-config-detail">
            ${renderActivePanel()}
          </section>
        </div>
        <div class="manage-ops-savebar">
          <input
            type="text"
            class="manage-ops-summary-input"
            data-ops-summary
            placeholder="变更说明（保存后写入 ops journal）"
            value="${escapeHtml(state.summaryText)}"
          />
          <div class="manage-ops-action-buttons">
            <button type="button" class="manage-ops-btn" data-ops-refresh-main>刷新</button>
            <button type="button" class="manage-ops-btn manage-ops-btn-primary" data-ops-save-main>保存</button>
          </div>
        </div>
      </div>
    `;

    const refreshBtn = editorRoot.querySelector("[data-ops-refresh-main]");
    const saveBtn = editorRoot.querySelector("[data-ops-save-main]");
    const addSourceBtn = editorRoot.querySelector("[data-add-source]");
    const addParamBtn = editorRoot.querySelector("[data-add-param-row]");

    if (refreshBtn) refreshBtn.addEventListener("click", loadOpsConfig);
    if (saveBtn) saveBtn.addEventListener("click", saveOpsConfig);
    if (addSourceBtn) addSourceBtn.addEventListener("click", handleAddSource);
    if (addParamBtn) addParamBtn.addEventListener("click", handleAddParamRow);

    editorRoot.querySelectorAll("[data-config-nav]").forEach((node) => {
      node.addEventListener("click", () => {
        syncEditorDraftsFromDom();
        if (!tryPersistCurrentSourceDetail()) return;
        state.selectedPanel = node.getAttribute("data-config-nav") || "params";
        if (isSourcePanel(state.selectedPanel)) {
          state.selectedSourceId = panelSourceId(state.selectedPanel);
        }
        renderEditor();
      });
    });

    editorRoot.querySelectorAll("input, textarea, select").forEach((node) => {
      node.addEventListener("input", () => {
        if (node.getAttribute("data-ops-json") === "raw") {
          state.rawOpsDirty = true;
        }
        markDirty();
      });
      node.addEventListener("change", () => {
        if (node.getAttribute("data-ops-json") === "raw") {
          state.rawOpsDirty = true;
        }
        markDirty();
      });
    });

    editorRoot.querySelectorAll('[data-source-field="kind"]').forEach((node) => {
      node.addEventListener("change", () => {
        syncEditorDraftsFromDom();
        if (!tryPersistCurrentSourceDetail()) return;
        renderEditor();
      });
    });

    editorRoot.querySelectorAll("[data-remove-source]").forEach((node) => {
      node.addEventListener("click", () => {
        syncEditorDraftsFromDom();
        if (!tryPersistCurrentSourceDetail()) return;
        const sourceId = node.getAttribute("data-remove-source");
        if (!sourceId || !state.ops.sources[sourceId]) return;
        delete state.ops.sources[sourceId];
        ensureSelectedSource();
        state.selectedPanel = state.selectedSourceId ? sourcePanelKey(state.selectedSourceId) : "params";
        markDirty();
        renderAll();
      });
    });

    editorRoot.querySelectorAll("[data-remove-param-row]").forEach((node) => {
      node.addEventListener("click", () => {
        syncEditorDraftsFromDom();
        const rowId = node.getAttribute("data-remove-param-row");
        state.paramRows = state.paramRows.filter((row) => row.id !== rowId);
        markDirty();
        renderEditor();
      });
    });

    setBusy(state.busy);
  }

  function setBusy(busy) {
    state.busy = !!busy;
    [summaryRoot, editorRoot].forEach((root) => {
      if (!root) return;
      root.querySelectorAll("button, input, textarea, select").forEach((node) => {
        node.disabled = state.busy;
      });
    });
  }

  function setEditorStatus(text, tone = "neutral") {
    if (!editorRoot) return;
    const statusEl = editorRoot.querySelector("[data-ops-editor-status]");
    if (!statusEl) return;
    statusEl.textContent = text;
    statusEl.dataset.tone = tone;
  }

  function renderAll() {
    renderSummary();
    renderEditor();
  }

  function parseJsonObject(raw, label) {
    const trimmed = String(raw || "").trim();
    if (!trimmed) return {};
    let parsed;
    try {
      parsed = JSON.parse(trimmed);
    } catch (error) {
      throw new Error(`${label} 解析失败：${error.message || error}`);
    }
    if (!isPlainObject(parsed)) {
      throw new Error(`${label} 必须是 JSON 对象`);
    }
    return parsed;
  }

  function parseLooseValue(raw) {
    const trimmed = String(raw || "").trim();
    if (!trimmed) return "";
    if (
      trimmed.startsWith("{") ||
      trimmed.startsWith("[") ||
      trimmed === "true" ||
      trimmed === "false" ||
      trimmed === "null" ||
      trimmed.startsWith('"') ||
      /^-?\d+(\.\d+)?$/.test(trimmed)
    ) {
      try {
        return JSON.parse(trimmed);
      } catch (_error) {
        return trimmed;
      }
    }
    return trimmed;
  }

  function buildParamsObjectFromRows(rows) {
    const params = {};
    for (const row of rows || []) {
      const key = String(row.key || "").trim();
      if (!key) continue;
      params[key] = parseLooseValue(row.valueText);
    }
    return params;
  }

  function syncParamRowsFromDom() {
    if (!editorRoot) return;
    const rows = [];
    editorRoot.querySelectorAll("[data-param-row]").forEach((rowEl) => {
      rows.push({
        id: rowEl.getAttribute("data-param-row") || nextParamRowId(),
        key: String(rowEl.querySelector("[data-param-key]")?.value || ""),
        valueText: String(rowEl.querySelector("[data-param-value]")?.value || ""),
      });
    });
    state.paramRows = rows;
  }

  function syncSummaryFromDom() {
    if (!editorRoot) return;
    state.summaryText = String(editorRoot.querySelector("[data-ops-summary]")?.value || "");
  }

  function syncJsonDraftsFromDom() {
    if (!editorRoot) return;
