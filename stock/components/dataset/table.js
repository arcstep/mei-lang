import {
  appendRuntimePerfDiagnostics,
  deferUntilDisplayed,
  escapeHtml,
  escapeHtmlAttr,
  fetchDatasetRows,
  getQueryState,
  mergeFilters,
  parseProps,
  queryStateIdOf,
  resolveDatasetQueryCapability,
  resolveRuntimeDataRef,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  setQueryState,
  subscribeQueryState,
} from "./runtime-query.js";
import {
  bindCellPreviewClick,
  bindRelativeTimeTicker,
  cellPopoverStyleBlock,
  cellTableChromeStyleBlock,
  cellValue,
  closeCellPopover,
  openCellPopover,
  renderFormattedCellHtml,
  resolveCellPopoverVariant,
  scheduleOverflowPreviewSync,
  stopRelativeTimeTicker,
} from "./table-runtime/cells.js";
import {
  activeTableColumnState,
  activeTableSort,
  cycleSingleColumnSort,
  ensureColumnStateForKeys,
  resolveColumnStateConfig,
  resolveSortConfig,
  sameColumnState,
  sameFilters,
  sameSort,
  withColumnOrder,
  withColumnVisibility,
} from "./table-runtime/state.js";
import { applyTableQueryResult } from "./table-runtime/query.js";
import {
  columnLayoutWeights,
  inlineStyleForColWidth,
  inlineStyleForColumn,
  resolveColumnDescriptors,
  resolveToneToken,
} from "./table-runtime/format.js";
import { formatTableRowCountLabel } from "./table-runtime/footer.js";

class MeiDatasetTable extends HTMLElement {
  connectedCallback() {
    this._fetchAbort = new AbortController();
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    this._deferUntilVisibleCleanup = deferUntilDisplayed(this, () => {
      this._deferUntilVisibleCleanup = null;
      this.bootstrapDatasetTable();
    });
  }

  bootstrapDatasetTable() {
    const props = parseProps(this);
    const data = resolveDataSource(props);
    const queryStateId = queryStateIdOf(props);
    const paging = resolveServerPaging(props, data);
    const initialPageSize = paging.defaultPageSize || 20;
    const initialRows = paging.server ? [] : data.rows;
    const initialTotal = paging.server ? 0 : Array.isArray(data.rows) ? data.rows.length : 0;
    this._props = props;
    this._queryStateId = queryStateId;
    this._datasetId = data.id;
    this._title = data.title;
    this._dragColumnKey = "";
    this._state = {
      paging,
      loading: paging.server,
      error: "",
      page: 1,
      pageSize: initialPageSize,
      total: initialTotal,
      hasMore: false,
      columns: data.columns,
      columnMeta: data.columnMeta,
      allRows: Array.isArray(data.rows) ? data.rows : [],
      rows: initialRows,
      search: String(getQueryState(queryStateId).search || "").trim(),
      filterField: "",
      filterValue: "",
      sharedFilters: getQueryState(queryStateId).filters,
      sort: resolveSortConfig(props),
      columnState: resolveColumnStateConfig(props),
      perf: null,
      summary: null,
    };
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
      this._unsubscribeQueryState = null;
    }
    this._unsubscribeQueryState = subscribeQueryState(queryStateId, (nextState) => {
      const nextFilters = mergeFilters(nextState?.filters);
      const nextSearch = String(nextState?.search || "").trim();
      const nextSort = Array.isArray(nextState?.sort) ? nextState.sort : [];
      const nextColumnState = nextState?.column_state || nextState?.columnState || null;
      const filtersChanged = !sameFilters(nextFilters, this._state.sharedFilters);
      const searchChanged = nextSearch !== String(this._state.search || "").trim();
      const sortChanged = !sameSort(nextSort, activeTableSort(this._props, this._queryStateId, this._state.sort));
      const columnStateChanged = !sameColumnState(
        nextColumnState,
        activeTableColumnState(this._props, this._queryStateId, this._state.columnState)
      );
      if (!filtersChanged && !searchChanged && !sortChanged && !columnStateChanged) return;
      this._state.sharedFilters = nextFilters;
      this._state.search = nextSearch;
      if (this._state.paging.server && (filtersChanged || searchChanged || sortChanged)) {
        this.loadPage(1);
      } else {
        this.render();
      }
    });
    if (paging.server) {
      this.loadPage(1, { skipStartRender: true });
    }
  }

  disconnectedCallback() {
    if (this._fetchAbort) {
      this._fetchAbort.abort();
      this._fetchAbort = null;
    }
    this.closeCellPopover();
    stopRelativeTimeTicker(this);
    if (typeof this._relativeTimeCleanup === "function") {
      this._relativeTimeCleanup();
      this._relativeTimeCleanup = null;
    }
    if (typeof this._cellPreviewCleanup === "function") {
      this._cellPreviewCleanup();
      this._cellPreviewCleanup = null;
    }
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
    }
  }

  async loadPage(page, options = {}) {
    if (!this._state.paging.server) return;
    if (!options.skipStartRender) {
      this._state.loading = true;
      this._state.error = "";
      this.render();
    }
    const signal = this._fetchAbort?.signal;
    try {
      const result = await fetchDatasetRows(this._props, {
        page,
        pageSize: this._state.pageSize,
        search: this._state.search || undefined,
        filters: this.activeFilters(),
        sort: activeTableSort(this._props, this._queryStateId, this._state.sort),
        columnState: activeTableColumnState(this._props, this._queryStateId, this._state.columnState),
        summary: true,
        signal,
        meta: runtimeCallerMeta(this, "mei-dataset-table"),
      });
      if (!result) {
        this._state.error =
          this._state?.paging?.capability?.reason ||
          "shared runtime dataset query capability is unavailable";
        return;
      }
      this._state = applyTableQueryResult(this._state, result);
      this._state.page = result?.page || page;
      this._state.pageSize = result?.page_size || this._state.pageSize;
      appendRuntimePerfDiagnostics(this._datasetId, this._state.perf, runtimePerfMeta(this));
    } catch (error) {
      if (error?.name === "AbortError") return;
      this._state.error = String(error?.message || error || "query failed");
      this._state.perf = null;
    } finally {
      this._state.loading = false;
      this.render();
    }
  }

  activeFilters() {
    return mergeFilters(this._state.sharedFilters, buildFilters(this._state.filterField, this._state.filterValue));
  }

  activeColumnState() {
    const base = ensureColumnStateForKeys(this._state.columnState, this._state.columns);
    return activeTableColumnState(this._props, this._queryStateId, base);
  }

  viewModel() {
    const state = this._state;
    const descriptors = resolveColumnDescriptors({
      columns: state.columns,
      headers: state.columns,
      columnMeta: state.columnMeta,
      columnState: this.activeColumnState(),
      columnFormats: this._props?.columnFormats ?? this._props?.column_formats,
      columnRules: this._props?.columnRules ?? this._props?.column_rules,
    });
    const activeSort = activeTableSort(this._props, this._queryStateId, state.sort);
    if (state.paging.server) {
      return { descriptors, rows: Array.isArray(state.rows) ? state.rows : [], activeSort };
    }
    const rows = applyClientView(Array.isArray(state.allRows) ? state.allRows : [], {
      filters: this.activeFilters(),
      search: state.search,
      sort: activeSort,
      descriptors,
    });
    return { descriptors, rows, activeSort };
  }

  render() {
    this.closeCellPopover();
    const state = this._state;
    const { descriptors, rows, activeSort } = this.viewModel();
    this._visibleDescriptors = descriptors;
    const visibleColumns = descriptors.map((item) => item.key);
    this._renderedColumnKeys = visibleColumns;
    const lazyMeta = state.paging.server ? `<span class="lazy-badge">server paging</span>` : "";
    const rowCount = state.paging.server
      ? Math.max(0, Number(state.total) || 0)
      : rows.length;
    const totalPages =
      state.paging.server && state.pageSize > 0
        ? Math.max(1, Math.ceil(rowCount / state.pageSize))
        : 1;
    const sortSummary = activeSort.length > 0 ? `${activeSort[0].field} ${activeSort[0].direction}` : "none";
    this._cellTextMap = new Map();
    const popoverVariant = resolveCellPopoverVariant(this._props);
    const colWidthPercents = columnLayoutWeights(descriptors, 120);
    const bodyHtml = renderTableBody(rows, descriptors, this._props, this._cellTextMap, state.loading);
    const columnChooserHtml = buildColumnChooserHtml(state.columns, this.activeColumnState());
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; width: 100%; max-width: 100%; min-width: 0; box-sizing: border-box; }
        .wrap { display: grid; gap: 12px; padding: 16px; border-radius: 14px; background: rgba(15,23,42,.72); border: 1px solid rgba(148,163,184,.18); color: #e2e8f0; min-width: 0; max-width: 100%; box-sizing: border-box; overflow: hidden; }
        .meta { display: flex; justify-content: space-between; gap: 12px; flex-wrap: wrap; color: #94a3b8; font-size: 12px; align-items: center; min-width: 0; }
        .meta strong { color: #e2e8f0; font-size: 13px; }
        .lazy-badge { display: inline-flex; align-items: center; padding: 2px 8px; border-radius: 999px; border: 1px solid rgba(59,130,246,.45); color: #93c5fd; font-size: 11px; }
        .toolbar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; min-width: 0; max-width: 100%; }
        .toolbar input, .toolbar select, .toolbar button, .toolbar summary { border-radius: 8px; border: 1px solid rgba(148,163,184,.25); background: rgba(15,23,42,.45); color: #e2e8f0; font-size: 12px; padding: 7px 9px; box-sizing: border-box; }
        .toolbar input, .toolbar select { flex: 1 1 140px; min-width: 0; max-width: 100%; }
        .toolbar button, .toolbar summary { flex: 0 0 auto; cursor: pointer; white-space: nowrap; }
        .toolbar .column-menu { flex: 0 0 auto; }
        .toolbar button[disabled] { opacity: .5; cursor: not-allowed; }
        .column-menu { position: relative; }
        .column-menu summary { list-style: none; user-select: none; }
        .column-menu summary::-webkit-details-marker { display: none; }
        .column-panel { position: absolute; right: 0; top: calc(100% + 6px); z-index: 5; width: 240px; display: grid; gap: 8px; padding: 10px; border-radius: 12px; border: 1px solid rgba(148,163,184,.25); background: rgba(15,23,42,.98); box-shadow: 0 16px 32px rgba(0,0,0,.3); }
        .column-row { display: flex; align-items: center; gap: 8px; font-size: 12px; color: #cbd5e1; }
        .column-row input { margin: 0; }
        .column-actions { display: flex; justify-content: flex-end; }
        .status { color: #94a3b8; font-size: 11px; min-height: 16px; display: flex; gap: 12px; flex-wrap: wrap; }
        .error { color: #fca5a5; }
        .table-wrap { overflow: auto; border-radius: 12px; border: 1px solid rgba(148,163,184,.16); min-height: 80px; max-width: 100%; min-width: 0; }
        table { width: 100%; min-width: 100%; border-collapse: collapse; table-layout: fixed; }
        col { min-width: 0; }
        th, td { border-bottom: 1px solid rgba(148,163,184,.12); font-size: 12px; min-width: 0; }
        th { background: rgba(30,41,59,.92); color: #f8fafc; position: sticky; top: 0; z-index: 1; }
        td { color: #cbd5e1; overflow: hidden; }
        ${cellTableChromeStyleBlock()}
        .th-shell { display: flex; align-items: center; gap: 8px; min-width: 0; }
        .drag-handle { cursor: grab; color: #64748b; font-size: 14px; }
        .th-button { display: inline-flex; align-items: center; gap: 6px; width: 100%; min-width: 0; border: none; background: transparent; color: inherit; font: inherit; padding: 0; cursor: pointer; text-align: inherit; }
        .th-button:hover { color: #bfdbfe; }
        .th-text { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
        .sort-indicator { flex: 0 0 auto; color: #93c5fd; font-size: 11px; }
        .th-drop-target { box-shadow: inset 0 -2px 0 rgba(96,165,250,.9); }
        .tone-red { color: #fca5a5; }
        .tone-orange { color: #fdba74; }
        .tone-yellow { color: #fde68a; }
        .tone-green { color: #86efac; }
        .tone-blue { color: #93c5fd; }
        .tone-slate { color: #cbd5e1; }
        .cell-tag { display: inline-flex; align-items: center; max-width: 100%; min-width: 0; padding: 1px 8px; border-radius: 999px; border: 1px solid currentColor; background: rgba(15,23,42,.25); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; vertical-align: middle; }
        .table-footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; padding: 8px 4px 2px; border-top: 1px solid rgba(148,163,184,.14); color: #94a3b8; font-size: 12px; min-height: 32px; }
        .row-total { flex: 0 0 auto; color: #cbd5e1; white-space: nowrap; }
        .table-footer .pager { flex: 1 1 auto; display: flex; align-items: center; justify-content: flex-end; gap: 8px; flex-wrap: wrap; }
        .pager-meta { color: #94a3b8; white-space: nowrap; }
        .table-footer button { border-radius: 8px; border: 1px solid rgba(148,163,184,.25); background: rgba(15,23,42,.45); color: #e2e8f0; font-size: 12px; padding: 6px 10px; cursor: pointer; white-space: nowrap; }
        .table-footer button[disabled] { opacity: .5; cursor: not-allowed; }
        ${cellPopoverStyleBlock(popoverVariant)}
      </style>
      <div class="wrap">
        <div class="meta">
          <strong>${escapeHtml(this._title)}</strong>
          <div>${lazyMeta}</div>
        </div>
        <div class="toolbar">
          <input id="search" type="text" placeholder="search all columns" value="${escapeHtmlAttr(state.search)}" />
          <select id="page-size">
            ${[20, 50, 100, 200, 500, 1000]
              .map((size) => `<option value="${size}" ${size === state.pageSize ? "selected" : ""}>${size}/page</option>`)
              .join("")}
          </select>
          <input id="filter-field" type="text" placeholder="filter field (optional)" value="${escapeHtmlAttr(state.filterField)}" />
          <input id="filter-value" type="text" placeholder="filter value (optional)" value="${escapeHtmlAttr(state.filterValue)}" />
          <details class="column-menu">
            <summary>columns</summary>
            ${columnChooserHtml}
          </details>
          <button id="apply" ${state.loading ? "disabled" : ""}>apply</button>
        </div>
        <div class="status ${state.error ? "error" : ""}">
          <span>${state.error ? escapeHtml(state.error) : state.loading ? "loading..." : ""}</span>
          <span>sort: ${escapeHtml(sortSummary)}</span>
        </div>
        <div class="table-wrap">
          <table>
            <colgroup>${descriptors
              .map(
                (descriptor, index) =>
                  `<col style="${escapeHtmlAttr(inlineStyleForColWidth(descriptor, colWidthPercents[index]))}" />`
              )
              .join("")}</colgroup>
            <thead>
              <tr>
                ${descriptors
                  .map((descriptor) => {
                    const sortState = activeSort.find((item) => item.field === descriptor.key)?.direction || "";
                    return `<th
                      draggable="true"
                      data-column-key="${escapeHtmlAttr(descriptor.key)}"
                      style="${escapeHtmlAttr(inlineStyleForColumn(descriptor, "header"))}"
                      title="${escapeHtmlAttr(descriptor.label)}"
                    >
                      <div class="th-shell">
                        <span class="drag-handle" data-drag-column="${escapeHtmlAttr(descriptor.key)}">::</span>
                        <button type="button" class="th-button" data-sort-column="${escapeHtmlAttr(descriptor.key)}">
                          <span class="th-text">${escapeHtml(descriptor.label)}</span>
                          <span class="sort-indicator">${escapeHtml(sortState === "asc" ? "asc" : sortState === "desc" ? "desc" : "")}</span>
                        </button>
                      </div>
                    </th>`;
                  })
                  .join("")}
              </tr>
            </thead>
            <tbody>${bodyHtml}</tbody>
          </table>
        </div>
        <div class="table-footer">
          <span class="row-total">${escapeHtml(formatTableRowCountLabel(rowCount))}</span>
          ${
            state.paging.server
              ? `<div class="pager">
            <span class="pager-meta">第 ${state.page} / ${totalPages} 页</span>
            <button type="button" id="prev" ${state.loading || state.page <= 1 ? "disabled" : ""}>上一页</button>
            <button type="button" id="next" ${state.loading || !state.hasMore ? "disabled" : ""}>下一页</button>
          </div>`
              : ""
          }
        </div>
      </div>
    `;
    this.bindEvents();
  }

  closeCellPopover() {
    closeCellPopover(this, this.shadowRoot);
  }

  openCellPopover(fullText, anchor, options = {}) {
    const layout = options.layout || "anchored";
    openCellPopover(this, this.shadowRoot, fullText, anchor, {
      topOffset: 6,
      focusOnOpen: layout === "modal" ? true : options.focusOnOpen !== false,
      variant: options.variant || resolveCellPopoverVariant(this._props),
      layout,
      title: options.title || "详细内容",
      subtitle: options.subtitle || "",
    });
  }

  bindEvents() {
    const searchEl = this.shadowRoot.getElementById("search");
    const pageSizeEl = this.shadowRoot.getElementById("page-size");
    const fieldEl = this.shadowRoot.getElementById("filter-field");
    const valueEl = this.shadowRoot.getElementById("filter-value");
    const applyBtn = this.shadowRoot.getElementById("apply");
    const prevBtn = this.shadowRoot.getElementById("prev");
    const nextBtn = this.shadowRoot.getElementById("next");
    const apply = () => {
      const nextSearch = String(searchEl?.value || "").trim();
      this._state.search = nextSearch;
      this._state.filterField = String(fieldEl?.value || "").trim();
      this._state.filterValue = String(valueEl?.value || "").trim();
      const nextPageSize = Number(pageSizeEl?.value || this._state.pageSize);
      if (Number.isFinite(nextPageSize) && nextPageSize > 0) {
        this._state.pageSize = nextPageSize;
      }
      const sharedSearch = this._queryStateId ? String(getQueryState(this._queryStateId).search || "").trim() : "";
      if (this._queryStateId && nextSearch !== sharedSearch) {
        setQueryState(
          this._queryStateId,
          { search: nextSearch },
          { transitionSource: "table_selection" }
        );
        return;
      }
      if (this._state.paging.server) {
        this.loadPage(1);
      } else {
        this.render();
      }
    };
    applyBtn?.addEventListener("click", apply);
    prevBtn?.addEventListener("click", () => this.loadPage(Math.max(1, this._state.page - 1)));
    nextBtn?.addEventListener("click", () => this.loadPage(this._state.page + 1));
    searchEl?.addEventListener("keydown", (event) => {
      if (event.key === "Enter") apply();
    });
    valueEl?.addEventListener("keydown", (event) => {
      if (event.key === "Enter") apply();
    });
    this.shadowRoot.querySelectorAll("[data-sort-column]").forEach((button) => {
      button.addEventListener("click", () => {
        const key = String(button.getAttribute("data-sort-column") || "").trim();
        const nextSort = cycleSingleColumnSort(
          activeTableSort(this._props, this._queryStateId, this._state.sort),
          key
        );
        this._state.sort = nextSort;
        if (this._queryStateId) {
          setQueryState(
            this._queryStateId,
            { sort: nextSort },
            { transitionSource: "table_selection" }
          );
          return;
        }
        if (this._state.paging.server) {
          this.loadPage(1);
        } else {
          this.render();
        }
      });
    });
    this.shadowRoot.querySelectorAll("[data-column-toggle]").forEach((checkbox) => {
      checkbox.addEventListener("change", () => {
        const key = String(checkbox.getAttribute("data-column-toggle") || "").trim();
        const nextColumnState = withColumnVisibility(this.activeColumnState(), key, !!checkbox.checked);
        this._state.columnState = nextColumnState;
        if (this._queryStateId) {
          setQueryState(
            this._queryStateId,
            { column_state: nextColumnState },
            { transitionSource: "table_selection" }
          );
          return;
        }
        this.render();
      });
    });
    this.shadowRoot.getElementById("reset-columns")?.addEventListener("click", () => {
      const nextColumnState = resolveColumnStateConfig(this._props);
      this._state.columnState = nextColumnState;
      if (this._queryStateId) {
        setQueryState(
          this._queryStateId,
          { column_state: nextColumnState },
          { transitionSource: "table_selection" }
        );
        return;
      }
      this.render();
    });
    this.shadowRoot.querySelectorAll("th[data-column-key]").forEach((header) => {
      header.addEventListener("dragstart", () => {
        this._dragColumnKey = String(header.getAttribute("data-column-key") || "").trim();
      });
      header.addEventListener("dragover", (event) => {
        event.preventDefault();
        header.classList.add("th-drop-target");
      });
      header.addEventListener("dragleave", () => {
        header.classList.remove("th-drop-target");
      });
      header.addEventListener("drop", (event) => {
        event.preventDefault();
        header.classList.remove("th-drop-target");
        const toKey = String(header.getAttribute("data-column-key") || "").trim();
        const fromKey = String(this._dragColumnKey || "").trim();
        if (!fromKey || !toKey || fromKey === toKey) return;
        const visible = Array.isArray(this._renderedColumnKeys) ? this._renderedColumnKeys.slice() : [];
        const fromIndex = visible.indexOf(fromKey);
        const toIndex = visible.indexOf(toKey);
        if (fromIndex < 0 || toIndex < 0) return;
        visible.splice(toIndex, 0, visible.splice(fromIndex, 1)[0]);
        const hidden = ensureColumnStateForKeys(this.activeColumnState(), this._state.columns).columns
          .filter((entry) => entry.hidden)
          .map((entry) => entry.key);
        const nextColumnState = withColumnOrder(this.activeColumnState(), [...visible, ...hidden]);
        this._state.columnState = nextColumnState;
        if (this._queryStateId) {
          setQueryState(
            this._queryStateId,
            { column_state: nextColumnState },
            { transitionSource: "table_selection" }
          );
          return;
        }
        this.render();
      });
    });
    if (typeof this._cellPreviewCleanup === "function") {
      this._cellPreviewCleanup();
    }
    this._cellPreviewCleanup = bindCellPreviewClick(
      this.shadowRoot,
      this._cellTextMap,
      (full, anchor, opts) => this.openCellPopover(full, anchor, opts),
      { getVariant: () => resolveCellPopoverVariant(this._props) }
    );
    if (typeof this._relativeTimeCleanup === "function") {
      this._relativeTimeCleanup();
    }
    this._relativeTimeCleanup = bindRelativeTimeTicker(
      this,
      this.shadowRoot,
      this._visibleDescriptors || [],
      this._props
    );
    scheduleOverflowPreviewSync(this, this.shadowRoot, this._cellTextMap, this._props);
  }
}

function buildColumnChooserHtml(columns, columnState) {
  const normalized = ensureColumnStateForKeys(columnState, columns);
  const visibility = new Map(normalized.columns.map((entry) => [entry.key, !entry.hidden]));
  return `
    <div class="column-panel">
      ${(Array.isArray(columns) ? columns : [])
        .map((key) => {
          const normalizedKey = String(key || "").trim();
          if (!normalizedKey) return "";
          return `<label class="column-row">
            <input type="checkbox" data-column-toggle="${escapeHtmlAttr(normalizedKey)}" ${visibility.get(normalizedKey) !== false ? "checked" : ""} />
            <span>${escapeHtml(normalizedKey)}</span>
          </label>`;
        })
        .join("")}
      <div class="column-actions">
        <button id="reset-columns" type="button">reset</button>
      </div>
    </div>
  `;
}

function renderTableBody(rows, descriptors, props, textMap, loading) {
  if (loading) {
    return `<tr><td colspan="${Math.max(descriptors.length, 1)}">loading...</td></tr>`;
  }
  if (!Array.isArray(rows) || rows.length === 0) {
    return `<tr><td colspan="${Math.max(descriptors.length, 1)}">no rows</td></tr>`;
  }
  return rows
    .map(
      (row, rowIndex) =>
        `<tr>${descriptors
          .map((descriptor, columnIndex) => {
            const raw = cellValue(row, descriptor.key, columnIndex);
            const cell = renderFormattedCellHtml(raw, descriptor, rowIndex, textMap, props);
            const tone = resolveToneToken(raw, descriptor);
            const toneClass = tone ? ` tone-${escapeHtmlAttr(tone)}` : "";
            const previewAttrs = ` data-cell-preview-key="${escapeHtmlAttr(
              `${rowIndex}::${descriptor.key}`
            )}" data-r="${rowIndex}" data-c="${escapeHtmlAttr(descriptor.key)}"`;
            const content = descriptor.tag
              ? `<span class="cell-tag${toneClass}${cell.tipClass}"${cell.titleAttr}${previewAttrs}>${cell.html}</span>`
              : `<span class="cell-inner${toneClass}${cell.tipClass}"${cell.titleAttr}${previewAttrs}>${cell.html}</span>`;
            return `<td style="${escapeHtmlAttr(inlineStyleForColumn(descriptor, "cell"))}">${content}</td>`;
          })
          .join("")}</tr>`
    )
    .join("");
}

function resolveDataSource(props) {
  const direct = props.data || props.value || null;
  if (direct && (direct.shape === "dataframe" || Array.isArray(direct.value))) {
    const rows = Array.isArray(direct.value) ? direct.value : [];
    return {
      title: direct.label || direct.id || "Dataframe",
      columns: columnsFromSchemaOrRows(direct.schema, rows),
      columnMeta: columnMetaFromSchema(direct.schema),
      rows,
      id: direct.id || null,
    };
  }
  if (direct && Array.isArray(direct.rows)) {
    return {
      title: direct.title || direct.id || "Dataset",
      columns: Array.isArray(direct.columns) ? direct.columns : columnsFromSchemaOrRows(direct.schema, direct.rows),
      columnMeta: columnMetaFromSchema(direct.schema),
      rows: direct.rows,
      source: direct.source || {},
      id: direct.id || null,
    };
  }
  const dataset = props.dataset?.dataset || props.dataset || {};
  const rows = Array.isArray(dataset.rows) ? dataset.rows : [];
  const columns = Array.isArray(dataset.columns)
    ? dataset.columns
    : columnsFromSchemaOrRows(dataset.schema, rows);
  return {
    title: dataset.title || dataset.id || "Dataset",
    columns,
    columnMeta: columnMetaFromSchema(dataset.schema),
    rows,
    source: dataset.source || {},
    id: dataset.id || null,
  };
}

function sourceLooksFileBacked(data) {
  const source = data?.source || {};
  const path = String(source.path || "").trim();
  const kind = String(source.kind || "").trim();
  if (!path || path.startsWith("dataset_view:")) return false;
  if (kind.toLowerCase() === "derived") return false;
  if (kind.toLowerCase() === "db") return true;
  let meta = {};
  try {
    meta = source.content ? JSON.parse(source.content) : {};
  } catch {
    meta = {};
  }
  if (meta.connection || meta.table || meta.query) return true;
  if (["csv", "json", "xlsx", "xls"].includes(kind.toLowerCase())) return true;
  return /\.(csv|json|xlsx|xls)$/i.test(path);
}

function resolveServerPaging(props, data) {
  const capability = resolveDatasetQueryCapability(props);
  const source = data?.source || {};
  const sourceMeta = parseSourceMeta(source.content);
  const pacing = sourceMeta?.lazy || {};
  const fileBacked = sourceLooksFileBacked(data);
  const dataRef = resolveRuntimeDataRef(props);
  const metricRef = resolveRuntimeMetricRef(props);
  const metricDataframe = metricRef && isDataframeBinding(props, data);
  const queryBound = !!queryStateIdOf(props);
  const requiresServer = !!(fileBacked || dataRef || metricDataframe || queryBound);
  return {
    server: requiresServer,
    canQuery: capability.enabled,
    capability,
    defaultPageSize: Number(pacing.default_page_size || 20),
    maxPageSize: Number(pacing.max_page_size || 1000),
  };
}

function isDataframeBinding(props, data) {
  const direct = props?.data || props?.value;
  if (direct?.shape === "dataframe") return true;
  if (Array.isArray(direct?.value)) return true;
  if (Array.isArray(data?.rows) && data.rows.length > 0) return true;
  if (Array.isArray(direct?.schema) && direct.schema.length > 0) return true;
  return false;
}

function parseSourceMeta(raw) {
  if (!raw || typeof raw !== "string") return {};
  try {
    return JSON.parse(raw);
  } catch {
    return {};
  }
}

function buildFilters(field, value) {
  if (!field || !value) return {};
  const out = {};
  out[field] = value;
  return out;
}

function runtimePerfMeta(element) {
  return runtimeCallerMeta(element, "mei-dataset-table");
}

function columnMetaFromSchema(schema) {
  if (!Array.isArray(schema)) return [];
  return schema
    .map((column) => {
      const name = String(column?.name || "").trim();
      if (!name) return null;
      return {
        name,
        type_name: String(column?.type_name || column?.type || "string"),
        sortable: true,
        filterable: true,
      };
    })
    .filter(Boolean);
}

function columnsFromSchemaOrRows(schema, rows) {
  if (Array.isArray(schema) && schema.length > 0) {
    const fromSchema = schema.map((column) => column?.name).filter(Boolean);
    if (fromSchema.length > 0) {
      return fromSchema;
    }
  }
  if (Array.isArray(rows) && rows.length > 0 && typeof rows[0] === "object" && rows[0] !== null) {
    return Object.keys(rows[0]);
  }
  return [];
}

function applyClientView(rows, { filters, search, sort, descriptors }) {
  const filtered = rows.filter((row) => rowMatchesLocal(row, filters, search));
  if (!Array.isArray(sort) || sort.length === 0) return filtered;
  const descriptorByKey = new Map(descriptors.map((item) => [item.key, item]));
  return filtered.slice().sort((left, right) => {
    for (const item of sort) {
      const descriptor = descriptorByKey.get(item.field);
      const result = compareCellValues(
        cellValue(left, item.field),
        cellValue(right, item.field),
        descriptor?.type
      );
      if (result !== 0) {
        return item.direction === "desc" ? -result : result;
      }
    }
    return 0;
  });
}

function rowMatchesLocal(row, filters, search) {
  const searchText = String(search || "").trim().toLowerCase();
  const filterEntries = Object.entries(filters || {});
  const passesFilters = filterEntries.every(([key, value]) => {
    const actual = String(cellValue(row, key) ?? "").toLowerCase();
    return actual.includes(String(value || "").trim().toLowerCase());
  });
  if (!passesFilters) return false;
  if (!searchText) return true;
  return Object.values(row || {}).some((value) =>
    String(value ?? "").toLowerCase().includes(searchText)
  );
}

function compareCellValues(left, right, type) {
  if (left == null && right == null) return 0;
  if (left == null) return 1;
  if (right == null) return -1;
  if (type === "number") {
    const lhs = Number(left);
    const rhs = Number(right);
    if (Number.isFinite(lhs) && Number.isFinite(rhs)) return lhs - rhs;
  }
  return String(left).localeCompare(String(right), "zh-CN", { numeric: true, sensitivity: "base" });
}

customElements.define("mei-dataset-table", MeiDatasetTable);
