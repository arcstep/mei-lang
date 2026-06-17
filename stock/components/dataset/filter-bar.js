import {
  escapeHtml,
  escapeHtmlAttr,
  fetchDatasetRows,
  getQueryState,
  mergeFilters,
  parseProps,
  queryStateIdOf,
  setQueryState,
  subscribeQueryState,
} from "./runtime-query.js";
import { COCKPIT_TYPE, cockpitCssVars, themeColor } from "../cockpit/tokens.js";
import {
  buildColumnProfiles,
  defaultOperatorForProfile,
  operatorOptionsForProfile,
} from "./filter-bar-infer.js";
import {
  createEmptyFilterRow,
  encodeFilterRow,
  filtersToRows,
} from "./filter-bar-expr.js";

const FILTER_PANEL_FONT = "var(--mei-font-1, 16px)";

class MeiDatasetFilterBar extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    this._queryStateId = queryStateIdOf(this._props);
    this._filterMode = resolveFilterBarMode(this._props);
    this._additiveMode = this._filterMode !== "classic";
    this._columnCatalog = resolveColumnCatalog(this._props);
    this._fields = this._additiveMode
      ? this._columnCatalog
      : Array.isArray(this._props.fields)
        ? this._props.fields
        : [];
    this._fieldOptions = new Map();
    this._columnProfiles = new Map();
    this._optionsLoaded = false;
    this._openDropdownKey = "";
    this._rowSeq = 0;
    this._additiveRows = [];
    this._pendingClassicMulti = new Map();
    this._panelCollapsed = resolvePanelCollapsed(this._props);
    this.attachShadow({ mode: "open" });
    const initialFilters = mergeFilters(this._props.default_filters);
    const current = getQueryState(this._queryStateId);
    if (
      this._queryStateId &&
      Object.keys(current.filters || {}).length === 0 &&
      Object.keys(initialFilters).length > 0
    ) {
      setQueryState(
        this._queryStateId,
        { filters: initialFilters },
        { filterIntentSource: "filter_bar", transitionSource: "filter_bar" },
      );
    }
    this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
      this._filters = state?.filters || {};
      if (this._additiveMode) {
        this._additiveRows = filtersToRows(
          this._filters,
          this._columnCatalog,
          this._columnProfiles,
          () => this.nextRowId(),
        );
      }
      this.render();
    });
    if (!this._queryStateId) {
      this._filters = initialFilters;
      if (this._additiveMode) {
        this._additiveRows = filtersToRows(
          this._filters,
          this._columnCatalog,
          this._columnProfiles,
          () => this.nextRowId(),
        );
      }
      this.render();
    }
    this._outsideClickHandler = (event) => {
      if (!this._openDropdownKey) return;
      const path = event.composedPath();
      if (path.includes(this)) return;
      this.syncAdditiveRowsFromDom();
      this._openDropdownKey = "";
      this.render();
    };
    document.addEventListener("click", this._outsideClickHandler);
    void this.loadDynamicOptions();
  }

  disconnectedCallback() {
    document.removeEventListener("click", this._outsideClickHandler);
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
    }
  }

  nextRowId() {
    this._rowSeq += 1;
    return `row-${this._rowSeq}`;
  }

  syncAdditiveRowsFromDom() {
    if (!this._additiveMode || !this.shadowRoot) return;
    if (!this.shadowRoot.querySelector("[data-additive-row]")) return;
    this._additiveRows = readAdditiveRowsFromDom(this.shadowRoot, this._additiveRows);
  }

  syncClassicMultiFromDom() {
    if (this._additiveMode || !this.shadowRoot) return;
    if (!this._pendingClassicMulti) {
      this._pendingClassicMulti = new Map();
    }
    for (const checkbox of this.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
      const key = String(checkbox.dataset.fieldKey || "").trim();
      if (!key) continue;
      const control = String(checkbox.dataset.fieldControl || "multi_select").trim();
      if (!this._pendingClassicMulti.has(key)) {
        this._pendingClassicMulti.set(key, { control, values: new Set() });
      }
      const entry = this._pendingClassicMulti.get(key);
      entry.control = control;
      const value = String(checkbox.value || "").trim();
      if (!value) continue;
      if (checkbox.checked) {
        entry.values.add(value);
      } else {
        entry.values.delete(value);
      }
    }
  }

  restoreClassicMultiToDom() {
    if (this._additiveMode || !this._pendingClassicMulti || !this.shadowRoot) return;
    for (const checkbox of this.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
      const key = String(checkbox.dataset.fieldKey || "").trim();
      const value = String(checkbox.value || "").trim();
      if (!key || !value) continue;
      const entry = this._pendingClassicMulti.get(key);
      checkbox.checked = entry?.values?.has(value) || false;
    }
  }

  async loadDynamicOptions() {
    const needsRowset = this._additiveMode
      ? this._columnCatalog.length > 0
      : this._fields.some((field) => shouldLoadRowsetOptions(field));
    if (!needsRowset) {
      this._optionsLoaded = true;
      if (this._additiveMode) {
        this._columnProfiles = buildColumnProfiles(this._columnCatalog, []);
      }
      return;
    }
    const datasetId = String(
      this._props.rowset_dataset_id || this._props.rowsetDatasetId || this._props.dataset?.id || "",
    ).trim();
    if (!datasetId) {
      this._optionsLoaded = true;
      return;
    }
    const props = {
      ...this._props,
      dataset: {
        ...(this._props.dataset || {}),
        id: datasetId,
        shape: "table",
      },
      data: {
        ...(this._props.data || {}),
        id: datasetId,
      },
    };
    try {
      const FILTER_OPTIONS_MAX_ROWS = 2048;
      const result = await fetchDatasetRows(props, {
        page: 1,
        pageSize: FILTER_OPTIONS_MAX_ROWS,
        full: false,
        meta: { component: "dataset.filter-bar", request_id: "filter-bar-options" },
      });
      const rows = Array.isArray(result?.rows) ? result.rows : [];
      if (this._additiveMode) {
        this._columnProfiles = buildColumnProfiles(this._columnCatalog, rows);
        for (const [column, profile] of this._columnProfiles.entries()) {
          if (Array.isArray(profile?.options) && profile.options.length > 0) {
            this._fieldOptions.set(column, profile.options);
          }
        }
      }
      for (const field of this._fields) {
        if (!shouldLoadRowsetOptions(field)) continue;
        const queryKey = fieldQueryKey(field);
        const optionsField = String(field?.options_field || field?.column || queryKey).trim();
        const control = normalizeControl(field);
        const values = new Set();
        for (const row of rows) {
          const raw = row && typeof row === "object" ? row[optionsField] : "";
          const text = String(raw ?? "").trim();
          if (!text) continue;
          if (control === "month_multi_select") {
            const month = extractYearMonth(text);
            if (month) values.add(month);
          } else {
            values.add(text);
          }
        }
        const sorted = Array.from(values).sort((a, b) => a.localeCompare(b, "zh-CN"));
        this._fieldOptions.set(queryKey, sorted);
      }
    } catch (_error) {
      // Keep empty options; filter bar still works for text fields.
    } finally {
      this._optionsLoaded = true;
      if (this._additiveMode && Array.isArray(this._additiveRows)) {
        this._additiveRows = this._additiveRows.map((row) => {
          const column = String(row?.column || "").trim();
          if (!column) return row;
          const profile = this._columnProfiles.get(column) || null;
          const field = findCatalogField(this._columnCatalog, column);
          return { ...row, operator: resolveRowOperator(row, profile, field) };
        });
      }
      this.render();
    }
  }

  render() {
    if (this._additiveMode) {
      this.renderAdditive();
      return;
    }
    this.renderClassic();
  }

  renderClassic() {
    const filters = this._filters || {};
    const loadingOptions = !this._optionsLoaded;
    this.shadowRoot.innerHTML = `
      <style>${sharedStyles()}</style>
      <section class="wrap">
        <h4 class="title">${escapeHtml(this._props.title || "过滤条件")}</h4>
        <div class="desc">${escapeHtml(this._props.description || "更新页面级 query_state，驱动多个 panel 联动刷新。")}</div>
        ${loadingOptions ? `<div class="loading">正在加载筛选项…</div>` : ""}
        <div class="fields">
          ${this._fields.map((field, index) => renderField(field, filters, index, this._fieldOptions, this._openDropdownKey)).join("")}
        </div>
        <div class="actions">
          <button id="clear" type="button" class="action">清空</button>
          <button id="apply" type="button" class="action primary">应用</button>
        </div>
      </section>
    `;
    this.bindClassicEvents();
    this.restoreClassicMultiToDom();
  }

  renderAdditive() {
    const loadingOptions = !this._optionsLoaded;
    const rows =
      Array.isArray(this._additiveRows) && this._additiveRows.length > 0
        ? this._additiveRows
        : [createEmptyFilterRow(() => this.nextRowId())];
    this._additiveRows = rows;
    const activeCount = countActiveFilterRows(rows, this._columnProfiles, this._columnCatalog);
    const collapsed = Boolean(this._panelCollapsed);
    const title = String(this._props.title || "筛选条件").trim();

    this.shadowRoot.innerHTML = `
      <style>${sharedStyles()}${additiveStyles()}</style>
      <section class="wrap ${collapsed ? "is-collapsed" : ""}">
        <div class="filter-panel-head">
          <button id="toggle-panel" type="button" class="panel-toggle" aria-expanded="${collapsed ? "false" : "true"}">
            <span class="panel-title">${escapeHtml(title)}</span>
            ${activeCount > 0 ? `<span class="panel-active-badge">${activeCount}</span>` : ""}
            <span class="panel-chevron" aria-hidden="true"></span>
          </button>
        </div>
        <div class="filter-panel-body">
          <div class="parallel-hint"><span class="parallel-badge">并行</span></div>
          ${loadingOptions ? `<div class="loading">正在加载筛选项…</div>` : ""}
          <div class="additive-rows">
            ${rows
              .map((row, index) =>
                renderAdditiveRow(
                  row,
                  index,
                  this._columnCatalog,
                  this._columnProfiles,
                  this._fieldOptions,
                  this._filters,
                  this._openDropdownKey,
                ),
              )
              .join("")}
          </div>
          <button id="add-row" type="button" class="add-row" ${loadingOptions ? "disabled" : ""}>+ 添加条件</button>
          <div class="actions">
            <button id="clear" type="button" class="action">清除</button>
            <button id="apply" type="button" class="action primary">应用</button>
          </div>
        </div>
      </section>
    `;
    this.bindAdditiveEvents();
  }

  bindClassicEvents() {
    this.shadowRoot.getElementById("apply")?.addEventListener("click", () => this.apply());
    this.shadowRoot.getElementById("clear")?.addEventListener("click", () => {
      for (const input of this.shadowRoot.querySelectorAll('input[type="text"][data-field-key]')) {
        input.value = "";
      }
      for (const checkbox of this.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
        checkbox.checked = false;
      }
      this._pendingClassicMulti = new Map();
      this.apply();
    });
    for (const trigger of this.shadowRoot.querySelectorAll("[data-multi-trigger]")) {
      trigger.addEventListener("click", (event) => {
        event.stopPropagation();
        if (this._additiveMode) {
          this.syncAdditiveRowsFromDom();
        } else {
          this.syncClassicMultiFromDom();
        }
        const key = String(trigger.dataset.multiTrigger || "").trim();
        this._openDropdownKey = this._openDropdownKey === key ? "" : key;
        this.render();
      });
    }
    for (const checkbox of this.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
      checkbox.addEventListener("change", (event) => {
        event.stopPropagation();
        if (this._additiveMode) {
          this.syncAdditiveRowsFromDom();
          this.render();
          return;
        }
        this.syncClassicMultiFromDom();
        if (this._props.live === true) {
          this.apply();
        }
      });
    }
    for (const input of this.shadowRoot.querySelectorAll('input[type="text"][data-field-key]')) {
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter") {
          this.apply();
        }
      });
      input.addEventListener("change", () => {
        if (this._props.live === true) {
          this.apply();
        }
      });
    }
  }

  bindAdditiveEvents() {
    this.shadowRoot.getElementById("toggle-panel")?.addEventListener("click", () => {
      this._panelCollapsed = !this._panelCollapsed;
      this.render();
    });
    this.shadowRoot.getElementById("apply")?.addEventListener("click", () => this.apply());
    this.shadowRoot.getElementById("clear")?.addEventListener("click", () => {
      this._additiveRows = [createEmptyFilterRow(() => this.nextRowId())];
      if (this._queryStateId) {
        const filters = buildAdditiveFilterMap(
          this._additiveRows,
          this._columnProfiles,
          this._columnCatalog,
          this._queryStateId,
        );
        setQueryState(
          this._queryStateId,
          { filters },
          { filterIntentSource: "filter_bar", transitionSource: "filter_bar" },
        );
      } else {
        this._filters = {};
        this.render();
      }
    });
    this.shadowRoot.getElementById("add-row")?.addEventListener("click", () => {
      this._additiveRows = [
        ...readAdditiveRowsFromDom(this.shadowRoot, this._additiveRows),
        createEmptyFilterRow(() => this.nextRowId()),
      ];
      this.render();
    });
    for (const button of this.shadowRoot.querySelectorAll("[data-remove-row]")) {
      button.addEventListener("click", () => {
        const rowId = String(button.dataset.removeRow || "").trim();
        const current = readAdditiveRowsFromDom(this.shadowRoot, this._additiveRows);
        const next = current.filter((row) => row.id !== rowId);
        this._additiveRows =
          next.length > 0 ? next : [createEmptyFilterRow(() => this.nextRowId())];
        this.render();
      });
    }
    for (const select of this.shadowRoot.querySelectorAll("select[data-row-column]")) {
      select.addEventListener("change", () => {
        this._additiveRows = readAdditiveRowsFromDom(this.shadowRoot, this._additiveRows);
        const rowId = String(select.dataset.rowColumn || "").trim();
        const row = this._additiveRows.find((entry) => entry.id === rowId);
        if (row) {
          const profile = this._columnProfiles.get(row.column) || null;
          row.operator = defaultOperatorForProfile(profile);
          row.negate = false;
          row.value = "";
          row.values = [];
          row.rangeStart = "";
          row.rangeEnd = "";
        }
        this.render();
      });
    }
    for (const select of this.shadowRoot.querySelectorAll("select[data-row-operator]")) {
      select.addEventListener("change", () => {
        this._additiveRows = readAdditiveRowsFromDom(this.shadowRoot, this._additiveRows);
        const rowId = String(select.dataset.rowOperator || "").trim();
        const row = this._additiveRows.find((entry) => entry.id === rowId);
        if (row) {
          row.value = "";
          row.values = [];
          row.rangeStart = "";
          row.rangeEnd = "";
        }
        this.render();
      });
    }
    for (const trigger of this.shadowRoot.querySelectorAll("[data-multi-trigger]")) {
      trigger.addEventListener("click", (event) => {
        event.stopPropagation();
        this.syncAdditiveRowsFromDom();
        const key = String(trigger.dataset.multiTrigger || "").trim();
        this._openDropdownKey = this._openDropdownKey === key ? "" : key;
        this.render();
      });
    }
    for (const checkbox of this.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
      checkbox.addEventListener("change", (event) => {
        event.stopPropagation();
        this.syncAdditiveRowsFromDom();
        this.render();
      });
    }
    for (const input of this.shadowRoot.querySelectorAll("[data-row-value], [data-row-range-start], [data-row-range-end]")) {
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter") {
          this.apply();
        }
      });
    }
  }

  collectFilters() {
    if (this._additiveMode) {
      this.syncAdditiveRowsFromDom();
      return buildAdditiveFilterMap(
        this._additiveRows,
        this._columnProfiles,
        this._columnCatalog,
        this._queryStateId,
      );
    }
    this.syncClassicMultiFromDom();
    const filters = {};
    for (const input of this.shadowRoot.querySelectorAll('input[type="text"][data-field-key]')) {
      const key = String(input.dataset.fieldKey || "").trim();
      const value = String(input.value || "").trim();
      if (!key || !value) continue;
      filters[key] = value;
    }
    for (const [key, entry] of this._pendingClassicMulti.entries()) {
      const values = Array.from(entry?.values || []).filter(Boolean);
      if (!key || values.length === 0) continue;
      const prefix = entry?.control === "month_multi_select" ? "m:" : "in:";
      filters[key] = `${prefix}${values.join(",")}`;
    }
    return filters;
  }

  apply() {
    if (this._additiveMode) {
      this.syncAdditiveRowsFromDom();
    } else {
      this.syncClassicMultiFromDom();
    }
    const filters = this.collectFilters();
    if (this._queryStateId) {
      setQueryState(
        this._queryStateId,
        { filters },
        { filterIntentSource: "filter_bar", transitionSource: "filter_bar" },
      );
    } else {
      this._filters = filters;
      this.render();
    }
  }
}

function isAdditiveMode(props) {
  return resolveFilterBarMode(props) !== "classic";
}

function resolveFilterBarMode(props) {
  const mode = String(props?.mode || props?.filter_mode || "").trim().toLowerCase();
  if (mode === "schema" || mode === "preset") return "schema";
  if (mode === "additive" || mode === "builder") return "additive";
  if (mode === "classic") return "classic";
  if (props?.schema_prepared === true || props?.schemaPrepared === true) return "schema";
  return "classic";
}

function resolvePanelCollapsed(props) {
  return (
    props?.default_collapsed === true ||
    props?.defaultCollapsed === true ||
    props?.collapsed === true
  );
}

function findCatalogField(catalog, column) {
  return (catalog || []).find((field) => fieldQueryKey(field) === column) || null;
}

function countActiveFilterRows(rows, profiles, catalog) {
  let count = 0;
  for (const row of rows || []) {
    const column = String(row?.column || "").trim();
    if (!column) continue;
    const field = findCatalogField(catalog, column);
    const profile = profileForColumn(column, profiles);
    const operator = resolveRowOperator(row, profile, field);
    const encoded = encodeFilterRow({ ...row, operator }, profile);
    if (encoded) count += 1;
  }
  return count;
}

function resolveColumnCatalog(props) {
  const raw = props?.column_catalog || props?.columnCatalog || props?.fields;
  if (!Array.isArray(raw)) return [];
  return raw
    .map((field) => {
      const column = String(field?.column || field?.key || field?.field || "").trim();
      if (!column) return null;
      const control = normalizeControl(field);
      const needsRowsetOptions =
        control === "multi_select" ||
        control === "month_multi_select" ||
        String(field?.options_from || field?.optionsFrom || "").trim() === "rowset";
      return {
        key: column,
        label: String(field?.label || column).trim() || column,
        column,
        control,
        options_from: needsRowsetOptions ? "rowset" : String(field?.options_from || ""),
        options_field: String(field?.options_field || field?.column || column).trim(),
        options: Array.isArray(field?.options) ? field.options : [],
      };
    })
    .filter(Boolean);
}

function valueIsSelected(selectedValues, optionValue) {
  const target = String(optionValue ?? "");
  return (selectedValues || []).some((item) => String(item ?? "") === target);
}

function resolveMultiOptions(options, selectedValues) {
  const items = [];
  const seen = new Set();
  for (const option of options || []) {
    const optionValue = typeof option === "string" ? option : option?.value || "";
    const text = String(optionValue).trim();
    if (!text || seen.has(text)) continue;
    seen.add(text);
    items.push({
      value: text,
      label: typeof option === "string" ? option : option?.label || text,
    });
  }
  for (const value of selectedValues || []) {
    const text = String(value).trim();
    if (!text || seen.has(text)) continue;
    seen.add(text);
    items.push({ value: text, label: text });
  }
  return items;
}

function profileForColumn(column, profiles) {
  return profiles?.get(column) || null;
}

function resolveRowOperator(row, profile, fieldHint = null) {
  const requested = String(row?.operator || fieldHint?.operator || "").trim();
  const options = operatorOptionsForProfile(profile);
  if (requested && options.some((entry) => entry.id === requested)) {
    return requested;
  }
  return defaultOperatorForProfile(profile, fieldHint);
}

function readAdditiveRowsFromDom(shadowRoot, previousRows = []) {
  const rows = [];
  for (const rowEl of shadowRoot.querySelectorAll("[data-additive-row]")) {
    const id = String(rowEl.dataset.additiveRow || "").trim();
    if (!id) continue;
    const previous = (previousRows || []).find((entry) => entry.id === id) || null;
    const column = String(rowEl.dataset.rowColumn || rowEl.querySelector("select[data-row-column]")?.value || "").trim();
    const operator = String(rowEl.querySelector("select[data-row-operator]")?.value || "").trim();
    const negate = Boolean(rowEl.querySelector("input[data-row-negate]")?.checked);
    const valueInput = rowEl.querySelector("[data-row-value]");
    const value = valueInput ? String(valueInput.value || "").trim() : "";
    const values = [];
    const checkboxes = rowEl.querySelectorAll('.multi-option input[type="checkbox"]');
    if (checkboxes.length > 0) {
      for (const checkbox of checkboxes) {
        if (!checkbox.checked) continue;
        const item = String(checkbox.value || "").trim();
        if (item) values.push(item);
      }
    } else if (Array.isArray(previous?.values) && previous.values.length > 0) {
      values.push(...previous.values.filter(Boolean));
    }
    const rangeStart = String(
      rowEl.querySelector("[data-row-range-start]")?.value || previous?.rangeStart || "",
    ).trim();
    const rangeEnd = String(
      rowEl.querySelector("[data-row-range-end]")?.value || previous?.rangeEnd || "",
    ).trim();
    rows.push({ id, column, operator, negate, value, values, rangeStart, rangeEnd });
  }
  return rows;
}

function catalogColumnKeys(catalog) {
  return new Set((catalog || []).map((field) => fieldQueryKey(field)).filter(Boolean));
}

function buildAdditiveFilterMap(rows, profiles, catalog, queryStateId) {
  const catalogKeys = catalogColumnKeys(catalog);
  const current = queryStateId ? getQueryState(queryStateId).filters || {} : {};
  const filters = {};
  for (const [key, value] of Object.entries(current)) {
    if (!catalogKeys.has(key)) {
      filters[key] = value;
    }
  }
  for (const row of rows || []) {
    const column = String(row?.column || "").trim();
    if (!column) continue;
    const profile = profileForColumn(column, profiles);
    const field = findCatalogField(catalog, column);
    const normalizedRow = { ...row, operator: resolveRowOperator(row, profile, field) };
    const encoded = encodeFilterRow(normalizedRow, profile);
    if (encoded) {
      filters[column] = encoded;
    } else {
      delete filters[column];
    }
  }
  return filters;
}

function renderAdditiveValueMarkup(row, profile, operator, options, openDropdownKey) {
  const rowId = String(row?.id || "");
  const selectedValues = Array.isArray(row?.values) ? row.values : [];
  const value = String(row?.value || "").trim();
  const rangeStart = String(row?.rangeStart || "").trim();
  const rangeEnd = String(row?.rangeEnd || "").trim();
  const isOpen = openDropdownKey === rowId;

  if (!String(row?.column || "").trim()) {
    return `<input type="text" data-row-value="${escapeHtmlAttr(rowId)}" placeholder="先选择字段" disabled />`;
  }
  if (operator === "in" || operator === "month_in") {
    const mergedOptions = resolveMultiOptions(options, selectedValues);
    const optionMarkup =
      mergedOptions.length > 0
        ? mergedOptions
            .map((option) => {
              const optionValue = option.value;
              const optionLabel = option.label;
              const checked = valueIsSelected(selectedValues, optionValue) ? "checked" : "";
              return `
                <label class="multi-option">
                  <input type="checkbox" value="${escapeHtmlAttr(optionValue)}" ${checked} />
                  <span>${escapeHtml(optionLabel)}</span>
                </label>
              `;
            })
            .join("")
        : `<div class="multi-empty">暂无可选项</div>`;
    return `
      <div class="row-value-multi">
        <button type="button" class="multi-trigger ${isOpen ? "is-open" : ""}" data-multi-trigger="${escapeHtmlAttr(rowId)}">
          ${escapeHtml(multiSelectSummary(selectedValues, operator === "month_in" ? "month_multi_select" : "multi_select"))}
        </button>
        <div class="multi-panel ${isOpen ? "is-open" : ""}">${optionMarkup}</div>
      </div>`;
  }
  if (operator === "month_range") {
    return `
      <div class="month-range">
        <input type="month" data-row-range-start="${escapeHtmlAttr(rowId)}" value="${escapeHtmlAttr(rangeStart)}" aria-label="起始月份" />
        <span class="month-range-sep">至</span>
        <input type="month" data-row-range-end="${escapeHtmlAttr(rowId)}" value="${escapeHtmlAttr(rangeEnd)}" aria-label="结束月份" />
      </div>`;
  }
  const inputType = profile?.kind === "number" ? "number" : "text";
  const placeholder =
    operator === "contains" ? "包含…" : operator === "eq" ? "等于…" : "输入数值…";
  return `<input
    type="${inputType}"
    data-row-value="${escapeHtmlAttr(rowId)}"
    placeholder="${escapeHtmlAttr(placeholder)}"
    value="${escapeHtmlAttr(value)}"
    step="any"
  />`;
}

function renderAdditiveRow(
  row,
  index,
  catalog,
  profiles,
  fieldOptions,
  appliedFilters,
  openDropdownKey,
) {
  const rowId = String(row?.id || `row-${index}`);
  const column = String(row?.column || "").trim();
  const fieldDef = findCatalogField(catalog, column);
  const profile = profileForColumn(column, profiles);
  const operator = resolveRowOperator(row, profile, fieldDef);
  const negate = Boolean(row?.negate);
  const optionValues = column ? fieldOptions?.get(column) || profile?.options || [] : [];
  const appliedRaw = String(appliedFilters?.[column] ?? "").trim();
  const rowEncoded = column ? encodeFilterRow({ ...row, operator, negate }, profile) : "";
  const applied = Boolean(appliedRaw && rowEncoded && appliedRaw === rowEncoded);

  const columnOptions = (catalog || [])
    .map((entry) => {
      const key = fieldQueryKey(entry);
      const label = entry?.label || key;
      const selected = key === column ? "selected" : "";
      return `<option value="${escapeHtmlAttr(key)}" ${selected}>${escapeHtml(label)}</option>`;
    })
    .join("");

  const operatorChoices = operatorOptionsForProfile(profile);
  const operatorOptions = operatorChoices
    .map((entry) => {
      const selected = entry.id === operator ? "selected" : "";
      return `<option value="${escapeHtmlAttr(entry.id)}" ${selected}>${escapeHtml(entry.label)}</option>`;
    })
    .join("");

  const valueMarkup = renderAdditiveValueMarkup(row, profile, operator, optionValues, openDropdownKey);
  const fieldMarkup = `<select data-row-column="${escapeHtmlAttr(rowId)}" aria-label="筛选字段">
        <option value="">选择字段</option>
        ${columnOptions}
      </select>`;

  return `
    <div class="additive-row ${applied ? "is-applied" : ""}" data-additive-row="${escapeHtmlAttr(rowId)}" data-row-column="${escapeHtmlAttr(column)}">
      <span class="row-index" aria-hidden="true">${index + 1}</span>
      <div class="row-stack">
        <label class="row-block">
          <span class="row-label">字段</span>
          ${fieldMarkup}
        </label>
        <label class="row-block">
          <span class="row-label">条件</span>
          <select data-row-operator="${escapeHtmlAttr(rowId)}" aria-label="筛选条件" ${column ? "" : "disabled"}>
            ${column ? operatorOptions : `<option value="">—</option>`}
          </select>
        </label>
        <label class="row-block row-block-value">
          <span class="row-label">值</span>
          <div class="row-value">${valueMarkup}</div>
        </label>
        <label class="row-negate">
          <input type="checkbox" data-row-negate="${escapeHtmlAttr(rowId)}" ${negate ? "checked" : ""} ${column ? "" : "disabled"} />
          <span>取反</span>
        </label>
      </div>
      <button type="button" class="row-remove" data-remove-row="${escapeHtmlAttr(rowId)}" aria-label="移除此条件">×</button>
    </div>
  `;
}

function sharedStyles() {
  return `
    :host {
      display: block;
      ${cockpitCssVars()}
    }
    .wrap { display: grid; gap: 10px; padding: 14px; border-radius: 14px; background: ${themeColor("filter_panel_bg", "rgba(10, 40, 78, 0.88)")}; border: 1px solid ${themeColor("filter_panel_border", "rgba(56, 160, 240, 0.22)")}; color: ${themeColor("text_body", "#e2e8f0")}; }
    .title { margin: 0; font-size: ${FILTER_PANEL_FONT}; color: ${themeColor("text_inverse", "#f8fafc")}; }
    .desc { color: ${themeColor("text_muted", "#a8c8e6")}; font-size: ${FILTER_PANEL_FONT}; line-height: 1.45; }
    .fields { display: grid; gap: 10px; grid-template-columns: 1fr; }
    label.field { display: grid; gap: 6px; font-size: ${FILTER_PANEL_FONT}; color: ${themeColor("text_body", "#e2e8f0")}; position: relative; }
    input[type="text"], input[type="date"], select, button { border-radius: 8px; border: 1px solid ${themeColor("drilldown_tab_border", "rgba(56, 160, 240, 0.32)")}; background: ${themeColor("drilldown_tab_bg", "rgba(10, 40, 78, 0.72)")}; color: ${themeColor("text_body", "#e2e8f0")}; font-size: ${FILTER_PANEL_FONT}; padding: 7px 9px; }
    .multi-trigger { width: 100%; text-align: left; cursor: pointer; display: flex; justify-content: space-between; gap: 8px; align-items: center; }
    .multi-trigger::after { content: "▾"; opacity: .7; }
    .multi-trigger.is-open::after { content: "▴"; }
    .multi-panel { display: none; position: absolute; left: 0; right: 0; top: calc(100% - 2px); z-index: 20; max-height: 220px; overflow: auto; border-radius: 8px; border: 1px solid ${themeColor("filter_panel_border", "rgba(56, 160, 240, 0.22)")}; background: ${themeColor("drilldown_panel_bottom", "rgba(10, 40, 78, 0.98)")}; box-shadow: 0 12px 28px rgba(2, 6, 23, 0.45); padding: 6px; }
    .multi-panel.is-open { display: block; }
    .multi-option { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 6px; cursor: pointer; font-size: ${FILTER_PANEL_FONT}; color: ${themeColor("text_body", "#e2e8f0")}; }
    .multi-option:hover { background: ${themeColor("table_row_hover", "rgba(32, 96, 168, 0.38)")}; }
    .multi-option input { margin: 0; }
    .actions { display: flex; gap: 8px; justify-content: flex-end; }
    button.action { cursor: pointer; }
    button.action.primary { border-color: rgba(56, 189, 248, 0.55); color: #e0f2fe; background: rgba(14, 116, 178, 0.35); }
    .loading { color: ${themeColor("text_muted", "#a8c8e6")}; font-size: ${FILTER_PANEL_FONT}; }
    .multi-empty { padding: 8px; color: ${themeColor("text_muted", "#a8c8e6")}; font-size: ${FILTER_PANEL_FONT}; }
  `;
}

function additiveStyles() {
  return `
    .filter-panel-head { display: flex; align-items: center; }
    .panel-toggle { width: 100%; display: flex; align-items: center; gap: 8px; padding: 0; border: 0; background: transparent; color: ${themeColor("text_inverse", "#f8fafc")}; font-size: ${FILTER_PANEL_FONT}; cursor: pointer; text-align: left; }
    .panel-title { flex: 1; font-weight: 600; }
    .panel-active-badge { display: inline-flex; align-items: center; justify-content: center; min-width: 20px; height: 20px; padding: 0 6px; border-radius: 999px; background: rgba(56, 189, 248, 0.22); color: #bae6fd; font-size: calc(${FILTER_PANEL_FONT} * 0.82); }
    .panel-chevron { width: 10px; height: 10px; border-right: 2px solid rgba(186, 230, 253, 0.85); border-bottom: 2px solid rgba(186, 230, 253, 0.85); transform: rotate(45deg); transition: transform 0.15s ease; margin-right: 4px; }
    .wrap.is-collapsed .panel-chevron { transform: rotate(-135deg); margin-top: 4px; }
    .filter-panel-body { display: grid; gap: 10px; }
    .wrap.is-collapsed .filter-panel-body { display: none; }
    .row-field-label { padding: 7px 9px; border-radius: 8px; border: 1px solid rgba(56, 160, 240, 0.18); background: rgba(8, 32, 68, 0.28); color: ${themeColor("text_body", "#e2e8f0")}; }
    .row-remove-spacer { width: 28px; height: 28px; flex: 0 0 28px; }
    .parallel-hint { display: flex; align-items: center; gap: 8px; }
    .parallel-badge { display: inline-flex; align-items: center; padding: 2px 8px; border-radius: 999px; border: 1px solid rgba(56, 189, 248, 0.35); color: #bae6fd; font-size: ${FILTER_PANEL_FONT}; letter-spacing: 0.04em; }
    .additive-rows { display: grid; gap: 10px; }
    .additive-row { display: grid; grid-template-columns: 22px minmax(0, 1fr) auto; gap: 8px; align-items: start; padding: 10px; border-radius: 10px; border: 1px solid rgba(56, 160, 240, 0.18); background: rgba(8, 32, 68, 0.35); }
    .additive-row.is-applied { border-color: rgba(56, 189, 248, 0.42); box-shadow: inset 0 0 0 1px rgba(56, 189, 248, 0.12); }
    .row-index { text-align: center; color: rgba(148, 163, 184, 0.75); font-size: ${FILTER_PANEL_FONT}; padding-top: 4px; }
    .row-stack { display: grid; gap: 8px; min-width: 0; }
    .row-block { display: grid; gap: 4px; font-size: ${FILTER_PANEL_FONT}; }
    .row-label { color: rgba(148, 163, 184, 0.9); font-size: calc(${FILTER_PANEL_FONT} * 0.88); }
    .row-block select, .row-block input[type="text"], .row-block input[type="number"], .row-block input[type="month"] { width: 100%; box-sizing: border-box; }
    .row-block-value .row-value { position: relative; min-width: 0; }
    .row-value-multi { position: relative; width: 100%; }
    .row-value-multi .multi-trigger { width: 100%; box-sizing: border-box; }
    .row-value-multi .multi-panel { left: 0; right: 0; }
    .row-negate { display: inline-flex; align-items: center; gap: 6px; color: ${themeColor("text_body", "#e2e8f0")}; font-size: ${FILTER_PANEL_FONT}; cursor: pointer; }
    .row-negate input { margin: 0; }
    .month-range { display: grid; grid-template-columns: 1fr auto 1fr; gap: 6px; align-items: center; }
    .month-range-sep { color: rgba(148, 163, 184, 0.85); font-size: ${FILTER_PANEL_FONT}; }
    .row-remove { width: 28px; height: 28px; padding: 0; display: inline-flex; align-items: center; justify-content: center; cursor: pointer; color: rgba(148, 163, 184, 0.8); background: transparent; }
    .row-remove:hover { color: #e2e8f0; background: rgba(15, 45, 82, 0.45); }
    .add-row { width: 100%; cursor: pointer; border-style: dashed; color: #bae6fd; background: rgba(8, 32, 68, 0.25); }
    .add-row:disabled { opacity: 0.55; cursor: not-allowed; }
  `;
}

function normalizeControl(field) {
  const control = String(field?.control || field?.type || "").trim().toLowerCase();
  if (control === "multi_select" || control === "month_multi_select" || control === "text") {
    return control;
  }
  if (Array.isArray(field?.options) && field.options.length > 0) {
    return "multi_select";
  }
  return "text";
}

function fieldQueryKey(field) {
  return String(field?.key || field?.field || field?.column || "").trim();
}

function shouldLoadRowsetOptions(field) {
  const control = normalizeControl(field);
  if (control !== "multi_select" && control !== "month_multi_select") {
    return false;
  }
  const source = String(field?.options_from || field?.optionsFrom || "").trim().toLowerCase();
  if (source === "rowset") return true;
  return !Array.isArray(field?.options) || field.options.length === 0;
}

function extractYearMonth(text) {
  const trimmed = String(text || "").trim();
  if (/^\d{4}-\d{2}/.test(trimmed)) {
    return trimmed.slice(0, 7);
  }
  const parsed = Date.parse(trimmed);
  if (!Number.isNaN(parsed)) {
    const date = new Date(parsed);
    const month = String(date.getMonth() + 1).padStart(2, "0");
    return `${date.getFullYear()}-${month}`;
  }
  return "";
}

function selectedValuesForField(filters, queryKey, control) {
  const raw = String(filters?.[queryKey] || "");
  if (!raw) return [];
  if (control === "month_multi_select" && raw.startsWith("m:")) {
    return raw
      .slice(2)
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);
  }
  if (control === "multi_select" && raw.startsWith("in:")) {
    return raw
      .slice(3)
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);
  }
  if (raw.includes(",")) {
    return raw
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);
  }
  return [raw];
}

function multiSelectSummary(selected, control) {
  if (selected.length === 0) {
    return control === "month_multi_select" ? "选择月份" : "全部";
  }
  if (selected.length <= 2) {
    return selected.join("、");
  }
  return `已选 ${selected.length} 项`;
}

function renderField(field, filters, index, fieldOptions, openDropdownKey) {
  const queryKey = fieldQueryKey(field);
  if (!queryKey) return "";
  const label = field?.label || queryKey;
  const placeholder = field?.placeholder || "";
  const control = normalizeControl(field);
  const selected = selectedValuesForField(filters, queryKey, control);
  const staticOptions = Array.isArray(field?.options) ? field.options : [];
  const dynamicOptions = fieldOptions?.get(queryKey) || [];
  const options = staticOptions.length > 0 ? staticOptions : dynamicOptions;
  const isOpen = openDropdownKey === queryKey;

  if (control === "multi_select" || control === "month_multi_select") {
    const mergedOptions = resolveMultiOptions(options, selected);
    const optionMarkup =
      mergedOptions.length > 0
        ? mergedOptions
            .map((option) => {
              const optionValue = option.value;
              const optionLabel = option.label;
              const checked = valueIsSelected(selected, optionValue) ? "checked" : "";
              return `
                <label class="multi-option">
                  <input
                    type="checkbox"
                    value="${escapeHtmlAttr(optionValue)}"
                    data-field-key="${escapeHtmlAttr(queryKey)}"
                    data-field-control="${escapeHtmlAttr(control)}"
                    data-field-index="${index}"
                    ${checked}
                  />
                  <span>${escapeHtml(optionLabel)}</span>
                </label>
              `;
            })
            .join("")
        : `<div class="multi-empty">暂无可选项</div>`;

    return `
      <label class="field">
        <span>${escapeHtml(label)}</span>
        <button
          type="button"
          class="multi-trigger ${isOpen ? "is-open" : ""}"
          data-multi-trigger="${escapeHtmlAttr(queryKey)}"
        >${escapeHtml(multiSelectSummary(selected, control))}</button>
        <div class="multi-panel ${isOpen ? "is-open" : ""}" data-multi-panel="${escapeHtmlAttr(queryKey)}">${optionMarkup}</div>
      </label>
    `;
  }

  const value = selected[0] || "";
  return `
    <label class="field">
      <span>${escapeHtml(label)}</span>
      <input
        type="text"
        data-field-key="${escapeHtmlAttr(queryKey)}"
        data-field-control="text"
        data-field-index="${index}"
        placeholder="${escapeHtmlAttr(placeholder)}"
        value="${escapeHtmlAttr(value)}"
      />
    </label>
  `;
}

customElements.define("mei-dataset-filter-bar", MeiDatasetFilterBar);
