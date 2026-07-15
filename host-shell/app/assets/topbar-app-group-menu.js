/**
 * Position topbar app-group <details> menus with position:fixed.
 *
 * CSS cannot keep overflow-y:visible when a parent uses overflow-x:auto
 * (computed overflow-y becomes auto and clips absolute menus under the scene).
 */
(function (global) {
  "use strict";

  const SELECTOR = "details.app-group-dropdown";

  function chromeZIndex() {
    const slot = global.document?.getElementById?.("mei-host-topbar-slot");
    const raw = slot ? global.getComputedStyle(slot).zIndex : "";
    const n = Number.parseInt(raw, 10);
    return Number.isFinite(n) && n > 0 ? String(n + 1) : "1401";
  }

  function clearMenuPosition(menu) {
    if (!(menu instanceof HTMLElement)) return;
    menu.style.position = "";
    menu.style.top = "";
    menu.style.left = "";
    menu.style.right = "";
    menu.style.zIndex = "";
    menu.style.maxWidth = "";
  }

  function positionOpenMenu(details) {
    if (!(details instanceof HTMLDetailsElement)) return;
    const menu = details.querySelector(":scope > .app-group-menu");
    const summary = details.querySelector(":scope > summary");
    if (!(menu instanceof HTMLElement) || !(summary instanceof HTMLElement)) return;
    if (!details.open) {
      clearMenuPosition(menu);
      return;
    }
    const rect = summary.getBoundingClientRect();
    const vw = global.innerWidth || document.documentElement.clientWidth || 0;
    const menuWidth = Math.min(520, Math.max(240, menu.offsetWidth || 240));
    let left = Math.round(rect.left);
    if (vw > 0 && left + menuWidth > vw - 8) {
      left = Math.max(8, Math.round(vw - menuWidth - 8));
    }
    menu.style.position = "fixed";
    menu.style.top = `${Math.round(rect.bottom + 4)}px`;
    menu.style.left = `${left}px`;
    menu.style.right = "auto";
    menu.style.zIndex = chromeZIndex();
    menu.style.maxWidth = `min(520px, ${Math.max(160, vw - 16)}px)`;
  }

  function closeOtherGroups(except) {
    const root = global.document;
    if (!root) return;
    root.querySelectorAll(`${SELECTOR}[open]`).forEach((el) => {
      if (el !== except) el.open = false;
    });
  }

  function onToggle(event) {
    const details = event.target;
    if (!(details instanceof HTMLDetailsElement)) return;
    if (!details.classList.contains("app-group-dropdown")) return;
    if (details.open) closeOtherGroups(details);
    positionOpenMenu(details);
  }

  function repositionOpenGroups() {
    global.document?.querySelectorAll?.(`${SELECTOR}[open]`)?.forEach(positionOpenMenu);
  }

  function bind() {
    const doc = global.document;
    if (!doc || bind.bound) return;
    bind.bound = true;
    doc.addEventListener("toggle", onToggle, true);
    global.addEventListener("resize", repositionOpenGroups);
    global.addEventListener("scroll", repositionOpenGroups, true);
    doc.addEventListener("mei:host-chrome-refreshed", () => {
      global.requestAnimationFrame(repositionOpenGroups);
    });
  }

  bind.bound = false;
  if (global.document?.readyState === "loading") {
    global.document.addEventListener("DOMContentLoaded", bind, { once: true });
  } else {
    bind();
  }

  global.MeiTopbarAppGroupMenu = {
    positionOpenMenu,
    repositionOpenGroups,
  };
})(typeof window !== "undefined" ? window : globalThis);
