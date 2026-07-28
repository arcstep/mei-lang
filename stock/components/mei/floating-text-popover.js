import { color } from "./theme-style.js";

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
      border: 1px solid rgba(56, 189, 248, 0.55);
      background:
        linear-gradient(
          165deg,
          rgba(18, 62, 112, 1) 0%,
          rgba(12, 44, 84, 1) 52%,
          rgba(8, 32, 64, 1) 100%
        );
      box-shadow:
        0 0 0 1px rgba(255, 255, 255, 0.08) inset,
        0 0 28px rgba(0, 120, 220, 0.28),
        0 20px 48px rgba(0, 0, 0, 0.72);
      color: ${color("text_inverse")};
      font-family: var(--mei-font-family-ui, "Microsoft YaHei", "PingFang SC", sans-serif);
      opacity: 1;
      pointer-events: auto;
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
      background: rgba(8, 32, 64, 1);
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
      width: ${large ? "96px" : "88px"};
      padding: ${large ? "10px 8px 10px 6px" : "8px 6px 8px 4px"};
      border-left: 1px solid rgba(148, 163, 184, 0.28);
      background: linear-gradient(180deg, rgba(6, 28, 58, 1) 0%, rgba(8, 34, 68, 1) 100%);
      text-align: right;
      pointer-events: auto;
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
      font-size: ${large ? "11px" : "10px"};
      font-weight: 600;
      color: rgba(226, 232, 240, 0.92);
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
      color: rgba(226, 232, 240, 0.92);
      font-size: ${large ? "11px" : "10px"};
      font-family: inherit;
      font-weight: 500;
      padding: ${large ? "4px 8px" : "3px 6px"};
      cursor: pointer;
      transition: color 120ms ease, background 120ms ease, border-color 120ms ease;
      white-space: nowrap;
      pointer-events: auto;
    }
    .cell-pop-actions button:hover {
      color: #f8fafc;
      background: rgba(56, 189, 248, 0.18);
      border-color: rgba(125, 211, 252, 0.35);
    }
    .cell-pop-close {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: ${large ? "24px" : "22px"};
      height: ${large ? "24px" : "22px"};
      padding: 0 !important;
      font-size: ${large ? "18px" : "16px"} !important;
      line-height: 1;
      color: rgba(248, 250, 252, 0.95) !important;
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
      background: rgba(8, 32, 64, 1);
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

const FLOATING_TEXT_POPOVER_STYLE_VERSION = "right-chrome-v12-drag-surface";

/** 飘窗可能挂 body 或 viewport stage（.mei-viewport-floating-in-stage），选择器须同时覆盖。 */
export function scopeFloatingPopoverCss(css) {
  const pops = [
    "body > .mei-floating-text-pop",
    ".mei-floating-text-pop.mei-viewport-floating-in-stage",
    "[data-mei-overlay-role=\"text_popover\"].mei-floating-text-pop",
  ];
  const modals = [
    "body > .cell-pop-backdrop",
    ".cell-pop-backdrop.mei-viewport-floating-in-stage",
  ];
  const joinPop = (suffix = "") => pops.map((p) => `${p}${suffix}`).join(", ");
  const joinModalChild = (suffix = "") => modals.map((m) => `${m} > .cell-pop${suffix}`).join(", ");
  const joinDesc = (sel) =>
    [...pops.map((p) => `${p} ${sel}`), ...modals.map((m) => `${m} ${sel}`)].join(", ");
  return css.replace(/(^|\n)(\s*)([^{}\n]+)\{/g, (_match, lead, indent, selectors) => {
    const scoped = selectors
      .split(",")
      .map((raw) => {
        const s = raw.trim();
        if (!s.startsWith(".cell-pop")) return s;
        if (s === ".cell-pop-backdrop") return modals.join(", ");
        if (s === ".cell-pop") {
          return `${joinPop(".cell-pop")}, ${joinPop("")}, ${joinModalChild("")}`;
        }
        if (/^\.cell-pop--(modal|large)\b/.test(s)) {
          const mod = s.slice(".cell-pop".length);
          return `${joinPop(`.cell-pop${mod}`)}, ${joinModalChild(mod)}`;
        }
        return joinDesc(s);
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
  const style = document.createElement("style");
  style.dataset.meiFloatingTextPopover = FLOATING_TEXT_POPOVER_STYLE_VERSION;
  const shell = scopeFloatingPopoverCss(textPopoverStyleBlock("large"));
  const popRoots = [
    "body > .mei-floating-text-pop",
    ".mei-floating-text-pop.mei-viewport-floating-in-stage",
    '[data-mei-overlay-role="text_popover"].mei-floating-text-pop',
  ];
  const popRoot = popRoots.join(", ");
  const popRootAfter = popRoots.map((s) => `${s}::after`).join(", ");
  const popRootDrag = popRoots.map((s) => `${s} .cell-pop-drag-handle`).join(", ");
  style.textContent = `
    @keyframes mei-text-pop-in {
      from {
        transform: translateY(8px) scale(0.98);
      }
      to {
        transform: translateY(0) scale(1);
      }
    }
    ${popRoot} {
      position: fixed;
      z-index: var(--mei-z-cockpit-text-popover, 2350);
      box-sizing: border-box;
      resize: both;
      overflow: hidden;
      min-width: 280px;
      min-height: 72px;
      height: auto;
      max-width: min(96vw, 960px);
      max-height: min(92vh, 860px);
      opacity: 1 !important;
      pointer-events: auto !important;
      animation: mei-text-pop-in 140ms ease-out;
    }
    ${popRootAfter} {
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
    ${popRootDrag} {
      cursor: grab;
      user-select: none;
      touch-action: none;
    }
    ${popRoots.map((s) => `${s}.is-dragging`).join(", ")},
    ${popRoots.map((s) => `${s}.is-dragging .cell-pop-drag-handle`).join(", ")} {
      cursor: grabbing;
      user-select: none;
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
  pop.setAttribute("data-mei-overlay-role", "text_popover");
  pop.style.position = "fixed";
  pop.style.removeProperty("z-index");
  pop.style.setProperty("opacity", "1", "important");
  pop.style.setProperty("pointer-events", "auto", "important");
  pop.style.setProperty("background", "#071e40", "important");
  pop.style.setProperty("isolation", "isolate");
  if (size.width) pop.style.width = `${Math.round(size.width)}px`;
  if (size.height) {
    pop.style.height = `${Math.round(size.height)}px`;
  } else {
    pop.style.height = "auto";
  }
  const boot = window.__meiLangBoot || {};
  if (typeof boot.mountRuntimeOverlay === "function") {
    boot.mountRuntimeOverlay(pop, { role: "text_popover" });
  } else {
    document.body.appendChild(pop);
  }
  if (!size.height) {
    fitFloatingPopoverToContent(pop, size);
  }
}

/** 打开正文飘窗时加一层低透明度遮罩，避免看穿底层驾驶舱内容。 */
export function mountTextPopoverBackdrop(owner) {
  if (!owner || typeof document === "undefined") return null;
  const existing = document.querySelector(".mei-floating-text-pop-backdrop[data-mei-text-pop-backdrop='true']");
  if (existing) {
    owner._textPopoverBackdrop = existing;
    return existing;
  }
  const backdrop = document.createElement("div");
  backdrop.className = "mei-floating-text-pop-backdrop cell-pop-backdrop";
  backdrop.setAttribute("data-mei-overlay-role", "text_popover");
  backdrop.setAttribute("data-mei-text-pop-backdrop", "true");
  backdrop.style.cssText =
    "position:fixed;inset:0;z-index:var(--mei-z-cockpit-text-popover, 2350);background:rgba(2,8,24,0.55);pointer-events:auto;";
  document.body.appendChild(backdrop);
  owner._textPopoverBackdrop = backdrop;
  return backdrop;
}

export function removeTextPopoverBackdrop(owner) {
  const backdrop = owner?._textPopoverBackdrop;
  if (backdrop?.isConnected) {
    try {
      backdrop.remove();
    } catch {
      /* ignore */
    }
  }
  if (owner) owner._textPopoverBackdrop = null;
}

export function bindFloatingPopoverDrag(pop, handle) {
  if (!pop) {
    return () => {};
  }
  // 短正文时右侧 chrome 几乎被标题/按钮占满；命中绑在整窗上，
  // 仅排除可交互控件与正文滚动区，避免「看起来能拖、实际拖不动」。
  const dragSurface = pop;
  const cursorHandle = handle instanceof HTMLElement ? handle : pop.querySelector?.(".cell-pop-drag-handle");
  let dragging = false;
  let pointerId = null;
  let startX = 0;
  let startY = 0;
  let startLeft = 0;
  let startTop = 0;

  const stopDragging = (event) => {
    if (!dragging) return;
    dragging = false;
    pop.classList.remove("is-dragging");
    if (cursorHandle) cursorHandle.style.cursor = "";
    const id = event?.pointerId ?? pointerId;
    pointerId = null;
    if (id != null) {
      try {
        dragSurface.releasePointerCapture?.(id);
      } catch {
        /* ignore */
      }
    }
    window.removeEventListener("pointermove", onPointerMove, true);
    window.removeEventListener("pointerup", onPointerUp, true);
    window.removeEventListener("pointercancel", onPointerUp, true);
  };

  const onPointerMove = (event) => {
    if (!dragging) return;
    if (pointerId != null && event.pointerId !== pointerId) return;
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
    event.preventDefault();
  };

  const onPointerUp = (event) => {
    if (pointerId != null && event.pointerId !== pointerId) return;
    stopDragging(event);
  };

  const onPointerDown = (event) => {
    if (event.button !== 0) return;
    // 只挡真正可点控件与正文滚动；勿排除整个 .cell-pop-actions 容器
    // （短窗时该容器几乎占满右侧栏，会导致拖拽失效）。
    if (
      event.target?.closest?.(
        "button, a, input, textarea, select, label, .cell-pop-scroll, .cell-pop-text",
      )
    ) {
      return;
    }
    dragging = true;
    pointerId = event.pointerId;
    const rect = pop.getBoundingClientRect();
    startX = event.clientX;
    startY = event.clientY;
    startLeft = rect.left;
    startTop = rect.top;
    pop.style.left = `${Math.round(startLeft)}px`;
    pop.style.top = `${Math.round(startTop)}px`;
    pop.style.right = "auto";
    pop.style.bottom = "auto";
    pop.classList.add("is-dragging");
    if (cursorHandle) cursorHandle.style.cursor = "grabbing";
    try {
      dragSurface.setPointerCapture?.(event.pointerId);
    } catch {
      /* ignore */
    }
    window.addEventListener("pointermove", onPointerMove, true);
    window.addEventListener("pointerup", onPointerUp, true);
    window.addEventListener("pointercancel", onPointerUp, true);
    event.preventDefault();
  };

  dragSurface.addEventListener("pointerdown", onPointerDown);
  return () => {
    stopDragging();
    dragSurface.removeEventListener("pointerdown", onPointerDown);
  };
}
