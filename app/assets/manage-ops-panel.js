(function initManageOpsPanel() {
  const OPS_CONFIG_TARGET = ".mei-config.json";
  let summaryRoot = null;
  let editorRoot = null;

  const state = {
    appId: "",
    configPath: "",
    journalRevision: 0,
    boundary: null,
    ops: normalizeOps({}),
    journal: { entries: [] },
    selectedPanel: "params",
    selectedSourceId: null,
    paramRows: [],
    paramRowSeq: 0,
    summaryText: "",
    basemapsText: "{}",
    themesText: "{}",
    rawOpsText: "{}",
    rawOpsDirty: false,
    isDirty: false,
    busy: false,
  };

  function resolveRoots() {
    summaryRoot = document.getElementById("manage-ops-summary");
    editorRoot = document.getElementById("manage-ops-editor-root");
    const appId = editorRoot?.dataset.appId || summaryRoot?.dataset.appId || "";
    if (appId) {
      state.appId = appId;
    }
    return !!state.appId && (!!summaryRoot || !!editorRoot);
  }

  function normalizeOps(ops) {
    return {
      params: isPlainObject(ops?.params) ? { ...ops.params } : {},
      basemaps: isPlainObject(ops?.basemaps) ? { ...ops.basemaps } : {},
      themes: isPlainObject(ops?.themes) ? { ...ops.themes } : {},
      sources: isPlainObject(ops?.sources) ? { ...ops.sources } : {},
    };
  }

  function isPlainObject(value) {
    return !!value && typeof value === "object" && !Array.isArray(value);
  }

  function escapeHtml(text) {
    return String(text || "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;");
  }

  function encodeFileTarget(target) {
    return encodeURIComponent(target).replaceAll("%2F", "/");
  }

  function editorHref() {
    return `/apps/manage/${state.appId}?file=${encodeFileTarget(OPS_CONFIG_TARGET)}&tab=preview`;
  }

  function availableSourceIds() {
    return Object.keys(state.ops.sources).sort();
  }

  function formatTimestamp(value) {
    const ms = Number(value || 0);
    if (!Number.isFinite(ms) || ms <= 0) return "未知时间";
    try {
      return new Date(ms).toLocaleString("zh-CN", {
        hour12: false,
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch (_error) {
      return "未知时间";
    }
  }

  function stringifyJson(value) {
    return JSON.stringify(value ?? {}, null, 2);
  }

  function stringifyParamValue(value) {
    if (typeof value === "string") return value;
    return JSON.stringify(value);
  }

  function nextParamRowId() {
    state.paramRowSeq += 1;
    return `param-${state.paramRowSeq}`;
  }

  function hydrateParamRows(params) {
    return Object.entries(params || {}).map(([key, value]) => ({
      id: nextParamRowId(),
      key,
      valueText: stringifyParamValue(value),
    }));
  }

  function formatCountLabel(label, count) {
    return `${label} ${Number(count || 0)}`;
  }

  function sourcePanelKey(id) {
    return `source:${id}`;
  }

  function isSourcePanel(panel) {
    return String(panel || "").startsWith("source:");
  }

  function panelSourceId(panel) {
    return isSourcePanel(panel) ? String(panel).slice("source:".length) : null;
  }

  function ensureSelectedSource() {
    const ids = availableSourceIds();
    if (!ids.length) {
      state.selectedSourceId = null;
      return;
    }
    if (!state.selectedSourceId || !state.ops.sources[state.selectedSourceId]) {
      state.selectedSourceId = ids[0];
    }
  }

  function ensureSelectedPanel() {
    ensureSelectedSource();
    const ids = availableSourceIds();
    if (isSourcePanel(state.selectedPanel)) {
      const sourceId = panelSourceId(state.selectedPanel);
      if (sourceId && state.ops.sources[sourceId]) {
        state.selectedSourceId = sourceId;
        return;
      }
    }
    if (state.selectedPanel === "sources") {
      state.selectedPanel = state.selectedSourceId ? sourcePanelKey(state.selectedSourceId) : "params";
      return;
    }
    const validPanels = new Set(["params", "basemaps", "themes", "raw", "journal"]);
    if (!validPanels.has(state.selectedPanel)) {
      state.selectedPanel = state.selectedSourceId ? sourcePanelKey(state.selectedSourceId) : "params";
    }
  }

  function sourceKindLabel(kind) {
    const normalized = String(kind || "").trim();
    if (!normalized) return "未设置";
    if (normalized === "db") return "数据库";
    return normalized.toUpperCase();
  }

  function renderSummary() {
    if (!summaryRoot) return;
    const latestEntry = Array.isArray(state.journal?.entries) && state.journal.entries.length
      ? state.journal.entries[state.journal.entries.length - 1]
      : null;
    summaryRoot.innerHTML = `
      <div class="manage-ops-summary-head">
        <div>
          <div class="manage-ops-summary-title">配置文件</div>
          <div class="manage-ops-summary-subtitle">${escapeHtml(state.configPath || ".mei-config.json")}</div>
        </div>
        <button type="button" class="manage-ops-btn" data-ops-refresh-summary>刷新</button>
      </div>
      <div class="manage-ops-summary-counts">
        <span class="manage-ops-summary-chip">${formatCountLabel("source", availableSourceIds().length)}</span>
        <span class="manage-ops-summary-chip">${formatCountLabel("param", Object.keys(state.ops.params).length)}</span>
      </div>
      <div class="manage-ops-summary-boundary">只有这个页面可写；.mei 和其他资源文件都保持只读查看。</div>
      <div class="manage-ops-summary-actions">
        <a class="manage-ops-btn manage-ops-btn-link" href="${editorHref()}">打开 .mei-config.json</a>
      </div>
      <div class="manage-ops-summary-latest-body">${latestEntry
        ? `最近：#${latestEntry.revision || 0} · ${escapeHtml(latestEntry.summary || "ops patch")}`
        : `当前 revision：${state.journalRevision || 0}`}</div>
    `;
    const refreshBtn = summaryRoot.querySelector("[data-ops-refresh-summary]");
    if (refreshBtn) {
      refreshBtn.disabled = state.busy;
      refreshBtn.addEventListener("click", loadOpsConfig);
    }
  }

  function fieldInput(key, label, value, placeholder, type = "text") {
    return `
      <label class="manage-ops-field">
        <span class="manage-ops-field-label">${label}</span>
        <input
          class="manage-ops-field-input"
          type="${type}"
          data-source-field="${key}"
          placeholder="${escapeHtml(placeholder || "")}"
          value="${escapeHtml(String(value ?? ""))}"
        />
      </label>
    `;
  }

  function fieldSelect(key, label, value, options) {
    return `
      <label class="manage-ops-field">
        <span class="manage-ops-field-label">${label}</span>
        <select class="manage-ops-field-input" data-source-field="${key}">
          ${options
            .map(
              (option) =>
                `<option value="${escapeHtml(option)}"${option === value ? " selected" : ""}>${escapeHtml(
                  option,
                )}</option>`,
            )
            .join("")}
        </select>
      </label>
    `;
  }

  function renderConfigNav() {
    const sourceItems = availableSourceIds()
      .map((id) => {
        const source = state.ops.sources[id] || {};
        const active = state.selectedPanel === sourcePanelKey(id);
        const meta = escapeHtml(source.path || source.connection || sourceKindLabel(source.kind || ""));
        return `
          <button
            type="button"
            class="manage-config-nav-item manage-config-nav-item-child${active ? " is-active" : ""}"
            data-config-nav="${escapeHtml(sourcePanelKey(id))}"
            title="${escapeHtml(id)}"
          >
            <span class="manage-config-nav-label">${escapeHtml(id)}</span>
            <span class="manage-config-nav-meta">${meta}</span>
          </button>
        `;
      })
      .join("");
    return `
      <aside class="manage-config-nav">
        <div class="manage-config-nav-group">
          <div class="manage-config-nav-group-head">
            <span class="manage-config-nav-group-title">数据源</span>
            <button type="button" class="manage-ops-btn" data-add-source>新增</button>
          </div>
          <div class="manage-config-nav-list">
            ${sourceItems || '<div class="manage-ops-empty-state">暂无数据源</div>'}
          </div>
        </div>
        <div class="manage-config-nav-group">
          <span class="manage-config-nav-group-title">配置段</span>
          <div class="manage-config-nav-list">
            ${renderNavButton("params", "运行参数", formatCountLabel("项", Object.keys(state.ops.params).length))}
            ${renderNavButton("basemaps", "底图配置", formatCountLabel("项", Object.keys(state.ops.basemaps).length))}
            ${renderNavButton("themes", "主题配置", formatCountLabel("项", Object.keys(state.ops.themes).length))}
            ${renderNavButton("raw", "JSON（ops）", "直接改 JSON")}
            ${renderNavButton("journal", "审计记录", formatCountLabel("rev", state.journalRevision))}
          </div>
        </div>
      </aside>
    `;
  }

  function renderNavButton(key, label, meta) {
    const active = state.selectedPanel === key;
    return `
      <button
        type="button"
        class="manage-config-nav-item${active ? " is-active" : ""}"
        data-config-nav="${escapeHtml(key)}"
      >
        <span class="manage-config-nav-label">${label}</span>
        <span class="manage-config-nav-meta">${escapeHtml(meta)}</span>
      </button>
    `;
  }

  function renderSourceDetail() {
    const id = state.selectedSourceId;
    if (!id || !state.ops.sources[id]) {
      return `
        <div class="manage-ops-empty-state">
          先从左侧数据源树中选择一个条目，或新建数据源。
        </div>
      `;
    }
    const source = state.ops.sources[id] || {};
    const kind = String(source.kind || "xlsx").trim() || "xlsx";
    const isDb = kind === "db";
    const supportsSheet = kind === "xlsx";
    const supportsHeaderRow = kind === "xlsx" || kind === "csv";
    const supportsPreviewRows = !isDb;
    return `
      <div class="manage-config-detail-head">
        <div>
          <div class="manage-config-detail-title">${escapeHtml(id)}</div>
          <div class="manage-config-detail-desc">当前编辑的是 <code>ops.sources.${escapeHtml(
            id,
          )}</code>。先填必需字段，其他字段按需补充。</div>
        </div>
        <button type="button" class="manage-ops-btn" data-remove-source="${escapeHtml(id)}">删除数据源</button>
      </div>
      <div class="manage-ops-detail-sections">
        <section class="manage-ops-subsection">
          <div class="manage-ops-subsection-title">基础字段</div>
          <div class="manage-ops-form-grid">
            ${fieldSelect("kind", "类型", kind, ["xlsx", "csv", "json", "geojson", "db"])}
            ${isDb
              ? fieldInput("connection", "连接串", source.connection || "", "sqlite:///data.db")
              : fieldInput("path", "路径", source.path || "", "upload/data.xlsx")}
            ${supportsSheet ? fieldInput("sheet", "工作表", source.sheet || "", "Sheet1") : ""}
            ${supportsHeaderRow
              ? fieldInput("header_row", "表头行", source.header_row ?? "", "1", "number")
              : ""}
            ${supportsPreviewRows
              ? fieldInput("preview_rows", "预览行数", source.preview_rows ?? "", "1000", "number")
              : ""}
          </div>
        </section>
        <section class="manage-ops-subsection">
          <div class="manage-ops-subsection-title">${isDb ? "数据库选项" : "兼容字段"}</div>
          <div class="manage-ops-form-grid">
            ${isDb
              ? `
                ${fieldInput("table", "数据表", source.table || "", "table_name")}
                ${fieldInput("query", "查询 SQL", source.query || "", "SELECT * FROM ...")}
                ${fieldInput("page_size", "分页大小", source.page_size ?? "", "20", "number")}
                ${fieldInput("max_page_size", "最大分页", source.max_page_size ?? "", "1000", "number")}
                ${fieldInput("path", "可选路径", source.path || "", "sqlite.db")}
              `
              : `
                ${fieldInput("connection", "可选连接串", source.connection || "", "通常留空")}
                ${fieldInput("table", "可选数据表", source.table || "", "通常留空")}
                ${fieldInput("query", "可选查询 SQL", source.query || "", "通常留空")}
              `}
          </div>
        </section>
      </div>
      <div class="manage-ops-field-help">当前类型：${escapeHtml(sourceKindLabel(kind))}。切换类型后，右侧表单会自动切换到对应字段集合。</div>
    `;
  }

  function renderParamRows() {
    if (!state.paramRows.length) {
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
        <div class="manage-config-header">
          <div>
            <div class="manage-config-header-title">配置文件 .mei-config.json</div>
            <div class="manage-config-header-subtitle">${escapeHtml(state.configPath || ".mei-config.json")}</div>
          </div>
          <span class="manage-ops-panel-status" data-ops-editor-status data-tone="neutral">${escapeHtml(
            state.isDirty ? "有未保存更改" : `rev ${state.journalRevision || 0}`,
          )}</span>
        </div>
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

  async function loadOpsConfig() {
    setBusy(true);
    try {
      const [configPayload, boundaryPayload, journalPayload] = await Promise.all([
        fetchJson(`/api/ops/config/${encodeURIComponent(state.appId)}`),
        fetchJson("/api/ops/boundary"),
        fetchJson(`/api/ops/journal/${encodeURIComponent(state.appId)}`),
      ]);
      state.configPath = configPayload.config_path || "";
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
    } catch (error) {
      setEditorStatus(`保存失败：${String(error?.message || error)}`, "danger");
      setBusy(false);
    }
  }

  function mountIfPresent() {
    if (!resolveRoots()) return;
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
