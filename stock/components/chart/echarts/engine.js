import {
  deferUntilDisplayed,
  shouldReactToPreviewUpdated,
  fetchDatasetRows,
  fetchPanelRuntimeMetrics,
  findRuntimeMetricInResults,
  parseProps,
  queryStateIdOf,
  resolveRuntimeDataRef,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  setQueryStateFilter,
  subscribeQueryState,
} from "../../dataset/runtime-query.js";
import { createComponentTracer } from "../../perf/render-trace.js";
import { cockpitCssVars, COCKPIT_Z_INDEX, readThemeTypography } from "../../cockpit/tokens.js";
import {
  bindFloatingPopoverDrag,
  buildTextPopoverBodyHtml,
  copyTextToClipboard,
  ensureFloatingTextPopoverStyles,
  mountFloatingPopoverOnBody,
  positionFloatingPopoverNearAnchor,
} from "../../mei/floating-text-popover.js";
import { ensureEChartsGlobal } from "../../vendor/runtime-libs.js";
const CARTESIAN_KINDS = new Set(["line", "area", "trend", "column", "bar", "scatter"]);
const PIE_KINDS = new Set(["pie", "donut", "rose"]);

function isAbortError(error) {
  if (!error) return false;
  if (error.name === "AbortError") return true;
  const msg = String(error.message || error || "");
  return msg.includes("aborted") || msg.includes("AbortError");
}

function pickRuntimeMetricFromResult(result, metricRef) {
  const metrics = Array.isArray(result?.metrics) ? result.metrics : [];
  if (!metricRef?.metric_id) {
    return metrics[0] || null;
  }
  return findRuntimeMetricInResults(metrics, metricRef);
}

function chartPropsNeedRuntimeFetch(props) {
  return Boolean(resolveRuntimeMetricRef(props) || resolveRuntimeDataRef(props));
}

function chartPropsHaveInitialRows(props) {
  return resolveRows(props).length > 0;
}

function isEChartsInstanceAlive(chart, chartEl) {
  if (!chart || !chartEl) return false;
  try {
    if (typeof chart.isDisposed === "function" && chart.isDisposed()) {
      return false;
    }
    const dom = typeof chart.getDom === "function" ? chart.getDom() : null;
    return Boolean(dom && dom === chartEl && dom.isConnected);
  } catch {
    return false;
  }
}

function releaseChartSurface(chartEl, chart) {
  if (chart) {
    try {
      chart.dispose();
    } catch (_) {
      /* ignore */
    }
  }
  if (chartEl) {
    chartEl.innerHTML = "";
  }
}

export function defineChartElement(tagName, chartKind, defaultTitle) {
  if (customElements.get(tagName)) {
    return;
  }
  class MeiChartElement extends HTMLElement {
    connectedCallback() {
      if (typeof this._deferUntilVisibleCleanup === "function") {
        this._deferUntilVisibleCleanup();
        this._deferUntilVisibleCleanup = null;
      }
      this._deferUntilVisibleCleanup = deferUntilDisplayed(this, () => {
        this._deferUntilVisibleCleanup = null;
        this.bootstrapChartElement();
      });
    }

    bootstrapChartElement() {
      if (!this.shadowRoot) {
        this.attachShadow({ mode: "open" });
      }
      const bootProps = parseProps(this);
      this.shadowRoot.innerHTML = chartShellHtml(defaultTitle, bootProps);
      this.chartEl = this.shadowRoot.querySelector(".chart");
      this.metaEl = this.shadowRoot.querySelector(".meta");
      this.errorEl = this.shadowRoot.querySelector(".error");
      this._props = parseProps(this);
      this._runtimeProps = null;
      this._sharedFilters = {};
      this._renderTrace = createComponentTracer(this, tagName, {
        chart_kind: chartKind,
      });
      this.refresh = () => {
        this._props = parseProps(this);
        const needsRuntime = chartPropsNeedRuntimeFetch(this._props);
        if (needsRuntime && !this._queryStateId) {
          void this.refreshRuntimeData();
          return;
        }
        this.renderChart();
        if (!this._queryStateId && needsRuntime) {
          void this.refreshRuntimeData();
        }
      };
      this._onPreviewUpdated = (event) => {
        if (!shouldReactToPreviewUpdated(event, this)) {
          return;
        }
        this.refresh();
      };
      window.addEventListener("meilang:preview-updated", this._onPreviewUpdated);
      this.resizeObserver = new ResizeObserver(() => {
        if (this.chart) {
          this.chart.resize();
        }
      });
      this.resizeObserver.observe(this);
      this._queryStateId = queryStateIdOf(this._props);
      this._renderTrace.mark("bootstrap", {
        query_state_id: this._queryStateId || "",
        needs_runtime: chartPropsNeedRuntimeFetch(this._props),
      });
      const needsRuntime = chartPropsNeedRuntimeFetch(this._props);
      if (!needsRuntime || chartPropsHaveInitialRows(this._props)) {
        this.renderChart();
      }
      this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
        this._sharedFilters = state?.filters || {};
        this.refreshRuntimeData();
      });
      if (!this._queryStateId && needsRuntime) {
        void this.refreshRuntimeData();
      }
    }

    disconnectedCallback() {
      this.closeLabelPopover();
      if (typeof this._deferUntilVisibleCleanup === "function") {
        this._deferUntilVisibleCleanup();
        this._deferUntilVisibleCleanup = null;
      }
      if (typeof this._onPreviewUpdated === "function") {
        window.removeEventListener("meilang:preview-updated", this._onPreviewUpdated);
      } else if (typeof this.refresh === "function") {
        window.removeEventListener("meilang:preview-updated", this.refresh);
      }
      if (typeof this._unsubscribeQueryState === "function") {
        this._unsubscribeQueryState();
      }
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
      }
      if (this.chart) {
        this.chart.dispose();
        this.chart = null;
      }
    }

    async renderChart() {
      const props = Object.assign({}, parseProps(this), this._runtimeProps || {}, { __host: this });
      this._renderTrace?.mark("render_start", {
        has_runtime_props: Boolean(this._runtimeProps),
      });
      const diagnostics = [];
      const model = buildChartModel(chartKind, props, diagnostics);
      this._selectionContext = resolveChartSelectionContext(chartKind, props, model);
      this.metaEl.textContent = model.meta;
      if (diagnostics.length > 0) {
        this.errorEl.textContent = diagnostics.join(" | ");
      } else {
        this.errorEl.textContent = "";
      }
      if (chartKind === "ranking" && model.layout === "above") {
        releaseChartSurface(this.chartEl, this.chart);
        this.chart = null;
        this._rankingFullLabels = Array.isArray(model.fullLabels) ? model.fullLabels : [];
        const pullUp = Math.max(0, Number(props.rankingPullUp ?? props.ranking_pull_up ?? 0));
        if (pullUp > 0) {
          this.style.overflow = "visible";
        } else {
          this.style.removeProperty("overflow");
        }
        renderRankingAboveDom(this.chartEl, model, props, (fullText, event) => {
          this.openLabelPopover(fullText, event);
        });
        return;
      }
      try {
        const renderSeq = (this._renderSeq = (this._renderSeq || 0) + 1);
        const hadDomRanking = Boolean(this.chartEl?.querySelector?.(".mei-rank-above"));
        const canReuse =
          !hadDomRanking && isEChartsInstanceAlive(this.chart, this.chartEl);
        if (!canReuse) {
          releaseChartSurface(this.chartEl, this.chart);
          this.chart = null;
        }
        this._renderTrace?.mark("echarts_load_start");
        const echarts = await ensureECharts();
        this._renderTrace?.mark("echarts_load_done");
        if (renderSeq !== this._renderSeq) {
          return;
        }
        if (!this.chart) {
          this.chart = echarts.init(this.chartEl);
        }
        this.chart.setOption(model.option, true);
        try {
          this.chart.resize();
        } catch (_) {
          /* ignore */
        }
        if (chartKind === "ranking") {
          this._rankingFullLabels = Array.isArray(model.fullLabels) ? model.fullLabels : [];
          const shellH = resolveRankingShellHeight(props, model.rowCount);
          if (model.rowCount > 0 && shellH > 0) {
            this.chartEl.style.minHeight = `${shellH}px`;
            if (isRankingCompact(props) && Number(props.chartHeight) > 0) {
              this.chartEl.style.height = `${shellH}px`;
              this.chartEl.style.maxHeight = `${shellH}px`;
            }
          } else {
            this.chartEl.style.minHeight = "";
            this.chartEl.style.height = "";
            this.chartEl.style.maxHeight = "";
          }
          this.setupRankingChartInteractions();
        } else {
          this._rankingFullLabels = null;
          this.setupSelectionInteractions(chartKind);
        }
        this._renderTrace?.mark("render_done", {
          rows: Array.isArray(model.rows) ? model.rows.length : 0,
        });
      } catch (error) {
        this.errorEl.textContent = "图表引擎加载失败: " + String(error?.message || error);
        this._renderTrace?.mark("render_error", {
          message: String(error?.message || error),
        });
      }
    }

    setupRankingChartInteractions() {
      if (!this.chart) return;
      this.chart.off("click");
      this.chart.on("click", (params) => {
        if (params.componentType !== "series") return;
        const full = this._rankingFullLabels?.[params.dataIndex];
        if (!full) return;
        this.openLabelPopover(full, params.event?.event);
      });
    }

    setupSelectionInteractions(chartKind) {
      if (!this.chart) return;
      this.chart.off("click");
      const selection = this._selectionContext;
      if (!selection?.queryStateId || !selection?.dimension) return;
      if (normalizeKind(chartKind) === "ranking") return;
      this.chart.on("click", (params) => {
        const selectedValue = resolveChartSelectionValue(chartKind, params, selection);
        if (!selectedValue) return;
        setQueryStateFilter(selection.queryStateId, selection.dimension, selectedValue, {
          filterIntentSource: "chart_selection",
          transitionSource: "chart_selection",
          toggle: selection.toggle,
        });
      });
    }

    openLabelPopover(fullText, anchorEvent) {
      this.closeLabelPopover();
      ensureFloatingTextPopoverStyles();
      const pop = document.createElement("div");
      pop.className = "cell-pop cell-pop--large";
      pop.setAttribute("role", "dialog");
      pop.setAttribute("aria-modal", "true");
      pop.setAttribute("aria-label", "完整名称");
      pop.innerHTML = `
        <div class="cell-pop-hd">
          <div class="cell-pop-title"><span>完整名称</span></div>
          <div class="cell-pop-actions">
            <button type="button" class="cell-pop-copy">复制</button>
            <button type="button" class="cell-pop-done">关闭</button>
            <button type="button" class="cell-pop-close" aria-label="关闭">×</button>
          </div>
        </div>
        ${buildTextPopoverBodyHtml(fullText, escapeHtml)}
      `;
      mountFloatingPopoverOnBody(pop, { width: 480, height: 340 });
      this._labelPopoverEl = pop;
      const anchor = anchorEvent?.target || anchorEvent;
      positionFloatingPopoverNearAnchor(pop, anchor, {
        topOffset: 8,
        defaultWidth: 480,
        defaultHeight: 340,
      });
      this._labelPopoverDragCleanup = bindFloatingPopoverDrag(
        pop,
        pop.querySelector(".cell-pop-hd"),
      );

      const onDoc = (ev) => {
        const path = ev.composedPath();
        if (path.includes(pop) || (anchor && path.includes(anchor))) return;
        this.closeLabelPopover();
      };
      setTimeout(() => document.addEventListener("pointerdown", onDoc, true), 0);
      this._labelPopoverDocCleanup = () => document.removeEventListener("pointerdown", onDoc, true);
      this._labelPopoverKeydown = (ev) => {
        if (ev.key === "Escape") {
          ev.stopPropagation();
          this.closeLabelPopover();
        }
      };
      document.addEventListener("keydown", this._labelPopoverKeydown, true);
      const close = () => this.closeLabelPopover();
      pop.querySelector(".cell-pop-close")?.addEventListener("click", close);
      pop.querySelector(".cell-pop-done")?.addEventListener("click", close);
      pop.querySelector(".cell-pop-copy")?.addEventListener("click", () => {
        copyTextToClipboard(fullText);
      });
      pop.querySelector(".cell-pop-done")?.focus();
    }

    closeLabelPopover() {
      if (typeof this._labelPopoverDragCleanup === "function") {
        this._labelPopoverDragCleanup();
        this._labelPopoverDragCleanup = null;
      }
      if (typeof this._labelPopoverDocCleanup === "function") {
        this._labelPopoverDocCleanup();
        this._labelPopoverDocCleanup = null;
      }
      if (typeof this._labelPopoverKeydown === "function") {
        document.removeEventListener("keydown", this._labelPopoverKeydown, true);
        this._labelPopoverKeydown = null;
      }
      if (this._labelPopoverEl) {
        this._labelPopoverEl.remove();
        this._labelPopoverEl = null;
      }
    }

    async refreshRuntimeData() {
      const props = this._props || parseProps(this);
      const metricRef = resolveRuntimeMetricRef(props);
      const dataRef = resolveRuntimeDataRef(props);
      if (!metricRef && !dataRef) {
        this._runtimeProps = null;
        this.renderChart();
        return;
      }
      try {
        this._renderTrace?.mark("runtime_query_start", {
          mode: metricRef ? "metric" : "dataset",
        });
        if (metricRef) {
          const metricId = String(metricRef?.metric_id || metricRef?.metricId || "").trim();
          const supportRole = String(props?.supportRole ?? props?.support_role ?? "").trim().toLowerCase();
          const dedicatedExplain =
            (metricId.includes("::") && !metricId.endsWith("::__scalar_rowset__")) ||
            supportRole === "composition" ||
            supportRole === "trend" ||
            supportRole === "attribution" ||
            (!metricId.includes("::") && /composition|trend|breakdown|attribution/i.test(metricId));
          const topN = Number(props?.top_n ?? props?.topN ?? 0);
          const pageSize = dedicatedExplain
            ? topN > 0
              ? Math.max(topN, 16)
              : 64
            : Number(props?.pageSize ?? props?.page_size ?? 0) > 0
              ? Number(props?.pageSize ?? props?.page_size)
              : 20;
          const rowsResult = await fetchDatasetRows(props, {
            queryStateId: this._queryStateId,
            filters: this._sharedFilters,
            page: 1,
            pageSize,
            full: false,
            meta: runtimeCallerMeta(this, tagName),
          });
          if (Array.isArray(rowsResult?.rows)) {
            const dataset = resolveDatasetSource(props);
            this._runtimeProps = {
              data: {
                ...dataset,
                columns: Array.isArray(rowsResult.columns) ? rowsResult.columns : dataset.columns || [],
                rows: rowsResult.rows,
              },
            };
            this._renderTrace?.mark("runtime_query_done", {
              mode: "dataset_metric",
              row_count: rowsResult.rows.length,
              client_total_ms: rowsResult?.perf?.client_total_ms ?? "",
              server_total_ms:
                rowsResult?.perf?.server_handler_total_ms ?? rowsResult?.perf?.total_ms ?? "",
            });
            await this.renderChart();
            return;
          }
          const result = await fetchPanelRuntimeMetrics(this, props, {
            filters: this._sharedFilters,
            meta: runtimeCallerMeta(this, tagName),
          });
          const metric = pickRuntimeMetricFromResult(result, metricRef);
          this._runtimeProps = metric
            ? props.value?.__mei_runtime_ref
              ? { value: metric }
              : { data: metric }
            : null;
          this._renderTrace?.mark("runtime_query_done", {
            mode: "metric",
            metric_count: Array.isArray(result?.metrics) ? result.metrics.length : 0,
            client_total_ms: result?.perf?.client_total_ms ?? "",
            server_total_ms: result?.perf?.server_handler_total_ms ?? result?.perf?.total_ms ?? "",
          });
        } else {
          const result = await fetchDatasetRows(props, {
            queryStateId: this._queryStateId,
            filters: this._sharedFilters,
            page: 1,
            pageSize: Number(props?.pageSize ?? props?.page_size ?? 0) > 0
              ? Number(props?.pageSize ?? props?.page_size)
              : 20,
            full: false,
            meta: runtimeCallerMeta(this, tagName),
          });
          const dataset = resolveDatasetSource(props);
          this._runtimeProps = {
            data: {
              ...dataset,
              columns: Array.isArray(result?.columns) ? result.columns : dataset.columns || [],
              rows: Array.isArray(result?.rows) ? result.rows : [],
            },
          };
          this._renderTrace?.mark("runtime_query_done", {
            mode: "dataset",
            row_count: Array.isArray(result?.rows) ? result.rows.length : 0,
            client_total_ms: result?.perf?.client_total_ms ?? "",
            server_total_ms: result?.perf?.server_handler_total_ms ?? result?.perf?.total_ms ?? "",
          });
        }
        await this.renderChart();
      } catch (error) {
        if (isAbortError(error)) return;
        this.errorEl.textContent = "运行时查询失败: " + String(error?.message || error);
        this._renderTrace?.mark("runtime_query_error", {
          message: String(error?.message || error),
        });
      }
    }
  }
  customElements.define(tagName, MeiChartElement);
}

function chartShellHtml(defaultTitle, props = {}) {
  const compact = props.compact === true || props.compact === "true";
  const chartHeight = Number(props.chartHeight) > 0 ? Number(props.chartHeight) : compact ? 64 : 260;
  const showHead = !compact && String(props.title ?? defaultTitle).trim().length > 0;
  return `
    <style>
      :host {
        display: block;
        width: 100%;
        min-width: 0;
        overflow: hidden;
        ${cockpitCssVars()}
      }
      .wrap {
        display: grid;
        gap: ${compact ? "0" : "8px"};
        padding: ${compact ? "0" : "14px"};
        border-radius: ${compact ? "0" : "14px"};
        border: ${compact ? "none" : "1px solid rgba(148,163,184,.2)"};
        background: ${compact ? "transparent" : "rgba(15,23,42,.64)"};
      }
      .head {
        display: ${showHead ? "flex" : "none"};
        justify-content: space-between;
        gap: 8px;
        align-items: baseline;
        color: #e2e8f0;
      }
      .title { margin: 0; font-size: var(--cockpit-font-chart-title); font-weight: 600; color: #f8fafc; }
      .meta { font-size: var(--cockpit-font-unit); color: #94a3b8; }
      .chart {
        width: 100%;
        min-height: ${chartHeight}px;
        height: ${compact ? chartHeight + "px" : "auto"};
        max-height: ${compact ? chartHeight + "px" : "none"};
        overflow: hidden;
        box-sizing: border-box;
      }
      .mei-rank-above {
        display: flex;
        flex-direction: column;
        height: 100%;
        min-height: 0;
        box-sizing: border-box;
        overflow: hidden;
      }
      .mei-rank-above-title {
        flex: 0 0 auto;
        margin: 0 0 2px;
        font-size: var(--cockpit-font-chart-title);
        font-weight: 600;
        color: #94a3b8;
        line-height: 1.1;
      }
      .mei-rank-above-list {
        flex: 1 1 0;
        min-height: 0;
        display: flex;
        flex-direction: column;
        justify-content: space-between;
        gap: 1px;
      }
      .mei-rank-above-row {
        flex: 1 1 0;
        min-height: 0;
        display: flex;
        flex-direction: column;
        justify-content: center;
        gap: 1px;
        cursor: pointer;
      }
      .mei-rank-above-head {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        gap: 8px;
        min-width: 0;
      }
      .mei-rank-above-label {
        flex: 1 1 0;
        min-width: 0;
        font-size: var(--cockpit-font-label);
        line-height: 1.2;
        font-weight: 500;
        color: #e2e8f0;
        display: block;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
      }
      .mei-rank-above-value {
        flex: 0 0 auto;
        font-size: var(--cockpit-font-label);
        font-weight: 600;
        color: #7dd3fc;
      }
      .mei-rank-above-track {
        position: relative;
        width: 100%;
        height: 5px;
        border-radius: 4px;
        overflow: hidden;
        background: rgba(148, 163, 184, 0.14);
        border: 1px solid rgba(100, 116, 139, 0.35);
        box-sizing: border-box;
      }
      .mei-rank-above-fill {
        position: absolute;
        left: 0;
        top: 0;
        height: 100%;
        min-width: 3px;
        border-radius: 3px;
        background: #38bdf8;
        box-shadow: 0 1px 5px rgba(56, 189, 248, 0.35);
        z-index: 1;
        pointer-events: none;
      }
      .error { min-height: ${compact ? "0" : "18px"}; font-size: var(--cockpit-font-unit); color: #fca5a5; }
    </style>
    <section class="wrap">
      <div class="head">
        <h4 class="title">${escapeHtml(String(props.title ?? defaultTitle))}</h4>
        <span class="meta"></span>
      </div>
      <div class="chart"></div>
      <div class="error"></div>
    </section>
  `;
}

async function ensureECharts() {
  return ensureEChartsGlobal();
}

function resolveTopN(props) {
  const raw = props?.top_n ?? props?.topN ?? props?.composition_top_n ?? props?.compositionTopN;
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : 0;
}

function limitCartesianRowsByTopY(rows, mapping, topN) {
  const limit = Number(topN);
  if (!Number.isFinite(limit) || limit <= 0 || !Array.isArray(rows) || rows.length === 0) {
    return rows;
  }
  const xField = mapping.x[0]?.field;
  const yFields = mapping.y.map((item) => item?.field).filter(Boolean);
  if (!xField || yFields.length === 0) {
    return rows;
  }
  const ranked = new Map();
  rows.forEach((row) => {
    const label = String(row?.[xField] ?? "").trim();
    if (!label) return;
    const total = yFields.reduce((sum, field) => {
      const value = toNumber(row?.[field]);
      return Number.isFinite(value) ? sum + value : sum;
    }, 0);
    if (!Number.isFinite(total)) return;
    ranked.set(label, (ranked.get(label) || 0) + total);
  });
  const keep = new Set(
    Array.from(ranked.entries())
      .sort((left, right) => right[1] - left[1])
      .slice(0, limit)
      .map(([label]) => label),
  );
  const categoryOrder = Array.from(ranked.entries())
    .sort((left, right) => right[1] - left[1])
    .slice(0, limit)
    .map(([label]) => label);
  const filtered = rows.filter((row) => keep.has(String(row?.[xField] ?? "").trim()));
  return reorderRowsByCategoryOrder(filtered, xField, categoryOrder);
}

function orderCartesianCategories(rows, mapping, yFields) {
  const xField = mapping.x[0]?.field;
  if (!xField || !Array.isArray(rows) || rows.length === 0) return [];
  const fields =
    Array.isArray(yFields) && yFields.length > 0
      ? yFields
      : mapping.y.map((item) => item?.field).filter(Boolean);
  if (fields.length === 0) return [];
  const totals = new Map();
  rows.forEach((row) => {
    const label = String(row?.[xField] ?? "").trim();
    if (!label) return;
    const total = fields.reduce((sum, field) => {
      const value = toNumber(row?.[field]);
      return Number.isFinite(value) ? sum + value : sum;
    }, 0);
    totals.set(label, (totals.get(label) || 0) + total);
  });
  return Array.from(totals.entries())
    .sort((left, right) => right[1] - left[1])
    .map(([label]) => label);
}

function reorderRowsByCategoryOrder(rows, xField, categoryOrder) {
  if (!xField || !Array.isArray(categoryOrder) || categoryOrder.length === 0) {
    return rows;
  }
  const rank = new Map(categoryOrder.map((label, index) => [label, index]));
  return [...rows].sort((left, right) => {
    const leftRank = rank.get(String(left?.[xField] ?? "").trim());
    const rightRank = rank.get(String(right?.[xField] ?? "").trim());
    if (leftRank === undefined && rightRank === undefined) return 0;
    if (leftRank === undefined) return 1;
    if (rightRank === undefined) return -1;
    return leftRank - rightRank;
  });
}

function buildChartModel(kind, props, diagnostics) {
  const rows = resolveRows(props);
  const columns = resolveColumns(props, rows);
  const mapping = resolveMapping(props, columns);
  const legacy = Object.assign(resolveLegacyBehavior(props), { __host: props.__host });
  const normalized = normalizeKind(kind);
  const topN = resolveTopN(props);
  const chartRows =
    topN > 0 && normalized === "column"
      ? limitCartesianRowsByTopY(rows, mapping, topN)
      : rows;
  if (topN > 0 && normalized === "column") {
    legacy.sortCategoriesByYTotal = true;
  }
  if (normalized === "ranking") {
    const layout = resolveRankingLayout(props);
    if (layout === "above") {
      const { items, valueName } = buildRankingItems(chartRows, mapping, diagnostics);
      const configuredMaxChars = resolveRankingLabelMaxChars(props, "above");
      const maxChars = configuredMaxChars > 0 ? configuredMaxChars : 20;
      return {
        kind: normalized,
        layout: "above",
        rows: chartRows,
        mapping,
        items,
        valueName,
        theme: resolveRankingTheme(props),
        maxChars,
        meta: `排名 ${items.length} 项 · 标签置顶（点击查看全文）`,
        fullLabels: items.map((item) => item.label),
        rowCount: items.length,
      };
    }
    const ranking = buildRankingSideOption(chartRows, mapping, props, diagnostics);
    return {
      kind: normalized,
      layout: "side",
      rows: chartRows,
      mapping,
      option: ranking.option,
      meta: ranking.meta,
      fullLabels: ranking.fullLabels,
      rowCount: ranking.rowCount,
    };
  }
  const option = buildOption(kind, chartRows, mapping, legacy, diagnostics);
  return {
    kind: normalized,
    rows: chartRows,
    mapping,
    option,
    meta: `${mapping.titleLeft} -> ${mapping.titleRight}`,
  };
}

function firstNonEmptyString(...values) {
  for (const value of values) {
    const text = String(value || "").trim();
    if (text) return text;
  }
  return "";
}

function resolveChartSelectionContext(kind, props, model) {
  const normalizedKind = normalizeKind(kind);
  const queryStateId = queryStateIdOf(props);
  if (!queryStateId) return null;
  const mapping = model?.mapping || {};
  const selectionDimension = firstNonEmptyString(
    props.selection_dimension,
    props.selectionDimension,
    normalizedKind === "pie" || normalizedKind === "donut" || normalizedKind === "rose"
      ? mapping.label?.[0]?.field || mapping.x?.[0]?.field
      : normalizedKind === "radar" || normalizedKind === "boxplot" || normalizedKind === "ranking"
        ? mapping.label?.[0]?.field || mapping.x?.[0]?.field
        : normalizedKind === "scatter"
          ? mapping.label?.[0]?.field
          : mapping.x?.[0]?.field
  );
  if (!selectionDimension) return null;
  return {
    queryStateId,
    dimension: selectionDimension,
    rows: Array.isArray(model?.rows) ? model.rows : [],
    mapping,
    toggle: props.selection_toggle !== false && props.selectionToggle !== false,
  };
}

function resolveChartSelectionValue(kind, params, selection) {
  const normalizedKind = normalizeKind(kind);
  if (normalizedKind === "scatter") {
    const labelField = selection?.mapping?.label?.[0]?.field;
    const row = Number.isFinite(Number(params?.dataIndex)) ? selection?.rows?.[params.dataIndex] : null;
    return firstNonEmptyString(labelField ? row?.[labelField] : "", params?.name);
  }
  if (normalizedKind === "pie" || normalizedKind === "donut" || normalizedKind === "rose") {
    return firstNonEmptyString(params?.name, params?.data?.name);
  }
  if (normalizedKind === "radar" || normalizedKind === "boxplot") {
    return firstNonEmptyString(params?.name, params?.axisValueLabel, params?.axisValue);
  }
  return firstNonEmptyString(params?.name, params?.axisValueLabel, params?.axisValue, params?.data?.name);
}

function resolveRows(props) {
  const candidates = [props.data, props.value];
  for (const source of candidates) {
    if (!source || typeof source !== "object") continue;
    if (Array.isArray(source.rows)) return source.rows;
    if (Array.isArray(source.value)) return source.value;
    if (
      source.shape === "dataframe" &&
      source.value &&
      typeof source.value === "object" &&
      Array.isArray(source.value.rows)
    ) {
      return source.value.rows;
    }
    if (source.shape === "scalar" && source.value && typeof source.value === "object") {
      return Object.entries(source.value).map(([label, value]) => ({
        label,
        value,
      }));
    }
  }
  const dataset = props.dataset?.dataset || props.dataset || {};
  if (Array.isArray(dataset.rows)) {
    return dataset.rows;
  }
  return [];
}

function resolveColumns(props, rows) {
  const candidates = [props.data, props.value, props.dataset];
  for (const source of candidates) {
    if (!source || typeof source !== "object") continue;
    if (Array.isArray(source.schema) && source.schema.length > 0) {
      return source.schema.map((col) => col?.name).filter(Boolean);
    }
    if (Array.isArray(source.columns) && source.columns.length > 0) {
      return source.columns;
    }
    if (source.dataset && Array.isArray(source.dataset.columns)) {
      return source.dataset.columns;
    }
  }
  if (rows.length > 0 && rows[0] && typeof rows[0] === "object") {
    return Object.keys(rows[0]);
  }
  return ["label", "value"];
}

function resolveMapping(props, columns) {
  const mapping = props.mapping || {};
  const x = channelList(mapping.x, props.labelField, columns[0] || "label");
  let y = channelList(mapping.y, props.valueField, columns[1] || "value");
  if (y.length === 0 && Array.isArray(props.metrics) && props.metrics.length > 0) {
    y = props.metrics
      .map((field) => String(field || "").trim())
      .filter(Boolean)
      .map((field) => ({ field, name: field }));
  }
  const label = channelList(mapping.label, null, x[0]?.field || "label");
  const group = channelList(mapping.group, null, "");
  const color = channelList(mapping.color, null, "");
  const shape = channelList(mapping.shape, null, "");
  const size = channelList(mapping.size, null, "");
  let boxplot = channelList(mapping.boxplot, null, "");
  if (boxplot.length === 0 && Array.isArray(props.boxplot)) {
    boxplot = props.boxplot
      .map((field) => String(field || "").trim())
      .filter(Boolean)
      .map((field) => ({ field, name: field }));
  }
  const radarDimensions = Array.isArray(props.dimensions)
    ? props.dimensions.map((field) => ({ field, name: field }))
    : [];
  return {
    x,
    y,
    label,
    group,
    color,
    shape,
    size,
    boxplot,
    radarDimensions,
    titleLeft: x[0]?.name || label[0]?.name || "x",
    titleRight: y[0]?.name || "y",
  };
}

function metricSparkBarItemStyle() {
  return {
    color: {
      type: "linear",
      x: 0,
      y: 0,
      x2: 0,
      y2: 1,
      colorStops: [
        { offset: 0.31, color: "#FFFFFF" },
        { offset: 0.83, color: "#12B0FF" },
      ],
    },
  };
}

/** 驾驶舱年度对比分组柱：深蓝 + 荧光浅蓝竖向渐变 */
function cockpitYearDuoBarItemStyle(seriesIndex, { emphasis = false } = {}) {
  const presets = [
    {
      color: {
        type: "linear",
        x: 0,
        y: 0,
        x2: 0,
        y2: 1,
        colorStops: [
          { offset: 0, color: emphasis ? "#6EC8FF" : "#4AB8FF" },
          { offset: 0.38, color: emphasis ? "#1E78E8" : "#1565C8" },
          { offset: 1, color: emphasis ? "#0C3A78" : "#082E5E" },
        ],
      },
      borderRadius: [4, 4, 0, 0],
      shadowBlur: emphasis ? 14 : 9,
      shadowColor: "rgba(30, 120, 232, 0.58)",
      shadowOffsetY: 1,
    },
    {
      color: {
        type: "linear",
        x: 0,
        y: 0,
        x2: 0,
        y2: 1,
        colorStops: [
          { offset: 0, color: emphasis ? "#FFFFFF" : "#F0FCFF" },
          { offset: 0.35, color: emphasis ? "#8AEEFF" : "#6FE4FF" },
          { offset: 1, color: emphasis ? "#22C8F5" : "#12B8F5" },
        ],
      },
      borderRadius: [4, 4, 0, 0],
      shadowBlur: emphasis ? 16 : 11,
      shadowColor: "rgba(111, 228, 255, 0.55)",
      shadowOffsetY: 1,
    },
  ];
  return presets[seriesIndex % presets.length];
}

function resolveColorPalette(props) {
  const raw = props?.palette ?? props?.color_palette ?? props?.colors;
  if (Array.isArray(raw)) {
    return raw.map((item) => String(item || "").trim()).filter(Boolean);
  }
  if (typeof raw === "string" && raw.trim()) {
    return raw
      .split(",")
      .map((item) => String(item || "").trim())
      .filter(Boolean);
  }
  return [];
}

/** 驾驶舱深色 tooltip 底 + 高对比文字（避免灰字落在 ECharts 默认浅黄/白底上） */
const ECHARTS_TOOLTIP_CHROME = {
  backgroundColor: "rgba(8, 24, 48, 0.94)",
  borderColor: "rgba(56, 189, 248, 0.45)",
  borderWidth: 1,
  padding: [8, 12],
};

const ECHARTS_TOOLTIP_TEXT = {
  primary: "#f0f9ff",
  secondary: "#bae6fd",
  muted: "#cbd5e1",
};

/** 驾驶舱紧凑柱/线图：浅灰绘图区底 + 低对比网格线（避免默认白线抢眼） */
const COCKPIT_CARTESIAN_GRID_BG = "rgba(148, 163, 184, 0.12)";
const COCKPIT_CARTESIAN_SPLIT_LINE = {
  show: true,
  lineStyle: {
    color: "rgba(148, 163, 184, 0.24)",
    width: 1,
  },
};

function echartsTooltipTextStyle(typography, role = "label") {
  const fontSize =
    role === "value"
      ? typography.value
      : role === "unit"
        ? typography.unit
        : role === "body"
          ? typography.body
          : typography.label;
  return {
    fontSize,
    color:
      role === "unit" || role === "muted"
        ? ECHARTS_TOOLTIP_TEXT.secondary
        : ECHARTS_TOOLTIP_TEXT.primary,
    lineHeight: Math.round(fontSize * 1.45),
  };
}

/** ECharts 飘窗：深色底 + 主题字号 + 二级看板内 z-index */
function echartsTooltip(typography, trigger, extra = {}) {
  const { textRole = "label", ...rest } = extra;
  const z = COCKPIT_Z_INDEX.tooltipInBoard;
  return {
    trigger,
    ...ECHARTS_TOOLTIP_CHROME,
    className: "mei-cockpit-echarts-tooltip",
    appendTo: typeof document !== "undefined" ? document.body : undefined,
    confine: false,
    extraCssText: `z-index:${z} !important;box-shadow:0 8px 24px rgba(0,0,0,0.35);border-radius:6px;`,
    textStyle: echartsTooltipTextStyle(typography, textRole),
    ...rest,
  };
}

function resolveLegacyBehavior(props) {
  const compact = props.compact === true || props.compact === "true";
  const variant = String(props.variant ?? "").trim().toLowerCase();
  const barGradient = String(props.barGradient ?? props.bar_gradient ?? "")
    .trim()
    .toLowerCase();
  return {
    stack: !!props.stack,
    percent: props.transform?.mode === "percent",
    dataZoom: props.dataZoom !== false,
    metrics: Array.isArray(props.metrics) ? props.metrics : [],
    compact,
    chartHeight: Number(props.chartHeight) > 0 ? Number(props.chartHeight) : 0,
    barGradient,
    palette: resolveColorPalette(props),
    barLine: variant === "bar-line" || props.barLine === true || props.bar_line === true,
    showLegend:
      props.showLegend !== false &&
      props.showLegend !== "false" &&
      props.show_legend !== false &&
      props.show_legend !== "false",
    chartProps: props,
  };
}

function readGridInset(props, side, fallback) {
  const key = side.toLowerCase();
  const camel = `grid${key.charAt(0).toUpperCase()}${key.slice(1)}`;
  const snake = `grid_${key}`;
  const raw = props?.[camel] ?? props?.[snake];
  if (raw === undefined || raw === null || raw === "") {
    return fallback;
  }
  const n = Number(raw);
  return Number.isFinite(n) ? n : raw;
}

function gridContainLabelEnabled(props, fallback = true) {
  if (props?.gridContainLabel === false || props?.grid_contain_label === false) {
    return false;
  }
  if (props?.gridContainLabel === true || props?.grid_contain_label === true) {
    return true;
  }
  return fallback;
}

function compactAxisValueLabel(value) {
  const n = Number(value);
  if (!Number.isFinite(n)) {
    return String(value ?? "");
  }
  const abs = Math.abs(n);
  if (abs >= 100000000) {
    return `${(n / 100000000).toFixed(abs >= 1000000000 ? 0 : 1)}亿`;
  }
  if (abs >= 10000) {
    return `${(n / 10000).toFixed(abs % 10000 === 0 ? 0 : 1)}万`;
  }
  if (abs >= 1000) {
    return `${(n / 1000).toFixed(0)}k`;
  }
  return String(n);
}

function resolveCompactCartesianGrid(props, legacy, categoryAxisRotate = 0) {
  const containLabel = gridContainLabelEnabled(props, true);
  const showLegend = legacy.showLegend === true;
  const bottomDefault = Math.abs(categoryAxisRotate) >= 30 ? 40 : 22;
  return {
    left: readGridInset(props, "left", containLabel ? 2 : 24),
    right: readGridInset(props, "right", showLegend ? 2 : 6),
    top: readGridInset(props, "top", showLegend ? 16 : 4),
    bottom: readGridInset(props, "bottom", bottomDefault),
    containLabel,
    backgroundColor: COCKPIT_CARTESIAN_GRID_BG,
    borderWidth: 0,
  };
}

function resolveCategoryAxisLabelFormatter(props) {
  const maxChars = Number(props?.label_max_chars);
  if (!Number.isFinite(maxChars) || maxChars <= 0) {
    return undefined;
  }
  return (value) => {
    const text = String(value ?? "").trim();
    if (text.length <= maxChars) {
      return text;
    }
    return `${text.slice(0, maxChars)}...`;
  };
}

function resolveCategoryAxisLabelRotate(props) {
  const raw =
    props?.axisLabelRotate ??
    props?.axis_label_rotate ??
    props?.categoryLabelRotate ??
    props?.category_label_rotate;
  const n = Number(raw);
  return Number.isFinite(n) ? n : 0;
}

function buildCategoryAxisLabel(chartProps, typography) {
  const formatter = resolveCategoryAxisLabelFormatter(chartProps);
  const rotate = resolveCategoryAxisLabelRotate(chartProps);
  const label = {
    fontSize: typography.unit,
    color: "#94a3b8",
    interval: 0,
  };
  if (formatter) {
    label.formatter = formatter;
  }
  if (rotate) {
    label.rotate = rotate;
  }
  return label;
}

function resolveChannelDisplayName(channels, field, fallback = "") {
  const channel = Array.isArray(channels)
    ? channels.find((item) => item?.field === field)
    : null;
  const name = String(channel?.name || "").trim();
  return name || fallback || String(field || "");
}

function channelList(channel, legacyField, fallbackField) {
  if (Array.isArray(channel) && channel.length > 0) {
    return channel
      .map((item) => {
        if (!item || typeof item !== "object") return null;
        const field = item.field || item.value || "";
        if (!field) return null;
        return { field, name: item.name || field };
      })
      .filter(Boolean);
  }
  const field = legacyField || fallbackField;
  if (!field) return [];
  return [{ field, name: field }];
}

function buildOption(kind, rows, mapping, legacy, diagnostics) {
  const chartKind = normalizeKind(kind);
  if (chartKind === "ranking") {
    return buildRankingOption(rows, mapping, {}, diagnostics).option;
  }
  if (chartKind === "radar") {
    return buildRadarOption(rows, mapping, legacy, diagnostics);
  }
  if (chartKind === "boxplot") {
    return buildBoxplotOption(rows, mapping, legacy, diagnostics);
  }
  if (PIE_KINDS.has(chartKind)) {
    return buildPieOption(chartKind, rows, mapping, diagnostics, legacy);
  }
  if (chartKind === "scatter") {
    return buildScatterOption(rows, mapping, diagnostics, legacy);
  }
  return buildCartesianOption(chartKind, rows, mapping, legacy, diagnostics);
}

function buildCartesianOption(kind, rows, mapping, legacy, diagnostics) {
  const xField = mapping.x[0]?.field;
  if (!xField) {
    diagnostics.push("缺少 mapping.x");
  }
  const yFields = mapping.y.length > 0 ? mapping.y.map((item) => item.field) : inferYFields(rows, xField);
  if (yFields.length === 0) {
    diagnostics.push("缺少 mapping.y");
  }
  if (kind === "trend" && yFields.length !== 1) {
    diagnostics.push("chart.trend 需要且仅支持一个 y 通道");
  }
  let categories = unique(rows.map((row) => String(row?.[xField] ?? ""))).filter(Boolean);
  if (legacy.sortCategoriesByYTotal) {
    const ranked = orderCartesianCategories(rows, mapping, yFields);
    if (ranked.length > 0) {
      categories = ranked.filter((label) => categories.includes(label));
    }
  }
  const grouped = mapping.group[0]?.field;
  const groups = grouped ? unique(rows.map((row) => String(row?.[grouped] ?? ""))).filter(Boolean) : [];
  const series = [];
  const isBar = kind === "column" || kind === "bar";
  const seriesType = isBar ? "bar" : "line";
  const compact = legacy.compact === true || legacy.compact === "true";
  const metricSpark = legacy.barGradient === "metric-spark";
  const palette = Array.isArray(legacy.palette) ? legacy.palette : [];
  for (const yField of yFields) {
    const yDisplayName = resolveChannelDisplayName(mapping.y, yField);
    if (groups.length === 0) {
      const data = categories.map((category) => aggregateValue(rows, xField, category, yField));
      if (legacy.barLine && !isBar) {
        series.push({
          name: `${yDisplayName} · 柱`,
          type: "bar",
          barWidth: compact ? 10 : 14,
          itemStyle: {
            color: {
              type: "linear",
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: "rgba(98, 190, 235, 0.95)" },
                { offset: 1, color: "rgba(98, 190, 235, 0.12)" },
              ],
            },
          },
          data,
          z: 1,
        });
        series.push({
          name: yDisplayName,
          type: "line",
          smooth: true,
          symbol: "circle",
          symbolSize: compact ? 4 : 6,
          itemStyle: { color: "#f8fafc", borderColor: "#7dd3fc", borderWidth: 1 },
          lineStyle: { color: "#7dd3fc", width: 1.5 },
          data,
          z: 3,
        });
      } else {
        const sparkBar =
          metricSpark && seriesType === "bar" && (kind === "column" || kind === "bar");
        const coloredData =
          !sparkBar && isBar && palette.length > 1
            ? data.map((value, index) => ({
                value,
                itemStyle: { color: palette[index % palette.length] },
              }))
            : data;
        series.push({
          name: yDisplayName,
          type: seriesType,
          smooth: kind === "trend",
          areaStyle: kind === "area" ? {} : undefined,
          stack: legacy.stack ? "total" : undefined,
          barWidth: sparkBar && compact ? 8 : undefined,
          itemStyle: sparkBar ? metricSparkBarItemStyle() : undefined,
          data: coloredData,
        });
      }
    } else {
      const yearDuoGradient = isBar && legacy.barGradient === "cockpit-year-duo";
      let groupSeriesIndex = 0;
      for (const groupName of groups) {
        const seriesItem = {
          name: `${groupName} · ${yDisplayName}`,
          type: seriesType,
          smooth: kind === "trend",
          areaStyle: kind === "area" ? {} : undefined,
          stack: legacy.stack ? "total" : undefined,
          data: categories.map((category) =>
            aggregateValue(rows, xField, category, yField, grouped, groupName),
          ),
        };
        if (yearDuoGradient) {
          seriesItem.barWidth = compact ? "34%" : "38%";
          seriesItem.barGap = "24%";
          seriesItem.itemStyle = cockpitYearDuoBarItemStyle(groupSeriesIndex);
          seriesItem.emphasis = {
            focus: "series",
            itemStyle: cockpitYearDuoBarItemStyle(groupSeriesIndex, { emphasis: true }),
          };
        }
        series.push(seriesItem);
        groupSeriesIndex += 1;
      }
    }
  }
  if (legacy.percent) {
    applyPercentTransform(series);
  }
  const chartProps = legacy.chartProps || {};
  const themeTypography = readThemeTypography(legacy.__host);
  const categoryAxisRotate = resolveCategoryAxisLabelRotate(chartProps);
  const compactGrid = metricSpark
    ? { left: 2, right: 2, top: 4, bottom: 4, containLabel: false }
    : resolveCompactCartesianGrid(chartProps, legacy, categoryAxisRotate);
  const categoryAxisLabel = buildCategoryAxisLabel(chartProps, themeTypography);
  const option = {
    backgroundColor: legacy.compact ? "transparent" : undefined,
    tooltip: echartsTooltip(themeTypography, "axis"),
    legend: legacy.showLegend
      ? {
          show: true,
          top: 0,
          right: 0,
          left: "auto",
          orient: "horizontal",
          itemWidth: 10,
          itemHeight: 8,
          itemGap: 6,
          textStyle: { fontSize: themeTypography.unit, color: "#94a3b8" },
        }
      : { show: false },
    toolbox: legacy.compact ? undefined : { feature: { saveAsImage: {} } },
    grid: legacy.compact
      ? compactGrid
      : { left: 44, right: 22, top: legacy.showLegend ? 38 : 28, bottom: 34 },
    xAxis: kind === "bar" ? { type: "value" } : { type: "category", data: categories },
    yAxis: kind === "bar" ? { type: "category", data: categories } : { type: "value" },
    series,
  };
  if (palette.length > 0) {
    option.color = palette;
  }
  if (legacy.compact && !metricSpark) {
    if (kind === "bar") {
      option.xAxis = {
        ...option.xAxis,
        axisLabel: {
          fontSize: themeTypography.unit,
          color: "#94a3b8",
          formatter: compactAxisValueLabel,
        },
        splitLine: COCKPIT_CARTESIAN_SPLIT_LINE,
      };
      option.yAxis = {
        ...option.yAxis,
        axisLabel: categoryAxisLabel,
      };
    } else {
      option.xAxis = {
        ...option.xAxis,
        axisLabel: categoryAxisLabel,
      };
      option.yAxis = {
        ...option.yAxis,
        axisLabel: {
          fontSize: themeTypography.unit,
          color: "#94a3b8",
          formatter: compactAxisValueLabel,
        },
        splitLine: COCKPIT_CARTESIAN_SPLIT_LINE,
        splitNumber: 4,
      };
    }
  }
  if (metricSpark) {
    const hideAxis = {
      show: false,
      axisLabel: { show: false },
      axisTick: { show: false },
      axisLine: { show: false },
      splitLine: { show: false },
    };
    option.xAxis = { ...option.xAxis, ...hideAxis };
    option.yAxis = { ...option.yAxis, ...hideAxis };
    option.tooltip = { show: false };
  }
  if (kind === "bar") {
    option.series = option.series.map((item) => ({
      ...item,
      data: categories.map((category) => {
        const value = item.data[categories.indexOf(category)] || 0;
        return value;
      }),
    }));
  }
  if (legacy.dataZoom && categories.length > 16) {
    option.dataZoom = [{ type: "inside" }, { type: "slider" }];
  }
  return option;
}

function buildPieOption(kind, rows, mapping, diagnostics, legacy = {}) {
  const labelField = mapping.label[0]?.field || mapping.x[0]?.field;
  const valueField = mapping.y[0]?.field;
  if (!labelField || !valueField) {
    diagnostics.push("pie/donut/rose 需要 mapping.label(x) 与 mapping.y");
  }
  const data = rows
    .map((row) => ({
      name: String(row?.[labelField] ?? ""),
      value: toNumber(row?.[valueField]),
    }))
    .filter((item) => item.name && Number.isFinite(item.value));
  const compact = legacy.compact === true || legacy.compact === "true";
  const chartHeight = Number(legacy.chartHeight) > 0 ? Number(legacy.chartHeight) : 0;
  const themeTypography = readThemeTypography(legacy.__host);
  const tight = compact && chartHeight > 0 && chartHeight <= 56;
  const donutRadius = tight
    ? ["58%", "82%"]
    : compact
      ? ["52%", "78%"]
      : ["45%", "72%"];
  const option = {
    tooltip: echartsTooltip(themeTypography, "item"),
    legend: compact
      ? { show: false }
      : {
          top: 0,
          textStyle: { fontSize: themeTypography.label, color: "#94a3b8" },
        },
    toolbox: compact ? undefined : { feature: { saveAsImage: {} } },
    series: [
      {
        type: "pie",
        radius: kind === "donut" ? donutRadius : tight ? "62%" : compact ? "68%" : "70%",
        center: compact ? ["50%", "50%"] : undefined,
        label: {
          show: !compact,
          fontSize: themeTypography.label,
          color: "#e2e8f0",
        },
        labelLine: { show: !compact },
        roseType: kind === "rose" ? "radius" : undefined,
        data,
      },
    ],
  };
  if (Array.isArray(legacy.palette) && legacy.palette.length > 0) {
    option.color = legacy.palette;
  }
  return option;
}

function buildScatterOption(rows, mapping, diagnostics, legacy = {}) {
  const themeTypography = readThemeTypography(legacy.__host);
  const xField = mapping.x[0]?.field;
  const yField = mapping.y[0]?.field;
  const sizeField = mapping.size[0]?.field;
  const colorField = mapping.color[0]?.field;
  if (!xField || !yField) {
    diagnostics.push("scatter 需要 mapping.x 与 mapping.y");
  }
  const groups = colorField
    ? unique(rows.map((row) => String(row?.[colorField] ?? "")).filter(Boolean))
    : [""];
  const series = groups.map((groupName) => {
    const points = rows
      .filter((row) => !colorField || String(row?.[colorField] ?? "") === groupName)
      .map((row) => {
        const point = [toNumber(row?.[xField]), toNumber(row?.[yField])];
        if (sizeField) point.push(toNumber(row?.[sizeField]));
        return point;
      })
      .filter((item) => Number.isFinite(item[0]) && Number.isFinite(item[1]));
    return {
      name: groupName || (mapping.y[0]?.name || yField),
      type: "scatter",
      symbolSize: (value) => {
        if (!sizeField) return 12;
        const size = Number(value?.[2]);
        if (!Number.isFinite(size)) return 10;
        return Math.max(6, Math.min(24, size / 8));
      },
      data: points,
    };
  });
  return {
    tooltip: echartsTooltip(themeTypography, "item"),
    legend: { top: 0, show: !!colorField },
    toolbox: { feature: { saveAsImage: {} } },
    xAxis: { type: "value" },
    yAxis: { type: "value" },
    series,
  };
}

function buildRadarOption(rows, mapping, legacy, diagnostics) {
  const themeTypography = readThemeTypography(legacy.__host);
  const dimensions = mapping.radarDimensions.length > 0
    ? mapping.radarDimensions
    : mapping.y;
  if (dimensions.length === 0) {
    diagnostics.push("radar 需要 dimensions 或 mapping.y");
  }
  const indicators = dimensions.map((item) => ({
    name: item.name || item.field,
    max: maxByField(rows, item.field) || 100,
  }));
  const labelField = mapping.label[0]?.field || mapping.x[0]?.field;
  const data = rows.slice(0, 12).map((row, index) => ({
    name: labelField ? String(row?.[labelField] ?? `item-${index + 1}`) : `item-${index + 1}`,
    value: dimensions.map((dim) => toNumber(row?.[dim.field])),
  }));
  if (legacy.percent) {
    normalizeRadarData(data);
  }
  return {
    tooltip: echartsTooltip(themeTypography, "item"),
    legend: { top: 0 },
    toolbox: { feature: { saveAsImage: {} } },
    radar: { indicator: indicators },
    series: [{ type: "radar", data }],
  };
}

function buildBoxplotOption(rows, mapping, legacy, diagnostics) {
  const themeTypography = readThemeTypography(legacy.__host);
  const labelField = mapping.x[0]?.field || "label";
  const def = mapping.boxplot;
  if (def.length >= 5) {
    const [minField, q1Field, medianField, q3Field, maxField] = def.map((item) => item.field);
    const labels = rows.map((row) => String(row?.[labelField] ?? ""));
    const data = rows.map((row) => [
      toNumber(row?.[minField]),
      toNumber(row?.[q1Field]),
      toNumber(row?.[medianField]),
      toNumber(row?.[q3Field]),
      toNumber(row?.[maxField]),
    ]);
    return {
      tooltip: echartsTooltip(themeTypography, "item"),
      xAxis: { type: "category", data: labels },
      yAxis: { type: "value" },
      series: [{ type: "boxplot", data }],
    };
  }
  const valueField = mapping.y[0]?.field;
  if (!valueField) {
    diagnostics.push("boxplot 需要 mapping.boxplot 或 mapping.y");
  }
  const grouped = groupNumbersBy(rows, labelField, valueField);
  const labels = Object.keys(grouped);
  const data = labels.map((label) => toBoxStats(grouped[label]));
  return {
    tooltip: echartsTooltip(themeTypography, "item"),
    xAxis: { type: "category", data: labels },
    yAxis: { type: "value" },
    series: [{ type: "boxplot", data }],
  };
}

function aggregateValue(rows, xField, category, yField, groupField, groupName) {
  const values = rows
    .filter((row) => {
      if (String(row?.[xField] ?? "") !== category) return false;
      if (!groupField) return true;
      return String(row?.[groupField] ?? "") === groupName;
    })
    .map((row) => toNumber(row?.[yField]))
    .filter((value) => Number.isFinite(value));
  if (values.length === 0) return 0;
  return values.reduce((sum, value) => sum + value, 0);
}

function applyPercentTransform(series) {
  if (!Array.isArray(series) || series.length === 0) return;
  const points = Math.max(...series.map((item) => item.data.length));
  for (let idx = 0; idx < points; idx += 1) {
    const total = series.reduce((sum, item) => sum + toNumber(item.data[idx]), 0);
    if (total <= 0) continue;
    for (const item of series) {
      item.data[idx] = Number(((toNumber(item.data[idx]) / total) * 100).toFixed(2));
    }
  }
}

function inferYFields(rows, xField) {
  if (!Array.isArray(rows) || rows.length === 0) return [];
  const first = rows.find((row) => row && typeof row === "object");
  if (!first) return [];
  return Object.keys(first).filter((key) => key !== xField && Number.isFinite(toNumber(first[key])));
}

function resolveRankingLayout(props) {
  const raw = String(
    props?.rankingLayout || props?.labelLayout || props?.layout || "side",
  )
    .trim()
    .toLowerCase();
  if (["above", "label_above", "label-above", "stacked", "top"].includes(raw)) {
    return "above";
  }
  return "side";
}

function buildRankingItems(rows, mapping, diagnostics) {
  const labelField =
    mapping.x[0]?.field || mapping.label[0]?.field || inferLabelField(rows);
  const valueField = mapping.y[0]?.field || "value";
  const valueName = mapping.y[0]?.name || valueField;
  if (!labelField) {
    diagnostics.push("ranking 需要 mapping.x（排名标签字段）");
  }
  const items = (Array.isArray(rows) ? rows : [])
    .map((row) => ({
      label: String(row?.[labelField] ?? "").trim(),
      value: toNumber(row?.[valueField]),
    }))
    .filter((item) => item.label && Number.isFinite(item.value))
    .sort((left, right) => right.value - left.value);
  return { items, valueName };
}

function buildRankingOption(rows, mapping, props, diagnostics) {
  return buildRankingSideOption(rows, mapping, props, diagnostics);
}

function renderRankingAboveDom(chartEl, model, props, onLabelClick) {
  const theme = model.theme || resolveRankingTheme(props);
  const chartHeight = Number(props.chartHeight) > 0 ? Number(props.chartHeight) : 152;
  const items = Array.isArray(model.items) ? model.items : [];
  const maxChars = Number(model.maxChars) > 0 ? Number(model.maxChars) : 28;
  const maxValue = rankingValueAxisMax(items.map((item) => item.value));
  const title = String(props.title || "").trim();
  const showTitle = (props.compact === true || props.compact === "true") && title.length > 0;
  const pullUp = Math.max(0, Number(props.rankingPullUp ?? props.ranking_pull_up ?? 0));
  const padLeft = Math.max(0, Number(props.contentPadLeft ?? props.content_pad_left ?? 0));
  const titleFontPx = readThemeTypography(props.__host).chartTitle;
  const titleH = showTitle ? Math.max(14, Math.ceil(titleFontPx * 1.15)) : 0;
  const listHeight = Math.max(48, chartHeight - titleH + pullUp);
  const slotPx = items.length > 0 ? Math.floor(listHeight / items.length) : 0;
  chartEl.style.height = `${chartHeight}px`;
  chartEl.style.minHeight = `${chartHeight}px`;
  chartEl.style.maxHeight = `${chartHeight}px`;
  chartEl.style.overflow = pullUp > 0 ? "visible" : "hidden";
  if (chartEl.parentElement) {
    chartEl.parentElement.style.overflow = pullUp > 0 ? "visible" : "hidden";
  }
  const barColor = theme.barColor || "#38bdf8";
  const trackBg = theme.barBackground || "rgba(148, 163, 184, 0.14)";
  const trackBorder = theme.barBackgroundBorder || "rgba(100, 116, 139, 0.35)";
  const rowsHtml = items
    .map((item, index) => {
      const ratio = maxValue > 0 ? item.value / maxValue : 0;
      const pct = Math.max(8, Math.min(100, Math.round(ratio * 100)));
      const label = formatRankingNameLabel(item.label, maxChars);
      return `<div class="mei-rank-above-row" data-idx="${index}" title="${escapeHtml(item.label)}" style="max-height:${Math.max(22, slotPx)}px">
        <div class="mei-rank-above-head">
          <span class="mei-rank-above-label">${escapeHtml(label.display)}</span>
          <span class="mei-rank-above-value">${escapeHtml(String(item.value))}</span>
        </div>
        <div class="mei-rank-above-track" style="background:${trackBg};border-color:${trackBorder}">
          <div class="mei-rank-above-fill" style="width:${pct}%;background-color:${barColor}"></div>
        </div>
      </div>`;
    })
    .join("");
  chartEl.innerHTML = `<div class="mei-rank-above" style="height:${chartHeight}px;padding-left:${padLeft}px;margin-top:${-pullUp}px;box-sizing:border-box">
    ${showTitle ? `<div class="mei-rank-above-title">${escapeHtml(title)}</div>` : ""}
    <div class="mei-rank-above-list">${rowsHtml}</div>
  </div>`;
  chartEl.querySelectorAll(".mei-rank-above-row").forEach((row) => {
    row.addEventListener("click", (event) => {
      const index = Number(row.getAttribute("data-idx"));
      const full = model.fullLabels?.[index];
      if (full && typeof onLabelClick === "function") {
        onLabelClick(full, event);
      }
    });
  });
}

function resolveRankingTheme(props) {
  return {
    barColor: String(props.barColor || props.bar_color || "").trim() || "#38bdf8",
    barBackground:
      String(props.barBackground || props.bar_background || "").trim() ||
      "rgba(148, 163, 184, 0.14)",
    barBackgroundBorder:
      String(props.barBackgroundBorder || props.bar_background_border || "").trim() ||
      "rgba(100, 116, 139, 0.35)",
  };
}

function resolveRankingTypography(props, slotPx = 0) {
  void slotPx;
  const typography = readThemeTypography(props.__host);
  return {
    labelFontSize: typography.label,
    valueFontSize: typography.value,
    axisLabelFontSize: typography.unit,
    labelLineHeight: Math.round(typography.label * 1.42),
  };
}

function estimateRankingMaxChars(widthPx, fontSize) {
  const unit = Math.max(7, fontSize * 0.58);
  return Math.max(6, Math.floor(widthPx / unit));
}

function formatRankingNameLabel(text, maxChars) {
  const full = String(text ?? "").trim();
  const display = truncateRankingLabel(full, maxChars);
  return { display, full, isTruncated: display !== full };
}

function rankingBarBackgroundStyle(theme, borderRadius = [0, 4, 4, 0]) {
  return {
    color: theme.barBackground,
    borderColor: theme.barBackgroundBorder,
    borderWidth: 1,
    borderRadius,
  };
}

function rankingBarItemStyle(theme, borderRadius = [0, 4, 4, 0]) {
  return {
    borderRadius,
    color: theme.barColor,
    shadowBlur: 6,
    shadowColor: "rgba(56, 189, 248, 0.3)",
    shadowOffsetY: 1,
  };
}

function buildRankingBarSeries({
  values,
  valueName,
  barMaxWidth,
  barCategoryGap,
  theme,
  borderRadius = [0, 4, 4, 0],
  valueLabel,
}) {
  return {
    name: valueName,
    type: "bar",
    data: values,
    barMaxWidth,
    barCategoryGap,
    z: 2,
    showBackground: true,
    backgroundStyle: rankingBarBackgroundStyle(theme, borderRadius),
    itemStyle: rankingBarItemStyle(theme, borderRadius),
    emphasis: {
      focus: "series",
      itemStyle: {
        ...rankingBarItemStyle(theme, borderRadius),
        shadowBlur: 10,
        shadowColor: "rgba(56, 189, 248, 0.42)",
      },
    },
    label: valueLabel || { show: false },
  };
}

function rankingTooltipFormatter(items, valueName, typography) {
  const labelPx = typography.label;
  const unitPx = typography.unit;
  return (params) => {
    const point = Array.isArray(params) ? params[0] : params;
    const idx = point?.dataIndex ?? 0;
    const item = items[idx];
    if (!item) return "";
    const title = escapeHtml(item.label);
    return `<div style="max-width:min(92vw,420px);line-height:1.45;word-break:break-all;font-size:${labelPx}px;color:${ECHARTS_TOOLTIP_TEXT.primary};">${title}<br/><span style="color:${ECHARTS_TOOLTIP_TEXT.secondary};font-size:${unitPx}px">${escapeHtml(valueName)}: ${item.value}</span></div>`;
  };
}

function rankingValueAxisMax(values) {
  const maxValue = values.reduce((peak, value) => Math.max(peak, value), 0);
  if (!Number.isFinite(maxValue) || maxValue <= 0) return 1;
  return Math.ceil(maxValue * 1.06);
}

function buildRankingSideOption(rows, mapping, props, diagnostics) {
  const { items, valueName } = buildRankingItems(rows, mapping, diagnostics);
  const theme = resolveRankingTheme(props);
  const configuredMaxChars = resolveRankingLabelMaxChars(props, "side");
  const categories = items.map((item) => item.label);
  const values = items.map((item) => item.value);
  const compact = props.compact === true || props.compact === "true";
  const chartHeight = Number(props.chartHeight) > 0 ? Number(props.chartHeight) : 0;
  let gridTop = compact ? 8 : 28;
  let gridBottom = compact ? 12 : 24;
  let barMaxWidth = 22;
  let slotPx = 0;
  if (compact && chartHeight > 0 && items.length > 0) {
    gridTop = 6;
    gridBottom = 8;
    const plotH = Math.max(20, chartHeight - gridTop - gridBottom);
    slotPx = plotH / items.length;
    barMaxWidth = Math.min(22, Math.max(8, Math.floor(slotPx * 0.58)));
  }
  const typography = resolveRankingTypography(props, slotPx);
  const themeTypography = readThemeTypography(props.__host);
  const charWidth = Math.max(8, Math.round(typography.labelFontSize * 0.58));
  const gridLeftMin = compact ? 76 : 100;
  const gridLeftMax = compact ? 220 : 380;
  const gridLeft = Math.min(
    gridLeftMax,
    Math.max(gridLeftMin, configuredMaxChars * charWidth + (compact ? 18 : 28)),
  );
  const labelWidthPx = gridLeft - (compact ? 14 : 20);
  const maxChars = Math.min(
    configuredMaxChars,
    estimateRankingMaxChars(labelWidthPx, typography.axisLabelFontSize),
  );
  const borderRadius = [0, 4, 4, 0];
  const option = {
    tooltip: echartsTooltip(themeTypography, "axis", {
      axisPointer: { type: "shadow" },
      formatter: rankingTooltipFormatter(items, valueName, themeTypography),
    }),
    grid: {
      left: gridLeft,
      right: compact ? 28 : 36,
      top: gridTop,
      bottom: gridBottom,
      containLabel: false,
    },
    xAxis: {
      type: "value",
      min: 0,
      max: rankingValueAxisMax(values),
      splitLine: { show: false },
      axisLabel: { show: false },
      axisTick: { show: false },
      axisLine: { show: false },
    },
    yAxis: {
      type: "category",
      data: categories,
      inverse: true,
      axisLabel: {
        color: "#e2e8f0",
        fontSize: typography.axisLabelFontSize,
        fontWeight: 500,
        width: labelWidthPx,
        overflow: "truncate",
        ellipsis: "…",
        formatter: (value) =>
          formatRankingNameLabel(value, maxChars).display,
      },
      axisTick: { show: false },
      axisLine: { show: false },
    },
    series: [
      buildRankingBarSeries({
        values,
        valueName,
        barMaxWidth,
        barCategoryGap: compact && chartHeight > 0 ? "30%" : "22%",
        theme,
        borderRadius,
        valueLabel: {
          show: true,
          position: "right",
          distance: 6,
          color: "#f1f5f9",
          fontSize: typography.valueFontSize,
          fontWeight: 600,
          formatter: ({ value }) => (Number.isFinite(value) ? String(value) : ""),
        },
      }),
    ],
  };
  return {
    option,
    fullLabels: items.map((item) => item.label),
    rowCount: items.length,
    meta: `排名 ${items.length} 项 · 左侧标签最多约 ${maxChars} 字（点击查看全文）`,
  };
}

function buildRankingLabelAboveOption(rows, mapping, props, diagnostics) {
  const { items, valueName } = buildRankingItems(rows, mapping, diagnostics);
  const theme = resolveRankingTheme(props);
  const configuredMaxChars = resolveRankingLabelMaxChars(props, "above");
  const values = items.map((item) => item.value);
  const compact = props.compact === true || props.compact === "true";
  const chartHeight = Number(props.chartHeight) > 0 ? Number(props.chartHeight) : 0;
  const gridLeft = compact ? 4 : 10;
  const gridRight = compact ? 36 : 44;
  let gridTop = compact ? 4 : 16;
  let gridBottom = compact ? 4 : 12;
  let barMaxWidth = 10;
  let labelOffsetPx = 20;
  let labelWidth = 300;
  let slotPx = 0;
  if (compact && chartHeight > 0 && items.length > 0) {
    const plotH = Math.max(28, chartHeight - gridTop - gridBottom);
    slotPx = plotH / items.length;
    labelOffsetPx = Math.max(18, Math.floor(slotPx * 0.5));
    barMaxWidth = Math.min(12, Math.max(6, Math.floor(slotPx * 0.22)));
    labelWidth = Math.max(160, Math.floor(chartHeight * 2.1));
  }
  const typography = resolveRankingTypography(props, slotPx);
  const themeTypography = readThemeTypography(props.__host);
  const maxChars = Math.min(
    configuredMaxChars,
    estimateRankingMaxChars(labelWidth, typography.labelFontSize),
  );
  const borderRadius = [0, 3, 3, 0];
  const option = {
    tooltip: echartsTooltip(themeTypography, "axis", {
      axisPointer: { type: "shadow" },
      formatter: rankingTooltipFormatter(items, valueName, themeTypography),
    }),
    grid: {
      left: gridLeft,
      right: gridRight,
      top: gridTop,
      bottom: gridBottom,
      containLabel: false,
    },
    xAxis: {
      type: "value",
      min: 0,
      max: rankingValueAxisMax(values),
      splitLine: { show: false },
      axisLabel: { show: false },
      axisTick: { show: false },
      axisLine: { show: false },
    },
    yAxis: {
      type: "category",
      data: items.map((_, index) => String(index + 1)),
      inverse: true,
      axisLabel: { show: false },
      axisTick: { show: false },
      axisLine: { show: false },
    },
    series: [
      buildRankingBarSeries({
        values,
        valueName,
        barMaxWidth,
        barCategoryGap: compact && chartHeight > 0 ? "28%" : "32%",
        theme,
        borderRadius,
        valueLabel: {
          show: true,
          position: [4, -labelOffsetPx],
          align: "left",
          verticalAlign: "bottom",
          fontSize: typography.labelFontSize,
          lineHeight: typography.labelLineHeight,
          width: labelWidth,
          overflow: "truncate",
          ellipsis: "…",
          color: "#e2e8f0",
          formatter: (params) => {
            const item = items[params.dataIndex];
            if (!item) return "";
            const name = formatRankingNameLabel(item.label, maxChars);
            return `{name|${name.display}}\n{val|${item.value}}`;
          },
          rich: {
            name: {
              fontSize: typography.labelFontSize,
              fontWeight: 500,
              color: "#e2e8f0",
              align: "left",
              lineHeight: typography.labelLineHeight,
              width: labelWidth,
              overflow: "truncate",
            },
            val: {
              fontSize: typography.valueFontSize,
              fontWeight: 600,
              color: "#7dd3fc",
              align: "right",
              lineHeight: Math.round(typography.valueFontSize * 1.25),
            },
          },
        },
      }),
    ],
  };
  return {
    option,
    fullLabels: items.map((item) => item.label),
    rowCount: items.length,
    meta: `排名 ${items.length} 项 · 标签置顶（约 ${maxChars} 字截断，点击查看全文）`,
  };
}

function isRankingCompact(props) {
  return props?.compact === true || props?.compact === "true";
}

function resolveRankingShellHeight(props, rowCount) {
  const compact = isRankingCompact(props);
  const chartHeight = Number(props?.chartHeight) > 0 ? Number(props.chartHeight) : 0;
  const layout = resolveRankingLayout(props);
  if (compact && chartHeight > 0) {
    return chartHeight;
  }
  const count = Number(rowCount) > 0 ? Number(rowCount) : 0;
  if (count > 0) {
    const perRow = layout === "above" ? 50 : 42;
    return Math.max(200, count * perRow);
  }
  return compact ? 64 : 260;
}

function resolveRankingLabelMaxChars(props, layout) {
  const direct = Number(props?.label_max_chars);
  if (Number.isFinite(direct) && direct > 0) return Math.floor(direct);
  const fromTheme = Number(props?._mei?.components?.chart_ranking?.label_max_chars);
  if (Number.isFinite(fromTheme) && fromTheme > 0) return Math.floor(fromTheme);
  return layout === "above" ? 20 : 20;
}

function truncateRankingLabel(text, maxChars) {
  const chars = [...String(text ?? "")];
  if (chars.length <= maxChars) return String(text ?? "");
  return `${chars.slice(0, maxChars).join("")}…`;
}

function inferLabelField(rows) {
  if (!Array.isArray(rows) || rows.length === 0) return "";
  const first = rows.find((row) => row && typeof row === "object");
  if (!first) return "";
  const keys = Object.keys(first);
  return keys.find((key) => key !== "value" && typeof first[key] === "string") || keys[0] || "";
}

function normalizeKind(kind) {
  if (kind === "chart.trend") return "trend";
  if (kind === "chart.ranking") return "ranking";
  return String(kind || "").replace("chart.", "");
}

function groupNumbersBy(rows, keyField, valueField) {
  const out = {};
  for (const row of rows) {
    const key = String(row?.[keyField] ?? "");
    const value = toNumber(row?.[valueField]);
    if (!key || !Number.isFinite(value)) continue;
    if (!out[key]) out[key] = [];
    out[key].push(value);
  }
  return out;
}

function toBoxStats(values) {
  const sorted = [...values].sort((a, b) => a - b);
  if (sorted.length === 0) return [0, 0, 0, 0, 0];
  return [
    sorted[0],
    quantile(sorted, 0.25),
    quantile(sorted, 0.5),
    quantile(sorted, 0.75),
    sorted[sorted.length - 1],
  ];
}

function quantile(sorted, q) {
  const pos = (sorted.length - 1) * q;
  const base = Math.floor(pos);
  const rest = pos - base;
  if (sorted[base + 1] !== undefined) {
    return sorted[base] + rest * (sorted[base + 1] - sorted[base]);
  }
  return sorted[base];
}

function normalizeRadarData(data) {
  if (!Array.isArray(data) || data.length === 0) return;
  const dims = data[0].value.length;
  for (let idx = 0; idx < dims; idx += 1) {
    const max = Math.max(...data.map((item) => toNumber(item.value[idx])), 1);
    for (const item of data) {
      item.value[idx] = Number(((toNumber(item.value[idx]) / max) * 100).toFixed(2));
    }
  }
}

function maxByField(rows, field) {
  const values = rows.map((row) => toNumber(row?.[field])).filter((value) => Number.isFinite(value));
  if (values.length === 0) return 0;
  return Math.max(...values);
}

function resolveDatasetSource(props) {
  const direct = props?.data || props?.value || null;
  if (direct && typeof direct === "object" && Array.isArray(direct.rows)) {
    return direct;
  }
  return props?.dataset?.dataset || props?.dataset || {};
}

function toNumber(value) {
  const n = Number(value);
  return Number.isFinite(n) ? n : NaN;
}

function unique(items) {
  return [...new Set(items)];
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}
