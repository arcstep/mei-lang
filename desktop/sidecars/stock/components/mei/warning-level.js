/**
 * 风险等级 / 预警等级业务色渲染。
 * - 风险等级：可含多色（蓝/黄/红）→ 三个连续色块
 * - 预警等级：单值 → 单个矩形块
 * 色值真源：theme.tokens.color.warning_level_*（可用 app.toml 覆盖）。
 */

import { readThemeColor } from "../cockpit/tokens.js";

/** 展示顺序：高 → 低 */
export const WARNING_LEVEL_KEYS = ["红", "黄", "蓝"];

const SEVERITY_RANK = { 红: 3, 黄: 2, 蓝: 1 };

const FALLBACK_COLORS = {
  红: "#E53935",
  黄: "#FFB300",
  蓝: "#1E88E5",
  灰: "#90A4AE",
};

export function isWarningLevelColumnKey(key) {
  const name = String(key || "").trim();
  return name === "风险等级" || name === "预警等级" || name === "级别" || name === "level";
}

/** 风险等级：多色组合，三色块 */
export function isRiskLevelMultiColumnKey(key) {
  return String(key || "").trim() === "风险等级";
}

/** 预警等级：单值，单色块 */
export function isAlertLevelSingleColumnKey(key) {
  const name = String(key || "").trim();
  return name === "预警等级" || name === "级别" || name === "level";
}

export function isWarningLevelDimension(field) {
  return isWarningLevelColumnKey(field);
}

/**
 * @returns {"multi"|"single"|null}
 * 列名优先于 format.kind，避免二级看板缓存/推断仍用旧 kind 时误判。
 */
export function resolveWarningLevelDisplayMode(descriptorOrKey) {
  const key =
    descriptorOrKey && typeof descriptorOrKey === "object"
      ? String(descriptorOrKey?.key || "").trim()
      : String(descriptorOrKey || "").trim();
  if (isRiskLevelMultiColumnKey(key)) return "multi";
  if (isAlertLevelSingleColumnKey(key)) return "single";

  if (descriptorOrKey && typeof descriptorOrKey === "object") {
    const kind = String(descriptorOrKey?.format?.kind || descriptorOrKey?.kind || "")
      .trim()
      .toLowerCase()
      .replace(/-/g, "_");
    if (kind === "risk_level_blocks" || kind === "warning_level_blocks") return "multi";
    if (kind === "warning_level_block" || kind === "alert_level_block") return "single";
  }
  return null;
}

export function readWarningLevelColors(host) {
  return {
    红: readThemeColor(host, "warning_level_red") || FALLBACK_COLORS.红,
    黄: readThemeColor(host, "warning_level_yellow") || FALLBACK_COLORS.黄,
    蓝: readThemeColor(host, "warning_level_blue") || FALLBACK_COLORS.蓝,
    灰: readThemeColor(host, "warning_level_grey") || FALLBACK_COLORS.灰,
  };
}

/** 从「蓝/黄/红」「黄/红」「/」等解析出激活色集合 */
export function parseWarningLevelKeys(raw) {
  const text = String(raw ?? "")
    .trim()
    .replace(/^[\/\|、,\s]+|[\/\|、,\s]+$/g, "");
  if (!text || text === "-" || text === "—" || text === "无") return [];
  const found = [];
  for (const key of WARNING_LEVEL_KEYS) {
    if (text.includes(key)) found.push(key);
  }
  return found;
}

export function highestWarningLevelKey(raw) {
  const keys = parseWarningLevelKeys(raw);
  if (!keys.length) return null;
  return keys.slice().sort((a, b) => (SEVERITY_RANK[b] || 0) - (SEVERITY_RANK[a] || 0))[0];
}

export function resolveWarningLevelSliceColor(label, colors) {
  const palette = colors || FALLBACK_COLORS;
  const top = highestWarningLevelKey(label);
  if (!top) return palette.灰 || FALLBACK_COLORS.灰;
  return palette[top] || FALLBACK_COLORS.灰;
}

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function renderLevelItemHtml(key, on, colors) {
  const fill = on ? colors[key] : "transparent";
  const border = on ? colors[key] : colors.灰;
  return (
    `<span class="mei-warning-level-item${on ? " is-on" : " is-off"}" style="background:${escapeHtml(fill)};border-color:${escapeHtml(border)};width:48px;height:28px;max-width:48px;flex:0 0 48px">` +
    `<span class="mei-warning-level-label">${escapeHtml(key)}</span>` +
    `</span>`
  );
}

/** 风险等级：三个连续矩形色块（字在块内）；未激活仅空框 */
export function renderRiskLevelMultiBlocksHtml(raw, host) {
  const colors = readWarningLevelColors(host);
  const active = new Set(parseWarningLevelKeys(raw));
  const blocks = WARNING_LEVEL_KEYS.map((key) => renderLevelItemHtml(key, active.has(key), colors)).join("");
  return `<span class="mei-warning-level-blocks is-multi" title="${escapeHtml(String(raw ?? "").trim())}">${blocks}</span>`;
}

/** 预警等级：单个矩形块；无值仅空框 */
export function renderAlertLevelSingleBlockHtml(raw, host) {
  const colors = readWarningLevelColors(host);
  const top = highestWarningLevelKey(raw);
  const on = Boolean(top);
  const fill = on ? colors[top] : "transparent";
  const border = on ? colors[top] : colors.灰;
  const item =
    `<span class="mei-warning-level-item${on ? " is-on" : " is-off"}" style="background:${escapeHtml(fill)};border-color:${escapeHtml(border)};width:48px;height:28px;max-width:48px;flex:0 0 48px">` +
    `<span class="mei-warning-level-label">${escapeHtml(on ? top : "")}</span>` +
    `</span>`;
  return `<span class="mei-warning-level-blocks is-single" title="${escapeHtml(String(raw ?? "").trim())}">${item}</span>`;
}

/** 按列类型渲染：风险等级=三色块，预警等级=单色块 */
export function renderWarningLevelBlocksHtml(raw, host, descriptorOrKey = null) {
  const mode = resolveWarningLevelDisplayMode(descriptorOrKey);
  // 未知列名时：宁可不误做成三色块
  if (mode === "single" || mode == null) {
    if (mode === "single") return renderAlertLevelSingleBlockHtml(raw, host);
  }
  if (mode === "multi") return renderRiskLevelMultiBlocksHtml(raw, host);
  return renderAlertLevelSingleBlockHtml(raw, host);
}

export function warningLevelBlocksCss() {
  return `
    /* 覆盖 table chrome 的 .td-cell { padding: 8px 12px !important } */
    .td-cell:has(.is-warning-level) {
      align-items: center !important;
      justify-content: center !important;
      padding: 5px 4px !important;
    }
    .cell-inner.is-warning-level {
      display: flex !important;
      align-items: center;
      justify-content: center;
      width: 100%;
      max-width: 100%;
      min-width: 0;
      height: 100%;
      overflow: visible !important;
      box-sizing: border-box;
    }
    .mei-warning-level-blocks {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 3px;
      min-width: 0;
      line-height: 1;
    }
    /* 色块：宽高比不超过 2:1（固定 48×28 ≈ 1.71:1） */
    .mei-warning-level-item {
      display: inline-flex !important;
      align-items: center;
      justify-content: center;
      box-sizing: border-box;
      height: 28px !important;
      width: 48px !important;
      max-width: 48px !important;
      min-width: 0 !important;
      flex: 0 0 48px !important;
      border-radius: 3px;
      border: 1.5px solid currentColor;
    }
    /* 风险等级三连：0 间距、无圆角、贴合为一条带（相邻块去掉左边框避免双线缝） */
    .mei-warning-level-blocks.is-multi {
      width: auto;
      max-width: 100%;
      justify-content: center;
      gap: 0 !important;
    }
    .mei-warning-level-blocks.is-multi .mei-warning-level-item {
      flex: 0 0 48px !important;
      width: 48px !important;
      max-width: 48px !important;
      border-radius: 0 !important;
    }
    .mei-warning-level-blocks.is-multi .mei-warning-level-item + .mei-warning-level-item {
      margin-left: 0;
      border-left-width: 0 !important;
    }
    .mei-warning-level-blocks.is-single {
      width: auto;
      max-width: 100%;
      justify-content: center;
    }
    .mei-warning-level-blocks.is-single .mei-warning-level-item {
      flex: 0 0 48px !important;
      width: 48px !important;
      max-width: 48px !important;
    }
    .mei-warning-level-item.is-off {
      background: transparent !important;
    }
    .mei-warning-level-label {
      font-size: 12px;
      font-weight: 700;
      letter-spacing: 0.02em;
      line-height: 1;
      user-select: none;
    }
    /* 激活：反色字（浅色字压在色块上） */
    .mei-warning-level-item.is-on .mei-warning-level-label {
      color: #ffffff;
      text-shadow: 0 1px 1px rgba(0, 0, 0, 0.28);
    }
    /* 空框：弱化字色，保留可读性 */
    .mei-warning-level-item.is-off .mei-warning-level-label {
      color: var(--mei-color-text-muted, #94a3b8);
      font-weight: 600;
    }
  `;
}
