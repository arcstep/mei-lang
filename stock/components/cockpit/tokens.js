/**
 * 驾驶舱大屏设计 token（群腐 chrome，Sketch @3x 逻辑尺寸）。
 */

export const QUNFU_LAYOUT = {
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

/** 字号由 scene `theme.font` 的 --mei-font-* 驱动，此处仅作 fallback */
export const QUNFU_TYPE = {
  headerTitle: "var(--mei-font-4, 36px)",
  panelTitle: "20px",
  panelTitleCompact: "18px",
  panelTitleLetterSpacing: "0.12em",
  panelTitleLetterSpacingWide: "0.08em",
  metricLabel: "var(--mei-font-2, 14px)",
  metricValue: "var(--mei-font-5, 24px)",
  metricUnit: "var(--mei-font-1, 12px)",
};

/** 标题+内容一体外框（对齐 module-bg 描边 #34526C） */
export const QUNFU_SECTION_SHELL = {
  border: "1px solid rgba(52, 82, 108, 0.5)",
  radius: "4px",
  /** 稿面板块间透出全页 bg，不在外壳再叠一层渐变 */
  fill: "transparent",
};

export const QUNFU_COLOR = {
  headerTitle: "#d8f0ff",
  panelTitle: "#ecfeff",
  metricLabel: "#94a3b8",
  metricValue: "#f0f9ff",
  metricValueRate: "#fde68a",
  metricUnit: "#7dd3fc",
};

/** Sketch caret：520 栏内约 x=150 / x=384（相对栏左缘 ~136px / ~370px） */
export const QUNFU_PANEL_TITLE_CARET = {
  compact: { left: 0.262, right: 0.712 },
  wide: { left: 0.262, right: 0.712 },
};

export const QUNFU_SPACING = {
  panelTitlePadding: "0",
  /** 问题办理 body y=313，图标 y=348 */
  statusDeckPadTop: "35px",
  /** 监督成效 body y=583，首行标签 y=616 */
  effectDeckPadTop: "33px",
  metricCardPadding: "29px 8px 14px",
  statusCardPadding: "12px 14px 12px 12px",
  statusIconGap: 12,
};

export const QUNFU_FONT = {
  headerFamily:
    'var(--mei-font-family-header, "YouSheBiaoTiHei", "YouShe BiaoTiHei", "DIN Alternate", "Microsoft YaHei", sans-serif)',
  uiFamily:
    'var(--mei-font-family-ui, "Microsoft YaHei", "PingFang SC", "DIN Alternate", sans-serif)',
};

export const QUNFU_SHADOW = {
  headerTitle: "0 20px 30px #0091ff, 0 0 4px #0d74c2",
  panelTitle: "0 0 10px rgba(0, 145, 255, 0.55), 0 0 2px rgba(13, 116, 194, 0.9)",
};

/** 注入为 :host 可用的 CSS 变量块 */
export function qunfuCssVars() {
  const L = QUNFU_LAYOUT;
  const T = QUNFU_TYPE;
  const C = QUNFU_COLOR;
  const S = QUNFU_SPACING;
  return `
    --qunfu-panel-width: ${L.panelWidth}px;
    --qunfu-header-height: ${L.headerHeight}px;
    --qunfu-cap-width: ${L.headerCapWidth}px;
    --qunfu-cap-height: ${L.headerCapHeight}px;
    --qunfu-cap-top: ${L.headerCapTop}px;
    --qunfu-panel-title-h: ${L.panelTitleWide}px;
    --qunfu-metric-h: ${L.metricCardHeight}px;
    --qunfu-cell-w: ${L.metricCellWidth}px;
    --qunfu-cell-h: ${L.metricCellHeight}px;
    --qunfu-cell-gap: ${L.metricCellGap}px;
    --qunfu-status-h: ${L.statusCardHeight}px;
    --qunfu-section-gap: ${L.sectionGap}px;
    --qunfu-metric-gap: ${L.metricColGap}px;
    --qunfu-font-header: ${T.headerTitle};
    --qunfu-font-panel: ${T.panelTitle};
    --qunfu-font-label: ${T.metricLabel};
    --qunfu-font-value: ${T.metricValue};
    --qunfu-font-unit: ${T.metricUnit};
    --qunfu-font-family-ui: ${QUNFU_FONT.uiFamily};
    --qunfu-font-family-header: ${QUNFU_FONT.headerFamily};
    --qunfu-color-header: ${C.headerTitle};
    --qunfu-color-panel: ${C.panelTitle};
    --qunfu-color-label: ${C.metricLabel};
    --qunfu-color-value: ${C.metricValue};
    --qunfu-color-value-rate: ${C.metricValueRate};
    --qunfu-color-unit: ${C.metricUnit};
    --qunfu-metric-pad: ${S.metricCardPadding};
    --qunfu-panel-pad: ${S.panelTitlePadding};
  `;
}
