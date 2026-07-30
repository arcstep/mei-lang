import {
  escapeHtml,
  escapeHtmlAttr,
  fetchDatasetRows,
  getQueryState,
  isStaticSkeletonDisplay,
  mergeFilters,
  parseProps,
  queryStateIdOf,
  setQueryState,
  subscribeQueryState,
} from "./runtime-query.js";
import { COCKPIT_TYPE, cockpitCssVars } from "../cockpit/tokens.js";
import { color } from "../mei/theme-style.js";
import {
  buildColumnProfiles,
  defaultOperatorForProfile,
  operatorOptionsForField,
  operatorsForField,
} from "./filter-bar-infer.js";
import {
  createEmptyFilterRow,
  encodeFilterRow,
  filtersToRows,
  sanitizeFiltersToCatalog,
  schemaToRows,
} from "./filter-bar-expr.js";

const FILTER_PANEL_FONT = COCKPIT_TYPE.filterPanel;

const CALENDAR_ICON_SVG = `<svg class="date-icon-svg" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false"><path fill="currentColor" d="M13.75 2.875H11.125V1.875c0-.069-.056-.125-.125-.125H10.125c-.069 0-.125.056-.125.125V2.875H6V1.875c0-.069-.056-.125-.125-.125H5c-.069 0-.125.056-.125.125V2.875H2.25A.5.5 0 0 0 1.75 3.375v10.375c0 .276.224.5.5.5h11.5a.5.5 0 0 0 .5-.5V3.375a.5.5 0 0 0-.5-.5Zm-.625 10.25H2.875V7.187h10.25v6.938ZM2.875 6.125V4h2v.75c0 .069.056.125.125.125H5.875c.069 0 .125-.056.125-.125V4h4v.75c0 .069.056.125.125.125H10.125c.069 0 .125-.056.125-.125V4h2v2.125H2.875Z"/></svg>`;

class MeiDatasetFilterBar extends HTMLElement {
  connectedCallback() {
    this._filterFloatingHostId =
      this._filterFloatingHostId || `mei-filter-float-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    this._props = parseProps(this);
    this._queryStateId = queryStateIdOf(this._props);
    this._filterMode = resolveFilterBarMode(this._props);
    this._schemaMode = isSchemaMode(this._props);
    this._additiveMode = !this._schemaMode && isAdditiveFilterMode(this._props);
    this._schemaFields = resolveSchemaFields(this._props);
    this._columnCatalog = resolveColumnCatalog(this._props);
    this._fields = this._schemaMode
      ? this._schemaFields
      : this._additiveMode
        ? this._columnCatalog
        : Array.isArray(this._props.fields)
          ? this._props.fields
          : [];
    this._fieldOptions = new Map();
    this._columnProfiles = new Map();
    this._optionsLoaded = false;
    this._openDropdownKey = "";
    this._openFieldPickerKey = "";
    this._multiPanelSearch = new Map();
    this._fieldPickerSearch = new Map();
    this._rowSeq = 0;
    this._additiveRows = [];
    this._additiveUserTouched = false;
    this._suppressRowSync = false;
    this._confirmErrorRowId = "";
    this._confirmErrorMessage = "";
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
      this.syncRowsFromFilters();
      this.render();
    });
    if (!this._queryStateId) {
      this._filters = initialFilters;
      this.syncRowsFromFilters();
      this.render();
    }
    this._outsideClickHandler = (event) => {
      if (!this._openDropdownKey && !this._openFieldPickerKey) return;
      const path = event.composedPath();
      const staysOpen = path.some((node) => {
        if (!node || typeof node !== "object" || !("dataset" in node)) return false;
        const ds = node.dataset || {};
        if (this._openDropdownKey) {
          if (ds.multiPanel === this._openDropdownKey) return true;
          if (ds.multiTrigger === this._openDropdownKey) return true;
          if (ds.multiSearch === this._openDropdownKey) return true;
        }
        if (this._openFieldPickerKey) {
          if (ds.fieldPickerPanel === this._openFieldPickerKey) return true;
          if (ds.fieldPickerTrigger === this._openFieldPickerKey) return true;
          if (ds.fieldPickerSearch === this._openFieldPickerKey) return true;
        }
        return false;
      });
      if (staysOpen) return;
      if (this._additiveMode) {
        this.syncAdditiveRowsFromDom();
      }
      const closingDropdown = this._openDropdownKey;
      const closingPicker = this._openFieldPickerKey;
      this._openDropdownKey = "";
      this._openFieldPickerKey = "";
      if (closingDropdown) this._multiPanelSearch.delete(closingDropdown);
      if (closingPicker) this._fieldPickerSearch.delete(closingPicker);
      this.render();
    };
    document.addEventListener("click", this._outsideClickHandler);
    void this.loadDynamicOptions();
  }

  disconnectedCallback() {
    document.removeEventListener("click", this._outsideClickHandler);
    teardownFloatingPanelListeners(this);
    cleanupHostFloatingPanels(this);
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
    }
  }

  nextRowId() {
    this._rowSeq += 1;
    return `row-${this._rowSeq}`;
  }

  syncRowsFromFilters() {
    if (this._suppressRowSync) return;
    if (this._schemaMode) {
      this._schemaRows = schemaToRows(
        this._schemaFields,
        this._filters,
        this._columnProfiles,
        () => this.nextRowId(),
      );
      return;
    }
    if (this._additiveMode) {
      const dropUnknown =
        !resolveAllowExtra(this._props) &&
        Array.isArray(this._columnCatalog) &&
        this._columnCatalog.length > 0;
      let effectiveFilters = this._filters || {};
      if (dropUnknown) {
        effectiveFilters = sanitizeFiltersToCatalog(effectiveFilters, this._columnCatalog);
        if (
          this._queryStateId &&
          filterMapsDiffer(this._filters, effectiveFilters)
        ) {
          this._suppressRowSync = true;
          try {
            setQueryState(
              this._queryStateId,
              { filters: effectiveFilters },
              {
                filterIntentSource: "filter_bar",
                transitionSource: "filter_bar_prune_unknown",
              },
            );
          } finally {
            this._suppressRowSync = false;
          }
          this._filters = effectiveFilters;
        }
      }
      const fromFilters =
        filtersToRows(
          effectiveFilters,
          this._columnCatalog,
          this._columnProfiles,
          () => this.nextRowId(),
        ) || [];
      if (dropUnknown && !this._additiveUserTouched) {
        this._additiveRows = mergePresetsWithFilterRows(
          this._columnCatalog,
          resolvePresetFilterCount(this._props),
          fromFilters,
          () => this.nextRowId(),
        );
        return;
      }
      this._additiveRows = mergeAdditiveRowsFromFilters(
        fromFilters,
        this._additiveRows,
        this._columnCatalog,
        resolvePresetFilterCount(this._props),
        () => this.nextRowId(),
        this._additiveUserTouched,
      );
    }
  }

  syncAdditiveRowsFromDom() {
    if (!this._additiveMode || !this.shadowRoot) return;
    if (!this.shadowRoot.querySelector("[data-additive-row]")) return;
    const allowedIds = new Set((this._additiveRows || []).map((entry) => entry.id));
    const domRows = readAdditiveRowsFromDom(this.shadowRoot, this._additiveRows).filter((entry) =>
      allowedIds.has(entry.id),
    );
    const byId = new Map((this._additiveRows || []).map((entry) => [entry.id, entry]));
    for (const domRow of domRows) {
      const previous = byId.get(domRow.id);
      if (!previous) continue;
      let next = {
        ...previous,
        column: domRow.column || previous.column,
        operator: domRow.operator || previous.operator,
        negate: domRow.negate,
        value: domRow.value,
        values: domRow.values,
        rangeStart: domRow.rangeStart,
        rangeEnd: domRow.rangeEnd,
        status: previous.status,
      };
      // contains_any 展示时勾选的是组合面值；用户改勾选后改为 in，避免针值语义错乱
      next = coerceContainsAnyDomSelectionToIn(next, previous);
      byId.set(domRow.id, next);
    }
    this._additiveRows = (this._additiveRows || []).map((entry) => byId.get(entry.id) || entry);
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
    if (typeof document !== "undefined") {
      for (const checkbox of document.querySelectorAll(
        '[data-mei-filter-floating="1"] .multi-option input[type="checkbox"][data-field-key]',
      )) {
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
    if (isStaticSkeletonDisplay(this._props)) {
      for (const field of this._fields) {
        if (!shouldLoadRowsetOptions(field)) continue;
        this._fieldOptions.set(fieldQueryKey(field), ["选项1", "选项2", "选项3"]);
      }
      this._optionsLoaded = true;
      this.render();
      return;
    }
    const profileCatalog = this._schemaMode ? this._schemaFields : this._columnCatalog;
    const needsRowset = this._schemaMode || this._additiveMode
      ? profileCatalog.length > 0
      : this._fields.some((field) => shouldLoadRowsetOptions(field));
    if (!needsRowset) {
      this._optionsLoaded = true;
      if (this._schemaMode || this._additiveMode) {
        this._columnProfiles = buildColumnProfiles(profileCatalog, []);
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
      const FACET_COLUMNS_MAX = 8;
      const facetColumns = [];
      const seen = new Set();
      const pushFacet = (value) => {
        const name = String(value || "").trim();
        if (!name || seen.has(name) || facetColumns.length >= FACET_COLUMNS_MAX) return;
        seen.add(name);
        facetColumns.push(name);
      };
      // Prefer fields that actually need rowset enums; avoid scanning every catalog column.
      for (const field of this._fields || []) {
        if (!shouldLoadRowsetOptions(field)) continue;
        pushFacet(field?.options_field || field?.column || fieldQueryKey(field));
      }
      for (const field of profileCatalog || []) {
        if (typeof field === "string") continue;
        const control = normalizeControl(field);
        if (control === "text" || control === "date_range") continue;
        if (Array.isArray(field?.options) && field.options.length > 0) continue;
        const optionsFrom = String(field?.options_from || field?.optionsFrom || "rowset").trim();
        if (optionsFrom && optionsFrom !== "rowset") continue;
        pushFacet(field?.options_field || field?.optionsField || field?.column || field?.key || field?.field);
      }
      // 选项枚举必须不受当前 query_state / default_filters 影响：
      // 否则入口注入「办理状态=待办」后，facet 只能看到「待办」一项，无法再改选在办/办结。
      const optionsProps = {
        ...props,
        query_state: "",
        queryState: "",
      };
      const result = await fetchDatasetRows(optionsProps, {
        page: 1,
        pageSize: 1,
        full: false,
        facetColumns,
        queryStateId: "",
        filters: {},
        meta: { component: "dataset.filter-bar", request_id: "filter-bar-options" },
      });
      const facets =
        result?.column_facets && typeof result.column_facets === "object"
          ? result.column_facets
          : {};
      const rows = Array.isArray(result?.rows) ? result.rows : [];
      if (this._schemaMode || this._additiveMode) {
        const profileRows =
          Object.keys(facets).length > 0
            ? Object.entries(facets).flatMap(([column, options]) =>
                normalizeFacetOptions(options).map((item) => ({ [column]: item.value })),
              )
            : rows;
        this._columnProfiles = buildColumnProfiles(profileCatalog, profileRows);
        for (const field of profileCatalog) {
          const column = fieldQueryKey(field);
          const optionsField = String(
            field?.options_field || field?.optionsField || field?.column || column,
          ).trim();
          if (!shouldLoadRowsetOptions(field)) {
            if (Array.isArray(field?.options) && field.options.length > 0) {
              this._fieldOptions.set(column, field.options);
            }
            continue;
          }
          // facets 按物理列名（主责单位）返回；query key 可能是 agency。
          const facetOptions = normalizeFacetOptions(
            facets[optionsField] || facets[column] || facets[String(field?.column || "").trim()] || [],
          );
          if (facetOptions.length > 0) {
            this._fieldOptions.set(column, facetOptions);
            continue;
          }
          const profile =
            this._columnProfiles.get(column) ||
            this._columnProfiles.get(optionsField) ||
            this._columnProfiles.get(String(field?.column || "").trim()) ||
            null;
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
        const facetOptions = normalizeFacetOptions(
          facets[optionsField] || facets[queryKey] || [],
        );
        if (facetOptions.length > 0) {
          if (control === "month_multi_select") {
            const monthCounts = new Map();
            for (const item of facetOptions) {
              const month = extractYearMonth(item.value);
              if (!month) continue;
              monthCounts.set(month, (monthCounts.get(month) || 0) + (item.count || 0));
            }
            const sorted = Array.from(monthCounts.entries())
              .map(([value, count]) => ({ value, count }))
              .sort((a, b) => b.count - a.count || a.value.localeCompare(b.value, "zh-CN"));
            this._fieldOptions.set(queryKey, sorted);
          } else {
            // Server already returns count-desc; keep that order.
            this._fieldOptions.set(queryKey, facetOptions);
          }
          continue;
        }
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
      if (this._schemaMode && Array.isArray(this._schemaRows)) {
        this._schemaRows = this._schemaRows.map((row) => {
          const column = String(row?.column || "").trim();
          if (!column) return row;
          const profile = this._columnProfiles.get(column) || null;
          const field = findCatalogField(this._schemaFields, column);
          return { ...row, operator: resolveRowOperator(row, profile, field) };
        });
      }
      if (this._additiveMode && Array.isArray(this._additiveRows)) {
        this._additiveRows = this._additiveRows.map((row) => {
          const column = String(row?.column || "").trim();
          if (!column) return row;
          const profile = this._columnProfiles.get(column) || null;
          const field = findCatalogField(this._columnCatalog, column);
          const allowed = new Set(operatorsForField(profile, field));
          const current = String(row?.operator || "").trim();
          if (current && allowed.has(current)) return row;
          const operator = defaultOperatorForProfile(profile, field);
          if (operator === row.operator) return row;
          const next = { ...row, operator };
          if (isRowDraft(row)) {
            next.value = "";
            next.values = [];
            next.rangeStart = "";
            next.rangeEnd = "";
          }
          return next;
        });
      }
      this.render();
    }
  }

  render() {
    if (this._schemaMode) {
      this.renderSchema();
      return;
    }
    if (this._additiveMode) {
      this.renderAdditive();
      return;
    }
    this.renderClassic();
  }

  renderClassic() {
    const filters = this._filters || {};
    const loadingOptions = !this._optionsLoaded;
    cleanupHostFloatingPanels(this);
    this.shadowRoot.innerHTML = `
      <style>${sharedStyles()}</style>
      <section class="wrap">
        <h4 class="title">${escapeHtml(this._props.title || "过滤条件")}</h4>
        <div class="desc">${escapeHtml(this._props.description || "更新页面级 query_state，驱动多个 panel 联动刷新。")}</div>
        ${loadingOptions ? `<div class="loading">正在加载筛选项…</div>` : ""}
        <div class="fields">
          ${this._fields.map((field, index) => renderField(field, filters, index, this._fieldOptions, this._openDropdownKey, this._multiPanelSearch)).join("")}
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

  renderSchema() {
    const loadingOptions = !this._optionsLoaded;
    const rows = Array.isArray(this._schemaRows) ? this._schemaRows : [];
    const activeCount = countActiveSchemaFilters(rows, this._schemaFields);
    const collapsed = Boolean(this._panelCollapsed);
    const title = String(this._props.title || "筛选条件").trim();
    const visibleFields = (this._schemaFields || []).filter((field) => field?.visible !== false);

    cleanupHostFloatingPanels(this);
    this.shadowRoot.innerHTML = `
      <style>${sharedStyles()}${schemaStyles()}</style>
      <section class="wrap schema-wrap ${collapsed ? "is-collapsed" : ""}">
        <div class="filter-panel-head">
          <button id="toggle-panel" type="button" class="panel-toggle" aria-expanded="${collapsed ? "false" : "true"}">
            <span class="panel-title">${escapeHtml(title)}</span>
            ${activeCount > 0 ? `<span class="panel-active-badge">${activeCount}</span>` : ""}
            <span class="panel-chevron" aria-hidden="true"></span>
          </button>
        </div>
        <div class="filter-panel-body">
          ${loadingOptions ? `<div class="loading">正在加载筛选项…</div>` : ""}
          <div class="schema-fields">
            ${visibleFields
              .map((field) => {
                const column = String(field?.column || field?.key || "").trim();
                const row =
                  rows.find((entry) => String(entry?.column || "").trim() === column) ||
                  createEmptyFilterRow(() => this.nextRowId());
                return renderSchemaField(
                  field,
                  row,
                  this._filters,
                  this._fieldOptions,
                  this._openDropdownKey,
                  this._multiPanelSearch,
                );
              })
              .join("")}
          </div>
          <div class="actions">
            <button id="clear" type="button" class="action">清除</button>
            <button id="apply" type="button" class="action primary">应用</button>
          </div>
        </div>
      </section>
    `;
    this.bindSchemaEvents();
  }

  renderAdditive() {
    const loadingOptions = !this._optionsLoaded;
    const presetCount = resolvePresetFilterCount(this._props);
    const rows =
      Array.isArray(this._additiveRows) && this._additiveRows.length > 0
        ? this._additiveRows
        : !this._additiveUserTouched && presetCount > 0
          ? buildPresetFilterRows(this._columnCatalog, presetCount, () => this.nextRowId())
          : [];
    this._additiveRows = rows.map((row) => normalizeAdditiveRow(row, this._columnCatalog, this._columnProfiles));
    const activeCount = countAppliedCatalogFilters(this._filters, this._columnCatalog);
    const collapsed = Boolean(this._panelCollapsed);
    const title = String(this._props.title || "筛选条件").trim();
    const catalogExhausted = allCatalogFieldsUsed(this._columnCatalog, rows);
    const addableFields = availableCatalogFieldsForAdd(this._columnCatalog, rows);

    const lockedContextMarkup = renderLockedFilterContext(this._props);
    cleanupHostFloatingPanels(this);
    this.shadowRoot.innerHTML = `
      <style>${sharedStyles()}${additiveStyles()}</style>
      <section class="wrap additive-wrap ${collapsed ? "is-collapsed" : ""}">
        <div class="filter-panel-head">
          <button id="toggle-panel" type="button" class="panel-toggle" aria-expanded="${collapsed ? "false" : "true"}">
            <span class="panel-title">${escapeHtml(title)}</span>
            ${activeCount > 0 ? `<span class="panel-active-badge">${activeCount}</span>` : ""}
            <span class="panel-chevron" aria-hidden="true"></span>
          </button>
        </div>
        <div class="filter-panel-body">
          ${lockedContextMarkup}
          ${loadingOptions ? `<div class="loading">正在加载筛选项…</div>` : ""}
          <div class="filter-panel-main">
            <div class="additive-rows">
              ${rows
                .map((row, index) => {
                  const usedColumnKeys = new Set(
                    rows
                      .filter((_, rowIndex) => rowIndex !== index)
                      .map((entry) => String(entry?.column || "").trim())
                      .filter(Boolean),
                  );
                  return renderAdditiveRow(
                    row,
                    index,
                    this._columnCatalog,
                    this._columnProfiles,
                    this._fieldOptions,
                    this._filters,
                    this._openDropdownKey,
                    usedColumnKeys,
                    this._confirmErrorRowId,
                    this._confirmErrorMessage,
                    this._multiPanelSearch,
                    this._openFieldPickerKey,
                    this._fieldPickerSearch,
                  );
                })
                .join("")}
            </div>
            <div class="actions actions-primary">
              <button id="clear" type="button" class="action">清除</button>
              <button id="apply" type="button" class="action primary">查询</button>
            </div>
          </div>
          <div class="filter-panel-footer">
            ${
              catalogExhausted
                ? `<p class="catalog-exhausted-hint">已添加全部可筛字段</p>`
                : renderAddableFieldPicker(
                    addableFields,
                    loadingOptions,
                    this._openFieldPickerKey,
                    this._fieldPickerSearch,
                  )
            }
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
    bindMultiPanelInteractions(this, {
      additiveMode: this._additiveMode,
      schemaMode: false,
      live: this._props.live === true,
      onCheckboxChange: () => {
        if (this._props.live === true && !this._additiveMode) {
          this.apply();
        }
      },
    });
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
    scheduleFloatingPanelSync(this);
    bindTallValueRowInteractions(this);
  }

  bindAdditiveEvents() {
    this.shadowRoot.getElementById("toggle-panel")?.addEventListener("click", () => {
      this._panelCollapsed = !this._panelCollapsed;
      this.render();
    });
    this.shadowRoot.getElementById("clear")?.addEventListener("click", () => {
      this._additiveUserTouched = true;
      this._additiveRows = buildPresetFilterRows(
        this._columnCatalog,
        resolvePresetFilterCount(this._props),
        () => this.nextRowId(),
      );
      this.render();
    });
    this.shadowRoot.getElementById("apply")?.addEventListener("click", () => {
      this.queryFilters();
    });
    for (const button of this.shadowRoot.querySelectorAll("[data-add-field]")) {
      button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        if (button.disabled) return;
        const column = String(button.dataset.addField || "").trim();
        if (!column || allCatalogFieldsUsed(this._columnCatalog, this._additiveRows)) return;
        if (usedCatalogFieldKeys(this._additiveRows).has(column)) return;
        this.syncAdditiveRowsFromDom();
        this._additiveUserTouched = true;
        this._openFieldPickerKey = "";
        this._fieldPickerSearch.delete(ADD_FIELD_PICKER_KEY);
        this._additiveRows = [
          ...this._additiveRows,
          createDraftFilterRowForColumn(
            () => this.nextRowId(),
            column,
            this._columnCatalog,
            this._columnProfiles,
          ),
        ];
        this.render();
      });
    }
    for (const button of this.shadowRoot.querySelectorAll("[data-pick-field-row]")) {
      button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        const rowId = String(button.dataset.pickFieldRow || "").trim();
        const column = String(button.dataset.fieldKey || "").trim();
        if (!rowId || !column) return;
        this._confirmErrorRowId = "";
        this._confirmErrorMessage = "";
        this.syncAdditiveRowsFromDom();
        const row = this._additiveRows.find((entry) => entry.id === rowId);
        if (!row || !isRowDraft(row)) return;
        row.column = column;
        const profile = this._columnProfiles.get(column) || null;
        const field = findCatalogField(this._columnCatalog, column);
        row.operator = defaultOperatorForProfile(profile, field);
        row.negate = false;
        row.value = "";
        row.values = [];
        row.rangeStart = "";
        row.rangeEnd = "";
        this._openFieldPickerKey = "";
        this._fieldPickerSearch.delete(rowFieldPickerKey(rowId));
        this.render();
      });
    }
    bindFieldPickerInteractions(this);
    bindTallValueRowInteractions(this);
    for (const button of this.shadowRoot.querySelectorAll("[data-confirm-row]")) {
      button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        this.confirmAdditiveRow(String(button.dataset.confirmRow || "").trim());
      });
    }
    for (const button of this.shadowRoot.querySelectorAll("[data-remove-row]")) {
      button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        const rowId = String(button.dataset.removeRow || "").trim();
        this.syncAdditiveRowsFromDom();
        const next = this._additiveRows.filter((entry) => entry.id !== rowId);
        this._additiveUserTouched = true;
        this._additiveRows = next;
        this.render();
      });
    }
    for (const select of this.shadowRoot.querySelectorAll("select[data-row-operator]")) {
      select.addEventListener("change", () => {
        this._confirmErrorRowId = "";
        this._confirmErrorMessage = "";
        this.syncAdditiveRowsFromDom();
        const rowId = String(select.dataset.rowOperator || "").trim();
        const row = this._additiveRows.find((entry) => entry.id === rowId);
        if (!row || !isRowDraft(row)) return;
        row.value = "";
        row.values = [];
        row.rangeStart = "";
        row.rangeEnd = "";
        this.render();
      });
    }
    bindMultiPanelInteractions(this, {
      additiveMode: true,
      schemaMode: false,
      live: false,
    });
    for (const input of this.shadowRoot.querySelectorAll("[data-row-value], [data-row-range-start], [data-row-range-end]")) {
      input.addEventListener("change", () => {
        this.syncAdditiveRowsFromDom();
      });
      input.addEventListener("keydown", (event) => {
        if (event.key !== "Enter") return;
        const rowEl = input.closest("[data-additive-row]");
        const rowId = String(rowEl?.dataset?.additiveRow || "").trim();
        if (rowId && rowEl?.classList.contains("is-draft")) {
          this.confirmAdditiveRow(rowId);
          return;
        }
        if (rowEl?.classList.contains("is-active")) {
          this.queryFilters();
        }
      });
    }
    scheduleFloatingPanelSync(this);
  }

  queryFilters() {
    this.syncAdditiveRowsFromDom();
    // 用户点「查询/应用」后视为已交互，避免 syncRowsFromFilters 走 mergePresets
    // 把入口注入但不在前 N 个预置位的条件（如办理状态）丢掉。
    this._additiveUserTouched = true;
    this.applyActiveFilters({ skipDomSync: true });
  }

  confirmAdditiveRow(rowId) {
    if (!rowId) return;
    const rowEl = this.shadowRoot?.querySelector(`[data-additive-row="${rowId}"]`);
    if (!rowEl) return;
    const previous = this._additiveRows.find((entry) => entry.id === rowId);
    if (!previous || !isRowDraft(previous)) return;

    const domRow = readSingleAdditiveRowFromDom(rowEl, previous);
    const column = String(domRow.column || previous.column || "").trim();
    if (!column) {
      this._confirmErrorRowId = rowId;
      this._confirmErrorMessage = "请先选择字段";
      this.render();
      return;
    }
    const profile = profileForColumn(column, this._columnProfiles);
    const field = findCatalogField(this._columnCatalog, column);
    const normalized = {
      ...previous,
      ...domRow,
      column,
      operator: resolveRowOperator({ ...previous, ...domRow, column }, profile, field),
      status: "draft",
    };
    const validationError = additiveDraftValidationError(normalized, profile, field);
    if (validationError) {
      this._confirmErrorRowId = rowId;
      this._confirmErrorMessage = validationError;
      this._additiveRows = this._additiveRows.map((entry) =>
        entry.id === rowId ? { ...normalized, status: "draft" } : entry,
      );
      this.render();
      return;
    }
    this._confirmErrorRowId = "";
    this._confirmErrorMessage = "";
    this._additiveUserTouched = true;
    this._additiveRows = this._additiveRows
      .filter((entry) => !(isRowActive(entry) && String(entry.column || "").trim() === column))
      .map((entry) =>
        entry.id === rowId ? { ...normalized, status: "active" } : entry,
      );
    this.render();
  }

  applyActiveFilters(options = {}) {
    if (!options.skipDomSync) {
      this.syncAdditiveRowsFromDom();
    }
    const filters = buildAdditiveFilterMap(
      this._additiveRows,
      this._columnProfiles,
      this._columnCatalog,
      this._queryStateId,
    );
    if (this._queryStateId) {
      this._suppressRowSync = true;
      const normalized = setQueryState(
        this._queryStateId,
        { filters },
        { filterIntentSource: "filter_bar", transitionSource: "filter_bar" },
      );
      this._filters = normalized?.filters || filters;
      this._suppressRowSync = false;
    } else {
      this._filters = filters;
      this.render();
    }
  }

  bindSchemaEvents() {
    this.shadowRoot.getElementById("toggle-panel")?.addEventListener("click", () => {
      this._panelCollapsed = !this._panelCollapsed;
      this.render();
    });
    this.shadowRoot.getElementById("apply")?.addEventListener("click", () => this.apply());
    this.shadowRoot.getElementById("clear")?.addEventListener("click", () => {
      if (this._queryStateId) {
        setQueryState(
          this._queryStateId,
          { filters: {} },
          { filterIntentSource: "filter_bar", transitionSource: "filter_bar" },
        );
      } else {
        this._filters = {};
        this.syncRowsFromFilters();
        this.render();
      }
    });
    bindMultiPanelInteractions(this, {
      additiveMode: false,
      schemaMode: true,
      live: this._props.live === true,
      onCheckboxChange: () => {
        if (this._props.live === true) {
          this.apply();
        }
      },
    });
    for (const input of this.shadowRoot.querySelectorAll("[data-schema-text], [data-schema-date-start], [data-schema-date-end]")) {
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
    scheduleFloatingPanelSync(this);
    bindTallValueRowInteractions(this);
  }

  collectFilters() {
    if (this._schemaMode) {
      return collectSchemaFilters(this.shadowRoot, this._schemaFields);
    }
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
    const catalog = this._schemaMode ? this._schemaFields : this._columnCatalog;
    for (const input of this.shadowRoot.querySelectorAll('input[type="text"][data-field-key]')) {
      const key = String(input.dataset.fieldKey || "").trim();
      const value = String(input.value || "").trim();
      if (!key || !value) continue;
      const field = findCatalogField(catalog, key);
      const stateKey = filterStateKey(field) || key;
      filters[stateKey] = value;
    }
    for (const [key, entry] of this._pendingClassicMulti.entries()) {
      const values = Array.from(entry?.values || []).filter(Boolean);
      if (!key || values.length === 0) continue;
      const field = findCatalogField(catalog, key);
      const stateKey = filterStateKey(field) || key;
      const prefix = entry?.control === "month_multi_select" ? "m:" : "in:";
      filters[stateKey] = `${prefix}${values.join(",")}`;
    }
    return filters;
  }

  apply() {
    if (this._additiveMode) {
      this.syncAdditiveRowsFromDom();
      this.applyActiveFilters();
      return;
    }
    this.syncClassicMultiFromDom();
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

function isSchemaMode(props) {
  return resolveFilterBarMode(props) === "schema";
}

function isAdditiveFilterMode(props) {
  const mode = resolveFilterBarMode(props);
  return mode === "additive" || mode === "builder";
}

function isAdditiveMode(props) {
  return isAdditiveFilterMode(props);
}

function resolveFilterBarMode(props) {
  const mode = String(props?.mode || props?.filter_mode || "").trim().toLowerCase();
  if (mode === "schema" || mode === "preset") return "schema";
  if (mode === "additive" || mode === "builder") return "additive";
  if (mode === "classic") return "classic";
  if (props?.schema_prepared === true || props?.schemaPrepared === true) return "schema";
  return "classic";
}

function resolveAllowExtra(props) {
  return props?.allow_extra === true || props?.allowExtra === true;
}

function filterMapsDiffer(left, right) {
  const a = left && typeof left === "object" && !Array.isArray(left) ? left : {};
  const b = right && typeof right === "object" && !Array.isArray(right) ? right : {};
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  for (const key of keys) {
    if (String(a[key] ?? "").trim() !== String(b[key] ?? "").trim()) return true;
  }
  return false;
}

function mergePresetsWithFilterRows(catalog, presetCount, filterRows, nextRowId) {
  const presets = buildPresetFilterRows(catalog, presetCount, nextRowId);
  const byColumn = new Map();
  for (const row of Array.isArray(filterRows) ? filterRows : []) {
    const column = String(row?.column || "").trim();
    if (!column) continue;
    byColumn.set(column, row);
  }
  const merged = presets.map((preset) => {
    const column = String(preset?.column || "").trim();
    const hit = column ? byColumn.get(column) : null;
    if (!hit) return preset;
    byColumn.delete(column);
    return {
      ...preset,
      operator: hit.operator || preset.operator,
      negate: hit.negate === true,
      value: hit.value ?? "",
      values: Array.isArray(hit.values) ? hit.values.slice() : [],
      rangeStart: hit.rangeStart || "",
      rangeEnd: hit.rangeEnd || "",
      status: "active",
    };
  });
  for (const row of byColumn.values()) {
    merged.push({ ...row, status: "active" });
  }
  return merged;
}

function resolvePresetFilterCount(props) {
  const raw = props?.preset_filter_count ?? props?.presetFilterCount ?? props?.default_preset_count;
  const parsed = Number(raw);
  if (Number.isFinite(parsed) && parsed >= 0) return Math.floor(parsed);
  return 0;
}

function buildPresetFilterRows(catalog, count, nextRowId) {
  const visible = (catalog || []).filter((field) => field?.visible !== false);
  const usedKeys = new Set();
  const rows = [];
  const limit = Math.max(0, Math.min(count, visible.length));
  for (const field of visible) {
    if (rows.length >= limit) break;
    const queryKey = fieldQueryKey(field);
    if (!queryKey || usedKeys.has(queryKey)) continue;
    usedKeys.add(queryKey);
    const row = createEmptyFilterRow(nextRowId);
    // 预置行绑定数据列名（如「风险事项」），避免只用逻辑 key（matter）导致查询维度错位。
    row.column = String(field?.column || queryKey).trim();
    row.operator = defaultOperatorForProfile(null, field);
    row.status = "active";
    rows.push(row);
  }
  return rows;
}

function createDraftFilterRowForColumn(nextRowId, column, catalog, profiles) {
  const key = String(column || "").trim();
  const row = createEmptyFilterRow(nextRowId, { status: "draft" });
  row.column = key;
  const field = findCatalogField(catalog, key);
  const profile = profileForColumn(key, profiles);
  row.operator = defaultOperatorForProfile(profile, field);
  return row;
}

function mergeAdditiveRowsFromFilters(fromFilters, previous, catalog, presetCount, nextRowId, userTouched = false) {
  const prev = Array.isArray(previous) ? previous : [];
  const fromState = Array.isArray(fromFilters) ? fromFilters : [];

  if (fromState.length === 0 && prev.length === 0) {
    if (!userTouched && presetCount > 0) {
      return buildPresetFilterRows(catalog, presetCount, nextRowId);
    }
    return [];
  }

  const stateByColumn = new Map();
  for (const row of fromState) {
    const column = String(row?.column || "").trim();
    if (!column) continue;
    stateByColumn.set(column, { ...row, status: "active" });
  }

  const merged = [];
  const seenColumns = new Set();

  for (const row of prev) {
    const column = String(row?.column || "").trim();
    if (isRowDraft(row)) {
      if (column && (stateByColumn.has(column) || seenColumns.has(column))) continue;
      merged.push({ ...row, status: "draft" });
      if (column) seenColumns.add(column);
      continue;
    }
    if (!isRowActive(row) || !column) continue;
    if (stateByColumn.has(column)) {
      merged.push({
        ...stateByColumn.get(column),
        id: row.id,
        status: "active",
      });
      stateByColumn.delete(column);
    } else {
      merged.push({ ...row, status: "active" });
    }
    seenColumns.add(column);
  }

  for (const row of stateByColumn.values()) {
    const column = String(row?.column || "").trim();
    if (!column || seenColumns.has(column)) continue;
    merged.push({ ...row, status: "active" });
    seenColumns.add(column);
  }

  return merged.length > 0
    ? merged
    : !userTouched && presetCount > 0
      ? buildPresetFilterRows(catalog, presetCount, nextRowId)
      : [];
}

function availableCatalogFieldsForAdd(catalog, rows) {
  const used = usedCatalogFieldKeys(rows);
  return visibleCatalogFields(catalog).filter((field) => {
    const key = fieldQueryKey(field);
    const column = String(field?.column || "").trim();
    if (!key) return false;
    if (used.has(key)) return false;
    if (column && used.has(column)) return false;
    return true;
  });
}

function visibleCatalogFields(catalog) {
  return (catalog || []).filter((field) => field?.visible !== false);
}

function usedCatalogFieldKeys(rows) {
  return new Set(
    (rows || []).map((row) => String(row?.column || "").trim()).filter(Boolean),
  );
}

function allCatalogFieldsUsed(catalog, rows) {
  const used = usedCatalogFieldKeys(rows);
  const visible = visibleCatalogFields(catalog);
  if (visible.length === 0) return true;
  return visible.every((field) => used.has(fieldQueryKey(field)));
}

function resolvePanelCollapsed(props) {
  return (
    props?.default_collapsed === true ||
    props?.defaultCollapsed === true ||
    props?.collapsed === true
  );
}

function filterStateKey(field) {
  // 物理列名优先：rowset/SQL 按列名匹配（办理状态、主责单位）。
  // 逻辑 key（agency / supervisionCategory）在无 dimension binding 的派生 dataset
  // （如 issue_handling_list）上会变成 unresolved，进而导致整次过滤失效、显示全量。
  // warning_list 等同时接受列名与逻辑 key 的 dataset 用列名也正确。
  const column = String(field?.column || "").trim();
  if (column) return column;
  return fieldQueryKey(field);
}

function findCatalogField(catalog, column) {
  const needle = String(column || "").trim();
  if (!needle) return null;
  return (
    (catalog || []).find((field) => {
      const queryKey = fieldQueryKey(field);
      const dataColumn = String(field?.column || "").trim();
      return queryKey === needle || dataColumn === needle || filterStateKey(field) === needle;
    }) || null
  );
}

function rowEncodedValue(row, profiles, catalog) {
  const column = String(row?.column || "").trim();
  if (!column) return "";
  const profile = profileForColumn(column, profiles);
  const field = findCatalogField(catalog, column);
  const operator = resolveRowOperator(row, profile, field);
  return encodeFilterRow({ ...row, operator }, profile);
}

function isRowDraft(row) {
  return String(row?.status || "draft") !== "active";
}

function isRowActive(row) {
  return String(row?.status || "") === "active";
}

function countAppliedCatalogFilters(filters, catalog) {
  const keys = catalogManagedFilterKeys(catalog);
  return Object.entries(filters || {}).filter(
    ([key, value]) => keys.has(key) && String(value ?? "").trim(),
  ).length;
}

function resolveColumnCatalog(props) {
  const raw = props?.column_catalog || props?.columnCatalog || props?.fields;
  if (!Array.isArray(raw)) return [];
  return raw
    .map((field) => {
      const queryKey = String(field?.key || field?.field || field?.column || "").trim();
      const column = String(field?.column || field?.key || field?.field || "").trim();
      if (!queryKey) return null;
      const control = normalizeControl(field);
      const staticOptions = Array.isArray(field?.options) ? field.options : [];
      const optionsFromRaw = String(field?.options_from || field?.optionsFrom || "")
        .trim()
        .toLowerCase();
      // 显式 static / 已声明 options 时保留，勿一律强制 rowset。
      let optionsFrom = optionsFromRaw;
      if (optionsFromRaw === "static" || (staticOptions.length > 0 && optionsFromRaw !== "rowset")) {
        optionsFrom = "static";
      } else if (
        control === "multi_select" ||
        control === "month_multi_select" ||
        optionsFromRaw === "rowset" ||
        !optionsFromRaw
      ) {
        optionsFrom = "rowset";
      }
      return {
        key: queryKey,
        label: String(field?.label || queryKey).trim() || queryKey,
        column,
        control,
        operator: String(field?.operator || field?.default_operator || field?.defaultOperator || "").trim(),
        placeholder: String(field?.placeholder || "").trim(),
        visible: field?.visible !== false,
        options_from: optionsFrom,
        options_field: String(field?.options_field || field?.column || column).trim(),
        options: staticOptions,
      };
    })
    .filter(Boolean);
}

/** Prefer declared static options over facet/rowset enums. */
function resolveSelectOptionsForField(fieldDef, fieldOptions, columnKey = "") {
  const staticOptions = Array.isArray(fieldDef?.options) ? fieldDef.options : [];
  const source = String(fieldDef?.options_from || fieldDef?.optionsFrom || "")
    .trim()
    .toLowerCase();
  if (source === "static" || (staticOptions.length > 0 && source !== "rowset")) {
    return staticOptions;
  }
  // fieldOptions 以 query key（如 warningLevel）为主键；row.column / options_field 也可能是中文列名
  const queryKey = fieldQueryKey(fieldDef);
  const passed = String(columnKey || "").trim();
  const column = String(fieldDef?.column || fieldDef?.options_field || fieldDef?.optionsField || "").trim();
  const candidates = [queryKey, passed, column].filter(Boolean);
  const seen = new Set();
  for (const candidate of candidates) {
    if (seen.has(candidate)) continue;
    seen.add(candidate);
    const found = fieldOptions?.get(candidate) || [];
    if (found.length > 0) return found;
  }
  return staticOptions;
}

/**
 * contains_any 存的是「蓝」等针值；选项是「蓝/黄」等组合面值。
 * 展示/勾选时把针值展开为所有包含该针值的组合项，以便与明细过滤结果对齐。
 */
function expandContainsAnySelection(needles, options) {
  const needleList = (needles || [])
    .map((item) => String(item ?? "").trim())
    .filter(Boolean);
  if (!needleList.length) return [];
  const expanded = [];
  const seen = new Set();
  for (const option of options || []) {
    const value =
      typeof option === "string"
        ? option
        : option?.value ?? option?.id ?? option?.label ?? "";
    const text = String(value ?? "").trim();
    if (!text || seen.has(text)) continue;
    if (needleList.some((needle) => text.includes(needle))) {
      seen.add(text);
      expanded.push(text);
    }
  }
  return expanded;
}

/** DOM 勾选的是展开后的组合面值时，把 contains_any 收成精确 in。 */
function coerceContainsAnyDomSelectionToIn(nextRow, previousRow) {
  const operator = String(nextRow?.operator || "").trim().toLowerCase();
  if (operator !== "contains_any") return nextRow;
  const values = Array.isArray(nextRow?.values)
    ? nextRow.values.map((item) => String(item ?? "").trim()).filter(Boolean)
    : [];
  if (!values.length) return nextRow;
  const prevWasContainsAny =
    String(previousRow?.operator || "").trim().toLowerCase() === "contains_any";
  // 仍是纯针值（红/黄/蓝）→ 保持 contains_any
  const membershipTokens = new Set(["红", "黄", "蓝"]);
  const allMembershipNeedles = values.every((value) => membershipTokens.has(value));
  if (allMembershipNeedles) return nextRow;
  // 勾选结果已是组合面值（或混有组合）→ 改为 in，与面板选项语义一致
  if (prevWasContainsAny || values.some((value) => value.includes("/") || !membershipTokens.has(value))) {
    return { ...nextRow, operator: "in", values };
  }
  return nextRow;
}

function valueIsSelected(selectedValues, optionValue, { operator = "" } = {}) {
  const target = String(optionValue ?? "");
  const selected = selectedValues || [];
  const op = String(operator || "").trim().toLowerCase();
  if (op === "contains_any") {
    return selected.some((needle) => {
      const token = String(needle ?? "").trim();
      return Boolean(token) && target.includes(token);
    });
  }
  return selected.some((item) => String(item ?? "") === target);
}

function resolveMultiOptions(options, selectedValues) {
  const items = [];
  const seen = new Set();
  for (const option of options || []) {
    const optionValue =
      typeof option === "string" ? option : option?.value ?? option?.id ?? "";
    const text = String(optionValue).trim();
    if (!text || seen.has(text)) continue;
    seen.add(text);
    const countRaw =
      typeof option === "object" && option
        ? option.count ?? option.n ?? option.total
        : null;
    const count = Number(countRaw);
    const hasCount = Number.isFinite(count) && count > 0;
    const baseLabel =
      typeof option === "string" ? option : option?.label || text;
    items.push({
      value: text,
      label: hasCount ? `${baseLabel}（${formatFacetCount(count)}）` : baseLabel,
      count: hasCount ? count : undefined,
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

function formatFacetCount(count) {
  const n = Math.floor(Number(count) || 0);
  if (n >= 10000) {
    const wan = n / 10000;
    return `${wan >= 10 ? Math.round(wan) : wan.toFixed(1).replace(/\.0$/, "")}万`;
  }
  return String(n);
}

function normalizeFacetOptions(rawOptions) {
  if (!Array.isArray(rawOptions)) return [];
  return rawOptions
    .map((item) => {
      if (typeof item === "string") {
        const value = String(item || "").trim();
        return value ? { value, count: 0 } : null;
      }
      if (!item || typeof item !== "object") return null;
      const value = String(item.value ?? item.id ?? item.label ?? "").trim();
      if (!value) return null;
      const count = Number(item.count ?? item.n ?? 0);
      return {
        value,
        count: Number.isFinite(count) && count > 0 ? count : 0,
      };
    })
    .filter(Boolean);
}

function profileForColumn(column, profiles) {
  return profiles?.get(column) || null;
}

function resolveRowOperator(row, profile, fieldHint = null) {
  const requested = String(row?.operator || "").trim();
  const allowed = new Set(operatorsForField(profile, fieldHint).map((id) => id));
  if (requested && allowed.has(requested)) return requested;
  return defaultOperatorForProfile(profile, fieldHint);
}

function normalizeAdditiveRow(row, catalog, profiles) {
  const column = String(row?.column || "").trim();
  const field = column ? findCatalogField(catalog, column) : null;
  const profile = column ? profileForColumn(column, profiles) : null;
  const status = isRowActive(row) ? "active" : "draft";
  const next = { ...row, status };
  if (!column || status !== "draft") return next;
  return { ...next, operator: resolveRowOperator(next, profile, field) };
}

function readSingleAdditiveRowFromDom(rowEl, previous = null) {
  if (!rowEl) {
    return {
      id: "",
      column: "",
      operator: "",
      negate: false,
      value: "",
      values: [],
      rangeStart: "",
      rangeEnd: "",
      status: "draft",
    };
  }
  const id = String(rowEl.dataset.additiveRow || previous?.id || "").trim();
  const column = String(
    rowEl.dataset.rowColumn || rowEl.querySelector("select[data-row-column]")?.value || previous?.column || "",
  ).trim();
  const operatorSelect = rowEl.querySelector("select[data-row-operator]");
  const operator = operatorSelect
    ? String(operatorSelect.value || "").trim()
    : String(previous?.operator || "").trim();
  const negate = Boolean(rowEl.querySelector("input[data-row-negate]")?.checked);
  const valueInput = rowEl.querySelector("[data-row-value]");
  const value = valueInput ? String(valueInput.value || "").trim() : "";
  const values = [];
  let checkboxes = rowEl.querySelectorAll('.multi-option input[type="checkbox"]');
  if (checkboxes.length === 0 && id) {
    const floated = findFloatingMultiPanel(id);
    if (floated) {
      checkboxes = floated.querySelectorAll('.multi-option input[type="checkbox"]');
    }
  }
  if (checkboxes.length > 0) {
    for (const checkbox of checkboxes) {
      if (!checkbox.checked) continue;
      const item = String(checkbox.value || "").trim();
      if (item) values.push(item);
    }
    // 未编辑本行时不要把已有多选值误清空：勾选态可能因浮层/重绘暂时全未勾选。
    if (
      values.length === 0 &&
      Array.isArray(previous?.values) &&
      previous.values.length > 0
    ) {
      const panelInRow = rowEl.querySelector(".multi-panel.is-open");
      const floated = id ? findFloatingMultiPanel(id) : null;
      const panelOpen = Boolean(
        panelInRow || (floated && floated.classList.contains("is-open")),
      );
      if (!panelOpen) {
        values.push(...previous.values.filter(Boolean));
      }
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
  return {
    id,
    column,
    operator: operator || previous?.operator || "",
    negate: previous?.negate ? true : negate,
    value,
    values,
    rangeStart,
    rangeEnd,
    status: previous ? String(previous.status || "draft") : "draft",
  };
}

function additiveDraftValidationError(row, profile, field) {
  const column = String(row?.column || "").trim();
  if (!column) return "请先选择字段";
  const operator = resolveRowOperator(row, profile, field);
  if (operator === "month_range" || operator === "date_range") {
    const start = String(row?.rangeStart || "").trim();
    const end = String(row?.rangeEnd || "").trim();
    // Open-ended ranges are allowed (only start = ≥, only end = ≤).
    if (!start && !end) return "请至少填写起始或结束日期";
  }
  return "";
}

function readAdditiveRowsFromDom(shadowRoot, previousRows = []) {
  const rows = [];
  for (const rowEl of shadowRoot.querySelectorAll("[data-additive-row]")) {
    const id = String(rowEl.dataset.additiveRow || "").trim();
    if (!id) continue;
    const previous = (previousRows || []).find((entry) => entry.id === id) || null;
    rows.push(readSingleAdditiveRowFromDom(rowEl, previous));
  }
  return rows;
}

const FLOATING_PANEL_MIN_WIDTH = 300;
const FLOATING_PANEL_MAX_WIDTH = 520;
const FLOATING_PANEL_MAX_HEIGHT = 360;
const FLOATING_PANEL_VIEWPORT_PADDING = 10;
const ADD_FIELD_PICKER_KEY = "__add_field__";
const FILTER_FLOATING_STYLE_ID = "mei-filter-bar-floating-styles";

function cssEscapeAttr(value) {
  const text = String(value || "");
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(text);
  }
  return text.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function ensureFilterFloatingStyles() {
  if (typeof document === "undefined") return;
  if (document.getElementById(FILTER_FLOATING_STYLE_ID)) return;
  const style = document.createElement("style");
  style.id = FILTER_FLOATING_STYLE_ID;
  style.textContent = `
    [data-mei-filter-floating="1"].multi-panel,
    [data-mei-filter-floating="1"].field-picker-panel {
      box-sizing: border-box;
      display: none;
      border-radius: 8px;
      border: 1px solid var(--mei-color-filter-panel-border, rgba(56, 160, 240, 0.28));
      background: var(--mei-color-drilldown-panel-bottom, rgba(8, 28, 58, 0.98));
      box-shadow: 0 20px 48px rgba(2, 6, 23, 0.58);
      padding: 6px;
      color: var(--mei-color-text-body, #e2e8f0);
      font-size: ${FILTER_PANEL_FONT};
      z-index: var(--mei-z-cockpit-text-popover, 2350);
    }
    [data-mei-filter-floating="1"].multi-panel.is-open,
    [data-mei-filter-floating="1"].field-picker-panel.is-open {
      display: block;
    }
    [data-mei-filter-floating="1"] .multi-search,
    [data-mei-filter-floating="1"] .field-picker-search {
      width: 100%;
      margin-bottom: 6px;
      position: sticky;
      top: 0;
      z-index: 1;
      box-sizing: border-box;
      min-height: 32px;
      border-radius: 8px;
      border: 1px solid var(--mei-color-drilldown-tab-border, rgba(56, 160, 240, 0.28));
      background: var(--mei-color-drilldown-tab-bg, rgba(10, 36, 68, 0.92));
      color: var(--mei-color-text-body, #e2e8f0);
      font-size: ${FILTER_PANEL_FONT};
      padding: 6px 9px;
    }
    [data-mei-filter-floating="1"] .multi-options,
    [data-mei-filter-floating="1"] .field-picker-options {
      display: flex;
      flex-direction: column;
      gap: 2px;
      min-height: 0;
    }
    [data-mei-filter-floating="1"].multi-panel,
    [data-mei-filter-floating="1"].field-picker-panel {
      overscroll-behavior: contain;
    }
    [data-mei-filter-floating="1"] .multi-option,
    [data-mei-filter-floating="1"] .field-picker-option {
      appearance: none;
      -webkit-appearance: none;
      display: flex;
      align-items: center;
      gap: 8px;
      width: 100%;
      box-sizing: border-box;
      padding: 6px 8px;
      border-radius: 6px;
      border: 1px solid var(--mei-color-drilldown-tab-border, rgba(56, 160, 240, 0.28));
      background: var(--mei-color-drilldown-tab-bg, rgba(10, 36, 68, 0.92));
      cursor: pointer;
      font-size: ${FILTER_PANEL_FONT};
      color: var(--mei-color-text-body, #e2e8f0);
      text-align: left;
      line-height: 1.4;
      white-space: normal;
      word-break: break-word;
    }
    [data-mei-filter-floating="1"] .multi-option[hidden],
    [data-mei-filter-floating="1"] .field-picker-option[hidden] {
      display: none !important;
    }
    [data-mei-filter-floating="1"] .multi-option:hover,
    [data-mei-filter-floating="1"] .field-picker-option:hover {
      background: var(--mei-color-table-row-hover, rgba(56, 160, 240, 0.18));
      border-color: var(--mei-color-table-btn-hover-border, rgba(113, 241, 234, 0.55));
      color: var(--mei-color-text-inverse, #ffffff);
    }
    [data-mei-filter-floating="1"] .multi-option input {
      margin: 0;
      flex-shrink: 0;
    }
    [data-mei-filter-floating="1"] .multi-filter-empty,
    [data-mei-filter-floating="1"] .field-picker-filter-empty {
      padding: 8px;
      color: var(--mei-color-text-muted, #94a3b8);
      font-size: calc(${FILTER_PANEL_FONT} * 0.9);
      text-align: center;
    }
    [data-mei-filter-floating="1"] .multi-filter-empty[hidden],
    [data-mei-filter-floating="1"] .field-picker-filter-empty[hidden] {
      display: none !important;
    }
  `;
  document.head.appendChild(style);
}

function cleanupHostFloatingPanels(host) {
  const hostId = String(host?._filterFloatingHostId || "").trim();
  if (!hostId || typeof document === "undefined") return;
  for (const panel of document.querySelectorAll(`[data-mei-filter-floating-host="${cssEscapeAttr(hostId)}"]`)) {
    panel.remove();
  }
}

function findFloatingMultiPanel(panelKey) {
  const key = String(panelKey || "").trim();
  if (!key || typeof document === "undefined") return null;
  return document.querySelector(
    `[data-mei-filter-floating="1"][data-multi-panel="${cssEscapeAttr(key)}"]`,
  );
}

function clearFloatingPanel(panel) {
  if (!panel) return;
  panel.classList.remove("is-floating");
  panel.removeAttribute("data-mei-filter-floating");
  panel.removeAttribute("data-mei-filter-floating-host");
  panel.style.removeProperty("position");
  panel.style.removeProperty("left");
  panel.style.removeProperty("right");
  panel.style.removeProperty("top");
  panel.style.removeProperty("bottom");
  panel.style.removeProperty("width");
  panel.style.removeProperty("min-width");
  panel.style.removeProperty("max-width");
  panel.style.removeProperty("max-height");
  panel.style.removeProperty("z-index");
  panel.style.removeProperty("overflow");
}

function resolveFloatingPanelWidth(panel, triggerWidth) {
  const maxWidth = Math.min(
    FLOATING_PANEL_MAX_WIDTH,
    window.innerWidth - FLOATING_PANEL_VIEWPORT_PADDING * 2,
  );
  const minWidth = Math.min(maxWidth, Math.max(FLOATING_PANEL_MIN_WIDTH, triggerWidth));
  const previousDisplay = panel.style.display;
  const previousVisibility = panel.style.visibility;
  const previousPosition = panel.style.position;
  const previousLeft = panel.style.left;
  const previousWidth = panel.style.width;
  const previousMaxWidth = panel.style.maxWidth;
  panel.style.visibility = "hidden";
  panel.style.display = "block";
  panel.style.position = "fixed";
  panel.style.left = "-10000px";
  panel.style.width = "max-content";
  panel.style.maxWidth = `${maxWidth}px`;
  const measured = Math.ceil(panel.getBoundingClientRect().width);
  panel.style.visibility = previousVisibility;
  panel.style.display = previousDisplay;
  panel.style.position = previousPosition;
  panel.style.left = previousLeft;
  panel.style.width = previousWidth;
  panel.style.maxWidth = previousMaxWidth;
  return Math.min(maxWidth, Math.max(minWidth, measured));
}

function copyThemeCssVarsOnto(panel, host) {
  if (!(panel instanceof HTMLElement) || typeof document === "undefined") return;
  // Tokens are set as inline --mei-* on compose root; layer2 inherits but has no own declarations.
  const themeRoot =
    document.getElementById("mei-compose-root") ||
    (host instanceof Element && host.closest?.("#mei-compose-root")) ||
    document.getElementById("mei-layer2-workspace");
  if (!(themeRoot instanceof HTMLElement)) return;
  for (const name of themeRoot.style) {
    if (!String(name).startsWith("--mei-")) continue;
    const value = themeRoot.style.getPropertyValue(name);
    if (value) panel.style.setProperty(name, value);
  }
}

function positionFloatingPanel(trigger, panel, options = {}) {
  const { preferDropUp = false } = options;
  const triggerRect = trigger.getBoundingClientRect();
  if (triggerRect.width <= 0 && triggerRect.height <= 0) return;

  const width = resolveFloatingPanelWidth(panel, triggerRect.width);
  const maxHeight = Math.min(
    FLOATING_PANEL_MAX_HEIGHT,
    window.innerHeight - FLOATING_PANEL_VIEWPORT_PADDING * 2,
  );

  let left = triggerRect.left;
  if (left + width > window.innerWidth - FLOATING_PANEL_VIEWPORT_PADDING) {
    left = Math.max(
      FLOATING_PANEL_VIEWPORT_PADDING,
      window.innerWidth - width - FLOATING_PANEL_VIEWPORT_PADDING,
    );
  }
  left = Math.max(FLOATING_PANEL_VIEWPORT_PADDING, left);

  ensureFilterFloatingStyles();
  const host = trigger.getRootNode()?.host;
  const hostId = String(host?._filterFloatingHostId || "").trim();
  panel.classList.add("is-floating");
  panel.setAttribute("data-mei-filter-floating", "1");
  if (hostId) panel.setAttribute("data-mei-filter-floating-host", hostId);
  panel.style.position = "fixed";
  panel.style.left = `${left}px`;
  panel.style.right = "auto";
  panel.style.width = `${width}px`;
  panel.style.minWidth = `${width}px`;
  panel.style.maxWidth = `${Math.min(FLOATING_PANEL_MAX_WIDTH, window.innerWidth - FLOATING_PANEL_VIEWPORT_PADDING * 2)}px`;
  panel.style.maxHeight = `${maxHeight}px`;
  panel.style.removeProperty("z-index");
  panel.setAttribute("data-mei-overlay-role", "text_popover");
  const boot = window.__meiLangBoot || {};
  if (typeof boot.mountRuntimeOverlay === "function") {
    boot.mountRuntimeOverlay(panel, { role: "text_popover", anchor: trigger });
  }
  copyThemeCssVarsOnto(panel, host instanceof Element ? host : null);
  panel.style.overflow = "auto";

  const spaceBelow = window.innerHeight - triggerRect.bottom - FLOATING_PANEL_VIEWPORT_PADDING;
  const spaceAbove = triggerRect.top - FLOATING_PANEL_VIEWPORT_PADDING;
  const panelHeight = Math.min(maxHeight, panel.scrollHeight || maxHeight);
  const dropUp = preferDropUp || (spaceBelow < panelHeight && spaceAbove > spaceBelow);

  if (dropUp) {
    panel.style.top = "auto";
    panel.style.bottom = `${window.innerHeight - triggerRect.top + 4}px`;
  } else {
    panel.style.top = `${triggerRect.bottom + 4}px`;
    panel.style.bottom = "auto";
  }
}

function hostFloatingPanels(host) {
  const hostId = String(host?._filterFloatingHostId || "").trim();
  const fromShadow = host?.shadowRoot
    ? Array.from(host.shadowRoot.querySelectorAll(".is-floating"))
    : [];
  if (!hostId || typeof document === "undefined") return fromShadow;
  const fromBody = Array.from(
    document.querySelectorAll(`[data-mei-filter-floating-host="${cssEscapeAttr(hostId)}"]`),
  );
  const seen = new Set(fromShadow);
  for (const panel of fromBody) {
    if (!seen.has(panel)) fromShadow.push(panel);
  }
  return fromShadow;
}

function syncFloatingPanels(host) {
  if (!host?.shadowRoot) return;
  for (const panel of hostFloatingPanels(host)) {
    if (!panel.classList.contains("is-open")) {
      clearFloatingPanel(panel);
      if (panel.hasAttribute("data-mei-filter-floating-host")) {
        panel.remove();
      }
    }
  }
  if (host._openDropdownKey) {
    const key = host._openDropdownKey;
    const trigger = host.shadowRoot.querySelector(`[data-multi-trigger="${cssEscapeAttr(key)}"]`);
    const panel =
      host.shadowRoot.querySelector(`[data-multi-panel="${cssEscapeAttr(key)}"]`) ||
      findFloatingMultiPanel(key);
    if (trigger && panel?.classList.contains("is-open")) {
      positionFloatingPanel(trigger, panel);
    }
  }
  if (host._openFieldPickerKey) {
    const key = host._openFieldPickerKey;
    const trigger = host.shadowRoot.querySelector(`[data-field-picker-trigger="${cssEscapeAttr(key)}"]`);
    const panel =
      host.shadowRoot.querySelector(`[data-field-picker-panel="${cssEscapeAttr(key)}"]`) ||
      document.querySelector(
        `[data-mei-filter-floating="1"][data-field-picker-panel="${cssEscapeAttr(key)}"]`,
      );
    if (trigger && panel?.classList.contains("is-open")) {
      positionFloatingPanel(trigger, panel, {
        preferDropUp: key === ADD_FIELD_PICKER_KEY,
      });
    }
  }
}

function ensureFloatingPanelListeners(host) {
  if (host._floatingPanelListenersBound) return;
  host._floatingPanelListenersBound = true;
  host._floatingPanelSyncHandler = (event) => {
    // 面板自身滚动不要触发 reposition（capture 阶段会收到），否则滚轮像被“锁死”。
    if (event?.type === "scroll") {
      const target = event.target;
      if (
        target instanceof Element &&
        (target.closest?.('[data-mei-filter-floating="1"]') ||
          target.classList?.contains("multi-panel") ||
          target.classList?.contains("field-picker-panel") ||
          target.classList?.contains("multi-options") ||
          target.classList?.contains("field-picker-options"))
      ) {
        return;
      }
    }
    scheduleFloatingPanelSync(host);
  };
  window.addEventListener("resize", host._floatingPanelSyncHandler);
  window.addEventListener("scroll", host._floatingPanelSyncHandler, true);
}

function teardownFloatingPanelListeners(host) {
  if (!host._floatingPanelListenersBound) return;
  window.removeEventListener("resize", host._floatingPanelSyncHandler);
  window.removeEventListener("scroll", host._floatingPanelSyncHandler, true);
  host._floatingPanelListenersBound = false;
  host._floatingPanelSyncHandler = null;
}

function scheduleFloatingPanelSync(host) {
  if (!host?.shadowRoot) return;
  if (!host._openDropdownKey && !host._openFieldPickerKey) {
    for (const panel of hostFloatingPanels(host)) {
      clearFloatingPanel(panel);
      if (panel.parentElement === document.body || panel.hasAttribute("data-mei-filter-floating-host")) {
        panel.remove();
      }
    }
    return;
  }
  ensureFloatingPanelListeners(host);
  requestAnimationFrame(() => {
    requestAnimationFrame(() => syncFloatingPanels(host));
  });
}

function isTallValueOperator(operator) {
  const normalized = String(operator || "").trim();
  return normalized === "date_range" || normalized === "month_range";
}

function tallValueRowClass(operator) {
  return isTallValueOperator(operator) ? " is-tall-value" : "";
}

function bindTallValueRowInteractions(host) {
  if (!host?.shadowRoot) return;
  for (const input of host.shadowRoot.querySelectorAll(
    "[data-row-range-start], [data-row-range-end], [data-schema-date-start], [data-schema-date-end]",
  )) {
    input.addEventListener("focus", () => {
      const row = input.closest("[data-additive-row], .schema-field");
      if (!row) return;
      row.classList.add("is-value-focused");
      const scrollHost = row.closest(".additive-rows, .schema-fields, .filter-panel-body");
      if (scrollHost && typeof row.scrollIntoView === "function") {
        row.scrollIntoView({ block: "nearest", behavior: "smooth" });
      }
    });
    input.addEventListener("blur", () => {
      requestAnimationFrame(() => {
        const row = input.closest("[data-additive-row], .schema-field");
        if (!row) return;
        const active = host.shadowRoot.activeElement;
        if (active && row.contains(active)) return;
        row.classList.remove("is-value-focused");
      });
    });
  }
}

function resolveMultiPanelKey(checkbox) {
  const panel = checkbox.closest("[data-multi-panel]");
  if (panel) {
    return String(panel.dataset.multiPanel || "").trim();
  }
  const rowEl = checkbox.closest("[data-additive-row]");
  if (rowEl) {
    return String(rowEl.dataset.additiveRow || "").trim();
  }
  return String(checkbox.dataset.fieldKey || "").trim();
}

function applyMultiPanelSearchFilter(searchInput) {
  const panel = searchInput.closest("[data-multi-panel]");
  if (!panel) return;
  const query = String(searchInput.value || "").trim().toLowerCase();
  let visibleCount = 0;
  for (const option of panel.querySelectorAll(".multi-option")) {
    const text = String(option.textContent || "").trim().toLowerCase();
    const show = !query || text.includes(query);
    option.hidden = !show;
    if (show) visibleCount += 1;
  }
  const empty = panel.querySelector(".multi-filter-empty");
  if (empty) {
    empty.hidden = visibleCount > 0;
  }
}

function bindMultiPanelInteractions(host, options = {}) {
  const { additiveMode = false, schemaMode = false, onCheckboxChange = null } = options;

  for (const trigger of host.shadowRoot.querySelectorAll("[data-multi-trigger]")) {
    trigger.addEventListener("click", (event) => {
      event.stopPropagation();
      if (additiveMode) {
        host.syncAdditiveRowsFromDom();
      } else if (!schemaMode) {
        host.syncClassicMultiFromDom();
      }
      const key = String(trigger.dataset.multiTrigger || "").trim();
      const nextKey = host._openDropdownKey === key ? "" : key;
      if (!nextKey && key) host._multiPanelSearch.delete(key);
      host._openDropdownKey = nextKey;
      host.render();
    });
  }

  for (const checkbox of host.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
    checkbox.addEventListener("click", (event) => {
      // 阻止冒泡到 document 的 outside-click，避免选中后立刻关面板。
      event.stopPropagation();
    });
    checkbox.addEventListener("change", (event) => {
      event.stopPropagation();
      const panelKey = resolveMultiPanelKey(checkbox);
      if (additiveMode) {
        host.syncAdditiveRowsFromDom();
      } else if (!schemaMode) {
        host.syncClassicMultiFromDom();
      }
      // 先钉住打开态，再回调 live apply（可能同步触发 query_state → render）。
      if (panelKey) {
        host._openDropdownKey = panelKey;
      }
      if (typeof onCheckboxChange === "function") {
        onCheckboxChange();
      }
      // 多选：保持下拉打开，仅点面板外才关闭。
      if (panelKey) {
        host._openDropdownKey = panelKey;
      }
      host.render();
    });
  }
  for (const option of host.shadowRoot.querySelectorAll(".multi-option")) {
    option.addEventListener("click", (event) => {
      event.stopPropagation();
    });
  }

  for (const input of host.shadowRoot.querySelectorAll("[data-multi-search]")) {
    input.addEventListener("click", (event) => event.stopPropagation());
    input.addEventListener("keydown", (event) => event.stopPropagation());
    input.addEventListener("input", (event) => {
      event.stopPropagation();
      const panelKey = String(input.dataset.multiSearch || "").trim();
      if (panelKey) {
        host._multiPanelSearch.set(panelKey, String(input.value || ""));
      }
      applyMultiPanelSearchFilter(input);
      scheduleFloatingPanelSync(host);
    });
    if (String(input.value || "").trim()) {
      applyMultiPanelSearchFilter(input);
    }
  }
  scheduleFloatingPanelSync(host);
}

function rowFieldPickerKey(rowId) {
  return `__pick_row__${String(rowId || "").trim()}`;
}

function catalogFieldsToPickerOptions(catalog, usedKeys = new Set()) {
  return (catalog || [])
    .filter((entry) => entry?.visible !== false)
    .map((entry) => {
      const key = fieldQueryKey(entry);
      if (!key || usedKeys.has(key)) return null;
      return { key, label: entry?.label || key };
    })
    .filter(Boolean);
}

function applyFieldPickerSearchFilter(searchInput) {
  const panel = searchInput.closest("[data-field-picker-panel]");
  if (!panel) return;
  const query = String(searchInput.value || "").trim().toLowerCase();
  let visibleCount = 0;
  for (const option of panel.querySelectorAll(".field-picker-option")) {
    const text = String(option.textContent || "").trim().toLowerCase();
    const show = !query || text.includes(query);
    option.hidden = !show;
    if (show) visibleCount += 1;
  }
  const empty = panel.querySelector(".field-picker-filter-empty");
  if (empty) {
    empty.hidden = visibleCount > 0;
  }
}

function bindFieldPickerInteractions(host) {
  for (const trigger of host.shadowRoot.querySelectorAll("[data-field-picker-trigger]")) {
    trigger.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (trigger.disabled) return;
      const pickerKey = String(trigger.dataset.fieldPickerTrigger || "").trim();
      if (!pickerKey) return;
      const nextKey = host._openFieldPickerKey === pickerKey ? "" : pickerKey;
      if (!nextKey) host._fieldPickerSearch.delete(pickerKey);
      host._openFieldPickerKey = nextKey;
      host.render();
    });
  }
  for (const input of host.shadowRoot.querySelectorAll("[data-field-picker-search]")) {
    input.addEventListener("click", (event) => event.stopPropagation());
    input.addEventListener("keydown", (event) => event.stopPropagation());
    input.addEventListener("input", (event) => {
      event.stopPropagation();
      const pickerKey = String(input.dataset.fieldPickerSearch || "").trim();
      if (pickerKey) {
        host._fieldPickerSearch.set(pickerKey, String(input.value || ""));
      }
      applyFieldPickerSearchFilter(input);
      scheduleFloatingPanelSync(host);
    });
    if (String(input.value || "").trim()) {
      applyFieldPickerSearchFilter(input);
    }
  }
  scheduleFloatingPanelSync(host);
}

function renderFieldPickerDropdown({
  pickerKey,
  fields,
  openPickerKey,
  disabled = false,
  triggerLabel = "+ 选择字段添加…",
  searchValue = "",
  optionPickAttr = "data-add-field",
  optionExtraAttrs = "",
  panelPlacement = "down",
}) {
  const isOpen = openPickerKey === pickerKey;
  const dropupClass = panelPlacement === "up" ? " is-dropup" : "";
  const items = Array.isArray(fields) ? fields : [];
  const showSearch = items.length >= 6;
  const searchMarkup = showSearch
    ? `<input
        type="search"
        class="field-picker-search cockpit-filter-control"
        data-field-picker-search="${escapeHtmlAttr(pickerKey)}"
        placeholder="搜索字段…"
        value="${escapeHtmlAttr(searchValue)}"
        autocomplete="off"
      />`
    : "";
  const optionMarkup = items
    .map((field) => {
      const key = String(field?.key || "").trim();
      const label = String(field?.label || key).trim();
      if (!key) return "";
      return `<button type="button" class="field-picker-option" ${optionPickAttr}="${escapeHtmlAttr(key)}" ${optionExtraAttrs}>${escapeHtml(label)}</button>`;
    })
    .filter(Boolean)
    .join("");
  const countHint =
    items.length > 0
      ? `<div class="field-picker-meta">还有 ${items.length} 个字段可用于过滤数据</div>`
      : "";
  const emptyMarkup =
    items.length === 0
      ? `<div class="field-picker-empty">暂无可选字段</div>`
      : `<div class="field-picker-filter-empty" hidden>无匹配字段</div>`;
  const bodyMarkup =
    panelPlacement === "up"
      ? `${countHint}<div class="field-picker-options">${optionMarkup}${emptyMarkup}</div>${searchMarkup}`
      : `${countHint}${searchMarkup}<div class="field-picker-options">${optionMarkup}${emptyMarkup}</div>`;
  return `
    <div class="field-picker">
      <button
        type="button"
        class="field-picker-trigger cockpit-filter-control ${isOpen ? "is-open" : ""}"
        data-field-picker-trigger="${escapeHtmlAttr(pickerKey)}"
        ${disabled ? "disabled" : ""}
        aria-haspopup="listbox"
        aria-expanded="${isOpen ? "true" : "false"}"
      >
        ${escapeHtml(triggerLabel)}
      </button>
      <div class="field-picker-panel${dropupClass} ${isOpen ? "is-open" : ""}" data-field-picker-panel="${escapeHtmlAttr(pickerKey)}" role="listbox">
        ${bodyMarkup}
      </div>
    </div>`;
}

function renderAddableFieldPicker(addableFields, disabled = false, openPickerKey = "", fieldPickerSearch = null) {
  const fields = (addableFields || [])
    .map((field) => {
      const key = fieldQueryKey(field);
      if (!key) return null;
      return { key, label: field?.label || key };
    })
    .filter(Boolean);
  return renderFieldPickerDropdown({
    pickerKey: ADD_FIELD_PICKER_KEY,
    fields,
    openPickerKey,
    disabled,
    triggerLabel: "+ 选择字段添加…",
    searchValue: fieldPickerSearch?.get?.(ADD_FIELD_PICKER_KEY) || "",
    optionPickAttr: "data-add-field",
    panelPlacement: "up",
  });
}

function renderDraftFieldPicker(catalog, usedColumnKeys, rowId, openPickerKey = "", fieldPickerSearch = null) {
  const pickerKey = rowFieldPickerKey(rowId);
  const fields = catalogFieldsToPickerOptions(catalog, usedColumnKeys);
  return renderFieldPickerDropdown({
    pickerKey,
    fields,
    openPickerKey,
    triggerLabel: "选择字段",
    searchValue: fieldPickerSearch?.get?.(pickerKey) || "",
    optionPickAttr: "data-field-key",
    optionExtraAttrs: `data-pick-field-row="${escapeHtmlAttr(rowId)}"`,
  });
}

function renderMultiSelectPanelMarkup({
  panelKey,
  selectedValues,
  options,
  control,
  isOpen,
  searchValue = "",
  checkboxExtraAttrs = "",
  triggerClass = "cockpit-filter-control",
  wrapperClass = "row-value-multi",
}) {
  const operator = String(control || "").trim().toLowerCase();
  // contains_any：勾选态按「组合面值是否包含针值」展开，摘要同步显示组合项
  const displaySelected =
    operator === "contains_any"
      ? expandContainsAnySelection(selectedValues, options)
      : selectedValues;
  const mergedOptions = resolveMultiOptions(options, displaySelected);
  const optionMarkup =
    mergedOptions.length > 0
      ? mergedOptions
          .map((option) => {
            const optionValue = option.value;
            const optionLabel = option.label;
            const checked = valueIsSelected(selectedValues, optionValue, { operator })
              ? "checked"
              : "";
            return `
              <label class="multi-option">
                <input type="checkbox" value="${escapeHtmlAttr(optionValue)}" ${checkboxExtraAttrs} ${checked} />
                <span>${escapeHtml(optionLabel)}</span>
              </label>
            `;
          })
          .join("")
      : "";
  const searchMarkup = `
    <input
      type="search"
      class="multi-search cockpit-filter-control"
      data-multi-search="${escapeHtmlAttr(panelKey)}"
      placeholder="搜索选项…"
      value="${escapeHtmlAttr(searchValue)}"
      autocomplete="off"
    />`;
  const emptyMarkup =
    mergedOptions.length === 0
      ? `<div class="multi-empty">暂无可选项</div>`
      : `<div class="multi-filter-empty" hidden>无匹配项</div>`;
  return `
    <div class="${wrapperClass}">
      <button type="button" class="multi-trigger ${triggerClass} ${isOpen ? "is-open" : ""}" data-multi-trigger="${escapeHtmlAttr(panelKey)}">
        ${escapeHtml(multiSelectSummary(displaySelected, control === "month_in" ? "month_multi_select" : "multi_select"))}
      </button>
      <div class="multi-panel ${isOpen ? "is-open" : ""}" data-multi-panel="${escapeHtmlAttr(panelKey)}">
        ${searchMarkup}
        <div class="multi-options">${optionMarkup}${emptyMarkup}</div>
      </div>
    </div>`;
}

function catalogManagedFilterKeys(catalog) {
  const keys = new Set();
  for (const field of catalog || []) {
    const queryKey = fieldQueryKey(field);
    const stateKey = filterStateKey(field);
    const column = String(field?.column || "").trim();
    // 列名与逻辑 key 都算受管，以便清掉历史 agency 等残留。
    if (queryKey) keys.add(queryKey);
    if (stateKey) keys.add(stateKey);
    if (column) keys.add(column);
  }
  return keys;
}

/** 024005：scope / identity 只读上下文，不进入 additive 可删行。 */
function renderLockedFilterContext(props) {
  const scope =
    props?.scope_filters && typeof props.scope_filters === "object" && !Array.isArray(props.scope_filters)
      ? props.scope_filters
      : props?.scopeFilters && typeof props.scopeFilters === "object" && !Array.isArray(props.scopeFilters)
        ? props.scopeFilters
        : {};
  const identity =
    props?.drilldown_filters && typeof props.drilldown_filters === "object" && !Array.isArray(props.drilldown_filters)
      ? props.drilldown_filters
      : props?.drilldownFilters && typeof props.drilldownFilters === "object" && !Array.isArray(props.drilldownFilters)
        ? props.drilldownFilters
        : {};
  const chips = [];
  const pushChips = (map, kindLabel) => {
    for (const [key, value] of Object.entries(map || {})) {
      const dim = String(key || "").trim();
      const raw = String(value ?? "").trim();
      if (!dim || !raw) continue;
      chips.push(
        `<span class="locked-chip" title="${escapeHtmlAttr(`${kindLabel} · 面板不可清除`)}"><span class="locked-chip-kind">${escapeHtml(kindLabel)}</span><span class="locked-chip-dim">${escapeHtml(dim)}</span><span class="locked-chip-val">${escapeHtml(raw)}</span></span>`,
      );
    }
  };
  pushChips(scope, "宇宙");
  pushChips(identity, "身份");
  if (!chips.length) return "";
  return `<div class="locked-filter-context" aria-label="锁定过滤上下文">${chips.join("")}</div>`;
}

function buildAdditiveFilterMap(rows, profiles, catalog, queryStateId) {
  const managedKeys = catalogManagedFilterKeys(catalog);
  const current = queryStateId ? getQueryState(queryStateId).filters || {} : {};
  const filters = {};
  for (const [key, value] of Object.entries(current)) {
    if (!managedKeys.has(key)) {
      filters[key] = value;
    }
  }
  for (const row of rows || []) {
    if (!isRowActive(row)) continue;
    const column = String(row?.column || "").trim();
    if (!column) continue;
    const profile = profileForColumn(column, profiles);
    const field = findCatalogField(catalog, column);
    const stateKey = filterStateKey(field) || column;
    const logicalKey = fieldQueryKey(field);
    const normalizedRow = { ...row, operator: resolveRowOperator(row, profile, field) };
    const encoded = encodeFilterRow(normalizedRow, profile);
    if (encoded) {
      filters[stateKey] = encoded;
    } else {
      delete filters[stateKey];
    }
    // 清掉历史逻辑 key（agency），避免与列名并存时派生 dataset 整次过滤失效。
    if (logicalKey && logicalKey !== stateKey) {
      delete filters[logicalKey];
    }
  }
  return filters;
}

function renderAdditiveValueMarkup(
  row,
  profile,
  operator,
  options,
  openDropdownKey,
  fieldDef = null,
  multiPanelSearch = null,
) {
  const rowId = String(row?.id || "");
  const selectedValues = Array.isArray(row?.values) ? row.values : [];
  const value = String(row?.value || "").trim();
  const rangeStart = String(row?.rangeStart || "").trim();
  const rangeEnd = String(row?.rangeEnd || "").trim();
  const isOpen = openDropdownKey === rowId;
  const placeholder = String(fieldDef?.placeholder || "").trim() || "请输入关键词";
  const searchValue = multiPanelSearch?.get?.(rowId) || "";

  if (!String(row?.column || "").trim()) {
    return `<input class="cockpit-filter-control" type="text" data-row-value="${escapeHtmlAttr(rowId)}" placeholder="先选择字段" disabled />`;
  }
  if (operator === "in" || operator === "contains_any" || operator === "month_in") {
    return renderMultiSelectPanelMarkup({
      panelKey: rowId,
      selectedValues,
      options,
      control: operator,
      isOpen,
      searchValue,
    });
  }
  if (operator === "month_range") {
    return `
      <div class="month-range">
        <input class="cockpit-filter-control" type="month" data-row-range-start="${escapeHtmlAttr(rowId)}" value="${escapeHtmlAttr(rangeStart)}" aria-label="起始月份" />
        <span class="month-range-sep">至</span>
        <input class="cockpit-filter-control" type="month" data-row-range-end="${escapeHtmlAttr(rowId)}" value="${escapeHtmlAttr(rangeEnd)}" aria-label="结束月份" />
      </div>`;
  }
  if (operator === "date_range") {
    // 窄侧栏默认纵向：开始/结束各占一行，避免 type=date 被挤扁不可读
    return `
      <div class="date-range-control date-range-control--stacked">
        <div class="date-input-wrap">
          <input class="cockpit-filter-control" type="date" data-row-range-start="${escapeHtmlAttr(rowId)}" value="${escapeHtmlAttr(rangeStart)}" aria-label="起始日期" />
          <span class="date-input-icon">${CALENDAR_ICON_SVG}</span>
        </div>
        <span class="date-range-sep">至</span>
        <div class="date-input-wrap">
          <input class="cockpit-filter-control" type="date" data-row-range-end="${escapeHtmlAttr(rowId)}" value="${escapeHtmlAttr(rangeEnd)}" aria-label="结束日期" />
          <span class="date-input-icon">${CALENDAR_ICON_SVG}</span>
        </div>
      </div>`;
  }
  const inputType = profile?.kind === "number" ? "number" : "text";
  const valuePlaceholder =
    operator === "contains" ? placeholder || "输入关键字…" : operator === "eq" ? "等于…" : "输入数值…";
  return `<input
    class="cockpit-filter-control"
    type="${inputType}"
    data-row-value="${escapeHtmlAttr(rowId)}"
    placeholder="${escapeHtmlAttr(valuePlaceholder)}"
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
  usedColumnKeys = new Set(),
  confirmErrorRowId = "",
  confirmErrorMessage = "",
  multiPanelSearch = null,
  openFieldPickerKey = "",
  fieldPickerSearch = null,
) {
  if (isRowActive(row)) {
    return renderActiveAdditiveRow(row, index, catalog, profiles, fieldOptions, openDropdownKey, multiPanelSearch);
  }
  return renderDraftAdditiveRow(
    row,
    index,
    catalog,
    profiles,
    fieldOptions,
    openDropdownKey,
    usedColumnKeys,
    confirmErrorRowId,
    confirmErrorMessage,
    multiPanelSearch,
    openFieldPickerKey,
    fieldPickerSearch,
  );
}

function renderActiveAdditiveRow(row, index, catalog, profiles, fieldOptions, openDropdownKey, multiPanelSearch = null) {
  const rowId = String(row?.id || `row-${index}`);
  const column = String(row?.column || "").trim();
  const fieldDef = findCatalogField(catalog, column);
  const profile = profileForColumn(column, profiles);
  const operator = resolveRowOperator(row, profile, fieldDef);
  const optionValues = column
    ? resolveSelectOptionsForField(fieldDef, fieldOptions, column)
    : [];
  const resolvedOptions =
    optionValues.length > 0 ? optionValues : profile?.options || [];
  const valueMarkup = renderAdditiveValueMarkup(
    row,
    profile,
    operator,
    resolvedOptions,
    openDropdownKey,
    fieldDef,
    multiPanelSearch,
  );
  const label = fieldDef?.label || column;
  const tallClass = tallValueRowClass(operator);
  return `
    <div class="additive-row is-active${tallClass}" data-additive-row="${escapeHtmlAttr(rowId)}" data-row-column="${escapeHtmlAttr(column)}">
      <div class="active-row-body">
        <span class="field-label">${escapeHtml(label)}</span>
        <div class="row-value">${valueMarkup}</div>
      </div>
      <button type="button" class="row-remove" data-remove-row="${escapeHtmlAttr(rowId)}" aria-label="移除此条件">×</button>
    </div>
  `;
}

function renderDraftAdditiveRow(
  row,
  index,
  catalog,
  profiles,
  fieldOptions,
  openDropdownKey,
  usedColumnKeys,
  confirmErrorRowId = "",
  confirmErrorMessage = "",
  multiPanelSearch = null,
  openFieldPickerKey = "",
  fieldPickerSearch = null,
) {
  const rowId = String(row?.id || `row-${index}`);
  const column = String(row?.column || "").trim();
  const fieldDef = findCatalogField(catalog, column);
  const profile = profileForColumn(column, profiles);
  const operator = resolveRowOperator(row, profile, fieldDef);
  const optionValues = column
    ? resolveSelectOptionsForField(fieldDef, fieldOptions, column)
    : [];
  const resolvedOptions =
    optionValues.length > 0 ? optionValues : profile?.options || [];

  const operatorOptions = operatorOptionsForField(profile, fieldDef)
    .map((entry) => {
      const selected = entry.id === operator ? "selected" : "";
      return `<option value="${escapeHtmlAttr(entry.id)}" ${selected}>${escapeHtml(entry.label)}</option>`;
    })
    .join("");

  const valueMarkup = renderAdditiveValueMarkup(
    row,
    profile,
    operator,
    resolvedOptions,
    openDropdownKey,
    fieldDef,
    multiPanelSearch,
  );

  const confirmError =
    String(confirmErrorRowId || "") === rowId ? String(confirmErrorMessage || "").trim() : "";

  const fieldBlock = column
    ? `<div class="row-block row-block-field">
        <span class="row-label">字段</span>
        <span class="field-label field-label--draft">${escapeHtml(fieldDef?.label || column)}</span>
      </div>`
    : `<div class="row-block row-block-field">
        <span class="row-label">字段</span>
        ${renderDraftFieldPicker(catalog, usedColumnKeys, rowId, openFieldPickerKey, fieldPickerSearch)}
      </div>`;

  return `
    <div class="additive-row is-draft${tallValueRowClass(operator)}" data-additive-row="${escapeHtmlAttr(rowId)}" data-row-column="${escapeHtmlAttr(column)}">
      <div class="row-stack">
        ${fieldBlock}
        <label class="row-block">
          <span class="row-label">条件</span>
          <select class="cockpit-filter-control" data-row-operator="${escapeHtmlAttr(rowId)}" aria-label="筛选条件" ${column ? "" : "disabled"}>
            ${column ? operatorOptions : `<option value="">—</option>`}
          </select>
        </label>
        <label class="row-block row-block-value">
          <span class="row-label">值</span>
          <div class="row-value">${valueMarkup}</div>
        </label>
        <button type="button" class="row-confirm" data-confirm-row="${escapeHtmlAttr(rowId)}">确认</button>
        <p class="row-confirm-hint">确认后固化为筛选控件；条件就绪后点底部「查询」</p>
        ${confirmError ? `<p class="row-confirm-error">${escapeHtml(confirmError)}</p>` : ""}
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
    .wrap { display: grid; gap: 10px; padding: 14px; border-radius: 0; background: ${color("filter_panel_bg")}; border: 1px solid ${color("filter_panel_border")}; color: ${color("text_body")}; }
    .title { margin: 0; font-size: ${FILTER_PANEL_FONT}; color: ${color("text_inverse")}; }
    .desc { color: ${color("text_muted")}; font-size: ${FILTER_PANEL_FONT}; line-height: 1.45; }
    .fields { display: grid; gap: 10px; grid-template-columns: 1fr; }
    label.field { display: grid; gap: 6px; font-size: ${FILTER_PANEL_FONT}; color: ${color("text_body")}; position: relative; }
    input[type="text"], input[type="date"], input[type="month"], select, button { border-radius: 8px; border: 1px solid ${color("drilldown_tab_border")}; background: ${color("drilldown_tab_bg")}; color: ${color("text_body")}; font-size: ${FILTER_PANEL_FONT}; padding: 7px 9px; }
    .multi-trigger { width: 100%; text-align: left; cursor: pointer; display: flex; justify-content: space-between; gap: 8px; align-items: center; }
    .multi-trigger::after { content: "▾"; opacity: .7; }
    .multi-trigger.is-open::after { content: "▴"; }
    .multi-panel { display: none; position: absolute; left: 0; right: 0; top: calc(100% - 2px); z-index: 20; max-height: 280px; overflow: auto; border-radius: 8px; border: 1px solid ${color("filter_panel_border")}; background: ${color("drilldown_panel_bottom")}; box-shadow: 0 12px 28px rgba(2, 6, 23, 0.45); padding: 6px; }
    .multi-panel.is-open { display: block; }
    .multi-search { width: 100%; margin-bottom: 6px; position: sticky; top: 0; z-index: 1; box-sizing: border-box; }
    .multi-options { display: flex; flex-direction: column; gap: 2px; }
    .multi-option { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 6px; cursor: pointer; font-size: ${FILTER_PANEL_FONT}; color: ${color("text_body")}; }
    .multi-option[hidden] { display: none !important; }
    .multi-option:hover { background: ${color("table_row_hover")}; }
    .multi-option input { margin: 0; }
    .multi-filter-empty { padding: 8px; color: ${color("text_muted")}; font-size: calc(${FILTER_PANEL_FONT} * 0.9); text-align: center; }
    .multi-filter-empty[hidden] { display: none !important; }
    .actions { display: flex; gap: 8px; justify-content: flex-end; }
    button.action { cursor: pointer; }
    button.action.primary { border-color: rgba(56, 189, 248, 0.55); color: ${color("text_highlight")}; background: rgba(14, 116, 178, 0.35); }
    .loading { color: ${color("text_muted")}; font-size: ${FILTER_PANEL_FONT}; }
    .multi-empty { padding: 8px; color: ${color("text_muted")}; font-size: ${FILTER_PANEL_FONT}; }
    .multi-panel.is-floating,
    .field-picker-panel.is-floating {
      right: auto !important;
      box-shadow: 0 20px 48px rgba(2, 6, 23, 0.58);
    }
    .multi-panel.is-floating .multi-option span,
    .field-picker-panel.is-floating .field-picker-option {
      white-space: normal;
      word-break: break-word;
      line-height: 1.4;
    }
  `;
}

function additiveStyles() {
  const { border, bg, radius, minHeight } = schemaControlTokens();
  return `
    :host {
      display: block;
      height: 100%;
      min-height: 0;
    }
    .wrap.additive-wrap {
      display: flex;
      flex-direction: column;
      height: 100%;
      min-height: 0;
      gap: 0;
      padding: 10px 10px 8px;
      border-radius: 0;
      box-sizing: border-box;
    }
    .filter-panel-head {
      display: flex;
      align-items: center;
      flex-shrink: 0;
      padding-bottom: 8px;
    }
    .panel-toggle {
      width: 100%;
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 0;
      border: 0;
      background: transparent;
      color: ${color("text_inverse")};
      font-size: ${FILTER_PANEL_FONT};
      cursor: pointer;
      text-align: left;
    }
    .panel-title { flex: 1; font-weight: 600; letter-spacing: 0.04em; }

    .locked-filter-context {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      margin: 0 0 8px;
    }
    .locked-chip {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      max-width: 100%;
      padding: 3px 8px;
      border-radius: 6px;
      border: 1px solid rgba(56, 160, 240, 0.35);
      background: rgba(14, 48, 88, 0.72);
      color: var(--mei-color-text-body, #e2e8f0);
      font-size: calc(${FILTER_PANEL_FONT} * 0.85);
      line-height: 1.3;
    }
    .locked-chip-kind {
      color: var(--mei-color-text-highlight, #7dd3fc);
      font-weight: 600;
    }
    .locked-chip-dim { opacity: 0.85; }
    .locked-chip-val {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      max-width: 10em;
    }
    .panel-active-badge {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      min-width: 20px;
      height: 20px;
      padding: 0 6px;
      border-radius: 999px;
      background: rgba(56, 189, 248, 0.22);
      color: ${color("text_highlight")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.82);
    }
    .panel-chevron {
      width: 10px;
      height: 10px;
      border-right: 2px solid rgba(186, 230, 253, 0.85);
      border-bottom: 2px solid rgba(186, 230, 253, 0.85);
      transform: rotate(45deg);
      transition: transform 0.15s ease;
      margin-right: 4px;
    }
    .wrap.is-collapsed .panel-chevron { transform: rotate(-135deg); margin-top: 4px; }
    .filter-panel-body {
      display: flex;
      flex-direction: column;
      flex: 1;
      min-height: 0;
      gap: 0;
      overflow: visible;
    }
    .wrap.is-collapsed .filter-panel-body { display: none; }
    .filter-panel-main {
      flex: 0 0 auto;
      display: flex;
      flex-direction: column;
      gap: 8px;
      min-height: 0;
    }
    .filter-panel-footer {
      flex-shrink: 0;
      margin-top: auto;
      padding-top: 8px;
      border-top: 1px dashed rgba(56, 160, 240, 0.16);
    }
    .additive-rows {
      flex: 0 1 auto;
      min-height: 0;
      max-height: min(52vh, 420px);
      overflow: auto;
      display: grid;
      gap: 10px;
      align-content: start;
      padding: 2px 0 0;
    }
    .additive-row {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 6px;
      align-items: start;
      padding: 8px;
      border-radius: ${radius};
      border: 1px solid rgba(56, 160, 240, 0.18);
      background: rgba(8, 32, 68, 0.28);
    }
    .additive-row.is-draft {
      border-style: dashed;
      border-color: rgba(56, 160, 240, 0.28);
      overflow: visible;
      z-index: 2;
    }
    .additive-row.is-active {
      border-color: rgba(56, 189, 248, 0.35);
    }
    .additive-row.is-tall-value {
      position: relative;
      overflow: visible;
      z-index: 1;
    }
    .additive-row.is-tall-value.is-value-focused,
    .additive-row.is-tall-value:focus-within {
      z-index: 35;
    }
    .additive-row.is-tall-value .active-row-body,
    .additive-row.is-tall-value .row-value,
    .additive-row.is-tall-value .row-stack {
      overflow: visible;
    }
    .active-row-body {
      display: grid;
      gap: 5px;
      min-width: 0;
    }
    .field-label {
      color: ${color("text_inverse")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.9);
      font-weight: 600;
      line-height: 1.35;
    }
    .row-confirm {
      width: 100%;
      min-height: ${minHeight};
      cursor: pointer;
      border-radius: ${radius};
      border: 1px solid rgba(56, 189, 248, 0.55);
      color: ${color("text_highlight")};
      background: linear-gradient(180deg, rgba(14, 116, 178, 0.55), rgba(8, 72, 120, 0.45));
      font-size: ${FILTER_PANEL_FONT};
      padding: 6px 10px;
    }
    .row-confirm-error {
      margin: 0;
      color: ${color("status_error")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.86);
      line-height: 1.35;
    }
    .row-confirm-hint {
      margin: 0;
      color: ${color("text_muted")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.82);
      line-height: 1.35;
    }
    .row-stack { display: grid; gap: 6px; min-width: 0; }
    .row-block { display: grid; gap: 5px; font-size: ${FILTER_PANEL_FONT}; min-width: 0; }
    .row-label {
      color: ${color("text_muted")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.88);
      line-height: 1.35;
      letter-spacing: 0.02em;
    }
    .row-block-value .row-value { position: relative; min-width: 0; }
    .row-value-multi { position: relative; width: 100%; }
    .cockpit-filter-control {
      width: 100%;
      box-sizing: border-box;
      min-height: ${minHeight};
      border-radius: ${radius};
      border: 1px solid ${border};
      background: ${bg};
      color: ${color("text_body")};
      font-size: ${FILTER_PANEL_FONT};
      padding: 6px 9px;
    }
    .cockpit-filter-control:disabled { opacity: 0.55; }
    .row-value-multi .multi-trigger.cockpit-filter-control {
      text-align: left;
      cursor: pointer;
      display: flex;
      justify-content: space-between;
      gap: 8px;
      align-items: center;
    }
    .row-value-multi .multi-trigger.cockpit-filter-control::after { content: "▾"; opacity: 0.72; color: ${color("text_highlight")}; }
    .row-value-multi .multi-trigger.cockpit-filter-control.is-open::after { content: "▴"; }
    .row-value-multi .multi-panel {
      display: none;
      position: absolute;
      left: 0;
      right: 0;
      top: calc(100% + 4px);
      z-index: 30;
      max-height: 280px;
      overflow: auto;
      border-radius: ${radius};
      border: 1px solid ${border};
      background: ${color("drilldown_panel_bottom")};
      box-shadow: 0 12px 28px rgba(2, 6, 23, 0.45);
      padding: 6px;
    }
    .row-value-multi .multi-panel.is-open { display: block; }
    .date-range-control--stacked {
      display: grid;
      gap: 6px;
    }
    .date-range-control--inline {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
      gap: 6px;
      align-items: center;
    }
    .date-range-control--inline .date-range-sep {
      padding: 0 2px;
      white-space: nowrap;
    }
    .date-range-control .date-input-wrap {
      position: relative;
      display: flex;
      align-items: center;
    }
    .date-range-control .date-input-wrap .cockpit-filter-control {
      padding-right: 30px;
    }
    .date-range-control .date-input-icon {
      position: absolute;
      right: 8px;
      color: ${color("text_unit")};
      pointer-events: none;
      display: inline-flex;
    }
    .date-range-sep, .month-range-sep {
      color: rgba(148, 163, 184, 0.85);
      font-size: calc(${FILTER_PANEL_FONT} * 0.88);
      text-align: center;
    }
    .month-range {
      display: grid;
      gap: 6px;
    }
    .row-negate {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      color: ${color("text_body")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.88);
      cursor: pointer;
    }
    .row-negate input { margin: 0; }
    .row-remove {
      width: 26px;
      height: 26px;
      padding: 0;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      color: rgba(148, 163, 184, 0.8);
      background: transparent;
      border: 0;
      border-radius: 6px;
      font-size: 18px;
      line-height: 1;
    }
    .row-remove:hover { color: ${color("text_body")}; background: rgba(15, 45, 82, 0.45); }
    .field-picker {
      position: relative;
      width: 100%;
      flex-shrink: 0;
    }
    .field-picker-trigger {
      width: 100%;
      min-height: ${minHeight};
      text-align: left;
      cursor: pointer;
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 8px;
      border-style: dashed;
      border-color: rgba(56, 189, 248, 0.35);
      color: ${color("text_highlight")};
      background: rgba(8, 32, 68, 0.22);
    }
    .field-picker-trigger::after {
      content: "▾";
      opacity: 0.72;
      color: ${color("text_highlight")};
      flex-shrink: 0;
    }
    .field-picker-trigger.is-open::after { content: "▴"; }
    .field-picker-trigger:disabled { opacity: 0.55; cursor: not-allowed; }
    .field-picker-panel {
      display: none;
      position: absolute;
      left: 0;
      right: 0;
      top: calc(100% + 4px);
      bottom: auto;
      z-index: 45;
      border-radius: ${radius};
      border: 1px solid ${border};
      background: ${color("drilldown_panel_bottom")};
      box-shadow: 0 12px 28px rgba(2, 6, 23, 0.45);
      padding: 6px;
      box-sizing: border-box;
      max-height: min(320px, 52vh);
      overflow: auto;
    }
    .field-picker-panel.is-dropup {
      top: auto;
      bottom: calc(100% + 4px);
      box-shadow: 0 -10px 28px rgba(2, 6, 23, 0.45);
    }
    .field-picker-panel.is-dropup .field-picker-search {
      position: sticky;
      top: auto;
      bottom: 0;
      margin-bottom: 0;
      margin-top: 6px;
    }
    .field-picker-panel.is-open { display: block; }
    .field-picker-meta {
      margin: 0 0 6px;
      padding: 0 4px;
      color: ${color("text_muted")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.82);
      line-height: 1.35;
    }
    .field-picker-search {
      width: 100%;
      margin-bottom: 6px;
      position: sticky;
      top: 0;
      z-index: 1;
      box-sizing: border-box;
    }
    .field-picker-options {
      display: flex;
      flex-direction: column;
      gap: 2px;
    }
    .field-picker-option {
      appearance: none;
      -webkit-appearance: none;
      display: block;
      width: 100%;
      box-sizing: border-box;
      min-height: 34px;
      padding: 7px 10px;
      border: 1px solid ${color("drilldown_tab_border")};
      border-radius: 6px;
      background: ${color("drilldown_tab_bg")};
      color: ${color("text_body")};
      font-size: ${FILTER_PANEL_FONT};
      text-align: left;
      cursor: pointer;
    }
    .field-picker-option:hover {
      background: ${color("table_row_hover")};
      border-color: ${color("table_btn_hover_border")};
      color: ${color("text_inverse")};
    }
    .field-picker-option[hidden] { display: none !important; }
    .field-picker-empty,
    .field-picker-filter-empty {
      padding: 8px 6px;
      color: ${color("text_muted")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.9);
      text-align: center;
    }
    .field-picker-filter-empty[hidden] { display: none !important; }
    .field-label--draft {
      display: flex;
      align-items: center;
      min-height: ${minHeight};
      padding: 0 2px;
      font-weight: 600;
      color: ${color("text_body")};
    }
    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
      border: 0;
    }
    .catalog-exhausted-hint {
      flex-shrink: 0;
      margin: 0;
      padding: 8px 10px;
      border-radius: ${radius};
      border: 1px dashed rgba(148, 163, 184, 0.28);
      color: ${color("text_muted")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.88);
      text-align: center;
      line-height: 1.4;
    }
    .actions { flex-shrink: 0; display: flex; gap: 8px; }
    .actions-primary {
      flex: 0 0 auto;
      padding: 0;
      margin: 0;
    }
    .actions .action { flex: 1; min-height: ${minHeight}; }
  `;
}

function schemaControlTokens() {
  const border = color("drilldown_tab_border");
  const bg = color("drilldown_tab_bg");
  const radius = "6px";
  const minHeight = "34px";
  return { border, bg, radius, minHeight };
}

function schemaStyles() {
  const { border, bg, radius, minHeight } = schemaControlTokens();
  return `
    :host {
      display: block;
      height: 100%;
      min-height: 0;
    }
    .wrap.schema-wrap {
      display: flex;
      flex-direction: column;
      height: 100%;
      min-height: 0;
      gap: 0;
      padding: 10px 10px 8px;
      border-radius: 0;
      box-sizing: border-box;
    }
    .filter-panel-head {
      display: flex;
      align-items: center;
      flex-shrink: 0;
      padding-bottom: 8px;
    }
    .panel-toggle {
      width: 100%;
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 0;
      border: 0;
      background: transparent;
      color: ${color("text_inverse")};
      font-size: ${FILTER_PANEL_FONT};
      cursor: pointer;
      text-align: left;
    }
    .panel-title { flex: 1; font-weight: 600; letter-spacing: 0.04em; }
    .panel-active-badge {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      min-width: 20px;
      height: 20px;
      padding: 0 6px;
      border-radius: 999px;
      background: rgba(56, 189, 248, 0.22);
      color: ${color("text_highlight")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.82);
    }
    .panel-chevron {
      width: 10px;
      height: 10px;
      border-right: 2px solid rgba(186, 230, 253, 0.85);
      border-bottom: 2px solid rgba(186, 230, 253, 0.85);
      transform: rotate(45deg);
      transition: transform 0.15s ease;
      margin-right: 4px;
    }
    .wrap.is-collapsed .panel-chevron { transform: rotate(-135deg); margin-top: 4px; }
    .filter-panel-body {
      display: flex;
      flex-direction: column;
      flex: 1;
      min-height: 0;
      gap: 0;
    }
    .wrap.is-collapsed .filter-panel-body { display: none; }
    .schema-fields {
      flex: 1;
      min-height: 0;
      overflow: auto;
      display: grid;
      grid-template-columns: 1fr;
      gap: 10px;
      padding: 2px 0 10px;
      align-content: start;
    }
    .schema-field {
      display: grid;
      gap: 5px;
      min-width: 0;
      position: relative;
    }
    .schema-field.is-applied .cockpit-filter-control,
    .schema-field.is-applied .multi-trigger {
      border-color: rgba(56, 189, 248, 0.42);
      box-shadow: inset 0 0 0 1px rgba(56, 189, 248, 0.12);
    }
    .schema-label {
      color: ${color("text_muted")};
      font-size: calc(${FILTER_PANEL_FONT} * 0.88);
      line-height: 1.35;
      letter-spacing: 0.02em;
    }
    .schema-control { min-width: 0; }
    .cockpit-filter-control {
      width: 100%;
      box-sizing: border-box;
      min-height: ${minHeight};
      border-radius: ${radius};
      border: 1px solid ${border};
      background: ${bg};
      color: ${color("text_body")};
      font-size: ${FILTER_PANEL_FONT};
      padding: 6px 9px;
    }
    .cockpit-filter-control::placeholder { color: rgba(148, 163, 184, 0.72); }
    .schema-control .multi-trigger.cockpit-filter-control {
      width: 100%;
      text-align: left;
      cursor: pointer;
      display: flex;
      justify-content: space-between;
      gap: 8px;
      align-items: center;
    }
    .schema-control .multi-trigger.cockpit-filter-control::after { content: "▾"; opacity: 0.72; color: ${color("text_highlight")}; }
    .schema-control .multi-trigger.cockpit-filter-control.is-open::after { content: "▴"; }
    .schema-control .multi-panel {
      left: 0;
      right: 0;
      z-index: 30;
    }
    .schema-field.is-date-range {
      position: relative;
      overflow: visible;
      z-index: 1;
    }
    .schema-field.is-date-range.is-value-focused,
    .schema-field.is-date-range:focus-within {
      z-index: 35;
    }
    .schema-field.is-date-range .schema-control {
      overflow: visible;
    }
    .date-range-control--inline {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
      gap: 6px;
      align-items: center;
    }
    .date-range-control--inline .date-range-sep {
      padding: 0 2px;
      white-space: nowrap;
      text-align: center;
      color: rgba(186, 230, 253, 0.82);
      font-size: calc(${FILTER_PANEL_FONT} * 0.88);
      line-height: 1.2;
    }
    .date-range-control--stacked {
      display: grid;
      grid-template-columns: 1fr;
      gap: 6px;
      align-items: stretch;
      width: 100%;
      min-width: 0;
    }
    .date-range-control--stacked .date-range-sep {
      text-align: center;
      color: rgba(186, 230, 253, 0.82);
      font-size: calc(${FILTER_PANEL_FONT} * 0.88);
      line-height: 1.2;
    }
    .date-range-control--stacked .date-input-wrap {
      width: 100%;
      min-width: 0;
    }
    .date-input-wrap {
      position: relative;
      display: flex;
      align-items: center;
      min-width: 0;
    }
    .date-input-wrap input[type="date"],
    .date-input-wrap input[type="month"] {
      width: 100%;
      box-sizing: border-box;
      min-width: 0;
      min-height: ${minHeight};
      padding-right: 34px;
      color-scheme: dark;
      font-variant-numeric: tabular-nums;
    }
    .date-input-wrap input[type="date"]::-webkit-calendar-picker-indicator,
    .date-input-wrap input[type="month"]::-webkit-calendar-picker-indicator {
      opacity: 0;
      position: absolute;
      right: 0;
      width: 34px;
      height: 100%;
      cursor: pointer;
    }
    .date-input-icon {
      position: absolute;
      right: 10px;
      top: 50%;
      transform: translateY(-50%);
      color: ${color("text_unit")};
      pointer-events: none;
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }
    .schema-field.is-month-multi .multi-option span {
      font-variant-numeric: tabular-nums;
    }
    .actions {
      flex-shrink: 0;
      display: flex;
      gap: 8px;
      justify-content: flex-end;
      margin-top: auto;
      padding-top: 10px;
      border-top: 1px solid rgba(56, 160, 240, 0.16);
    }
    button.action {
      min-width: 72px;
      min-height: ${minHeight};
      border-radius: ${radius};
      transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
    }
    button.action:hover {
      border-color: rgba(125, 211, 252, 0.45);
      background: rgba(14, 72, 128, 0.42);
    }
    button.action.primary {
      background: linear-gradient(180deg, rgba(14, 116, 178, 0.72) 0%, rgba(8, 72, 132, 0.88) 100%);
      box-shadow: inset 0 0 0 1px rgba(125, 211, 252, 0.18);
    }
    button.action.primary:hover {
      background: linear-gradient(180deg, rgba(14, 136, 208, 0.82) 0%, rgba(8, 88, 152, 0.95) 100%);
      color: #f0f9ff;
    }
  `;
}

function resolveSchemaFields(props) {
  const explicit = props?.schema_fields || props?.schemaFields;
  if (Array.isArray(explicit) && explicit.length > 0) {
    return explicit
      .map((field) => normalizeSchemaField(field))
      .filter((field) => field && field.visible !== false);
  }
  if (!isSchemaMode(props)) {
    return [];
  }
  return resolveColumnCatalog(props).filter((field) => String(field?.control || "").trim());
}

function normalizeSchemaField(field) {
  if (!field || typeof field !== "object") return null;
  const column = String(field?.column || field?.key || "").trim();
  if (!column) return null;
  return {
    key: String(field?.key || column).trim(),
    label: String(field?.label || column).trim(),
    column,
    control: normalizeControl(field),
    visible: field?.visible !== false,
    placeholder: String(field?.placeholder || "").trim(),
    options: Array.isArray(field?.options) ? field.options : undefined,
    options_from: String(field?.options_from || field?.optionsFrom || "").trim(),
    options_field: String(field?.options_field || field?.optionsField || column).trim(),
  };
}

function schemaFieldKey(field) {
  return String(field?.key || field?.column || "").trim();
}

function countActiveSchemaFilters(rows, schemaFields) {
  let count = 0;
  for (const field of schemaFields || []) {
    const column = String(field?.column || "").trim();
    if (!column) continue;
    const row = (rows || []).find((entry) => String(entry?.column || "").trim() === column);
    if (!row) continue;
    const encoded = encodeSchemaFieldValue(field, row);
    if (encoded) count += 1;
  }
  return count;
}

function parseDateRangeFilterValue(raw) {
  const text = String(raw || "").trim();
  if (!text) return { start: "", end: "" };
  if (text.startsWith("mrange:")) {
    const [start, end] = text.slice(7).split("..");
    const startMonth = String(start || "").trim();
    const endMonth = String(end || "").trim();
    return {
      start: startMonth.length === 7 ? `${startMonth}-01` : startMonth,
      end: endMonth.length === 7 ? `${endMonth}-01` : endMonth,
    };
  }
  if (text.startsWith("drange:")) {
    const [start, end] = text.slice(7).split("..");
    return { start: String(start || "").trim(), end: String(end || "").trim() };
  }
  if (text.includes("..")) {
    const [start, end] = text.split("..");
    return { start: String(start || "").trim(), end: String(end || "").trim() };
  }
  return { start: "", end: "" };
}

function encodeSchemaDateRange(start, end) {
  const lo = String(start || "").trim();
  const hi = String(end || "").trim();
  if (!lo && !hi) return "";
  if (lo && hi) {
    if (/^\d{4}-\d{2}-\d{2}$/.test(lo) && /^\d{4}-\d{2}-\d{2}$/.test(hi)) {
      return `drange:${lo}..${hi}`;
    }
    const startMonth = lo.slice(0, 7);
    const endMonth = hi.slice(0, 7);
    if (/^\d{4}-\d{2}$/.test(startMonth) && /^\d{4}-\d{2}$/.test(endMonth)) {
      return `mrange:${startMonth}..${endMonth}`;
    }
    return `drange:${lo}..${hi}`;
  }
  // Open-ended: only start (≥) or only end (≤).
  if (lo && !hi) {
    if (/^\d{4}-\d{2}-\d{2}$/.test(lo)) return `drange:${lo}..`;
    const startMonth = lo.slice(0, 7);
    if (/^\d{4}-\d{2}$/.test(startMonth)) return `mrange:${startMonth}..`;
    return `drange:${lo}..`;
  }
  if (/^\d{4}-\d{2}-\d{2}$/.test(hi)) return `drange:..${hi}`;
  const endMonth = hi.slice(0, 7);
  if (/^\d{4}-\d{2}$/.test(endMonth)) return `mrange:..${endMonth}`;
  return `drange:..${hi}`;
}

function encodeSchemaFieldValue(field, row) {
  const control = normalizeControl(field);
  if (control === "date_range") {
    const rangeStart = String(row?.rangeStart || "").trim();
    const rangeEnd = String(row?.rangeEnd || "").trim();
    return encodeSchemaDateRange(rangeStart, rangeEnd);
  }
  if (control === "multi_select" || control === "month_multi_select") {
    const values = Array.isArray(row?.values) ? row.values.filter(Boolean) : [];
    if (!values.length) return "";
    const prefix = control === "month_multi_select" ? "m:" : "in:";
    return `${prefix}${values.join(",")}`;
  }
  const value = String(row?.value || "").trim();
  if (!value) return "";
  if (control === "text") {
    // 文本控件默认关键字过滤；SQL 仅识别 contains: 前缀。
    return `contains:${value}`;
  }
  return value;
}

function readSchemaFieldValueFromDom(shadowRoot, field) {
  const key = schemaFieldKey(field);
  const control = normalizeControl(field);
  if (!key || !shadowRoot) return "";
  if (control === "date_range") {
    const start = String(
      shadowRoot.querySelector(`[data-schema-date-start="${CSS.escape(key)}"]`)?.value || "",
    ).trim();
    const end = String(
      shadowRoot.querySelector(`[data-schema-date-end="${CSS.escape(key)}"]`)?.value || "",
    ).trim();
    return encodeSchemaDateRange(start, end);
  }
  if (control === "multi_select" || control === "month_multi_select") {
    const values = [];
    const selectors = [
      shadowRoot,
      findFloatingMultiPanel(key),
    ].filter(Boolean);
    const seen = new Set();
    for (const root of selectors) {
      for (const checkbox of root.querySelectorAll(
        `.multi-option input[type="checkbox"][data-field-key="${CSS.escape(key)}"]`,
      )) {
        if (!checkbox.checked) continue;
        const value = String(checkbox.value || "").trim();
        if (!value || seen.has(value)) continue;
        seen.add(value);
        values.push(value);
      }
    }
    if (!values.length) return "";
    const prefix = control === "month_multi_select" ? "m:" : "in:";
    return `${prefix}${values.join(",")}`;
  }
  const value = String(
    shadowRoot.querySelector(`[data-schema-text="${CSS.escape(key)}"]`)?.value || "",
  ).trim();
  if (!value) return "";
  return `contains:${value}`;
}

function collectSchemaFilters(shadowRoot, schemaFields) {
  const filters = {};
  for (const field of schemaFields || []) {
    const key = schemaFieldKey(field);
    if (!key) continue;
    const encoded = readSchemaFieldValueFromDom(shadowRoot, field);
    if (encoded) {
      filters[key] = encoded;
    }
  }
  return filters;
}

function renderSchemaDateRange(field, row, appliedFilters) {
  const key = schemaFieldKey(field);
  const label = field?.label || key;
  const appliedRaw = String(appliedFilters?.[key] ?? "").trim();
  const currentEncoded = encodeSchemaFieldValue(field, row);
  const isApplied = Boolean(appliedRaw && currentEncoded && appliedRaw === currentEncoded);
  const parsed = parseDateRangeFilterValue(appliedRaw || currentEncoded);
  const rangeStart = String(row?.rangeStart || parsed.start || "").trim();
  const rangeEnd = String(row?.rangeEnd || parsed.end || "").trim();
  return `
    <div class="schema-field is-date-range is-tall-value ${isApplied ? "is-applied" : ""}">
      <span class="schema-label">${escapeHtml(label)}</span>
      <div class="schema-control date-range-control date-range-control--stacked">
        <div class="date-input-wrap">
          <input
            class="cockpit-filter-control"
            type="date"
            data-schema-date-start="${escapeHtmlAttr(key)}"
            value="${escapeHtmlAttr(rangeStart)}"
            aria-label="${escapeHtmlAttr(`${label}起始日期`)}"
          />
          <span class="date-input-icon">${CALENDAR_ICON_SVG}</span>
        </div>
        <span class="date-range-sep">至</span>
        <div class="date-input-wrap">
          <input
            class="cockpit-filter-control"
            type="date"
            data-schema-date-end="${escapeHtmlAttr(key)}"
            value="${escapeHtmlAttr(rangeEnd)}"
            aria-label="${escapeHtmlAttr(`${label}结束日期`)}"
          />
          <span class="date-input-icon">${CALENDAR_ICON_SVG}</span>
        </div>
      </div>
    </div>
  `;
}

function renderSchemaMultiSelect(field, row, appliedFilters, fieldOptions, openDropdownKey, control, multiPanelSearch = null) {
  const key = schemaFieldKey(field);
  const label = field?.label || key;
  const column = String(field?.column || key).trim();
  const appliedRaw = String(appliedFilters?.[key] ?? "").trim();
  const selectedValues = Array.isArray(row?.values)
    ? row.values
    : selectedValuesForField({ [key]: appliedRaw }, key, control, field);
  const options = resolveSelectOptionsForField(field, fieldOptions, column);
  const isOpen = openDropdownKey === key;
  const searchValue = multiPanelSearch?.get?.(key) || "";
  const currentEncoded = encodeSchemaFieldValue(field, { ...row, values: selectedValues });
  const isApplied = Boolean(appliedRaw && currentEncoded && appliedRaw === currentEncoded);
  const monthClass = control === "month_multi_select" ? " is-month-multi" : "";
  const panelMarkup = renderMultiSelectPanelMarkup({
    panelKey: key,
    selectedValues,
    options,
    control: control === "month_multi_select" ? "month_in" : "in",
    isOpen,
    searchValue,
    checkboxExtraAttrs: `data-field-key="${escapeHtmlAttr(key)}" data-field-control="${escapeHtmlAttr(control)}"`,
    wrapperClass: "schema-control row-value-multi",
  });
  return `
    <div class="schema-field${monthClass} ${isApplied ? "is-applied" : ""}">
      <span class="schema-label">${escapeHtml(label)}</span>
      ${panelMarkup}
    </div>
  `;
}

function renderSchemaText(field, row, appliedFilters) {
  const key = schemaFieldKey(field);
  const label = field?.label || key;
  const appliedRaw = String(appliedFilters?.[key] ?? "").trim();
  const value = String(row?.value || selectedValuesForField({ [key]: appliedRaw }, key, "text")[0] || "").trim();
  const placeholder = field?.placeholder || "请输入关键词";
  const currentEncoded = encodeSchemaFieldValue(field, { value });
  const isApplied = Boolean(appliedRaw && currentEncoded && appliedRaw === currentEncoded);
  return `
    <div class="schema-field ${isApplied ? "is-applied" : ""}">
      <span class="schema-label">${escapeHtml(label)}</span>
      <div class="schema-control">
        <input
          class="cockpit-filter-control"
          type="text"
          data-schema-text="${escapeHtmlAttr(key)}"
          placeholder="${escapeHtmlAttr(placeholder)}"
          value="${escapeHtmlAttr(value)}"
        />
      </div>
    </div>
  `;
}

function renderSchemaField(field, row, appliedFilters, fieldOptions, openDropdownKey, multiPanelSearch = null) {
  const control = normalizeControl(field);
  if (control === "date_range") {
    return renderSchemaDateRange(field, row, appliedFilters);
  }
  if (control === "multi_select" || control === "month_multi_select") {
    return renderSchemaMultiSelect(
      field,
      row,
      appliedFilters,
      fieldOptions,
      openDropdownKey,
      control,
      multiPanelSearch,
    );
  }
  return renderSchemaText(field, row, appliedFilters);
}

function normalizeControl(field) {
  const control = String(field?.control || field?.type || "").trim().toLowerCase();
  if (
    control === "date_range" ||
    control === "multi_select" ||
    control === "month_multi_select" ||
    control === "text"
  ) {
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

function selectedValuesForField(filters, queryKey, control, field = null) {
  const column = field ? String(field?.column || "").trim() : "";
  const raw = String(filters?.[queryKey] ?? (column ? filters?.[column] : "") ?? "");
  if (!raw) return [];
  if (control === "text" && raw.startsWith("contains:")) {
    return [raw.slice("contains:".length)];
  }
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
  if (control === "multi_select" && raw.startsWith("contains_any:")) {
    return raw
      .slice("contains_any:".length)
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

function renderField(field, filters, index, fieldOptions, openDropdownKey, multiPanelSearch = null) {
  const queryKey = fieldQueryKey(field);
  if (!queryKey) return "";
  const label = field?.label || queryKey;
  const placeholder = field?.placeholder || "";
  const control = normalizeControl(field);
  const selected = selectedValuesForField(filters, queryKey, control, field);
  const staticOptions = Array.isArray(field?.options) ? field.options : [];
  const dynamicOptions = fieldOptions?.get(queryKey) || [];
  const options = staticOptions.length > 0 ? staticOptions : dynamicOptions;
  const isOpen = openDropdownKey === queryKey;
  const searchValue = multiPanelSearch?.get?.(queryKey) || "";

  if (control === "multi_select" || control === "month_multi_select") {
    const panelMarkup = renderMultiSelectPanelMarkup({
      panelKey: queryKey,
      selectedValues: selected,
      options,
      control: control === "month_multi_select" ? "month_in" : "in",
      isOpen,
      searchValue,
      checkboxExtraAttrs: `data-field-key="${escapeHtmlAttr(queryKey)}" data-field-control="${escapeHtmlAttr(control)}" data-field-index="${index}"`,
      triggerClass: "",
      wrapperClass: "row-value-multi",
    });
    return `
      <label class="field">
        <span>${escapeHtml(label)}</span>
        ${panelMarkup}
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

// v2：避免浏览器 CE 注册表钉住曾把风险等级锁成红/黄/蓝的旧模块。
// 同一 constructor 只能 define 一次；兼容旧标签须用子类。
if (!customElements.get("mei-dataset-filter-bar-v2")) {
  customElements.define("mei-dataset-filter-bar-v2", MeiDatasetFilterBar);
}
if (!customElements.get("mei-dataset-filter-bar")) {
  class MeiDatasetFilterBarLegacy extends MeiDatasetFilterBar {}
  customElements.define("mei-dataset-filter-bar", MeiDatasetFilterBarLegacy);
}
