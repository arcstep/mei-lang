import { color } from "../mei/theme-style.js";
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
 * MeiLang viewport Z bands (SSOT with app-shell.css --mei-z-*).
 * T0: 0–1000 | T1: 1001–2000 | T2: 2001–3000 | P: 5000–5399 | C: 5400–5799 | Host: 5800+
 */
export const COCKPIT_Z_INDEX = {
  map: 1,
  panel: 1001,
  header: 1110,
  mapTools: 1210,
  tooltip: 1300,
  drilldown: 2001,
  drilldownBoard: 2010,
  layer2Workspace: 2001,
  drilldownContext: 2210,
  filterFloatingPanel: 2250,
  /** T2 board 内 ECharts / 地图飘窗 */
  tooltipInBoard: 2300,
  /** 长文本「查看全文」飘窗 */
  textPopover: 2350,
};

export const PRESENTATION_Z_INDEX = {
  slide: 5000,
  caption: 5100,
  spaLoading: 5050,
};

export const COPILOT_Z_INDEX = {
  assistant: 5400,
  drawer: 5450,
  fab: 5500,
  fabElevated: 5510,
  overlay: 5520,
  accessChat: 5410,
  accessChatOverlay: 5420,
};

export const HOST_Z_INDEX = {
  feedback: 5800,
  heartbeat: 5810,
};

/** @deprecated use COCKPIT_Z_INDEX + PRESENTATION_Z_INDEX + COPILOT_Z_INDEX */
export const MEI_Z_LAYERS = {
  t0: { min: 0, max: 1000, default: COCKPIT_Z_INDEX.map },
  t1: { min: 1001, max: 2000, default: COCKPIT_Z_INDEX.panel },
  t2: { min: 2001, max: 3000, default: COCKPIT_Z_INDEX.drilldown },
  presentation: { min: 5000, max: 5399, default: PRESENTATION_Z_INDEX.slide },
  copilot: { min: 5400, max: 5799, default: COPILOT_Z_INDEX.assistant },
  host: { min: 5800, max: 99999, default: HOST_Z_INDEX.feedback },
};

/** 字号由 theme 文字角色配方（--mei-*-font-size / .mei-text-*）驱动，字阶仅作 fallback */
export const COCKPIT_TYPE = {
  headerTitle: "var(--mei-header-title-font-size, var(--mei-font-5, var(--mei-font-4, 32px)))",
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
  body: "var(--mei-body-font-size, var(--mei-font-2, 14px))",
  muted: "var(--mei-muted-font-size, var(--mei-font-1, 12px))",
};

/** Utility class names for composed text roles (prefer over ad-hoc font-size). */
export const COCKPIT_TEXT_CLASS = {
  headerTitle: "mei-text-header-title",
  panelHead: "mei-text-panel-head",
  body: "mei-text-body",
  muted: "mei-text-muted",
  metricLabel: "mei-text-metric-label",
  metricValue: "mei-text-metric-value",
  metricUnit: "mei-text-metric-unit",
  chartTitle: "mei-text-chart-title",
  chartLabel: "mei-text-chart-label",
  tableHead: "mei-text-table-head",
  tableBody: "mei-text-table-body",
  filterPanel: "mei-text-filter-panel",
};

const CHART_COLOR_KEYS = ["chart_1", "chart_2", "chart_3", "chart_4", "chart_5", "chart_6"];
const CHART_CATEGORICAL_COLOR_KEYS = [
  "chart_cat_1",
  "chart_cat_2",
  "chart_cat_3",
  "chart_cat_4",
  "chart_cat_5",
  "chart_cat_6",
  "chart_cat_7",
  "chart_cat_8",
];

/** 静态 fallback（无 DOM / 无 theme 注入时）— 默认绿色系单色阶梯（ECharts 需实色 hex） */
export const COCKPIT_CHART_PALETTE_FALLBACK = [
  "#d1fae5",
  "#a7f3d0",
  "#6ee7b7",
  "#34d399",
  "#10b981",
  "#059669",
];

/** 饼/环/玫瑰分类色板 fallback（避开红/黄/蓝预警业务色） */
export const COCKPIT_CHART_CATEGORICAL_PALETTE_FALLBACK = [
  "#14b8a6",
  "#22c55e",
  "#f97316",
  "#8b5cf6",
  "#0ea5e9",
  "#ec4899",
  "#84cc16",
  "#64748b",
];

/** @deprecated 请用 readThemeChartPalette(host)；保留作静态 fallback */
export const COCKPIT_CHART_PALETTE = [...COCKPIT_CHART_PALETTE_FALLBACK];

import { fallbackColor, fallbackShadow } from "../mei/theme-fallback.js";

/** theme.tokens.color.* → CSS var（fallback 仅来自 theme-fallback.js） */
/** @deprecated 第二参数 fallback 已废弃，请改用 `mei/theme-style.js` 的 `color()` */
export function themeColor(name, fallback) {
  const key = String(name ?? "")
    .trim()
    .replace(/_/g, "-");
  if (!key) {
    return fallback ?? "inherit";
  }
  const fb =
    fallback != null && String(fallback).length > 0
      ? fallback
      : fallbackColor(String(name ?? "").trim());
  return `var(--mei-color-${key}, ${fb})`;
}

/** theme.tokens.shadow.* → CSS var */
export function themeShadow(name, fallback) {
  const key = String(name ?? "")
    .trim()
    .replace(/_/g, "-");
  if (!key) {
    return fallback ?? "none";
  }
  const fb =
    fallback != null && String(fallback).length > 0
      ? fallback
      : fallbackShadow(String(name ?? "").trim());
  return `var(--mei-shadow-${key}, ${fb})`;
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

/**
 * 主题最小字号（px）：
 * 1) tokens.typography.min_font_size → --mei-typography-min-font-size
 * 2) 否则字阶最低档 --mei-font-1
 */
export function readThemeMinFontPx(host, fallback = 16) {
  if (typeof window !== "undefined" && host instanceof Element) {
    const style = window.getComputedStyle(host);
    const explicit = parseThemeFontPx(
      style.getPropertyValue("--mei-typography-min-font-size").trim(),
      0,
    );
    if (explicit > 0) {
      return Math.round(explicit);
    }
  }
  return Math.round(readThemeFontPx(host, "1", fallback));
}

/** 将任意派生字号钳到主题最小字号（禁止图表写死 10/11px 等） */
export function clampThemeFontPx(host, px, fallback = 16) {
  const minPx = readThemeMinFontPx(host, fallback);
  const n = Number(px);
  const size = Number.isFinite(n) && n > 0 ? n : minPx;
  return Math.max(minPx, Math.round(size));
}

/** ECharts / 运行时排版：读 theme 文字角色配方，再回退 font 档位，并钳到最小字阶 */
export function readThemeTypography(host) {
  const min = readThemeMinFontPx(host, 16);
  let chartTitle = 18;
  let body = 18;
  if (typeof window !== "undefined" && host instanceof Element) {
    const style = window.getComputedStyle(host);
    chartTitle = parseThemeFontPx(style.getPropertyValue("--mei-chart-title-font-size"), 0);
    if (chartTitle <= 0) {
      chartTitle = readThemeFontPx(host, "2", 18);
    }
    body = parseThemeFontPx(style.getPropertyValue("--mei-body-font-size"), 0);
    if (body <= 0) {
      body = readThemeFontPx(host, "2", 18);
    }
  } else {
    body = 18;
  }
  return {
    min,
    unit: Math.max(min, readThemeRoleFontPx(host, "--mei-metric-unit-font-size", "1", 16)),
    label: Math.max(min, readThemeRoleFontPx(host, "--mei-chart-label-font-size", "1", 16)),
    body: Math.max(min, body),
    value: Math.max(min, readThemeRoleFontPx(host, "--mei-metric-value-font-size", "3", 26)),
    chartTitle: Math.max(min, chartTitle),
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

/** 从宿主读取 theme.tokens.color.chart_cat_* 分类色板（饼/环/玫瑰按类目轮转） */
export function readThemeChartCategoricalPalette(host) {
  if (typeof window === "undefined" || !(host instanceof Element)) {
    return [...COCKPIT_CHART_CATEGORICAL_PALETTE_FALLBACK];
  }
  const style = window.getComputedStyle(host);
  const colors = CHART_CATEGORICAL_COLOR_KEYS.map((key, index) => {
    const cssKey = key.replace(/_/g, "-");
    const raw = style.getPropertyValue(`--mei-color-${cssKey}`).trim();
    return raw || COCKPIT_CHART_CATEGORICAL_PALETTE_FALLBACK[index];
  }).filter(Boolean);
  return colors.length > 0 ? colors : [...COCKPIT_CHART_CATEGORICAL_PALETTE_FALLBACK];
}

/** 从宿主 computed style 读取 theme.tokens.color.* 实色（ECharts/canvas 用） */
export function readThemeColor(host, name) {
  const token = String(name ?? "").trim();
  const key = token.replace(/_/g, "-");
  const fb = fallbackColor(token);
  if (!key) {
    return fb;
  }
  if (typeof window === "undefined" || !(host instanceof Element)) {
    return fb;
  }
  const raw = window.getComputedStyle(host).getPropertyValue(`--mei-color-${key}`).trim();
  return raw || fb;
}

/** 字面量色值直通；`var(--mei-color-*)` 或 token 名从宿主解析 */
export function resolveRuntimeColor(host, value, tokenName) {
  const text = String(value ?? "").trim();
  if (/^#([0-9a-f]{3,8})$/i.test(text)) {
    return text;
  }
  if (/^rgba?\(/i.test(text) || /^hsla?\(/i.test(text)) {
    return text;
  }
  if (text && !text.startsWith("var(")) {
    return readThemeColor(host, text);
  }
  return readThemeColor(host, String(tokenName ?? "").trim());
}

const STYLE_CSS_KEYWORDS = new Set([
  "solid",
  "dashed",
  "dotted",
  "double",
  "none",
  "hidden",
  "inset",
  "outset",
]);

function isStyleWordLiteral(word) {
  if (!word) {
    return true;
  }
  if (/^#([0-9a-f]{3,8})$/i.test(word)) {
    return true;
  }
  if (/^rgba?\([^)]*\)$/i.test(word) || /^hsla?\([^)]*\)$/i.test(word)) {
    return true;
  }
  if (/^var\([^)]*\)$/i.test(word)) {
    return true;
  }
  if (/^[\d.]+(px|rem|%)?$/.test(word)) {
    return true;
  }
  return false;
}

/** border / box-shadow 等复合值：逐词解析颜色 token */
export function resolveRuntimeStyleValue(host, value) {
  const text = String(value ?? "").trim();
  if (!text) {
    return text;
  }
  return text
    .split(/\s+/)
    .map((word) => {
      if (!word || STYLE_CSS_KEYWORDS.has(word) || isStyleWordLiteral(word)) {
        return word;
      }
      return resolveRuntimeColor(host, word, word);
    })
    .join(" ");
}

/** ECharts 文本 fontFamily：解析 --mei-font-family-ui */
export function readThemeUiFontFamily(host) {
  const fallback = '"Microsoft YaHei", "PingFang SC", "DIN Alternate", sans-serif';
  if (typeof window === "undefined" || !(host instanceof Element)) {
    return fallback;
  }
  const raw = window.getComputedStyle(host).getPropertyValue("--mei-font-family-ui").trim();
  return raw || fallback;
}

/** 标题+内容一体外框 */
export const COCKPIT_SECTION_SHELL = {
  border: `1px solid ${color("section_border")}`,
  radius: "4px",
  fill: "transparent",
};

/** 语义色别名（指向 --mei-color-* / --mei-metric-*-color） */
export const COCKPIT_COLOR = {
  headerTitle: color("text_primary"),
  panelTitle: color("panel_title"),
  metricLabel: "var(--mei-metric-label-color, var(--mei-color-text-muted, #94a3b8))",
  metricValue: "var(--mei-metric-value-color, var(--mei-color-text-value, #f0f9ff))",
  metricValueRate: color("text_accent"),
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
  headerTitle: themeShadow("header_title"),
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
