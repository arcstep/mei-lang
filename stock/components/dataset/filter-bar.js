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

class MeiDatasetFilterBar extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    this._queryStateId = queryStateIdOf(this._props);
    this._fields = Array.isArray(this._props.fields) ? this._props.fields : [];
    this._fieldOptions = new Map();
    this._optionsLoaded = false;
    this._openDropdownKey = "";
    this.attachShadow({ mode: "open" });
    const initialFilters = mergeFilters(this._props.default_filters);
    const current = getQueryState(this._queryStateId);
    if (this._queryStateId && Object.keys(current.filters || {}).length === 0 && Object.keys(initialFilters).length > 0) {
      setQueryState(
        this._queryStateId,
        { filters: initialFilters },
        { filterIntentSource: "filter_bar", transitionSource: "filter_bar" }
      );
    }
    this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
      this._filters = state?.filters || {};
      this.render();
    });
    if (!this._queryStateId) {
      this._filters = initialFilters;
      this.render();
    }
    this._outsideClickHandler = (event) => {
      if (!this._openDropdownKey) return;
      const path = event.composedPath();
      if (path.includes(this)) return;
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

  async loadDynamicOptions() {
    const needsRowset = this._fields.some((field) => shouldLoadRowsetOptions(field));
    if (!needsRowset) {
      this._optionsLoaded = true;
      return;
    }
    const datasetId = String(
      this._props.rowset_dataset_id || this._props.rowsetDatasetId || this._props.dataset?.id || ""
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
      this.render();
    }
  }

  render() {
    const filters = this._filters || {};
    const loadingOptions = !this._optionsLoaded;
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .wrap { display: grid; gap: 10px; padding: 14px; border-radius: 14px; background: rgba(15,23,42,.72); border: 1px solid rgba(148,163,184,.18); color: #e2e8f0; }
        .title { margin: 0; font-size: 14px; color: #f8fafc; }
        .desc { color: #94a3b8; font-size: 12px; }
        .fields { display: grid; gap: 10px; grid-template-columns: 1fr; }
        label.field { display: grid; gap: 6px; font-size: 12px; color: #cbd5e1; position: relative; }
        input[type="text"], button { border-radius: 8px; border: 1px solid rgba(148,163,184,.25); background: rgba(15,23,42,.45); color: #e2e8f0; font-size: 12px; padding: 7px 9px; }
        .multi-trigger { width: 100%; text-align: left; cursor: pointer; display: flex; justify-content: space-between; gap: 8px; align-items: center; }
        .multi-trigger::after { content: "▾"; opacity: .7; }
        .multi-trigger.is-open::after { content: "▴"; }
        .multi-panel { position: absolute; left: 0; right: 0; top: calc(100% - 2px); z-index: 20; max-height: 220px; overflow: auto; border-radius: 8px; border: 1px solid rgba(148,163,184,.28); background: rgba(15,23,42,.96); box-shadow: 0 12px 28px rgba(2,6,23,.45); padding: 6px; }
        .multi-option { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 6px; cursor: pointer; font-size: 12px; color: #e2e8f0; }
        .multi-option:hover { background: rgba(51,65,85,.45); }
        .multi-option input { margin: 0; }
        .actions { display: flex; gap: 8px; justify-content: flex-end; }
        button.action { cursor: pointer; }
        .loading { color: #94a3b8; font-size: 12px; }
        .multi-empty { padding: 8px; color: #94a3b8; font-size: 12px; }
      </style>
      <section class="wrap">
        <h4 class="title">${escapeHtml(this._props.title || "过滤条件")}</h4>
        <div class="desc">${escapeHtml(this._props.description || "更新页面级 query_state，驱动多个 panel 联动刷新。")}</div>
        ${loadingOptions ? `<div class="loading">正在加载筛选项…</div>` : ""}
        <div class="fields">
          ${this._fields.map((field, index) => renderField(field, filters, index, this._fieldOptions, this._openDropdownKey)).join("")}
        </div>
        <div class="actions">
          <button id="clear" type="button" class="action">清空</button>
          <button id="apply" type="button" class="action">应用</button>
        </div>
      </section>
    `;
    this.bindEvents();
  }

  bindEvents() {
    this.shadowRoot.getElementById("apply")?.addEventListener("click", () => this.apply());
    this.shadowRoot.getElementById("clear")?.addEventListener("click", () => {
      for (const input of this.shadowRoot.querySelectorAll('input[type="text"][data-field-key]')) {
        input.value = "";
      }
      for (const checkbox of this.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
        checkbox.checked = false;
      }
      this.apply();
    });
    for (const trigger of this.shadowRoot.querySelectorAll("[data-multi-trigger]")) {
      trigger.addEventListener("click", (event) => {
        event.stopPropagation();
        const key = String(trigger.dataset.multiTrigger || "").trim();
        this._openDropdownKey = this._openDropdownKey === key ? "" : key;
        this.render();
      });
    }
    for (const checkbox of this.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
      checkbox.addEventListener("change", (event) => {
        event.stopPropagation();
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

  collectFilters() {
    const filters = {};
    for (const input of this.shadowRoot.querySelectorAll('input[type="text"][data-field-key]')) {
      const key = String(input.dataset.fieldKey || "").trim();
      const value = String(input.value || "").trim();
      if (!key || !value) continue;
      filters[key] = value;
    }
    const grouped = new Map();
    for (const checkbox of this.shadowRoot.querySelectorAll('.multi-option input[type="checkbox"]')) {
      const key = String(checkbox.dataset.fieldKey || "").trim();
      const control = String(checkbox.dataset.fieldControl || "multi_select").trim();
      if (!key || !checkbox.checked) continue;
      const value = String(checkbox.value || "").trim();
      if (!value) continue;
      if (!grouped.has(key)) {
        grouped.set(key, { control, values: [] });
      }
      grouped.get(key).values.push(value);
    }
    for (const [key, entry] of grouped.entries()) {
      const prefix = entry.control === "month_multi_select" ? "m:" : "in:";
      filters[key] = `${prefix}${entry.values.join(",")}`;
    }
    return filters;
  }

  apply() {
    const filters = this.collectFilters();
    if (this._queryStateId) {
      setQueryState(
        this._queryStateId,
        { filters },
        { filterIntentSource: "filter_bar", transitionSource: "filter_bar" }
      );
    } else {
      this._filters = filters;
      this.render();
    }
  }
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
    return raw.slice(2).split(",").map((part) => part.trim()).filter(Boolean);
  }
  if (control === "multi_select" && raw.startsWith("in:")) {
    return raw.slice(3).split(",").map((part) => part.trim()).filter(Boolean);
  }
  if (raw.includes(",")) {
    return raw.split(",").map((part) => part.trim()).filter(Boolean);
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
    const optionMarkup =
      options.length > 0
        ? options
            .map((option) => {
              const optionValue = typeof option === "string" ? option : option?.value || "";
              const optionLabel = typeof option === "string" ? option : option?.label || optionValue;
              const checked = selected.includes(String(optionValue)) ? "checked" : "";
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
        ${isOpen ? `<div class="multi-panel" data-multi-panel="${escapeHtmlAttr(queryKey)}">${optionMarkup}</div>` : ""}
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
