import {
  escapeHtml,
  escapeHtmlAttr,
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
  }

  disconnectedCallback() {
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
    }
  }

  render() {
    const filters = this._filters || {};
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .wrap { display: grid; gap: 10px; padding: 14px; border-radius: 14px; background: rgba(15,23,42,.72); border: 1px solid rgba(148,163,184,.18); color: #e2e8f0; }
        .title { margin: 0; font-size: 14px; color: #f8fafc; }
        .desc { color: #94a3b8; font-size: 12px; }
        .fields { display: grid; gap: 10px; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); }
        label { display: grid; gap: 6px; font-size: 12px; color: #cbd5e1; }
        input, select, button { border-radius: 8px; border: 1px solid rgba(148,163,184,.25); background: rgba(15,23,42,.45); color: #e2e8f0; font-size: 12px; padding: 7px 9px; }
        .actions { display: flex; gap: 8px; justify-content: flex-end; }
        button { cursor: pointer; }
      </style>
      <section class="wrap">
        <h4 class="title">${escapeHtml(this._props.title || "过滤条件")}</h4>
        <div class="desc">${escapeHtml(this._props.description || "更新页面级 query_state，驱动多个 panel 联动刷新。")}</div>
        <div class="fields">
          ${this._fields.map((field, index) => renderField(field, filters, index)).join("")}
        </div>
        <div class="actions">
          <button id="clear" type="button">清空</button>
          <button id="apply" type="button">应用</button>
        </div>
      </section>
    `;
    this.bindEvents();
  }

  bindEvents() {
    this.shadowRoot.getElementById("apply")?.addEventListener("click", () => this.apply());
    this.shadowRoot.getElementById("clear")?.addEventListener("click", () => {
      for (const input of this.shadowRoot.querySelectorAll("[data-field-key]")) {
        input.value = "";
      }
      this.apply();
    });
    for (const input of this.shadowRoot.querySelectorAll("[data-field-key]")) {
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

  apply() {
    const filters = {};
    for (const input of this.shadowRoot.querySelectorAll("[data-field-key]")) {
      const key = String(input.dataset.fieldKey || "").trim();
      const value = String(input.value || "").trim();
      if (!key || !value) continue;
      filters[key] = value;
    }
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

function renderField(field, filters, index) {
  const key = String(field?.key || field?.field || "").trim();
  if (!key) return "";
  const label = field?.label || key;
  const placeholder = field?.placeholder || "";
  const value = String(filters?.[key] || "");
  const options = Array.isArray(field?.options) ? field.options : [];
  if (options.length > 0) {
    return `
      <label>
        <span>${escapeHtml(label)}</span>
        <select data-field-key="${escapeHtmlAttr(key)}" data-field-index="${index}">
          <option value="">全部</option>
          ${options
            .map((option) => {
              const optionValue = typeof option === "string" ? option : option?.value || "";
              const optionLabel = typeof option === "string" ? option : option?.label || optionValue;
              const selected = String(optionValue) === value ? "selected" : "";
              return `<option value="${escapeHtmlAttr(optionValue)}" ${selected}>${escapeHtml(optionLabel)}</option>`;
            })
            .join("")}
        </select>
      </label>
    `;
  }
  return `
    <label>
      <span>${escapeHtml(label)}</span>
      <input
        type="text"
        data-field-key="${escapeHtmlAttr(key)}"
        data-field-index="${index}"
        placeholder="${escapeHtmlAttr(placeholder)}"
        value="${escapeHtmlAttr(value)}"
      />
    </label>
  `;
}

customElements.define("mei-dataset-filter-bar", MeiDatasetFilterBar);
