import {
  appendRuntimePerfDiagnostics,
  deferUntilDisplayed,
  escapeHtml,
  fetchDatasetRows,
  fetchPanelRuntimeMetrics,
  parseProps,
  queryStateIdOf,
  resolveRuntimeDataRef,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  subscribeQueryState,
} from "./runtime-query.js";
import { formatMetricNumber } from "../mei/metric-number-format.js";

class MeiDatasetSummaryCards extends HTMLElement {
  connectedCallback() {
    this._fetchAbort = new AbortController();
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    this._deferUntilVisibleCleanup = deferUntilDisplayed(this, () => {
      this._deferUntilVisibleCleanup = null;
      this.bootstrapSummaryCards();
    });
  }

  bootstrapSummaryCards() {
    this._props = parseProps(this);
    this._queryStateId = queryStateIdOf(this._props);
    this._sharedFilters = {};
    this._state = {
      loading: false,
      error: "",
      metrics: resolveInitialMetrics(this._props),
    };
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
      this._unsubscribeQueryState = null;
    }
    this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
      this._sharedFilters = state?.filters || {};
      this.refreshRuntime();
    });
    if (!this._queryStateId) {
      this.refreshRuntime();
    }
  }

  disconnectedCallback() {
    if (this._fetchAbort) {
      this._fetchAbort.abort();
      this._fetchAbort = null;
    }
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
    }
  }

  async refreshRuntime() {
    const metricRef = resolveRuntimeMetricRef(this._props);
    const dataRef = resolveRuntimeDataRef(this._props);
    if (!metricRef && !dataRef) {
      this.render();
      return;
    }
    this._state.loading = true;
    this._state.error = "";
    this.render();
    const signal = this._fetchAbort?.signal;
    const datasetId =
      metricRef?.dataset_id ||
      dataRef?.dataset_id ||
      String(this._props?.data?.id || this._props?.dataset?.id || "");
    const callerMeta = runtimeCallerMeta(this, "mei-dataset-summary-cards");
    try {
      if (metricRef) {
        const result = await fetchPanelRuntimeMetrics(this, this._props, {
          filters: this._sharedFilters,
          signal,
          meta: callerMeta,
        });
        const metric = Array.isArray(result?.metrics) ? result.metrics[0] : null;
        this._state.metrics = metric ? metricsFromScalarMetric(metric) : [];
        if (result?.perf) {
          appendRuntimePerfDiagnostics(datasetId, result.perf, runtimePerfMeta(this));
        }
      } else {
        const result = await fetchDatasetRows(this._props, {
          filters: this._sharedFilters,
          full: true,
          signal,
          meta: callerMeta,
        });
        const rows = Array.isArray(result?.rows) ? result.rows : [];
        const columns = Array.isArray(result?.columns) ? result.columns : [];
        this._state.metrics = normalizeMetrics(this._props.metrics, columns, rows);
        if (result?.perf) {
          appendRuntimePerfDiagnostics(datasetId, result.perf, runtimePerfMeta(this));
        }
      }
    } catch (error) {
      if (error?.name === "AbortError") {
        return;
      }
      this._state.error = String(error?.message || error || "runtime query failed");
      this._state.metrics = resolveInitialMetrics(this._props);
    } finally {
      this._state.loading = false;
      this.render();
    }
  }

  render() {
    const metrics = Array.isArray(this._state.metrics) ? this._state.metrics : [];
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .wrap { display: grid; gap: 10px; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: 12px; }
        .card { display: grid; gap: 6px; padding: 12px; border-radius: 14px; background: linear-gradient(150deg, rgba(15,23,42,.92), rgba(30,41,59,.75)); border: 1px solid rgba(59,130,246,.25); }
        .label { color: ${color("text_muted")}; font-size: 12px; }
        .value { color: ${color("text_inverse")}; font-size: 24px; font-weight: 800; line-height: 1.1; }
        .unit { color: ${color("text_muted")}; font-size: 12px; }
        .status { min-height: 16px; color: ${color("text_muted")}; font-size: 12px; }
        .status.error { color: ${color("status_error")}; }
      </style>
      <section class="wrap">
        <div class="status ${this._state.error ? "error" : ""}">
          ${this._state.error ? escapeHtml(this._state.error) : this._state.loading ? "loading..." : ""}
        </div>
        <section class="grid">
          ${metrics.map((metric) => `
            <article class="card">
              <div class="label">${escapeHtml(metric.label)}</div>
              <div class="value">${escapeHtml(metric.value)}</div>
              <div class="unit">${escapeHtml(metric.unit || "")}</div>
            </article>
          `).join("")}
        </section>
      </section>
    `;
  }
}

function runtimePerfMeta(element) {
  return runtimeCallerMeta(element, "mei-dataset-summary-cards");
}

function resolveScalarMetric(props) {
  const metric = props.value || props.data || null;
  if (!metric || metric.shape !== "scalar" || typeof metric.value !== "object" || metric.value === null) {
    return null;
  }
  return metricsFromScalarMetric(metric);
}

function normalizeMetrics(rawMetrics, columns, rows) {
  if (Array.isArray(rawMetrics) && rawMetrics.length > 0) {
    return rawMetrics.map((metric) => evaluateMetric(metric, rows));
  }
  const fallbackMetrics = [{ label: "总行数", op: "count", column: null }];
  const numericColumn = firstNumericColumn(columns, rows);
  if (numericColumn) {
    fallbackMetrics.push({ label: "数值总和", op: "sum", column: numericColumn });
  }
  return fallbackMetrics.map((metric) => evaluateMetric(metric, rows));
}

function firstNumericColumn(columns, rows) {
  return (columns || []).find((column) =>
    rows.some((row) => Number.isFinite(Number(row?.[column])))
  );
}

function evaluateMetric(metric, rows) {
  const label = metric?.label || "指标";
  const column = metric?.column ?? null;
  const op = metric?.op || "count";
  const unit = metric?.unit || "";
  const values = column
    ? rows
        .map((row) => Number(row?.[column]))
        .filter((value) => Number.isFinite(value))
    : [];
  let numericResult = 0;
  if (op === "sum") {
    numericResult = values.reduce((sum, value) => sum + value, 0);
  } else if (op === "avg") {
    numericResult = values.length === 0 ? 0 : values.reduce((sum, value) => sum + value, 0) / values.length;
  } else if (op === "max") {
    numericResult = values.length === 0 ? 0 : Math.max(...values);
  } else if (op === "min") {
    numericResult = values.length === 0 ? 0 : Math.min(...values);
  } else {
    numericResult = rows.length;
  }
  const value = op === "avg" ? numericResult.toFixed(1) : String(Math.round(numericResult));
  return {
    label,
    value,
    unit: unit || "",
  };
}

function formatMetricValue(value, unit = "", format = null) {
  return formatMetricNumber(value, { unit, format });
}

function resolveInitialMetrics(props) {
  const scalarMetric = resolveScalarMetric(props);
  const dataset = props.data?.rows ? props.data : props.dataset?.dataset || props.dataset || {};
  const rows = Array.isArray(dataset.rows) ? dataset.rows : [];
  return scalarMetric || normalizeMetrics(props.metrics, dataset.columns || [], rows);
}

function metricsFromScalarMetric(metric) {
  const schema = Array.isArray(metric.schema) ? metric.schema : [];
  const units = new Map(
    schema
      .filter((column) => column && typeof column.name === "string")
      .map((column) => [column.name, column.unit || ""])
  );
  const entries = Object.entries(metric.value || {});
  const metricLabel =
    typeof metric.label === "string" && metric.label.trim() ? metric.label.trim() : "";
  const metricUnit =
    typeof metric.unit === "string" && metric.unit.trim() ? metric.unit.trim() : "";
  if (entries.length === 1) {
    const [key, rawValue] = entries[0];
    const colUnit = units.get(key) || "";
    const unit = metricUnit || colUnit;
    const valueFormat = metric.value_format ?? metric.valueFormat ?? null;
    const value = formatMetricValue(rawValue, unit, valueFormat);
    return [
      {
        label: metricLabel || key,
        value,
        unit,
      },
    ];
  }
  const valueFormat = metric.value_format ?? metric.valueFormat ?? null;
  return entries.map(([key, rawValue]) => {
    const colUnit = units.get(key) || "";
    const unit = colUnit || metricUnit;
    const value = formatMetricValue(rawValue, unit, valueFormat);
    return {
      label: key,
      value,
      unit,
    };
  });
}

customElements.define("mei-dataset-summary-cards", MeiDatasetSummaryCards);
