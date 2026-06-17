import { COCKPIT_Z_INDEX } from "../cockpit/tokens.js";

/** 高于二级看板（1620）与 GIS tooltip（1550） */
export const FLOATING_TEXT_POPOVER_Z = COCKPIT_Z_INDEX.textPopover;

let stylesReady = false;

export function copyTextToClipboard(text) {
  const value = String(text ?? "");
  if (!value) return Promise.resolve(false);
  if (navigator.clipboard?.writeText) {
    return navigator.clipboard.writeText(value).then(() => true).catch(() => copyTextFallback(value));
  }
  return Promise.resolve(copyTextFallback(value));
}

function copyTextFallback(text) {
  try {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.cssText = "position:fixed;left:-9999px;top:0;opacity:0;";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    return ok;
  } catch {
    return false;
  }
}

/** @param {(s: string) => string} escapeHtml */
export function buildTextPopoverBodyHtml(fullText, escapeHtml) {
  return `<div class="cell-pop-scroll"><span class="cell-pop-text">${escapeHtml(String(fullText ?? ""))}</span></div>`;
}

/** 表格/图表「查看全文」飘窗视觉（large 为默认锚定飘窗尺寸） */
export function textPopoverStyleBlock(variant = "large") {
  const large = variant !== "default";
  return `
    .cell-pop-backdrop {
      position: fixed;
      inset: 0;
      z-index: var(--mei-z-cockpit-text-popover, 1700);
      display: flex;
      align-items: center;
      justify-content: center;
      padding: clamp(12px, 4vw, 32px);
      background: rgba(2, 8, 24, 0.62);
      backdrop-filter: blur(3px);
    }
    .cell-pop {
      position: fixed;
      z-index: var(--mei-z-cockpit-text-popover, 1700);
      min-width: ${large ? "420px" : "360px"};
      max-width: min(96vw, ${large ? "920px" : "800px"});
      max-height: min(88vh, ${large ? "760px" : "700px"});
      display: flex;
      flex-direction: column;
      gap: 0;
      padding: 0;
      overflow: hidden;
      border-radius: ${large ? "12px" : "10px"};
      border: 2px solid rgba(34, 211, 238, 0.88);
      background: linear-gradient(
        165deg,
        rgba(22, 78, 138, 0.99) 0%,
        rgba(12, 48, 92, 0.99) 48%,
        rgba(8, 32, 68, 1) 100%
      );
      box-shadow:
        0 0 0 1px rgba(255, 255, 255, 0.12) inset,
        0 0 48px rgba(0, 145, 255, 0.42),
        0 24px 64px rgba(0, 0, 0, 0.72);
      color: #f8fafc;
      font-family: var(--mei-font-family-ui, "Microsoft YaHei", "PingFang SC", sans-serif);
    }
    .cell-pop--modal {
      position: relative;
      left: auto !important;
      top: auto !important;
      width: min(96vw, ${large ? "920px" : "760px"});
      max-height: min(90vh, ${large ? "800px" : "720px"});
    }
    .cell-pop-hd {
      position: relative;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      flex: 0 0 auto;
      padding: ${large ? "14px 18px 12px" : "12px 16px 10px"};
      border-bottom: 1px solid rgba(96, 180, 255, 0.45);
      background: linear-gradient(90deg, rgba(34, 211, 238, 0.22) 0%, rgba(0, 145, 255, 0.08) 55%, transparent 100%);
    }
    .cell-pop-hd::before {
      content: "";
      position: absolute;
      left: 0;
      right: 0;
      top: 0;
      height: 3px;
      background: linear-gradient(90deg, #22d3ee 0%, #38bdf8 42%, rgba(56, 189, 248, 0.2) 100%);
      pointer-events: none;
    }
    .cell-pop-title {
      display: grid;
      gap: 2px;
      min-width: 0;
    }
    .cell-pop-title > span {
      font-size: ${large ? "18px" : "16px"};
      font-weight: 700;
      color: #ffffff;
      letter-spacing: 0.08em;
      text-shadow: 0 0 12px rgba(34, 211, 238, 0.65);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .cell-pop-subtitle {
      font-size: ${large ? "13px" : "12px"};
      font-weight: 400;
      color: #bae6fd;
      line-height: 1.35;
    }
    .cell-pop-actions {
      display: flex;
      gap: 8px;
      align-items: center;
      flex: 0 0 auto;
      flex-wrap: nowrap;
    }
    .cell-pop-actions button {
      border-radius: 6px;
      border: 1px solid rgba(125, 211, 252, 0.55);
      background: rgba(8, 47, 73, 0.85);
      color: #e0f2fe;
      font-size: ${large ? "14px" : "13px"};
      font-family: inherit;
      font-weight: 500;
      padding: ${large ? "7px 14px" : "6px 12px"};
      cursor: pointer;
      transition: background 120ms ease, border-color 120ms ease, box-shadow 120ms ease;
    }
    .cell-pop-actions button:hover {
      background: rgba(14, 116, 178, 0.95);
      border-color: #7dd3fc;
      box-shadow: 0 0 12px rgba(34, 211, 238, 0.35);
    }
    .cell-pop-done {
      border-color: #22d3ee !important;
      background: linear-gradient(180deg, #0ea5e9 0%, #0369a1 100%) !important;
      color: #ffffff !important;
      font-weight: 700;
      min-width: ${large ? "72px" : "64px"};
    }
    .cell-pop-close {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: ${large ? "32px" : "30px"};
      height: ${large ? "32px" : "30px"};
      border: 1px solid rgba(125, 211, 252, 0.4) !important;
      border-radius: 6px !important;
      background: rgba(8, 47, 73, 0.6) !important;
      color: #e0f2fe !important;
      font-size: ${large ? "20px" : "18px"};
      line-height: 1;
      padding: 0 !important;
      min-width: 0 !important;
    }
    .cell-pop-scroll {
      flex: 1 1 auto;
      min-height: 140px;
      overflow: auto;
      margin: 0;
      padding: ${large ? "16px 20px 18px" : "14px 16px 16px"};
      scrollbar-width: thin;
      scrollbar-color: rgba(125, 211, 252, 0.55) rgba(8, 32, 68, 0.5);
    }
    .cell-pop-text {
      display: block;
      color: #f8fafc;
      font-size: ${large ? "17px" : "16px"};
      font-weight: 400;
      line-height: 1.75;
      white-space: pre-wrap;
      word-break: break-word;
      letter-spacing: 0.02em;
    }
    .cell-pop-scroll::-webkit-scrollbar {
      width: 8px;
    }
    .cell-pop-scroll::-webkit-scrollbar-thumb {
      border-radius: 999px;
      background: rgba(125, 211, 252, 0.5);
    }
    .cell-pop-scroll::-webkit-scrollbar-track {
      background: rgba(8, 32, 68, 0.45);
      border-radius: 999px;
    }
  `;
}

export function ensureFloatingTextPopoverStyles() {
  if (stylesReady || typeof document === "undefined") return;
  stylesReady = true;
  const z = FLOATING_TEXT_POPOVER_Z;
  const style = document.createElement("style");
  style.dataset.meiFloatingTextPopover = "true";
  const shell = textPopoverStyleBlock("large").replace(/\.cell-pop/g, "body > .cell-pop");
  style.textContent = `
    @keyframes mei-text-pop-in {
      from {
        opacity: 0;
        transform: translateY(10px) scale(0.96);
      }
      to {
        opacity: 1;
        transform: translateY(0) scale(1);
      }
    }
    body > .mei-floating-text-pop {
      position: fixed;
      z-index: ${z};
      box-sizing: border-box;
      resize: both;
      overflow: hidden;
      min-width: 360px;
      min-height: 220px;
      max-width: min(96vw, 960px);
      max-height: min(92vh, 860px);
      animation: mei-text-pop-in 200ms ease-out;
    }
    body > .mei-floating-text-pop::after {
      content: "";
      position: absolute;
      right: 4px;
      bottom: 4px;
      width: 14px;
      height: 14px;
      pointer-events: none;
      opacity: 0.75;
      background:
        linear-gradient(135deg, transparent 42%, #7dd3fc 42%, #7dd3fc 50%, transparent 50%),
        linear-gradient(135deg, transparent 58%, #38bdf8 58%, #38bdf8 66%, transparent 66%);
    }
    body > .mei-floating-text-pop .cell-pop-hd {
      cursor: move;
      user-select: none;
      touch-action: none;
    }
    ${shell}
  `;
  document.head.appendChild(style);
}

export function resolvePopoverAnchorRect(anchor) {
  if (anchor && typeof anchor.getBoundingClientRect === "function") {
    return anchor.getBoundingClientRect();
  }
  if (anchor?.target && typeof anchor.target.getBoundingClientRect === "function") {
    return anchor.target.getBoundingClientRect();
  }
  if (Number.isFinite(anchor?.clientX) && Number.isFinite(anchor?.clientY)) {
    const x = anchor.clientX;
    const y = anchor.clientY;
    return { left: x, top: y, right: x + 1, bottom: y + 1, width: 1, height: 1 };
  }
  return { left: 80, top: 80, right: 120, bottom: 100, width: 40, height: 20 };
}

export function positionFloatingPopoverNearAnchor(pop, anchor, options = {}) {
  if (!pop) return;
  const rect = resolvePopoverAnchorRect(anchor);
  const topOffset = Number(options.topOffset ?? 8);
  const gap = Number(options.gap ?? 8);
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const width = pop.offsetWidth || Number(options.defaultWidth) || 480;
  const height = pop.offsetHeight || Number(options.defaultHeight) || 340;
  let left = rect.left;
  let top = rect.bottom + topOffset;
  if (left + width > vw - gap) {
    left = Math.max(gap, vw - width - gap);
  }
  if (top + height > vh - gap) {
    top = Math.max(gap, rect.top - height - topOffset);
  }
  if (top < gap) top = gap;
  if (left < gap) left = gap;
  pop.style.left = `${Math.round(left)}px`;
  pop.style.top = `${Math.round(top)}px`;
  pop.style.right = "auto";
  pop.style.bottom = "auto";
}

export function mountFloatingPopoverOnBody(pop, size = {}) {
  ensureFloatingTextPopoverStyles();
  pop.classList.add("mei-floating-text-pop");
  pop.style.position = "fixed";
  pop.style.zIndex = String(FLOATING_TEXT_POPOVER_Z);
  if (size.width) pop.style.width = `${Math.round(size.width)}px`;
  if (size.height) pop.style.height = `${Math.round(size.height)}px`;
  document.body.appendChild(pop);
}

export function bindFloatingPopoverDrag(pop, handle) {
  if (!pop || !handle) {
    return () => {};
  }
  let dragging = false;
  let startX = 0;
  let startY = 0;
  let startLeft = 0;
  let startTop = 0;

  const onPointerDown = (event) => {
    if (event.button !== 0) return;
    if (event.target?.closest?.("button, a, input, textarea, select, label, .cell-pop-scroll")) return;
    dragging = true;
    const rect = pop.getBoundingClientRect();
    startX = event.clientX;
    startY = event.clientY;
    startLeft = rect.left;
    startTop = rect.top;
    pop.style.left = `${Math.round(startLeft)}px`;
    pop.style.top = `${Math.round(startTop)}px`;
    pop.style.right = "auto";
    pop.style.bottom = "auto";
    handle.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  };

  const onPointerMove = (event) => {
    if (!dragging) return;
    const dx = event.clientX - startX;
    const dy = event.clientY - startY;
    const width = pop.offsetWidth;
    const height = pop.offsetHeight;
    const maxLeft = Math.max(8, window.innerWidth - width - 8);
    const maxTop = Math.max(8, window.innerHeight - height - 8);
    const left = Math.min(maxLeft, Math.max(8, startLeft + dx));
    const top = Math.min(maxTop, Math.max(8, startTop + dy));
    pop.style.left = `${Math.round(left)}px`;
    pop.style.top = `${Math.round(top)}px`;
  };

  const onPointerUp = (event) => {
    if (!dragging) return;
    dragging = false;
    try {
      handle.releasePointerCapture?.(event.pointerId);
    } catch {
      /* ignore */
    }
  };

  handle.addEventListener("pointerdown", onPointerDown);
  handle.addEventListener("pointermove", onPointerMove);
  handle.addEventListener("pointerup", onPointerUp);
  handle.addEventListener("pointercancel", onPointerUp);
  return () => {
    handle.removeEventListener("pointerdown", onPointerDown);
    handle.removeEventListener("pointermove", onPointerMove);
    handle.removeEventListener("pointerup", onPointerUp);
    handle.removeEventListener("pointercancel", onPointerUp);
  };
}
