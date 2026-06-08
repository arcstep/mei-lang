import {
  deferUntilDisplayed,
  fetchPanelRuntimeMetrics,
  parseProps,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  subscribeQueryState,
} from "../dataset/runtime-query.js";
import { escapeHtml } from "./shared.js";
import { QUNFU_FONT, qunfuCssVars } from "./tokens.js";

function metricRows(metric) {
  if (!metric || typeof metric !== "object") return [];
  if (Array.isArray(metric.rows)) return metric.rows;
  if (Array.isArray(metric.value)) return metric.value;
  return [];
}

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
  }

  renderShell() {
    const h = Number(this._props?.height) > 0 ? Number(this._props.height) : 104;
    const compact = this._props?.compact === true || this._props?.compact === "true";
    const listPad = compact ? "2px 2px 0" : "4px 2px 2px";
    const listLayout = compact ? "flex-start" : "space-between";
    const listGap = compact ? "2px" : "0";
    const rowPad = compact ? "3px 2px 4px" : "6px 4px 8px";
    const rowGap = compact ? "6px" : "8px";
    const nameSize = compact ? "11px" : "12px";
    const nameLh = compact ? "1.2" : "1.35";
    const valueSize = compact ? "13px" : "15px";
    const unitSize = compact ? "10px" : "11px";
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          min-width: 0;
          height: ${h}px;
          min-height: ${h}px;
          font-family: ${QUNFU_FONT.uiFamily};
          ${qunfuCssVars()}
        }
        .list {
          display: flex;
          flex-direction: column;
          justify-content: ${listLayout};
          gap: ${listGap};
          height: 100%;
          min-height: ${h}px;
          padding: ${listPad};
          box-sizing: border-box;
        }
        .row {
          display: grid;
          grid-template-columns: minmax(0, 1fr) auto;
          align-items: baseline;
          gap: ${rowGap};
          padding: ${rowPad};
          border-bottom: 1px solid rgba(52, 82, 108, 0.45);
        }
        .row:last-child {
          border-bottom: none;
          padding-bottom: ${compact ? "2px" : "2px"};
        }
        .name {
          font-size: ${nameSize};
          line-height: ${nameLh};
          color: #cbd5e1;
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
          color: #e8f4ff;
          line-height: 1;
        }
        .unit {
          font-size: ${unitSize};
          color: #94a3b8;
          font-weight: 600;
        }
        .status {
          font-size: 10px;
          color: #64748b;
          text-align: center;
          padding: 12px 0;
        }
        .status.error { color: #fca5a5; }
      </style>
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
    const result = await fetchPanelRuntimeMetrics(this, lineProps, {
      filters: this._sharedFilters,
      metricIds: [metricRef.metric_id].filter(Boolean),
      meta: runtimeCallerMeta(this, "mei-cockpit-park-amount-list"),
    });
    const metrics = Array.isArray(result?.metrics) ? result.metrics : [];
    const metric = metrics.find((m) => m.id === metricRef.metric_id) || metrics[0];
    return metricRows(metric);
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
      this.listEl.innerHTML = "";
      this.statusEl.textContent = String(error?.message || error);
      this.statusEl.className = "status error";
    }
  }
}

if (!customElements.get("mei-cockpit-park-amount-list")) {
  customElements.define("mei-cockpit-park-amount-list", MeiCockpitParkAmountList);
}
