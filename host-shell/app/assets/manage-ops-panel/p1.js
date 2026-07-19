(function initManageOpsPanel() {
  const OPS_CONFIG_TARGET = ".mei-config.json";
  let summaryRoot = null;
  let editorRoot = null;

  const state = {
    appId: "",
    journalRevision: 0,
    boundary: null,
    ops: normalizeOps({}),
    journal: { entries: [] },
    selectedPanel: "params",
    selectedSourceId: null,
    deepLinkScope: "",
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
        <div class="manage-ops-summary-title">配置文件</div>
        <button type="button" class="manage-ops-btn" data-ops-refresh-summary>刷新</button>
      </div>
      <div class="manage-ops-summary-counts">
        <span class="manage-ops-summary-chip">${formatCountLabel("source", availableSourceIds().length)}</span>
        <span class="manage-ops-summary-chip">${formatCountLabel("param", Object.keys(state.ops.params).length)}</span>
      </div>
      <div class="manage-ops-summary-boundary">只有这个页面可写；.mei 和其他资源文件都保持只读查看。</div>
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
            ${renderNavButton(
              
              "布局调优",
            )}
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
