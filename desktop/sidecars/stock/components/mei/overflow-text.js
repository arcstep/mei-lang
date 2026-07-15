/**
 * 驾驶舱长文统一截断：单行省略 + 「…」展开 + floating-text-popover 全文。
 * 与 table cell-shell / mei.text overflowExpand 同构，自定义组件应复用本模块，勿再写私有 title 截断。
 */
import { escapeHtml, escapeAttr } from "../cockpit/shared.js";
import { color } from "./theme-style.js";
import {
  bindFloatingPopoverDrag,
  buildTextPopoverShellHtml,
  copyTextToClipboard,
  ensureFloatingTextPopoverStyles,
  mountFloatingPopoverOnBody,
  positionFloatingPopoverNearAnchor,
  scopeFloatingPopoverCss,
  textPopoverStyleBlock,
} from "./floating-text-popover.js";

const EXPAND_LABEL_DEFAULT = "查看全文";
const POPOVER_STYLE_VERSION = "overflow-text-v1";

/** Shadow DOM 内与 table / mei.text 一致的预览壳样式 */
export function overflowTextShellStyleBlock() {
  return `
    .mei-overflow-shell {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      align-items: center;
      gap: 4px;
      width: 100%;
      max-width: 100%;
      min-width: 0;
      min-height: 0;
    }
    .mei-overflow-shell[data-expanded-hidden="true"] {
      grid-template-columns: minmax(0, 1fr);
    }
    .mei-overflow-preview {
      margin: 0;
      padding: 0;
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      max-width: 100%;
    }
    button.mei-overflow-expand-btn {
      flex: 0 0 auto;
      display: none;
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
      color: ${color("text_unit")};
      cursor: pointer;
      white-space: nowrap;
    }
    button.mei-overflow-expand-btn[data-visible="true"] {
      display: inline-flex;
    }
    button.mei-overflow-expand-btn:hover {
      background: rgba(59, 130, 246, 0.38);
      border-color: rgba(147, 197, 253, 0.85);
      color: ${color("text_highlight")};
    }
    button.mei-overflow-expand-btn:focus-visible {
      outline: 2px solid rgba(147, 197, 253, 0.9);
      outline-offset: 2px;
    }
  `;
}

/**
 * @param {string} text
 * @param {{ key?: string, expandLabel?: string, className?: string }} [options]
 */
export function formatOverflowTextShellHtml(text, options = {}) {
  const key = String(options.key || "").trim();
  const expandLabel = String(options.expandLabel || EXPAND_LABEL_DEFAULT).trim() || EXPAND_LABEL_DEFAULT;
  const className = String(options.className || "").trim();
  const previewClass = className ? `mei-overflow-preview ${className}` : "mei-overflow-preview";
  const keyAttr = key ? ` data-overflow-key="${escapeAttr(key)}"` : "";
  return `<span class="mei-overflow-shell" data-expanded-hidden="true"${keyAttr}>
    <span class="${previewClass}">${escapeHtml(String(text ?? ""))}</span>
    <button type="button" class="mei-overflow-expand-btn" aria-label="${escapeAttr(expandLabel)}">…</button>
  </span>`;
}

function isHorizontallyOverflowing(node) {
  return !!(node instanceof HTMLElement && node.clientWidth > 0 && node.scrollWidth - node.clientWidth > 1);
}

/** 按实际溢出显示/隐藏「…」；短文不展示按钮 */
export function syncOverflowTextButtons(root, { minChars = 8 } = {}) {
  if (!root) return;
  root.querySelectorAll(".mei-overflow-shell").forEach((shell) => {
    if (!(shell instanceof HTMLElement)) return;
    const preview = shell.querySelector(".mei-overflow-preview");
    const btn = shell.querySelector(".mei-overflow-expand-btn");
    if (!(preview instanceof HTMLElement) || !(btn instanceof HTMLElement)) return;
    const full = String(preview.textContent || "").trim();
    const show = full.length > minChars && isHorizontallyOverflowing(preview);
    btn.dataset.visible = show ? "true" : "false";
    btn.setAttribute("aria-hidden", show ? "false" : "true");
    shell.dataset.expandedHidden = show ? "false" : "true";
  });
}

export function scheduleOverflowTextSync(owner, root, options = {}) {
  if (!owner || !root) return;
  if (typeof requestAnimationFrame !== "function") {
    syncOverflowTextButtons(root, options);
    return;
  }
  if (owner._overflowTextRafA != null) cancelAnimationFrame(owner._overflowTextRafA);
  if (owner._overflowTextRafB != null) cancelAnimationFrame(owner._overflowTextRafB);
  owner._overflowTextRafA = requestAnimationFrame(() => {
    owner._overflowTextRafA = null;
    owner._overflowTextRafB = requestAnimationFrame(() => {
      owner._overflowTextRafB = null;
      syncOverflowTextButtons(root, options);
    });
  });
}

function ensureOverflowPopoverGlobalStyles() {
  if (typeof document === "undefined") return;
  if (document.querySelector(`style[data-mei-overflow-text-pop="${POPOVER_STYLE_VERSION}"]`)) return;
  document.querySelectorAll("style[data-mei-overflow-text-pop]").forEach((node) => node.remove());
  const style = document.createElement("style");
  style.dataset.meiOverflowTextPop = POPOVER_STYLE_VERSION;
  style.textContent = scopeFloatingPopoverCss(textPopoverStyleBlock("large"));
  document.head.appendChild(style);
}

export function closeOverflowTextPopover(owner) {
  if (!owner) return;
  if (typeof owner._overflowPopoverDocCleanup === "function") {
    owner._overflowPopoverDocCleanup();
    owner._overflowPopoverDocCleanup = null;
  }
  if (typeof owner._overflowPopoverDragCleanup === "function") {
    owner._overflowPopoverDragCleanup();
    owner._overflowPopoverDragCleanup = null;
  }
  if (owner._overflowPopoverKeydown) {
    document.removeEventListener("keydown", owner._overflowPopoverKeydown, true);
    owner._overflowPopoverKeydown = null;
  }
  owner._overflowPopoverEl?.remove?.();
  owner._overflowPopoverEl = null;
}

/**
 * @param {object} owner
 * @param {string} fullText
 * @param {Element|EventTarget|null} anchor
 * @param {{ title?: string, variant?: string }} [options]
 */
export function openOverflowTextPopover(owner, fullText, anchor, options = {}) {
  if (!owner) return;
  const text = String(fullText ?? "").trim();
  if (!text) return;
  closeOverflowTextPopover(owner);
  ensureFloatingTextPopoverStyles();
  ensureOverflowPopoverGlobalStyles();
  const title = String(options.title || EXPAND_LABEL_DEFAULT).trim() || "详细内容";
  const large = String(options.variant || "large").toLowerCase() !== "default";
  const pop = document.createElement("div");
  pop.className = `cell-pop${large ? " cell-pop--large" : ""}`;
  pop.setAttribute("role", "dialog");
  pop.setAttribute("aria-modal", "true");
  pop.setAttribute("aria-label", title);
  pop.innerHTML = buildTextPopoverShellHtml({ title, subtitle: "", fullText: text }, escapeHtml);
  const defaultWidth = large ? 480 : 420;
  mountFloatingPopoverOnBody(pop, { width: defaultWidth });
  owner._overflowPopoverEl = pop;
  positionFloatingPopoverNearAnchor(pop, anchor, { topOffset: 8, defaultWidth });
  owner._overflowPopoverDragCleanup = bindFloatingPopoverDrag(
    pop,
    pop.querySelector(".cell-pop-drag-handle"),
  );
  const requestClose = () => closeOverflowTextPopover(owner);
  const onDoc = (ev) => {
    const path = ev.composedPath?.() || [];
    if (path.includes(pop) || (anchor && path.includes(anchor))) return;
    requestClose();
  };
  setTimeout(() => document.addEventListener("pointerdown", onDoc, true), 0);
  owner._overflowPopoverDocCleanup = () => document.removeEventListener("pointerdown", onDoc, true);
  owner._overflowPopoverKeydown = (ev) => {
    if (ev.key === "Escape") {
      ev.stopPropagation();
      requestClose();
    }
  };
  document.addEventListener("keydown", owner._overflowPopoverKeydown, true);
  pop.querySelector(".cell-pop-close")?.addEventListener("click", requestClose);
  pop.querySelector(".cell-pop-copy")?.addEventListener("click", () => copyTextToClipboard(text));
}

/**
 * 在 root 上委托点击 `.mei-overflow-expand-btn`；全文取自同壳 preview 文本。
 * @returns {() => void} cleanup
 */
export function bindOverflowTextExpand(owner, root, { titleForKey } = {}) {
  if (!owner || !root) return () => {};
  const onClick = (event) => {
    const btn = event.target?.closest?.(".mei-overflow-expand-btn");
    if (!btn || !root.contains(btn)) return;
    event.preventDefault();
    event.stopPropagation();
    const shell = btn.closest(".mei-overflow-shell");
    const preview = shell?.querySelector?.(".mei-overflow-preview");
    const full = String(preview?.textContent || "").trim();
    if (!full) return;
    const key = String(shell?.getAttribute?.("data-overflow-key") || "").trim();
    const title =
      (typeof titleForKey === "function" ? titleForKey(key, shell) : null) ||
      btn.getAttribute("aria-label") ||
      EXPAND_LABEL_DEFAULT;
    openOverflowTextPopover(owner, full, btn, { title, variant: "large" });
  };
  root.addEventListener("click", onClick);
  return () => {
    root.removeEventListener("click", onClick);
    closeOverflowTextPopover(owner);
  };
}
