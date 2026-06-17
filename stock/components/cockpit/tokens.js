/**
 * 驾驶舱大屏设计 token（通用 chrome，Sketch @3x 逻辑尺寸）。
 * 颜色真源：theme.tokens.color → --mei-color-*（见项目 .mei-config.json ops.themes.cockpit）。
 */

export const COCKPIT_LAYOUT = {
  pageMargin: 14,
  panelWidth: 520,
  headerHeight: 72,
  headerCapWidth: 633,
  headerCapHeight: 62,
  headerCapTop: 8,
  panelTitleWide: 59,
  panelTitleCompact: 38,
  metricCardHeight: 102,
  /** 监督预警内容区 160−59≈101px */
  warningMetricHeight: 101,
  metricCardAspect: "520 / 102",
  metricCellWidth: 152,
  metricCellHeight: 74,
  metricCellGap: 8,
  statusCardHeight: 82,
  statusRateHeight: 82,
  sectionGap: 0,
  metricColGap: 10,
  /** 外框 panel-block 内指标区间距 */
  metricColGapEmbedded: 4,
  statusIconSize: 48,
  leftBlockHeight: 242,
  contentBodyHeight: 202,
  bottomSectionHeight: 262,
  widePanelWidth: 836,
  quadStatHeight: 60,
  mapWidth: 651,
  mapMinHeight: 508,
  /** 相对中栏内容左缘（与 center_stat x542 对齐后地图 x633） */
  mapOffsetLeft: 91,
  statStripHeight: 118,
  leftMiniRowHeight: 56,
  leftChartRowHeight: 64,
  sectionMinHeights: {
    supervisionWarning: 160,
    issueHandling: 262,
    supervisionEffect: 262,
    typicalCases: 262,
    leftBlock: 242,
  },
};

/**
 * 首屏驾驶舱叠层（自低到高）：底图 / GIS 图层 < 各板块 < GIS 工具 < 飘窗提示 < 二级看板
 * 与 app-shell.css `--mei-z-cockpit-*` 保持一致。
 */
export const COCKPIT_Z_INDEX = {
  map: 1,
  panel: 100,
  header: 110,
  mapTools: 1520,
  tooltip: 1550,
  drilldown: 1600,
  drilldownBoard: 1620,
};

/** 字号由 theme 语义角色（--mei-metric-*）与 font 档位（--mei-font-*）驱动，此处仅作 fallback */
export const COCKPIT_TYPE = {
  headerTitle: "var(--mei-font-5, var(--mei-font-4, 32px))",
  panelTitle: "var(--mei-panel-head-font-size, var(--mei-font-4, 32px))",
  panelTitleCompact: "var(--mei-panel-head-font-size, var(--mei-font-4, 32px))",
  panelTitleLetterSpacing: "0.12em",
  panelTitleLetterSpacingWide: "0.08em",
  metricLabel: "var(--mei-metric-label-font-size, var(--mei-font-2, 18px))",
  metricValue: "var(--mei-metric-value-font-size, var(--mei-font-3, 26px))",
  metricUnit: "var(--mei-metric-unit-font-size, var(--mei-font-1, 16px))",
  chartTitle: "var(--mei-chart-title-font-size, var(--mei-font-2, 18px))",
  chartLabel: "var(--mei-chart-label-font-size, var(--mei-font-1, 16px))",
  tableHead: "var(--mei-table-head-font-size, var(--mei-font-2, 18px))",
  tableBody: "var(--mei-table-body-font-size, var(--mei-font-2, 18px))",
  filterPanel: "var(--mei-filter-panel-font-size, var(--mei-font-2, 18px))",
};

const CHART_COLOR_KEYS = ["chart_1", "chart_2", "chart_3", "chart_4", "chart_5", "chart_6"];

/** 静态 fallback（无 DOM / 无 theme 注入时） */
export const COCKPIT_CHART_PALETTE_FALLBACK = [
  "#22d3ee",
  "#38bdf8",
  "#0ea5e9",
  "#0369a1",
  "#62beeb",
  "#475569",
];

/** @deprecated 请用 readThemeChartPalette(host)；保留作静态 fallback */
export const COCKPIT_CHART_PALETTE = [...COCKPIT_CHART_PALETTE_FALLBACK];

/** theme.tokens.color.* → CSS var（带 fallback） */
export function themeColor(name, fallback) {
  const key = String(name ?? "")
    .trim()
    .replace(/_/g, "-");
  if (!key) {
    return fallback ?? "inherit";
  }
  return fallback != null && String(fallback).length > 0
    ? `var(--mei-color-${key}, ${fallback})`
    : `var(--mei-color-${key})`;
}

/** theme.tokens.shadow.* → CSS var */
export function themeShadow(name, fallback) {
  const key = String(name ?? "")
    .trim()
    .replace(/_/g, "-");
  if (!key) {
    return fallback ?? "none";
  }
  return fallback != null && String(fallback).length > 0
    ? `var(--mei-shadow-${key}, ${fallback})`
    : `var(--mei-shadow-${key})`;
}

export function parseThemeFontPx(raw, fallback) {
  const value = Number.parseFloat(String(raw ?? "").trim());
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

/** 从宿主元素读取 theme.font 注入的 --mei-font-*（px 数值） */
export function readThemeFontPx(host, tokenKey, fallback) {
  if (typeof window === "undefined" || !(host instanceof Element)) {
    return fallback;
  }
  const style = window.getComputedStyle(host);
  const raw = style.getPropertyValue(`--mei-font-${String(tokenKey ?? "").trim()}`).trim();
  return parseThemeFontPx(raw, fallback);
}

/** 优先读 theme 语义角色变量（--mei-metric-*-font-size），再回退 font 档位 */
export function readThemeRoleFontPx(host, roleCssVar, fontTokenKey, fallback) {
  if (typeof window === "undefined" || !(host instanceof Element)) {
    return fallback;
  }
  const style = window.getComputedStyle(host);
  const fromRole = parseThemeFontPx(style.getPropertyValue(roleCssVar).trim(), 0);
  if (fromRole > 0) {
    return fromRole;
  }
  return readThemeFontPx(host, fontTokenKey, fallback);
}

/** ECharts / 运行时排版：读 theme metric_* / chart_title 语义角色，再回退 font 档位 */
export function readThemeTypography(host) {
  let chartTitle = 18;
  if (typeof window !== "undefined" && host instanceof Element) {
    const style = window.getComputedStyle(host);
    chartTitle = parseThemeFontPx(style.getPropertyValue("--mei-chart-title-font-size"), 0);
    if (chartTitle <= 0) {
      chartTitle = readThemeFontPx(host, "2", 18);
    }
  }
  return {
    unit: readThemeRoleFontPx(host, "--mei-metric-unit-font-size", "1", 16),
    label: readThemeRoleFontPx(host, "--mei-chart-label-font-size", "1", 16),
    body: readThemeFontPx(host, "2", 18),
    value: readThemeRoleFontPx(host, "--mei-metric-value-font-size", "3", 26),
    chartTitle,
  };
}

/** 从宿主读取 theme.tokens.color.chart_* 色板（ECharts 等运行时消费） */
export function readThemeChartPalette(host) {
  if (typeof window === "undefined" || !(host instanceof Element)) {
    return [...COCKPIT_CHART_PALETTE_FALLBACK];
  }
  const style = window.getComputedStyle(host);
  const colors = CHART_COLOR_KEYS.map((key, index) => {
    const cssKey = key.replace(/_/g, "-");
    const raw = style.getPropertyValue(`--mei-color-${cssKey}`).trim();
    return raw || COCKPIT_CHART_PALETTE_FALLBACK[index];
  }).filter(Boolean);
  return colors.length > 0 ? colors : [...COCKPIT_CHART_PALETTE_FALLBACK];
}

/** 标题+内容一体外框 */
export const COCKPIT_SECTION_SHELL = {
  border: `1px solid ${themeColor("section_border", "rgba(52, 82, 108, 0.5)")}`,
  radius: "4px",
  fill: "transparent",
};

/** 语义色别名（指向 --mei-color-* / --mei-metric-*-color） */
export const COCKPIT_COLOR = {
  headerTitle: themeColor("text_primary", "#d8f0ff"),
  panelTitle: themeColor("panel_title", "#ecfeff"),
  metricLabel: "var(--mei-metric-label-color, var(--mei-color-text-muted, #94a3b8))",
  metricValue: "var(--mei-metric-value-color, var(--mei-color-text-value, #f0f9ff))",
  metricValueRate: themeColor("text_accent", "#fde68a"),
  metricUnit: "var(--mei-metric-unit-color, var(--mei-color-text-unit, #7dd3fc))",
};

/** Sketch caret：520 栏内约 x=150 / x=384（相对栏左缘 ~136px / ~370px） */
export const COCKPIT_PANEL_TITLE_CARET = {
  compact: { left: 0.262, right: 0.712 },
  wide: { left: 0.262, right: 0.712 },
};

export const COCKPIT_SPACING = {
  panelTitlePadding: "0",
  /** 问题办理 body y=313，图标 y=348 */
  statusDeckPadTop: "35px",
  /** 监督成效 body y=583，首行标签 y=616 */
  effectDeckPadTop: "33px",
  metricCardPadding: "29px 8px 14px",
  statusCardPadding: "12px 14px 12px 12px",
  statusIconGap: 12,
};

export const COCKPIT_FONT = {
  headerFamily:
    'var(--mei-font-family-header, "YouSheBiaoTiHei", "YouShe BiaoTiHei", "DIN Alternate", "Microsoft YaHei", sans-serif)',
  uiFamily:
    'var(--mei-font-family-ui, "Microsoft YaHei", "PingFang SC", "DIN Alternate", sans-serif)',
};

export const COCKPIT_SHADOW = {
  headerTitle: themeShadow("header_title", "0 20px 30px #0091ff, 0 0 4px #0d74c2"),
  panelTitle: themeShadow(
    "panel_title",
    "0 0 10px rgba(0, 145, 255, 0.55), 0 0 2px rgba(13, 116, 194, 0.9)",
  ),
};

/** 注入为 :host 可用的 CSS 变量块（别名层，指向 theme --mei-* 语义变量） */
export function cockpitCssVars() {
  const L = COCKPIT_LAYOUT;
  const T = COCKPIT_TYPE;
  const C = COCKPIT_COLOR;
  const S = COCKPIT_SPACING;
  return `
    --cockpit-panel-width: ${L.panelWidth}px;
    --cockpit-header-height: ${L.headerHeight}px;
    --cockpit-cap-width: ${L.headerCapWidth}px;
    --cockpit-cap-height: ${L.headerCapHeight}px;
    --cockpit-cap-top: ${L.headerCapTop}px;
    --cockpit-panel-title-h: ${L.panelTitleWide}px;
    --cockpit-metric-h: ${L.metricCardHeight}px;
    --cockpit-cell-w: ${L.metricCellWidth}px;
    --cockpit-cell-h: ${L.metricCellHeight}px;
    --cockpit-cell-gap: ${L.metricCellGap}px;
    --cockpit-status-h: ${L.statusCardHeight}px;
    --cockpit-section-gap: ${L.sectionGap}px;
    --cockpit-metric-gap: ${L.metricColGap}px;
    --cockpit-font-header: ${T.headerTitle};
    --cockpit-font-panel: ${T.panelTitle};
    --cockpit-font-label: ${T.metricLabel};
    --cockpit-font-value: ${T.metricValue};
    --cockpit-font-unit: ${T.metricUnit};
    --cockpit-font-chart-title: ${T.chartTitle};
    --cockpit-font-chart-label: ${T.chartLabel};
    --cockpit-font-table-head: ${T.tableHead};
    --cockpit-font-table-body: ${T.tableBody};
    --cockpit-font-filter: ${T.filterPanel};
    --cockpit-font-family-ui: ${COCKPIT_FONT.uiFamily};
    --cockpit-font-family-header: ${COCKPIT_FONT.headerFamily};
    --cockpit-color-header: ${C.headerTitle};
    --cockpit-color-panel: ${C.panelTitle};
    --cockpit-color-label: ${C.metricLabel};
    --cockpit-color-value: ${C.metricValue};
    --cockpit-color-value-rate: ${C.metricValueRate};
    --cockpit-color-unit: ${C.metricUnit};
    --cockpit-metric-pad: ${S.metricCardPadding};
    --cockpit-panel-pad: ${S.panelTitlePadding};
  `;
}
