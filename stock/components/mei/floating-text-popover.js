import { COCKPIT_Z_INDEX } from "../cockpit/tokens.js";
import { color } from "./theme-style.js";

/** 高于 T2 board 与 T2 tooltip 子带 */
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

/** 右侧边栏布局：标题/说明/操作贴最右，正文占左侧主区 */
export function buildTextPopoverShellHtml(
  { title = "详细内容", subtitle = "", fullText = "" },
  escapeHtml,
) {
  const subtitleHtml = subtitle
    ? `<span class="cell-pop-subtitle">${escapeHtml(subtitle)}</span>`
    : "";
  const titleHtml = title
    ? `<div class="cell-pop-title">
        <span>${escapeHtml(title)}</span>
        ${subtitleHtml}
      </div>`
    : "";
  return `
    <div class="cell-pop-stage">
      <div class="cell-pop-body">
        ${buildTextPopoverBodyHtml(fullText, escapeHtml)}
      </div>
      <aside class="cell-pop-chrome cell-pop-drag-handle" title="拖动">
        ${titleHtml}
        <div class="cell-pop-actions">
          <button type="button" class="cell-pop-close" aria-label="关闭">×</button>
          <button type="button" class="cell-pop-copy">复制</button>
          <button type="button" class="cell-pop-done">关闭</button>
        </div>
      </aside>
    </div>
  `;
}

/** 表格/图表「查看全文」飘窗视觉（large 为默认锚定飘窗尺寸） */
export function textPopoverStyleBlock(variant = "large") {
  const large = variant !== "default";
  return `
    .cell-pop-backdrop {
      position: fixed;
      inset: 0;
      z-index: var(--mei-z-cockpit-text-popover, 2350);
      display: flex;
      align-items: center;
      justify-content: center;
      padding: clamp(12px, 4vw, 32px);
      background: rgba(2, 8, 24, 0.62);
      backdrop-filter: blur(3px);
    }
    .cell-pop {
      position: fixed;
      z-index: var(--mei-z-cockpit-text-popover, 2350);
      min-width: ${large ? "420px" : "360px"};
      max-width: min(96vw, ${large ? "920px" : "800px"});
      max-height: min(88vh, ${large ? "760px" : "700px"});
      display: flex;
      flex-direction: column;
      gap: 0;
      padding: 0;
      overflow: hidden;
      border-radius: ${large ? "12px" : "10px"};
      border: 1px solid rgba(56, 189, 248, 0.42);
      background: linear-gradient(
        165deg,
        rgba(16, 58, 108, 0.98) 0%,
        rgba(10, 40, 78, 0.99) 52%,
        rgba(6, 28, 58, 1) 100%
      );
      box-shadow:
        0 0 0 1px rgba(255, 255, 255, 0.06) inset,
        0 0 28px rgba(0, 120, 220, 0.22),
        0 20px 48px rgba(0, 0, 0, 0.55);
      color: ${color("text_inverse")};
      font-family: var(--mei-font-family-ui, "Microsoft YaHei", "PingFang SC", sans-serif);
    }
    .cell-pop--modal {
      position: relative;
      left: auto !important;
      top: auto !important;
      width: min(96vw, ${large ? "920px" : "760px"});
      max-height: min(90vh, ${large ? "800px" : "720px"});
    }
    .cell-pop-stage {
      position: relative;
      flex: 1 1 auto;
      display: flex;
      flex-direction: row;
      align-items: stretch;
      min-height: 0;
      height: auto;
      overflow: hidden;
    }
    .cell-pop-body {
      position: relative;
      flex: 1 1 auto;
      min-width: 0;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }
    .cell-pop-chrome {
      position: relative;
      z-index: 2;
      flex: 0 0 auto;
      margin-left: auto;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      align-items: flex-end;
      gap: ${large ? "10px" : "8px"};
      width: ${large ? "88px" : "80px"};
      padding: ${large ? "10px 8px 10px 6px" : "8px 6px 8px 4px"};
      border-left: 1px solid rgba(148, 163, 184, 0.18);
      background: linear-gradient(180deg, rgba(2, 12, 32, 0.22) 0%, rgba(2, 12, 32, 0.08) 100%);
      text-align: right;
    }
    .cell-pop-drag-handle {
      cursor: move;
      user-select: none;
      touch-action: none;
    }
    .cell-pop-title {
      display: grid;
      gap: 2px;
      min-width: 0;
      width: 100%;
      justify-items: end;
    }
    .cell-pop-title > span {
      font-size: ${large ? "10px" : "9px"};
      font-weight: 500;
      color: rgba(148, 163, 184, 0.68);
      letter-spacing: 0.04em;
      line-height: 1.3;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      max-width: 100%;
    }
    .cell-pop-subtitle {
      font-size: ${large ? "9px" : "8px"};
      font-weight: 400;
      color: rgba(100, 116, 139, 0.6);
      line-height: 1.25;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      max-width: 100%;
    }
    .cell-pop-actions {
      display: flex;
      flex-direction: column;
      align-items: flex-end;
      gap: 4px;
      width: 100%;
    }
    .cell-pop-actions button {
      border-radius: 4px;
      border: 1px solid transparent;
      background: transparent;
      color: rgba(148, 163, 184, 0.62);
      font-size: ${large ? "10px" : "9px"};
      font-family: inherit;
      font-weight: 400;
      padding: ${large ? "3px 6px" : "2px 5px"};
      cursor: pointer;
      transition: color 120ms ease, background 120ms ease;
      white-space: nowrap;
    }
    .cell-pop-actions button:hover {
      color: rgba(224, 242, 254, 0.92);
      background: rgba(15, 45, 82, 0.35);
    }
    .cell-pop-close {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: ${large ? "22px" : "20px"};
      height: ${large ? "22px" : "20px"};
      padding: 0 !important;
      font-size: ${large ? "16px" : "14px"} !important;
      line-height: 1;
    }
    .cell-pop-scroll {
      position: relative;
      z-index: 1;
      flex: 1 1 auto;
      min-height: 0;
      max-height: min(68vh, 560px);
      overflow: auto;
      margin: 0;
      padding: 20px;
      scrollbar-width: thin;
      scrollbar-color: rgba(125, 211, 252, 0.4) rgba(8, 32, 68, 0.3);
    }
    .cell-pop-text {
      display: block;
      color: var(--mei-color-text-primary, #f8fafc);
      font-size: var(--mei-font-2, 18px);
      font-weight: 400;
      line-height: 1.65;
      white-space: pre-wrap;
      word-break: break-word;
      letter-spacing: 0.01em;
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

const FLOATING_TEXT_POPOVER_STYLE_VERSION = "right-chrome-v5";

/** body 挂载飘窗：子元素须挂在根节点下，不能写成 body > .cell-pop-* */
export function scopeFloatingPopoverCss(css) {
  const pop = "body > .mei-floating-text-pop";
  const modal = "body > .cell-pop-backdrop";
  return css.replace(/(^|\n)(\s*)([^{}\n]+)\{/g, (_match, lead, indent, selectors) => {
    const scoped = selectors
      .split(",")
      .map((raw) => {
        const s = raw.trim();
        if (!s.startsWith(".cell-pop")) return s;
        if (s === ".cell-pop-backdrop") return modal;
        if (s === ".cell-pop") {
          return `${pop}.cell-pop, body > .cell-pop.mei-floating-text-pop, ${modal} > .cell-pop`;
        }
        if (/^\.cell-pop--(modal|large)\b/.test(s)) {
          const mod = s.slice(".cell-pop".length);
          return `${pop}.cell-pop${mod}, body > .cell-pop.mei-floating-text-pop${mod}, ${modal} > .cell-pop${mod}`;
        }
        return `${pop} ${s}, ${modal} ${s}`;
      })
      .join(", ");
    return `${lead}${indent}${scoped} {`;
  });
}

export function ensureFloatingTextPopoverStyles() {
  if (typeof document === "undefined") return;
  if (document.querySelector(`style[data-mei-floating-text-popover="${FLOATING_TEXT_POPOVER_STYLE_VERSION}"]`)) return;
  document.querySelectorAll("style[data-mei-floating-text-popover]").forEach((node) => node.remove());
  stylesReady = true;
  const z = FLOATING_TEXT_POPOVER_Z;
  const style = document.createElement("style");
  style.dataset.meiFloatingTextPopover = FLOATING_TEXT_POPOVER_STYLE_VERSION;
  const shell = scopeFloatingPopoverCss(textPopoverStyleBlock("large"));
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
      min-width: 280px;
      min-height: 72px;
      height: auto;
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
    body > .mei-floating-text-pop .cell-pop-drag-handle {
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

/** 按正文高度收紧飘窗，避免短文本留下大块空白 */
export function fitFloatingPopoverToContent(pop, options = {}) {
  if (!pop) return null;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const minW = Number(options.minWidth) || 280;
  const maxW = Number(options.maxWidth) || Math.min(vw * 0.92, 720);
  const minH = Number(options.minHeight) || 72;
  const maxH = Number(options.maxHeight) || Math.min(vh * 0.85, 680);
  const prefW = Number(options.width ?? options.preferredWidth) || Math.min(maxW, Math.max(minW, 420));

  pop.style.width = `${Math.round(prefW)}px`;
  pop.style.height = "auto";
  pop.style.maxHeight = `${Math.round(maxH)}px`;
  void pop.offsetHeight;
  const contentH = Math.ceil(pop.getBoundingClientRect().height);
  const nextH = Math.min(maxH, Math.max(minH, contentH));
  pop.style.height = contentH > maxH ? `${Math.round(maxH)}px` : `${nextH}px`;
  return { width: prefW, height: nextH };
}

export function mountFloatingPopoverOnBody(pop, size = {}) {
  ensureFloatingTextPopoverStyles();
  pop.classList.add("mei-floating-text-pop");
  pop.style.position = "fixed";
  pop.style.zIndex = String(FLOATING_TEXT_POPOVER_Z);
  if (size.width) pop.style.width = `${Math.round(size.width)}px`;
  if (size.height) {
    pop.style.height = `${Math.round(size.height)}px`;
  } else {
    pop.style.height = "auto";
  }
  document.body.appendChild(pop);
  if (!size.height) {
    fitFloatingPopoverToContent(pop, size);
  }
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
    if (event.target?.closest?.("button, a, input, textarea, select, label, .cell-pop-scroll, .cell-pop-actions")) return;
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
