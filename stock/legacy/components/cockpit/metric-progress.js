import { parseProps } from "./shared.js";
import { color } from "../mei/theme-style.js";
import {
  fetchPanelRuntimeMetrics,
  fetchRuntimeMetrics,
  findRuntimeMetricInResults,
  resolveRuntimeMetricRef,
  subscribeQueryState,
} from "../dataset/runtime-query.js";

function parsePercent(raw) {
  const text = String(raw ?? "").trim();
  if (!text) return 0;
  const numeric = Number(text.replace(/%/g, "").trim());
  if (!Number.isFinite(numeric)) return 0;
  if (text.includes("%")) return Math.max(0, Math.min(1, numeric / 100));
  if (numeric <= 1) return Math.max(0, Math.min(1, numeric));
  return Math.max(0, Math.min(1, numeric / 100));
}

function progressShell(props) {
  const shell = props.progress_shell ?? props.progressShell ?? {};
  return {
    insetX: String(shell.inset_x ?? shell.insetX ?? "14px"),
    extendX: String(shell.extend_x ?? shell.extendX ?? "0px"),
    height: String(shell.height ?? "14px"),
    radius: String(shell.border_radius ?? shell.borderRadius ?? "0"),
    fill: String(shell.fill ?? shell.background ?? color("text_unit")),
  };
}

function metricRefOf(props) {
  const content = props?.content ?? props?.value;
  if (!content || typeof content !== "object" || Array.isArray(content)) {
    return null;
  }
  if (content.__mei_runtime_ref || content.__ref === "metric" || content.shape) {
    return content;
  }
  return null;
}

function scalarPercentFromMetric(metric) {
  if (!metric || metric.shape !== "scalar") return null;
  const values = metric.value && typeof metric.value === "object" ? metric.value : null;
  if (!values) return null;
  if (values.value != null) return values.value;
  if (values.desc != null) return values.desc;
  const first = Object.values(values)[0];
  return first ?? null;
}

function renderProgress(host, percent, shell) {
  if (!host.shadowRoot) {
    host.attachShadow({ mode: "open" });
  }
  const pct = Math.round(percent * 1000) / 10;
  const pctLabel = `${pct.toFixed(1)}%`;
  host.shadowRoot.innerHTML = `
    <style>
      :host {
        display: flex;
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        align-items: center;
        justify-content: center;
        box-sizing: border-box;
      }
      .wrap {
        width: calc(100% + ${shell.extendX} + ${shell.extendX});
        max-width: none;
        height: 100%;
        margin: 0 -${shell.extendX};
        padding: 0 ${shell.insetX};
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 2px;
      }
      .label {
        font-family: Microsoft YaHei, MicrosoftYaHei, PingFang SC, sans-serif;
        font-size: 12px;
        font-weight: 400;
        line-height: 1;
        color: rgba(255, 255, 255, 0.88);
        letter-spacing: 0;
      }
      .track {
        width: 100%;
        height: ${shell.height};
        background: transparent;
        border: 1px solid rgba(201, 233, 248, 0.28);
        border-radius: ${shell.radius};
        overflow: hidden;
        box-sizing: border-box;
      }
      .fill {
        width: ${Math.max(0, Math.min(100, pct))}%;
        height: 100%;
        background: ${shell.fill};
        border-radius: ${shell.radius};
      }
    </style>
    <div class="wrap">
      <div class="label">${pctLabel}</div>
      <div class="track">
        <div class="fill"></div>
      </div>
    </div>
  `;
}

class MeiCockpitMetricProgress extends HTMLElement {
  connectedCallback() {
    this._unsub = null;
    this._renderFromProps();
  }

  disconnectedCallback() {
    if (typeof this._unsub === "function") {
      this._unsub();
      this._unsub = null;
    }
  }

  _renderFromProps() {
    const props = parseProps(this);
    const shell = progressShell(props);
    const metricContent = metricRefOf(props);
    if (!metricContent) {
      const percent = parsePercent(props.value ?? props.percent);
      renderProgress(this, percent, shell);
      return;
    }

    const paint = (metric) => {
      const raw = scalarPercentFromMetric(metric);
      renderProgress(this, parsePercent(raw), shell);
    };

    const runtimeRef = resolveRuntimeMetricRef(props) || resolveRuntimeMetricRef(metricContent);
    renderProgress(this, 0, shell);
    const run = async () => {
      try {
        const results =
          (await fetchPanelRuntimeMetrics?.(this, props)) ||
          (await fetchRuntimeMetrics?.(props)) ||
          (await fetchPanelRuntimeMetrics?.(this, [metricContent])) ||
          (await fetchRuntimeMetrics?.([metricContent])) ||
          [];
        const list = Array.isArray(results)
          ? results
          : Array.isArray(results?.metrics)
            ? results.metrics
            : [];
        const metric =
          (runtimeRef && findRuntimeMetricInResults?.(list, runtimeRef)) || list[0] || null;
        if (metric) paint(metric);
      } catch {
        /* keep zero fill */
      }
    };
    run();
    if (typeof subscribeQueryState === "function") {
      this._unsub = subscribeQueryState(() => {
        run();
      });
    }
  }
}

if (!customElements.get("mei-cockpit-metric-progress")) {
  customElements.define("mei-cockpit-metric-progress", MeiCockpitMetricProgress);
}
