import { escapeHtml, escapeHtmlAttr } from "../runtime-query.js";
import {
  DEFAULT_CELL_PADDING,
  descriptorUsesRelativeTime,
  descriptorsHaveRelativeTime,
  foldHeaderKey,
  formatCellPresentation,
  formatRelativeTimeForRaw,
  isDepartmentLikeColumnKey,
  isLongTextColumnKey,
  toRelativeAtIso,
} from "./format.js";

export function cellValue(row, column, index) {
  if (!row || typeof row !== "object") return "";
  const key = String(column || "").trim();
  const resolved = resolveRowFieldValue(row, key);
  if (resolved !== "") return resolved;
  if (Number.isFinite(index)) {
    const alt = row[`col_${index}`] ?? row[`col${index}`];
    if (alt != null && alt !== "") return alt;
    const keys = ["col_a", "col_b", "col_c", "col_d"];
    if (keys[index] != null && row[keys[index]] != null) return row[keys[index]];
  }
  const value = row[column];
  if (value == null || value === "") return "";
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

/** 逻辑列名与 Excel 源表头不一致时（如带换行/括号的表头变体）回退匹配。 */
export function resolveRowFieldValue(row, column) {
  if (!row || typeof row !== "object") return "";
  const key = String(column || "").trim();
  if (!key) return "";
  const direct = row[key];
  if (direct != null && direct !== "") return stringifyCellScalar(direct);
  const folded = foldHeaderKey(key);
  if (!folded) return "";
  for (const candidate of Object.keys(row)) {
    if (candidate === key) continue;
    if (foldHeaderKey(candidate) === folded) {
      const value = row[candidate];
      if (value != null && value !== "") return stringifyCellScalar(value);
    }
    if (candidate.startsWith(key)) {
      const value = row[candidate];
      if (value != null && value !== "") return stringifyCellScalar(value);
    }
  }
  return "";
}

function stringifyCellScalar(value) {
  if (value == null || value === "") return "";
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

export function resolveCellPreviewMaxChars(props, fallback = 30) {
  const direct = Number(props?.cellPreviewMaxChars ?? props?.cell_preview_max_chars);
  if (Number.isFinite(direct)) return Math.floor(direct);
  const fromTheme = Number(props?._mei?.components?.dataset_table?.cell_preview_max_chars);
  if (Number.isFinite(fromTheme)) return Math.floor(fromTheme);
  if (props?.embedded === true || props?.embedded === "true") {
    return 16;
  }
  return fallback;
}

/** 截断单元格「…」按钮的 aria-label（不渲染为可见文案）。 */
export function resolveExpandButtonLabel(props) {
  const raw = String(props?.cellExpandLabel ?? props?.cell_expand_label ?? "").trim();
  if (raw) return raw;
  return "查看全文";
}

export function resolveOverflowPreviewMinChars(props, fallback = 10) {
  const direct = Number(props?.cellOverflowMinChars ?? props?.cell_overflow_min_chars);
  if (Number.isFinite(direct) && direct > 0) return Math.floor(direct);
  return fallback;
}

export function previewCellKey(rowIndex, column) {
  const row = Number(rowIndex);
  const key = String(column || "").trim();
  if (!Number.isFinite(row) || !key) return "";
  return `${row}::${key}`;
}

function isTruthyFlag(value) {
  if (value === true || value === 1) return true;
  const text = String(value ?? "").trim().toLowerCase();
  return text === "true" || text === "yes" || text === "1";
}

function isFalseyFlag(value) {
  if (value === false || value === 0) return true;
  const text = String(value ?? "").trim().toLowerCase();
  return text === "false" || text === "no" || text === "0";
}

/**
 * 字符截断优先级（与列宽像素无关）：
 * 1. `truncate: false` → 不截断
 * 2. `maxChars` / `max_chars` > 0 → 使用该字数
 * 3. `truncate: true` → `cellPreviewMaxChars`
 * 4. 长文本列名启发式 → `cellPreviewMaxChars`
 * 5. 否则 0（仅受列宽物理裁剪，compact 列应尽量用 max-content 轨避免误裁）
 */
export function resolveTruncateMaxChars(descriptor, props, fallback = 30) {
  const format = descriptor?.format || {};
  if (isFalseyFlag(format.truncate)) return 0;
  const fromFormat = Number(format.maxChars ?? format.max_chars);
  if (Number.isFinite(fromFormat)) {
    return fromFormat > 0 ? Math.floor(fromFormat) : 0;
  }
  if (isTruthyFlag(format.truncate)) {
    if (isDepartmentLikeColumnKey(descriptor?.key) && props?.embedded === true) {
      return 14;
    }
    return resolveCellPreviewMaxChars(props, fallback);
  }
  if (isLongTextColumnKey(descriptor?.key)) {
    return resolveCellPreviewMaxChars(props, fallback);
  }
  return 0;
}

export function cellTableChromeStyleBlock() {
  return `
    th, td, .th-cell, .td-cell {
      padding: ${DEFAULT_CELL_PADDING} !important;
      box-sizing: border-box;
    }
    .th-cell, .td-cell {
      display: flex;
      align-items: center;
      min-width: 0;
    }
    .cell-shell {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      align-items: center;
      gap: 4px;
      width: 100%;
      max-width: 100%;
      min-width: 0;
      vertical-align: middle;
    }
    .cell-shell--plain {
      display: inline;
    }
    td > .cell-inner,
    td > .cell-tag,
    .td-cell > .cell-inner,
    .td-cell > .cell-tag {
      display: block;
      width: 100%;
      max-width: 100%;
      min-width: 0;
      overflow: hidden;
    }
    .cell-preview-text {
      min-width: 0;
      overflow: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
    }
    button.cell-ellipsis.cell-expand-btn {
      flex: 0 0 auto;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      margin: 0;
      padding: 1px 7px;
      min-width: 22px;
      min-height: 20px;
      border-radius: 4px;
      border: 1px solid rgba(59, 130, 246, 0.55);
      background: rgba(37, 99, 235, 0.2);
      font: inherit;
      font-size: 12px;
      font-weight: 700;
      line-height: 1;
      letter-spacing: 0.02em;
      color: #93c5fd;
      cursor: pointer;
      vertical-align: middle;
    }
    button.cell-ellipsis.cell-expand-btn:hover {
      background: rgba(59, 130, 246, 0.38);
      border-color: rgba(147, 197, 253, 0.85);
      color: #e0f2fe;
    }
    button.cell-ellipsis.cell-expand-btn:focus-visible {
      outline: 2px solid rgba(147, 197, 253, 0.9);
      outline-offset: 2px;
    }
    .cell-tag button.cell-ellipsis.cell-expand-btn {
      min-height: 18px;
      padding: 0 6px;
      font-size: 11px;
    }
    td, .td-cell { overflow: hidden; }
    .cell-inner {
      display: block;
      max-width: 100%;
      min-width: 0;
      overflow: hidden;
    }
    .cell-tag.cell-shell,
    .cell-tag .cell-shell { max-width: 100%; }
  `;
}

export function resolveCellPopoverVariant(props) {
  const raw = String(
    props?.cellPopoverVariant ?? props?.cell_popover_variant ?? props?._mei?.cell_popover_variant ?? ""
  )
    .trim()
    .toLowerCase();
  if (raw === "large" || raw === "cockpit") return "large";
  if (props?.embedded === true || props?.embedded === "true") return "large";
  return "default";
}

export function cellPopoverStyleBlock(variant = "default") {
  const large = variant === "large";
  return `
    .cell-pop-backdrop {
      position: fixed;
      inset: 0;
      z-index: 99998;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: clamp(12px, 4vw, 32px);
      background: rgba(2, 6, 23, 0.78);
      backdrop-filter: blur(2px);
    }
    .cell-pop {
      position: fixed;
      z-index: 99999;
      min-width: ${large ? "360px" : "320px"};
      max-width: min(96vw, ${large ? "880px" : "760px"});
      max-height: min(82vh, ${large ? "720px" : "680px"});
      display: flex;
      flex-direction: column;
      gap: ${large ? "14px" : "12px"};
      padding: ${large ? "18px 20px" : "14px 16px"};
      border-radius: ${large ? "16px" : "14px"};
      border: 1px solid rgba(148, 163, 184, 0.35);
      background: rgba(15, 23, 42, 0.98);
      box-shadow: 0 20px 48px rgba(0, 0, 0, 0.5);
      color: #e2e8f0;
    }
    .cell-pop--modal {
      position: relative;
      left: auto !important;
      top: auto !important;
      width: min(96vw, ${large ? "880px" : "720px"});
      max-height: min(86vh, ${large ? "760px" : "680px"});
    }
    .cell-pop-hd {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 10px;
      font-size: ${large ? "18px" : "15px"};
      font-weight: 600;
      color: #cbd5e1;
    }
    .cell-pop-title { display: grid; gap: 4px; min-width: 0; }
    .cell-pop-subtitle {
      font-size: ${large ? "15px" : "12px"};
      font-weight: 400;
      color: #94a3b8;
      line-height: 1.35;
    }
    .cell-pop-actions { display: flex; gap: 8px; align-items: center; flex: 0 0 auto; flex-wrap: wrap; }
    .cell-pop-actions button {
      border-radius: 8px;
      border: 1px solid rgba(148, 163, 184, 0.3);
      background: rgba(30, 41, 59, 0.9);
      color: #e2e8f0;
      font-size: ${large ? "15px" : "14px"};
      padding: ${large ? "8px 14px" : "7px 14px"};
      cursor: pointer;
    }
    .cell-pop-actions button:hover { background: rgba(51, 65, 85, 0.95); }
    .cell-pop-done {
      border-color: rgba(59, 130, 246, 0.55) !important;
      background: rgba(37, 99, 235, 0.88) !important;
      color: #f8fafc !important;
      font-weight: 600;
      min-width: ${large ? "88px" : "72px"};
    }
    .cell-pop-done:hover { background: rgba(59, 130, 246, 0.95) !important; }
    .cell-pop-close {
      border: none !important;
      background: transparent !important;
      color: #94a3b8 !important;
      font-size: ${large ? "28px" : "24px"} !important;
      line-height: 1 !important;
      padding: 0 6px !important;
    }
    .cell-pop-body {
      flex: 1;
      min-height: ${large ? "220px" : "200px"};
      width: 100%;
      resize: vertical;
      border-radius: 10px;
      border: 1px solid rgba(148, 163, 184, 0.22);
      background: rgba(2, 6, 23, 0.55);
      color: #e2e8f0;
      font-size: ${large ? "22px" : "18px"};
      line-height: 1.55;
      padding: ${large ? "14px 16px" : "12px 14px"};
      font-family: ui-sans-serif, system-ui, sans-serif;
      white-space: pre-wrap;
      word-break: break-word;
    }
    .cell-with-tip[title] { cursor: help; }
    .cell-relative-time { display: inline; max-width: 100%; }
    .cell-relative-label { display: inline; }
  `;
}

export function formatCellInnerHtml(
  displayText,
  maxChars,
  rowIndex,
  column,
  cellTextMap,
  expandLabel = "查看全文",
  fullTextOverride = null
) {
  const raw = String(fullTextOverride ?? displayText ?? "");
  const key = previewCellKey(rowIndex, column);
  if (key && cellTextMap && raw) {
    cellTextMap.set(key, raw);
  }
  if (!Number.isFinite(maxChars) || maxChars <= 0) {
    return escapeHtml(raw);
  }
  const chars = [...raw];
  if (chars.length <= maxChars) {
    return escapeHtml(raw);
  }
  const vis = chars.slice(0, maxChars).join("");
  return `<span class="cell-shell"><span class="cell-preview-text">${escapeHtml(
    vis
  )}</span><button type="button" class="cell-ellipsis cell-expand-btn cell-more" data-r="${rowIndex}" data-c="${escapeHtmlAttr(
    column
  )}" aria-label="${escapeHtmlAttr(expandLabel)}">…</button></span>`;
}

export function formatOverflowCellInnerHtml(displayText, rowIndex, column, expandLabel = "查看全文") {
  return `<span class="cell-shell"><span class="cell-preview-text">${escapeHtml(
    String(displayText ?? "")
  )}</span><button type="button" class="cell-ellipsis cell-expand-btn cell-more" data-r="${rowIndex}" data-c="${escapeHtmlAttr(
    column
  )}" aria-label="${escapeHtmlAttr(expandLabel)}">…</button></span>`;
}

function isHorizontallyOverflowing(node) {
  return !!(node instanceof HTMLElement && node.clientWidth > 0 && node.scrollWidth - node.clientWidth > 1);
}

export function syncOverflowPreviewButtons(shadowRoot, cellTextMap, props = {}) {
  if (!shadowRoot) return;
  const expandLabel = resolveExpandButtonLabel(props);
  const minChars = resolveOverflowPreviewMinChars(props);
  shadowRoot.querySelectorAll("[data-cell-preview-key]").forEach((host) => {
    if (!(host instanceof HTMLElement)) return;
    if (host.querySelector(".cell-expand-btn, .cell-more")) return;
    if (host.querySelector(".cell-relative-time")) return;
    const rowIndex = Number(host.getAttribute("data-r"));
    const column = String(host.getAttribute("data-c") || "").trim();
    const key = String(host.getAttribute("data-cell-preview-key") || "").trim();
    if (!Number.isFinite(rowIndex) || !column || !key) return;
    const full = cellTextMap?.get(key);
    if (full == null || full === "") return;
    if ([...String(full)].length <= minChars) return;
    const probe =
      host.querySelector(".cell-preview-text") ||
      host.querySelector(".cell-shell") ||
      host.firstElementChild ||
      host;
    if (!isHorizontallyOverflowing(probe) && !isHorizontallyOverflowing(host)) return;
    const previewText = String(host.textContent || "").trim() || String(full);
    host.innerHTML = formatOverflowCellInnerHtml(previewText, rowIndex, column, expandLabel);
  });
}

export function scheduleOverflowPreviewSync(owner, shadowRoot, cellTextMap, props = {}) {
  if (!owner || !shadowRoot) return;
  if (typeof requestAnimationFrame !== "function" || typeof cancelAnimationFrame !== "function") {
    syncOverflowPreviewButtons(shadowRoot, cellTextMap, props);
    return;
  }
  if (owner._overflowPreviewRafA != null) {
    cancelAnimationFrame(owner._overflowPreviewRafA);
    owner._overflowPreviewRafA = null;
  }
  if (owner._overflowPreviewRafB != null) {
    cancelAnimationFrame(owner._overflowPreviewRafB);
    owner._overflowPreviewRafB = null;
  }
  const run = () => {
    owner._overflowPreviewRafB = null;
    syncOverflowPreviewButtons(shadowRoot, cellTextMap, props);
  };
  owner._overflowPreviewRafA = requestAnimationFrame(() => {
    owner._overflowPreviewRafA = null;
    owner._overflowPreviewRafB = requestAnimationFrame(run);
  });
}

function renderRelativeTimeInner(raw, descriptor, rowIndex, cellTextMap, props, displayOverride) {
  const iso = toRelativeAtIso(raw);
  const { relative, absolute } = formatRelativeTimeForRaw(raw, descriptor);
  const displayText = displayOverride != null ? String(displayOverride) : relative;
  const maxChars = resolveTruncateMaxChars(descriptor, props, resolveCellPreviewMaxChars(props));
  const expandLabel = resolveExpandButtonLabel(props);
  const fullForExpand = absolute || displayText;
  let body = "";
  if (maxChars > 0 && [...displayText].length > maxChars) {
    body = formatCellInnerHtml(
      displayText,
      maxChars,
      rowIndex,
      descriptor.key,
      cellTextMap,
      expandLabel,
      fullForExpand
    );
  } else {
    body = `<span class="cell-relative-label">${escapeHtml(displayText)}</span>`;
  }
  const maxAttr = maxChars > 0 ? ` data-relative-max-chars="${maxChars}"` : "";
  const inner = `<span class="cell-relative-time" data-relative-at="${escapeHtmlAttr(iso)}"${maxAttr}>${body}</span>`;
  const titleAttr = absolute ? ` title="${escapeHtmlAttr(absolute)}"` : "";
  return {
    html: inner,
    titleAttr,
    tipClass: absolute ? " cell-with-tip" : "",
    presentation: { display: displayText, detail: absolute, title: absolute },
    isTruncated: maxChars > 0 && [...displayText].length > maxChars,
  };
}

export function resolveRelativeTimeTickMs(props) {
  const raw = Number(props?.relativeTimeTickMs ?? props?.relative_time_tick_ms ?? 15000);
  if (!Number.isFinite(raw)) return 15000;
  return Math.max(5000, Math.min(Math.floor(raw), 300000));
}

export function refreshRelativeTimeCells(shadowRoot) {
  if (!shadowRoot) return;
  shadowRoot.querySelectorAll(".cell-relative-time[data-relative-at]").forEach((el) => {
    const iso = String(el.getAttribute("data-relative-at") || "").trim();
    if (!iso) return;
    const { relative, absolute } = formatRelativeTimeForRaw(iso, {});
    const label = el.querySelector(".cell-relative-label");
    if (label) {
      label.textContent = relative;
    } else {
      const textEl = el.querySelector(".cell-preview-text");
      const maxChars = Number(el.getAttribute("data-relative-max-chars"));
      if (textEl && Number.isFinite(maxChars) && maxChars > 0) {
        const chars = [...relative];
        textEl.textContent = chars.length <= maxChars ? relative : chars.slice(0, maxChars).join("");
      } else {
        const preview = el.querySelector(".cell-preview");
        if (preview && Number.isFinite(maxChars) && maxChars > 0) {
          const chars = [...relative];
          const truncated = chars.length > maxChars;
          const textEl = preview.querySelector(".cell-preview-text");
          if (textEl) {
            textEl.textContent = truncated ? chars.slice(0, maxChars).join("") : relative;
          } else {
            preview.textContent = truncated ? chars.slice(0, maxChars).join("") : relative;
          }
        } else {
          el.textContent = relative;
        }
      }
    }
    const tipHost = el.closest(".cell-inner, .cell-tag, .td-cell, .th-cell");
    if (tipHost && absolute) {
      tipHost.setAttribute("title", absolute);
      tipHost.classList.add("cell-with-tip");
    }
  });
}

export function stopRelativeTimeTicker(owner) {
  if (!owner || owner._relativeTimeTimer == null) return;
  clearInterval(owner._relativeTimeTimer);
  owner._relativeTimeTimer = null;
}

export function bindRelativeTimeTicker(owner, shadowRoot, descriptors, props = {}) {
  stopRelativeTimeTicker(owner);
  if (!owner || !shadowRoot || !descriptorsHaveRelativeTime(descriptors)) {
    return () => stopRelativeTimeTicker(owner);
  }
  const tick = () => refreshRelativeTimeCells(shadowRoot);
  tick();
  owner._relativeTimeTimer = setInterval(() => {
    if (!owner.isConnected) {
      stopRelativeTimeTicker(owner);
      return;
    }
    tick();
  }, resolveRelativeTimeTickMs(props));
  return () => stopRelativeTimeTicker(owner);
}

export function renderFormattedCellHtml(raw, descriptor, rowIndex, cellTextMap, props = {}, displayOverride = null) {
  if (raw != null && raw !== "" && descriptorUsesRelativeTime(descriptor)) {
    return renderRelativeTimeInner(raw, descriptor, rowIndex, cellTextMap, props, displayOverride);
  }
  const base = formatCellPresentation(raw, descriptor);
  const presentation =
    displayOverride != null
      ? { ...base, display: String(displayOverride) }
      : base;
  const maxChars = resolveTruncateMaxChars(descriptor, props, resolveCellPreviewMaxChars(props));
  const display = String(presentation.display ?? "");
  const detail = String(presentation.detail ?? "");
  const fullForExpand = detail || display;
  const expandLabel = resolveExpandButtonLabel(props);
  const inner = formatCellInnerHtml(
    display,
    maxChars,
    rowIndex,
    descriptor.key,
    cellTextMap,
    expandLabel,
    fullForExpand
  );
  const isTruncated = maxChars > 0 && [...display].length > maxChars;
  const hoverDetail = !isTruncated && detail && detail !== display ? detail : "";
  const titleAttr = hoverDetail ? ` title="${escapeHtmlAttr(hoverDetail)}"` : "";
  const tipClass = titleAttr ? " cell-with-tip" : "";
  return { html: inner, titleAttr, tipClass, presentation, isTruncated };
}

export function bindCellPreviewClick(shadowRoot, cellTextMap, openPopover, { getVariant } = {}) {
  const handler = (ev) => {
    const btn = ev.target?.closest?.(".cell-expand-btn, .cell-more");
    if (!btn) return;
    ev.preventDefault();
    ev.stopPropagation();
    const rowIndex = Number(btn.dataset.r);
    const column = String(btn.dataset.c || "");
    if (!Number.isFinite(rowIndex) || !column) return;
    const full = cellTextMap?.get(`${rowIndex}::${column}`);
    if (full == null) return;
    const variant = typeof getVariant === "function" ? getVariant() : "large";
    const title = String(btn.getAttribute("aria-label") || "").trim() || "全文";
    openPopover(full, btn, { variant, layout: "modal", title });
  };
  shadowRoot.addEventListener("click", handler);
  return () => shadowRoot.removeEventListener("click", handler);
}

function removePopoverNodes(owner, shadowRoot) {
  if (owner._cellPopoverBackdrop && shadowRoot?.contains(owner._cellPopoverBackdrop)) {
    try {
      owner._cellPopoverBackdrop.remove();
    } catch (_) {
      /* ignore */
    }
  }
  owner._cellPopoverBackdrop = null;
  if (owner._cellPopoverEl && shadowRoot?.contains(owner._cellPopoverEl)) {
    try {
      owner._cellPopoverEl.remove();
    } catch (_) {
      /* ignore */
    }
  }
  owner._cellPopoverEl = null;
}

export function closeCellPopover(owner, shadowRoot) {
  if (!owner) return;
  if (typeof owner._cellPopoverDocCleanup === "function") {
    try {
      owner._cellPopoverDocCleanup();
    } catch (_) {
      /* ignore */
    }
    owner._cellPopoverDocCleanup = null;
  }
  removePopoverNodes(owner, shadowRoot);
  if (typeof owner._cellPopoverKeydown === "function") {
    try {
      document.removeEventListener("keydown", owner._cellPopoverKeydown, true);
    } catch (_) {
      /* ignore */
    }
    owner._cellPopoverKeydown = null;
  }
}

export function openCellPopover(
  owner,
  shadowRoot,
  fullText,
  anchor,
  { topOffset = 6, focusOnOpen = false, variant = "default", layout = "anchored", subtitle = "", title = "详细内容" } = {}
) {
  if (!owner || !shadowRoot) return;
  closeCellPopover(owner, shadowRoot);
  const useModal = layout === "modal";
  const effectiveVariant = useModal ? "large" : variant;
  const large = effectiveVariant === "large";

  const backdrop = useModal ? document.createElement("div") : null;
  if (backdrop) {
    backdrop.className = "cell-pop-backdrop";
    backdrop.setAttribute("data-cell-pop-backdrop", "true");
    shadowRoot.appendChild(backdrop);
    owner._cellPopoverBackdrop = backdrop;
  }

  const pop = document.createElement("div");
  pop.className = `cell-pop${large ? " cell-pop--large" : ""}${useModal ? " cell-pop--modal" : ""}`;
  pop.setAttribute("role", "dialog");
  pop.setAttribute("aria-modal", "true");
  pop.setAttribute("aria-label", title);
  const subtitleHtml = subtitle
    ? `<span class="cell-pop-subtitle">${escapeHtml(subtitle)}</span>`
    : "";
  pop.innerHTML = `
      <div class="cell-pop-hd">
        <div class="cell-pop-title">
          <span>${escapeHtml(title)}</span>
          ${subtitleHtml}
        </div>
        <div class="cell-pop-actions">
          <button type="button" class="cell-pop-copy">复制</button>
          <button type="button" class="cell-pop-done">关闭</button>
          <button type="button" class="cell-pop-close" aria-label="关闭">×</button>
        </div>
      </div>
      <textarea class="cell-pop-body" readonly spellcheck="false"></textarea>
    `;
  const ta = pop.querySelector(".cell-pop-body");
  if (ta) ta.value = String(fullText ?? "");

  if (backdrop) {
    backdrop.appendChild(pop);
  } else {
    shadowRoot.appendChild(pop);
  }
  owner._cellPopoverEl = pop;

  if (!useModal && anchor) {
    const rect = anchor.getBoundingClientRect();
    let left = rect.left;
    let top = rect.bottom + Number(topOffset || 0);
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const estW = Math.min(large ? 880 : 760, vw * 0.96);
    if (left + estW > vw - 8) left = Math.max(8, vw - estW - 8);
    if (top + 280 > vh) top = Math.max(8, rect.top - Math.min(large ? 680 : 620, vh * 0.72) - 8);
    pop.style.left = `${left}px`;
    pop.style.top = `${top}px`;
  }

  const requestClose = () => closeCellPopover(owner, shadowRoot);

  const onDoc = (ev) => {
    if (useModal) return;
    const path = ev.composedPath();
    if (path.includes(pop) || (anchor && path.includes(anchor))) return;
    requestClose();
  };

  if (useModal) {
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) requestClose();
    });
    pop.addEventListener("click", (ev) => ev.stopPropagation());
  } else {
    setTimeout(() => {
      document.addEventListener("pointerdown", onDoc, true);
    }, 0);
    owner._cellPopoverDocCleanup = () => {
      document.removeEventListener("pointerdown", onDoc, true);
    };
  }

  owner._cellPopoverKeydown = (ev) => {
    if (ev.key === "Escape") {
      ev.stopPropagation();
      requestClose();
    }
  };
  document.addEventListener("keydown", owner._cellPopoverKeydown, true);

  pop.querySelector(".cell-pop-close")?.addEventListener("click", requestClose);
  pop.querySelector(".cell-pop-done")?.addEventListener("click", requestClose);
  pop.querySelector(".cell-pop-copy")?.addEventListener("click", async () => {
    const text = String(fullText ?? "");
    try {
      await navigator.clipboard.writeText(text);
    } catch (_) {
      try {
        ta?.focus();
        ta?.select();
        document.execCommand("copy");
      } catch (_) {
        /* ignore */
      }
    }
  });

  const focusTarget = pop.querySelector(".cell-pop-done") || ta;
  if (focusOnOpen || useModal) {
    try {
      focusTarget?.focus();
    } catch (_) {
      /* ignore */
    }
  }
}
