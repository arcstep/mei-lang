import {
  deferUntilDisplayed,
  fetchDatasetRows,
  parseProps,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  subscribeHomeRuntimeResume,
  subscribeQueryState,
} from "../dataset/runtime-query.js";
import { createComponentTracer } from "../perf/render-trace.js";
import { ensureEChartsGlobal } from "../vendor/runtime-libs.js";

function ensureECharts() {
  return ensureEChartsGlobal();
}

function metricRows(metric) {
  if (!metric || typeof metric !== "object") return [];
  if (Array.isArray(metric.rows)) return metric.rows;
  if (Array.isArray(metric.value)) return metric.value;
  return [];
}

function rowValue(row, field) {
  const raw = row?.[field];
  const n = Number(raw);
  return Number.isFinite(n) ? n : 0;
}

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

function groupFieldFromProps(props) {
  return String(
    props?.groupField ?? props?.group_field ?? props?.categoryField ?? props?.category_field ?? ""
  ).trim();
}

function rowGroupLabel(row, groupField) {
  if (groupField) {
    return String(row?.[groupField] ?? "").trim();
  }
  for (const key of ["label", "name", "category", "title", "id"]) {
    const text = String(row?.[key] ?? "").trim();
    if (text) return text;
  }
  const fallbackKey = Object.keys(row || {}).find((key) => key !== "value" && row[key] != null);
  return fallbackKey ? String(row[fallbackKey] ?? "").trim() : "";
}

function truthyProp(props, ...keys) {
  for (const key of keys) {
    const value = props?.[key];
    if (value === true || value === "true") return true;
    if (value === false || value === "false") return false;
  }
  return false;
}

function legendItemsFromProps(props) {
  const custom = props?.legend ?? props?.legendItems ?? props?.legend_items;
  if (Array.isArray(custom) && custom.length > 0) {
    return custom
      .map((item) => ({
        label: String(item?.label ?? item?.name ?? "").trim(),
        color: String(item?.color ?? "#62beeb").trim(),
      }))
      .filter((item) => item.label);
  }
  return [
    {
      label: String(
        props?.legendOkLabel ??
          props?.legend_ok_label ??
          props?.legendRateLabel ??
          props?.legend_rate_label ??
          "无违规"
      ).trim(),
      color: String(
        props?.legendOkColor ?? props?.legend_ok_color ?? props?.legendRateColor ?? props?.legend_rate_color ?? "#62beeb"
      ).trim(),
    },
    {
      label: String(
        props?.legendViolLabel ??
          props?.legend_viol_label ??
          props?.legendRestLabel ??
          props?.legend_rest_label ??
          "违规"
      ).trim(),
      color: String(
        props?.legendViolColor ??
          props?.legend_viol_color ??
          props?.legendRestColor ??
          props?.legend_rest_color ??
          "#c47a3a"
      ).trim(),
    },
  ].filter((item) => item.label);
}

function donutSliceLabels(props) {
  const items = legendItemsFromProps(props);
  return {
    okLabel: items[0]?.label || "无违规",
    violLabel: items[1]?.label || "违规",
    okColor: items[0]?.color || "#62beeb",
    violColor: items[1]?.color || "#c47a3a",
  };
}

function buildProgressDonutOption(rate, sliceLabels) {
  const pct = Math.max(0, Math.min(100, Math.round(rate)));
  const rest = Math.max(0, 100 - pct);
  const { okLabel, violLabel, okColor, violColor } = sliceLabels;
  return {
    animation: false,
    tooltip: { show: false },
    legend: { show: false },
    series: [
      {
        type: "pie",
        radius: ["58%", "78%"],
        center: ["50%", "46%"],
        avoidLabelOverlap: true,
        label: {
          show: true,
          position: "center",
          formatter: `${pct}%`,
          color: "#e8f4ff",
          fontSize: 15,
          fontWeight: 700,
        },
        labelLine: { show: false },
        data: [
          {
            value: pct,
            name: okLabel,
            itemStyle: { color: okColor },
          },
          {
            value: rest,
            name: violLabel,
            itemStyle: { color: violColor },
            label: { show: false },
            emphasis: { disabled: true },
          },
        ],
      },
    ],
  };
}

function renderLegendRailHtml(items) {
  return items
    .map(
      (item) =>
        `<div class="legend-item"><span class="swatch" style="background:${item.color}"></span><span class="label">${item.label}</span></div>`
    )
    .join("");
}

function legendRailWidth(props) {
  const raw = Number(props?.legendWidth ?? props?.legend_width);
  return Number.isFinite(raw) && raw > 0 ? raw : 52;
}

class MeiCockpitDonutTrio extends HTMLElement {
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
    this._groupField = groupFieldFromProps(this._props);
    this._sharedFilters = {};
    this._charts = [];
    this._renderTrace = createComponentTracer(this, "mei-cockpit-donut-trio", {});
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
      this._sharedFilters = state?.filters || {};
      this.refreshData();
    });
    this._unsubscribeHomeRuntimeResume = subscribeHomeRuntimeResume(() => {
      this.refreshData();
    });
    this.renderShell();
    this._renderTrace.mark("bootstrap", {
      query_state_id: this._queryStateId || "",
    });
    this.refreshData();
    this.resizeObserver = new ResizeObserver(() => {
      this._charts.forEach((chart) => chart?.resize?.());
    });
    this.resizeObserver.observe(this);
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
    if (this.resizeObserver) {
      this.resizeObserver.disconnect();
    }
    this._charts.forEach((chart) => chart?.dispose?.());
    this._charts = [];
  }

  renderShell() {
    const h = Number(this._props?.chartHeight) > 0 ? Number(this._props.chartHeight) : 88;
    const showLegend = truthyProp(this._props, "showLegend", "show_legend");
    const legendItems = legendItemsFromProps(this._props);
    const legendW = legendRailWidth(this._props);
    const chartH = Math.max(48, h - 22);
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          min-width: 0;
          height: 100%;
          font-family: ui-sans-serif, system-ui, "PingFang SC", "Microsoft YaHei", sans-serif;
        }
        .wrap {
          display: flex;
          flex-direction: row;
          align-items: stretch;
          height: 100%;
          min-height: ${h}px;
          min-width: 0;
        }
        .charts {
          flex: 1;
          min-width: 0;
          display: grid;
          grid-template-columns: repeat(3, minmax(0, 1fr));
          gap: 6px;
          align-items: stretch;
        }
        .slot {
          display: flex;
          flex-direction: column;
          align-items: center;
          min-width: 0;
        }
        .chart {
          width: 100%;
          height: ${chartH}px;
          min-height: ${chartH}px;
        }
        .cap {
          margin-top: 2px;
          text-align: center;
          font-size: 11px;
          line-height: 1.3;
          color: #94a3b8;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          max-width: 100%;
          padding: 0 2px;
        }
        .legend-rail {
          flex: 0 0 ${legendW}px;
          width: ${legendW}px;
          display: flex;
          flex-direction: column;
          justify-content: center;
          align-items: flex-start;
          gap: 10px;
          padding: 0 0 0 4px;
          box-sizing: border-box;
        }
        .legend-rail[hidden] {
          display: none;
        }
        .legend-item {
          display: flex;
          align-items: center;
          gap: 5px;
          min-width: 0;
        }
        .legend-item .label {
          font-size: 10px;
          line-height: 1.2;
          color: #94a3b8;
          white-space: nowrap;
        }
        .swatch {
          width: 8px;
          height: 8px;
          border-radius: 2px;
          flex-shrink: 0;
        }
        .status {
          font-size: 10px;
          color: #64748b;
          text-align: center;
          padding: 8px 0;
        }
        .status.error { color: #fca5a5; }
      </style>
      <div class="wrap">
        <div class="charts">
          <div class="slot"><div class="chart" data-idx="0"></div><div class="cap"></div></div>
          <div class="slot"><div class="chart" data-idx="1"></div><div class="cap"></div></div>
          <div class="slot"><div class="chart" data-idx="2"></div><div class="cap"></div></div>
        </div>
        <aside class="legend-rail" ${showLegend ? "" : "hidden"} aria-label="图例">${renderLegendRailHtml(legendItems)}</aside>
      </div>
      <div class="status"></div>
    `;
    this.statusEl = this.shadowRoot.querySelector(".status");
    this.slotEls = [...this.shadowRoot.querySelectorAll(".slot")];
    this._sliceLabels = donutSliceLabels(this._props);
  }

  async fetchRowsForMetric(resolvedValue) {
    const lineProps = propsWithMetricValue(this._props, resolvedValue);
    if (!resolveRuntimeMetricRef(lineProps)) return [];
    const limit = Number(this._props?.limit) > 0 ? Number(this._props.limit) : 3;
    const result = await fetchDatasetRows(lineProps, {
      filters: this._sharedFilters,
      page: 1,
      pageSize: Math.max(limit + 4, 16),
      meta: runtimeCallerMeta(this, "mei-cockpit-donut-trio"),
    });
    return Array.isArray(result?.rows) ? result.rows : [];
  }

  async refreshData() {
    this._renderTrace?.mark("render_start");
    const totalValue = this._props?.totalMetric ?? this._props?.total_metric;
    const numerValue =
      this._props?.numerMetric ??
      this._props?.numer_metric ??
      this._props?.noViolMetric ??
      this._props?.no_viol_metric;
    const totalRef = resolveRuntimeMetricRef(propsWithMetricValue(this._props, totalValue));
    if (!totalRef?.dataset_id) {
      this.statusEl.textContent = "缺少 totalMetric";
      return;
    }
    this.statusEl.textContent = "";
    try {
      const [totalRows, numerRows] = await Promise.all([
        this.fetchRowsForMetric(totalValue),
        numerValue ? this.fetchRowsForMetric(numerValue) : Promise.resolve([]),
      ]);
      const numerMap = new Map(
        numerRows.map((row) => [rowGroupLabel(row, this._groupField), rowValue(row, "value")])
      );
      const groups = totalRows
        .map((row) => {
          const name = rowGroupLabel(row, this._groupField);
          const total = rowValue(row, "value");
          const numer = numerMap.get(name) ?? 0;
          const rate = total > 0 ? (numer / total) * 100 : 0;
          return { name, total, numer, rate };
        })
        .filter((item) => item.name);
      const limit = Number(this._props?.limit) > 0 ? Number(this._props.limit) : 3;
      const items = groups.slice(0, limit);
      while (items.length < limit) {
        items.push({ name: "—", total: 0, numer: 0, rate: 0 });
      }
      this._renderTrace?.mark("echarts_load_start");
      const echarts = await ensureECharts();
      this._renderTrace?.mark("echarts_load_done");
      items.forEach((item, index) => {
        const slot = this.slotEls[index];
        if (!slot) return;
        const chartEl = slot.querySelector(".chart");
        const capEl = slot.querySelector(".cap");
        capEl.textContent = item.name;
        let chart = this._charts[index];
        if (!chart) {
          chart = echarts.init(chartEl);
          this._charts[index] = chart;
        }
        chart.setOption(buildProgressDonutOption(item.rate, this._sliceLabels), true);
        requestAnimationFrame(() => chart?.resize?.());
      });
      requestAnimationFrame(() => {
        this._charts.forEach((chart) => chart?.resize?.());
      });
      this._renderTrace?.mark("render_done", {
        item_count: items.length,
      });
    } catch (error) {
      this.statusEl.textContent = String(error?.message || error || "加载失败");
      this.statusEl.className = "status error";
      this._renderTrace?.mark("render_error", {
        message: String(error?.message || error || "加载失败"),
      });
    }
  }
}

if (!customElements.get("mei-cockpit-donut-trio")) {
  customElements.define("mei-cockpit-donut-trio", MeiCockpitDonutTrio);
}
