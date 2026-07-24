/**
 * Thunder 右栏过程区：
 * - 上：Eabs + 闪频（ECharts，均分）
 * - 下：光学帧缩略图（4 列，溢出滚动）
 * 级别变化改由回看条 / 指标依据表达，不再占时序图。
 */
import { deferUntilDisplayed } from "../dataset/runtime-query.js";
import { parseProps, escapeHtml, escapeAttr } from "../cockpit/shared.js";
import {
  COCKPIT_TYPE,
  cockpitCssVars,
  readThemeColor,
  readThemeTypography,
  readThemeUiFontFamily,
} from "../cockpit/tokens.js";
import { color } from "../mei/theme-style.js";
import { ensureEChartsGlobal } from "../vendor/runtime-libs.js";
import { getThunderStore, subscribeThunderState } from "./event-bus.js";
import { openThunderT2 } from "./t2-open.js";
import {
  EFIELD_ABS_THRESHOLDS,
  LIGHTNING_FREQ_THRESHOLDS,
} from "./thresholds.js";

const SPLIT_LINE = {
  show: true,
  lineStyle: { color: "rgba(148, 163, 184, 0.18)", type: "dashed", width: 1 },
};

const PANELS = [
  {
    key: "efield",
    title: "Eabs 电场强度 · 参考 3/7/9 kV/m",
    field: "Eabs",
    unit: " kV/m",
    colorMode: "solid",
    solidColorToken: "chart_2",
    solidFallback: "#38bdf8",
    thresholds: EFIELD_ABS_THRESHOLDS,
  },
  {
    key: "frequency",
    title: "闪电频次 5min · 参考 ≥1次 / >3次 / >5次",
    field: "次数",
    unit: " 次",
    colorMode: "solid",
    solidColorToken: "status_error",
    solidFallback: "#f87171",
    thresholds: LIGHTNING_FREQ_THRESHOLDS,
  },
];

function maxOf(rows, field) {
  let max = 0;
  for (const row of rows) {
    const n = Number(row?.[field]);
    if (Number.isFinite(n) && n > max) max = n;
  }
  return max || 0;
}

function scaleMax(dataMax, thresholds) {
  let max = Math.max(1, Number(dataMax) || 1);
  for (const t of thresholds || []) {
    const v = Number(t?.value);
    if (Number.isFinite(v) && v > max) max = v;
  }
  return Math.ceil(max * 1.08 * 10) / 10;
}

function markLineFromThresholds(thresholds, typography) {
  const list = Array.isArray(thresholds) ? thresholds : [];
  if (!list.length) return undefined;
  return {
    symbol: "none",
    silent: true,
    animation: false,
    data: list.map((t) => ({
      yAxis: Number(t.value),
      name: String(t.label || t.tag || t.value),
      lineStyle: {
        type: "dashed",
        width: 1.25,
        color: t.color,
        opacity: 0.92,
      },
      label: {
        show: true,
        formatter: String(t.tag || t.label || t.value),
        position: "insideStartTop",
        color: t.color,
        fontSize: Math.max(10, Math.min(12, (typography?.unit || 12) - 2)),
        fontWeight: 600,
        backgroundColor: "rgba(8, 24, 48, 0.78)",
        padding: [1, 4],
        borderRadius: 2,
      },
    })),
  };
}

function buildBarOption({ host, rows, panel, playbackAt }) {
  const list = Array.isArray(rows) ? rows : [];
  const typography = readThemeTypography(host);
  const muted = readThemeColor(host, "text_muted") || "#94a3b8";
  const body = readThemeColor(host, "text_body") || "#e2e8f0";
  const fontFamily = readThemeUiFontFamily(host) || "sans-serif";
  const categories = list.map((row) => String(row?.["时段"] ?? ""));
  const dataMax = maxOf(list, panel.field);
  const yMax = scaleMax(dataMax, panel.thresholds);
  const solid =
    readThemeColor(host, panel.solidColorToken) || panel.solidFallback;
  const labelSize = Math.max(10, (typography.unit || 12) - 1);

  const data = list.map((row) => {
    const x = String(row?.["时段"] ?? "");
    const v = Number(row?.[panel.field] ?? 0);
    const active = x === playbackAt;
    return {
      value: Number.isFinite(v) ? v : 0,
      itemStyle: {
        color: solid,
        opacity: active ? 1 : 0.72,
        borderColor: active ? "rgba(255,255,255,0.55)" : "transparent",
        borderWidth: active ? 1 : 0,
        borderRadius: [2, 2, 0, 0],
      },
    };
  });

  const markLine = markLineFromThresholds(panel.thresholds, typography);

  return {
    backgroundColor: "transparent",
    animationDuration: 280,
    textStyle: { fontFamily, color: muted },
    grid: {
      left: 28,
      right: 8,
      top: markLine ? 18 : 10,
      bottom: 22,
      containLabel: false,
    },
    tooltip: {
      trigger: "axis",
      axisPointer: { type: "shadow" },
      backgroundColor: "rgba(8, 24, 48, 0.92)",
      borderColor: "rgba(56, 189, 248, 0.45)",
      borderWidth: 1,
      textStyle: {
        color: body,
        fontSize: typography.label || 12,
        fontFamily,
      },
      formatter(params) {
        const items = Array.isArray(params) ? params : [params];
        const head = items[0];
        if (!head) return "";
        const name = String(head.axisValueLabel ?? head.name ?? "");
        const lines = items
          .filter((p) => p.seriesType === "bar")
          .map((p) => `${p.marker}${panel.field}: <b>${p.value}${panel.unit || ""}</b>`);
        const refs = (panel.thresholds || [])
          .map((t) => `<span style="color:${t.color}">${escapeHtml(t.tag)}</span>`)
          .join(" · ");
        return [`<div>${escapeHtml(name)}</div>`, ...lines, refs ? `<div style="margin-top:4px;opacity:.85">${refs}</div>` : ""]
          .filter(Boolean)
          .join("");
      },
    },
    xAxis: {
      type: "category",
      data: categories,
      axisTick: { show: false },
      axisLine: { lineStyle: { color: "rgba(148,163,184,0.35)" } },
      axisLabel: {
        color: muted,
        fontSize: labelSize,
        fontFamily,
        hideOverlap: true,
        interval: 0,
        rotate: categories.length > 8 ? 30 : 0,
      },
    },
    yAxis: {
      type: "value",
      min: 0,
      max: yMax,
      splitNumber: 4,
      axisTick: { show: false },
      axisLine: { show: false },
      axisLabel: {
        color: muted,
        fontSize: labelSize,
        fontFamily,
        formatter: (v) => String(v),
      },
      splitLine: SPLIT_LINE,
    },
    series: [
      {
        type: "bar",
        name: panel.field,
        data,
        barMaxWidth: 18,
        barCategoryGap: "28%",
        markLine,
        z: 2,
      },
    ],
  };
}

/** P0 无真实图库时用 SVG 占位，带时间戳可读 */
function opticalThumbSrc(frame, index) {
  const explicit = String(frame?.thumb || frame?.url || "").trim();
  if (explicit) return explicit;
  const at = escapeHtml(String(frame?.at || "").trim() || `F${index + 1}`);
  const hue = 200 + (index % 5) * 12;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 90">
    <defs>
      <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="hsl(${hue},55%,18%)"/>
        <stop offset="100%" stop-color="hsl(${hue + 20},40%,8%)"/>
      </linearGradient>
    </defs>
    <rect width="120" height="90" fill="url(#g)"/>
    <path d="M62 12 L48 48 H58 L52 78 L78 40 H66 Z" fill="rgba(250,204,21,0.85)" stroke="rgba(255,255,255,0.35)" stroke-width="1"/>
    <text x="8" y="82" fill="rgba(226,232,240,0.9)" font-size="11" font-family="sans-serif">${at}</text>
  </svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function renderOpticalGrid(frames, playbackAt) {
  const list = Array.isArray(frames) ? frames : [];
  if (!list.length) {
    return `<div class="optical-empty">暂无光学帧</div>`;
  }
  return `<div class="optical-grid" role="list">
    ${list
      .map((frame, index) => {
        const at = String(frame?.at || "").trim();
        const site = String(frame?.site || frame?.site_id || "").trim();
        const active = at && playbackAt && at.slice(0, 5) === playbackAt.slice(0, 5);
        const title = [at, site, "光学帧"].filter(Boolean).join(" · ");
        return `<button type="button" class="optical-tile${active ? " is-active" : ""}" role="listitem" title="${escapeAttr(
          title,
        )}" data-frame-id="${escapeAttr(frame?.id || `f-${index}`)}">
          <img src="${escapeAttr(opticalThumbSrc(frame, index))}" alt="${escapeAttr(at || "光学帧")}" loading="lazy" />
          <span class="optical-cap">${escapeHtml(at || "—")}</span>
        </button>`;
      })
      .join("")}
  </div>`;
}

class MeiThunderEventCharts extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    this._charts = [];
    this._state = getThunderStore();
    this.style.display = "block";
    this.style.width = "100%";
    this.style.height = "100%";
    this.style.minHeight = "0";
    this.style.overflow = "hidden";
    this.style.boxSizing = "border-box";
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this._unsub = subscribeThunderState((detail) => {
      this._state = detail || getThunderStore();
      this.schedulePaint();
    });
    this._deferUntilVisibleCleanup = deferUntilDisplayed(this, () => {
      this._deferUntilVisibleCleanup = null;
      this.bootstrap();
    });
  }

  bootstrap() {
    this.renderShell();
    this.resizeObserver = new ResizeObserver(() => {
      this._charts.forEach((chart) => chart?.resize?.());
    });
    this.resizeObserver.observe(this);
    this.schedulePaint();
  }

  disconnectedCallback() {
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    if (typeof this._unsub === "function") {
      this._unsub();
      this._unsub = null;
    }
    if (this._paintRaf != null) {
      cancelAnimationFrame(this._paintRaf);
      this._paintRaf = null;
    }
    if (this.resizeObserver) {
      this.resizeObserver.disconnect();
      this.resizeObserver = null;
    }
    this._charts.forEach((chart) => chart?.dispose?.());
    this._charts = [];
  }

  renderShell() {
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          height: 100%;
          min-height: 0;
          max-height: 100%;
          overflow: hidden;
          box-sizing: border-box;
          ${cockpitCssVars()}
          font-family: var(--cockpit-font-family-ui);
        }
        .root {
          display: grid;
          /* 上：两图均分；下：光学帧（约四成高度，内部滚动） */
          grid-template-rows: minmax(0, 1.15fr) minmax(0, 0.85fr);
          gap: 6px;
          width: 100%;
          height: 100%;
          min-height: 0;
          box-sizing: border-box;
        }
        .charts {
          display: grid;
          grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
          gap: 6px;
          min-height: 0;
          overflow: hidden;
        }
        .panel {
          display: flex;
          flex-direction: column;
          min-width: 0;
          min-height: 0;
          max-height: 100%;
          padding: 4px 6px 2px;
          border-radius: 4px;
          background: transparent;
          border: 1px solid rgba(56, 160, 240, 0.28);
          box-sizing: border-box;
          overflow: hidden;
        }
        .title {
          flex: 0 0 auto;
          font-size: ${COCKPIT_TYPE.chartTitle};
          line-height: 1.2;
          color: ${color("text_muted")};
          margin-bottom: 2px;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .chart {
          flex: 1 1 auto;
          width: 100%;
          min-height: 0;
          min-width: 0;
          position: relative;
        }
        .chart .empty,
        .optical-empty {
          position: absolute;
          inset: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          color: ${color("text_muted")};
          font-size: ${COCKPIT_TYPE.chartLabel};
          text-align: center;
          padding: 8px;
        }
        .optical-empty {
          position: static;
          min-height: 64px;
        }
        .optical {
          display: flex;
          flex-direction: column;
          min-height: 0;
          overflow: hidden;
        }
        .optical-scroll {
          flex: 1 1 auto;
          min-height: 0;
          overflow-x: hidden;
          overflow-y: auto;
          padding-right: 2px;
        }
        .optical-grid {
          display: grid;
          grid-template-columns: repeat(4, minmax(0, 1fr));
          gap: 6px;
          align-content: start;
        }
        .optical-tile {
          display: flex;
          flex-direction: column;
          gap: 2px;
          margin: 0;
          padding: 0;
          border: 1px solid rgba(56, 160, 240, 0.28);
          border-radius: 4px;
          background: rgba(2, 12, 28, 0.55);
          cursor: pointer;
          overflow: hidden;
          min-width: 0;
        }
        .panel[data-panel="efield"] .chart,
        .panel[data-panel="frequency"] .chart {
          cursor: pointer;
        }
        .optical-tile.is-active {
          border-color: rgba(56, 189, 248, 0.85);
          box-shadow: 0 0 0 1px rgba(56, 189, 248, 0.35);
        }
        .optical-tile img {
          display: block;
          width: 100%;
          aspect-ratio: 4 / 3;
          object-fit: cover;
          background: rgba(8, 24, 48, 0.9);
        }
        .optical-cap {
          font-size: ${COCKPIT_TYPE.chartLabel};
          line-height: 1.2;
          color: ${color("text_muted")};
          text-align: center;
          padding: 0 2px 3px;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
      </style>
      <div class="root">
        <div class="charts">
          ${PANELS.map(
            (panel, idx) => `
            <div class="panel" data-panel="${escapeHtml(panel.key)}">
              <div class="title">${escapeHtml(panel.title)}</div>
              <div class="chart" data-idx="${idx}"></div>
            </div>`,
          ).join("")}
        </div>
        <div class="panel optical" data-panel="optical">
          <div class="title">光学帧</div>
          <div class="optical-scroll" data-optical></div>
        </div>
      </div>
    `;
    this._chartEls = [...this.shadowRoot.querySelectorAll(".chart")];
    this._opticalEl = this.shadowRoot.querySelector("[data-optical]");
    this.bindT2Clicks();
  }

  bindT2Clicks() {
    if (!this.shadowRoot || this._t2Bound) return;
    this._t2Bound = true;
    this.shadowRoot.addEventListener("click", (event) => {
      const panel = event.target?.closest?.("[data-panel]");
      if (!panel) return;
      const key = panel.getAttribute("data-panel");
      if (key === "efield") {
        openThunderT2("efield", { host: this });
        return;
      }
      if (key === "frequency") {
        openThunderT2("lightning", { host: this });
        return;
      }
      if (key === "optical") {
        openThunderT2("optical", { host: this });
      }
    });
  }

  schedulePaint() {
    if (this._paintRaf != null) cancelAnimationFrame(this._paintRaf);
    this._paintRaf = requestAnimationFrame(() => {
      this._paintRaf = null;
      this.paint().catch(() => {});
    });
  }

  async paint() {
    if (!this.shadowRoot || !this._chartEls?.length) return;
    const echarts = await ensureEChartsGlobal();
    if (!this.isConnected) return;
    const event = this._state?.event || null;
    const playbackAt = String(this._state?.playbackAt || "").trim();
    const charts = event?.charts || {};
    const seriesByKey = {
      efield: charts.efield,
      frequency: charts.frequency,
    };

    for (let i = 0; i < PANELS.length; i += 1) {
      const panel = PANELS[i];
      const el = this._chartEls[i];
      if (!el) continue;
      const rows = seriesByKey[panel.key];
      const empty = !Array.isArray(rows) || rows.length === 0;

      if (empty) {
        if (this._charts[i]) {
          this._charts[i].dispose();
          this._charts[i] = null;
        }
        el.innerHTML = `<div class="empty">无数据</div>`;
        continue;
      }

      if (el.querySelector(".empty")) el.innerHTML = "";
      let chart = this._charts[i];
      if (!chart) {
        chart = echarts.init(el, null, { renderer: "canvas" });
        this._charts[i] = chart;
      }
      chart.setOption(
        buildBarOption({
          host: this,
          rows,
          panel,
          playbackAt,
        }),
        true,
      );
      chart.resize();
    }

    if (this._opticalEl) {
      this._opticalEl.innerHTML = renderOpticalGrid(event?.opticalFrames, playbackAt);
    }
  }
}

if (!customElements.get("mei-thunder-event-charts")) {
  customElements.define("mei-thunder-event-charts", MeiThunderEventCharts);
}
