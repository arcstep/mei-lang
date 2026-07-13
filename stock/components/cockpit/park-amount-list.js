import {
  deferUntilDisplayed,
  fetchDatasetRows,
  isAbortError,
  parseProps,
  recordRuntimeDatasetQueryError,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  subscribeHomeRuntimeResume,
  subscribeQueryState,
} from "../dataset/runtime-query.js";
import { escapeHtml } from "./shared.js";
import { COCKPIT_FONT, COCKPIT_TYPE, cockpitCssVars } from "./tokens.js";
import { color } from "../mei/theme-style.js";

function formatWanYuan(raw) {
  const n = Number(raw);
  if (!Number.isFinite(n)) return "0.0";
  return (n / 10000).toFixed(1);
}

/** 保持 props.value 为带 __mei_runtime_ref 的完整 metric 对象，供 fetchRuntimeMetrics 解析 */
function propsWithMetricValue(props, resolvedValue) {
  if (!resolvedValue || typeof resolvedValue !== "object") {
    return props;
  }
  if (resolvedValue.__mei_runtime_ref) {
    return { ...props, value: resolvedValue };
  }
  if (resolvedValue.kind === "metric" && resolvedValue.dataset_id) {
    return {
      ...props,
      value: {
        __mei_runtime_ref: resolvedValue,
        id: resolvedValue.metric_id,
      },
    };
  }
  return { ...props, value: resolvedValue };
}

class MeiCockpitParkAmountList extends HTMLElement {
  static get observedAttributes() {
    return ["data-props"];
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (
      name !== "data-props" ||
      oldValue === newValue ||
      !this.isConnected ||
      !this._bootstrapped
    ) {
      return;
    }
    queueMicrotask(() => {
      if (!this.isConnected || !this._bootstrapped) return;
      this.applyUpdatedProps();
    });
  }

  connectedCallback() {
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    this._deferUntilVisibleCleanup = deferUntilDisplayed(this, () => {
      this._deferUntilVisibleCleanup = null;
      this.bootstrap();
    });
  }

  bootstrap() {
    this._props = parseProps(this);
    this._queryStateId = String(this._props?.query_state ?? this._props?.queryState ?? "").trim();
    this._sharedFilters = {};
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
      this._sharedFilters = state?.filters || {};
      this.refreshData();
    });
    this._unsubscribeHomeRuntimeResume = subscribeHomeRuntimeResume(() => {
      this.refreshData();
    });
    this.renderShell();
    this.refreshData();
    this._bootstrapped = true;
  }

  applyUpdatedProps() {
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
    }
    this._props = parseProps(this);
    this._queryStateId = String(this._props?.query_state ?? this._props?.queryState ?? "").trim();
    this._sharedFilters = {};
    this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
      this._sharedFilters = state?.filters || {};
      this.refreshData();
    });
    this.renderShell();
    this.refreshData();
  }

  disconnectedCallback() {
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
    }
    if (typeof this._unsubscribeHomeRuntimeResume === "function") {
      this._unsubscribeHomeRuntimeResume();
    }
    this._bootstrapped = false;
  }

  renderShell() {
    const h = Number(this._props?.height) > 0 ? Number(this._props.height) : 104;
    const compact = this._props?.compact === true || this._props?.compact === "true";
    const listPad = compact ? "2px 2px 0" : "4px 2px 2px";
    const listLayout = compact ? "flex-start" : "space-between";
    const listGap = compact ? "2px" : "0";
    const rowPad = compact ? "3px 2px 4px" : "6px 4px 8px";
    const rowGap = compact ? "6px" : "8px";
    const titleText = String(this._props?.title ?? "").trim();
    const nameSize = COCKPIT_TYPE.chartLabel;
    const nameLh = compact ? "1.2" : "1.35";
    const valueSize = COCKPIT_TYPE.chartTitle;
    const unitSize = COCKPIT_TYPE.chartLabel;
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: flex;
          flex-direction: column;
          width: 100%;
          min-width: 0;
          height: ${h}px;
          min-height: ${h}px;
          box-sizing: border-box;
          font-family: ${COCKPIT_FONT.uiFamily};
          ${cockpitCssVars()}
        }
        .head {
          flex: 0 0 auto;
          margin: 0 0 ${compact ? "2px" : "4px"};
          padding: 0 2px;
          font-size: ${COCKPIT_TYPE.chartTitle};
          font-weight: 600;
          line-height: 1.2;
          color: ${color("text_inverse")};
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .list {
          display: flex;
          flex-direction: column;
          justify-content: ${listLayout};
          gap: ${listGap};
          flex: 1 1 auto;
          min-height: 0;
          height: auto;
          padding: ${listPad};
          box-sizing: border-box;
        }
        .row {
          display: grid;
          grid-template-columns: minmax(0, 1fr) auto;
          align-items: baseline;
          gap: ${rowGap};
          padding: ${rowPad};
          border-bottom: 1px solid ${color("section_border_soft")};
        }
        .row:last-child {
          border-bottom: none;
          padding-bottom: ${compact ? "2px" : "2px"};
        }
        .name {
          font-size: ${nameSize};
          line-height: ${nameLh};
          color: ${color("text_body")};
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .amount {
          display: flex;
          align-items: baseline;
          gap: 2px;
          flex: 0 0 auto;
          font-variant-numeric: tabular-nums;
        }
        .value {
          font-size: ${valueSize};
          font-weight: 700;
          color: ${color("text_highlight")};
          line-height: 1;
        }
        .unit {
          font-size: ${unitSize};
          color: var(--cockpit-color-label);
          font-weight: 600;
        }
        .status {
          font-size: ${COCKPIT_TYPE.chartLabel};
          color: ${color("text_secondary")};
          text-align: center;
          padding: 12px 0;
        }
        .status.error { color: ${color("status_error")}; }
      </style>
      ${titleText ? `<h4 class="head">${escapeHtml(titleText)}</h4>` : ""}
      <div class="list"></div>
      <div class="status"></div>
    `;
    this.listEl = this.shadowRoot.querySelector(".list");
    this.statusEl = this.shadowRoot.querySelector(".status");
  }

  async fetchMetricRows(resolvedValue) {
    const lineProps = propsWithMetricValue(this._props, resolvedValue);
    const metricRef = resolveRuntimeMetricRef(lineProps);
    if (!metricRef) return [];
    const limit = Math.max(1, Number(this._props?.limit) || 3);
    const result = await fetchDatasetRows(lineProps, {
      filters: this._sharedFilters,
      page: 1,
      pageSize: Math.max(limit + 4, 16),
      meta: runtimeCallerMeta(this, "mei-cockpit-park-amount-list"),
    });
    return Array.isArray(result?.rows) ? result.rows : [];
  }

  async refreshData() {
    if (!this.listEl) return;
    const resolvedValue = this._props?.metric ?? this._props?.data ?? this._props?.value;
    const metricRef = resolveRuntimeMetricRef(propsWithMetricValue(this._props, resolvedValue));
    const labelField = String(this._props?.labelField ?? "园区名称").trim();
    const valueField = String(this._props?.valueField ?? "value").trim();
    const limit = Math.max(1, Number(this._props?.limit) || 3);
    const unit = String(this._props?.unit ?? "万元").trim();

    if (!metricRef) {
      this.listEl.innerHTML = "";
      this.statusEl.textContent = "未绑定园区罚没指标";
      this.statusEl.className = "status error";
      if (this.hasAttribute("data-props")) {
        const meta = runtimeCallerMeta(this, "mei-cockpit-park-amount-list");
        recordRuntimeDatasetQueryError({
          kind: "component_metric_binding",
          datasetId: "__cockpit_park_amount_list__",
          message: "未绑定园区罚没指标",
          sceneId: meta.scene_id,
          target: meta.target,
          component: meta.component,
          panelId: meta.panel_id,
          phase: "metric_binding",
        });
      }
      return;
    }

    try {
      const rows = await this.fetchMetricRows(resolvedValue);
      const items = rows
        .map((row) => ({
          label: String(row?.[labelField] ?? "").trim(),
          value: Number(row?.[valueField]),
        }))
        .filter((item) => item.label)
        .sort((a, b) => b.value - a.value)
        .slice(0, limit);

      if (items.length === 0) {
        this.listEl.innerHTML = "";
        this.statusEl.textContent = "暂无园区罚没数据";
        this.statusEl.className = "status";
        return;
      }

      this.statusEl.textContent = "";
      this.listEl.innerHTML = items
        .map(
          (item) => `
        <div class="row">
          <div class="name" title="${escapeHtml(item.label)}">${escapeHtml(item.label)}</div>
          <div class="amount">
            <span class="value">${escapeHtml(formatWanYuan(item.value))}</span>
            <span class="unit">${escapeHtml(unit)}</span>
          </div>
        </div>`,
        )
        .join("");
    } catch (error) {
      if (isAbortError(error)) {
        return;
      }
      this.listEl.innerHTML = "";
      this.statusEl.textContent = String(error?.message || error);
      this.statusEl.className = "status error";
      const meta = runtimeCallerMeta(this, "mei-cockpit-park-amount-list");
      recordRuntimeDatasetQueryError({
        kind: "component_metric_query",
        datasetId: metricRef.dataset_id,
        message: String(error?.message || error || "加载失败"),
        sceneId: meta.scene_id,
        target: meta.target,
        component: meta.component,
        panelId: meta.panel_id,
        metricId: metricRef.metric_id,
        phase: "metric_fetch",
      });
    }
  }
}

if (!customElements.get("mei-cockpit-park-amount-list")) {
  customElements.define("mei-cockpit-park-amount-list", MeiCockpitParkAmountList);
}
