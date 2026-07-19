/**
 * Topbar app-group menus: portal to document.body while open.
 *
 * Why: CSS cannot keep overflow-y:visible under overflow-x:auto ancestors, and
 * position:fixed inside sticky/isolation topbar stacking contexts is still
 * painted under post-SPA scene layers (transform / will-change). Portaling
 * escapes both traps.
 */
(function (global) {
  "use strict";

  const SELECTOR = "details.app-group-dropdown";
  const MENU_SEL = ":scope > .app-group-menu";
  const MARKER = "data-mei-agm-marker";
  const PORTALED = "data-mei-agm-portaled";
  const OWNER = "data-mei-agm-owner";

  let seq = 0;

  function menuZIndex() {
    const root = global.document?.documentElement;
    const raw = root
      ? global.getComputedStyle(root).getPropertyValue("--mei-z-host-chrome-menu").trim()
      : "";
    const n = Number.parseInt(raw, 10);
    return Number.isFinite(n) && n > 0 ? String(n) : "5700";
  }

  function clearMenuInline(menu) {
    if (!(menu instanceof HTMLElement)) return;
    menu.style.position = "";
    menu.style.top = "";
    menu.style.left = "";
    menu.style.right = "";
    menu.style.zIndex = "";
    menu.style.maxWidth = "";
    menu.style.width = "";
    menu.style.display = "";
  }

  function ownerIdFor(details) {
    if (!details.dataset.meiAgmId) {
      seq += 1;
      details.dataset.meiAgmId = `agm-${seq}`;
    }
    return details.dataset.meiAgmId;
  }

  function findMarker(id) {
    return global.document?.querySelector?.(`[${MARKER}="${id}"]`) || null;
  }

  function restoreMenu(menu) {
    if (!(menu instanceof HTMLElement)) return;
    const id = menu.getAttribute(OWNER);
    const marker = id ? findMarker(id) : null;
    if (marker && marker.parentElement) {
      marker.replaceWith(menu);
    } else if (menu.isConnected && menu.parentElement === global.document.body) {
      menu.remove();
    }
    menu.removeAttribute(PORTALED);
    menu.removeAttribute(OWNER);
    clearMenuInline(menu);
  }

  function restoreAllPortaled() {
    const list = global.document?.querySelectorAll?.(".app-group-menu[" + PORTALED + "]");
    if (!list) return;
    Array.from(list).forEach(restoreMenu);
  }

  function positionPortaledMenu(details, menu) {
    const summary = details.querySelector(":scope > summary");
    if (!(summary instanceof HTMLElement)) return;
    const rect = summary.getBoundingClientRect();
    const vw = global.innerWidth || global.document.documentElement.clientWidth || 0;
    menu.style.display = "flex";
    const preferEnd = details.classList.contains("topbar-account-dropdown");
    const isMoreMenu = details.classList.contains("topbar-more-dropdown");
    const preferredWidth = isMoreMenu ? 880 : preferEnd ? 288 : 520;
    const minimumWidth = isMoreMenu ? 320 : preferEnd ? 200 : 240;
    const menuWidth = Math.min(
      preferredWidth,
      Math.max(minimumWidth, menu.offsetWidth || minimumWidth),
    );
    let left = preferEnd
      ? Math.round(rect.right - menuWidth)
      : Math.round(rect.left);
    if (left < 8) left = 8;
    if (vw > 0 && left + menuWidth > vw - 8) {
      left = Math.max(8, Math.round(vw - menuWidth - 8));
    }
    menu.style.position = "fixed";
    menu.style.top = `${Math.round(rect.bottom + 4)}px`;
    menu.style.left = `${left}px`;
    menu.style.right = "auto";
    menu.style.zIndex = menuZIndex();
    menu.style.maxWidth = `min(${preferredWidth}px, ${Math.max(160, vw - 16)}px)`;
    if (isMoreMenu) {
      menu.style.width = `min(${preferredWidth}px, ${Math.max(minimumWidth, vw - 16)}px)`;
    }
  }

  function portalOpenMenu(details) {
    if (!(details instanceof HTMLDetailsElement) || !details.open) return;
    // Prefer already-portaled node (after chrome refresh, in-details query misses it).
    const id = ownerIdFor(details);
    let menu =
      global.document.querySelector(`.app-group-menu[${OWNER}="${id}"][${PORTALED}]`) ||
      details.querySelector(MENU_SEL);
    if (!(menu instanceof HTMLElement)) return;
    if (!menu.hasAttribute(PORTALED)) {
      const marker = global.document.createElement("span");
      marker.hidden = true;
      marker.setAttribute(MARKER, id);
      menu.before(marker);
      menu.setAttribute(OWNER, id);
      menu.setAttribute(PORTALED, "1");
      global.document.body.appendChild(menu);
    }
    positionPortaledMenu(details, menu);
  }

  function closeDetailsMenu(details) {
    if (!(details instanceof HTMLDetailsElement)) return;
    const summary = details.querySelector(":scope > summary");
    if (summary instanceof HTMLElement) {
      summary.setAttribute("aria-expanded", "false");
    }
    const id = details.dataset.meiAgmId;
    const portaled =
      (id &&
        global.document.querySelector(
          `.app-group-menu[${OWNER}="${id}"][${PORTALED}]`,
        )) ||
      details.querySelector(MENU_SEL);
    if (portaled instanceof HTMLElement && portaled.hasAttribute(PORTALED)) {
      restoreMenu(portaled);
    } else if (portaled instanceof HTMLElement) {
      clearMenuInline(portaled);
    }
  }

  function closeAllOpenGroups() {
    global.document?.querySelectorAll?.(`${SELECTOR}[open]`)?.forEach((el) => {
      el.open = false;
      closeDetailsMenu(el);
    });
    restoreAllPortaled();
  }

  function closeOtherGroups(except) {
    global.document?.querySelectorAll?.(`${SELECTOR}[open]`)?.forEach((el) => {
      if (el !== except) {
        el.open = false;
        closeDetailsMenu(el);
      }
    });
  }

  function onToggle(event) {
    const details = event.target;
    if (!(details instanceof HTMLDetailsElement)) return;
    if (!details.classList.contains("app-group-dropdown")) return;
    if (details.open) {
      closeOtherGroups(details);
      const summary = details.querySelector(":scope > summary");
      if (summary instanceof HTMLElement) {
        summary.setAttribute("aria-expanded", "true");
      }
      portalOpenMenu(details);
      global.requestAnimationFrame(() => {
        if (!details.open) return;
        portalOpenMenu(details);
        if (!details.classList.contains("topbar-more-dropdown")) return;
        const id = details.dataset.meiAgmId;
        const menu =
          (id &&
            global.document.querySelector(
              `.app-group-menu[${OWNER}="${id}"][${PORTALED}]`,
            )) ||
          details.querySelector(MENU_SEL);
        const target =
          menu?.querySelector?.(".topbar-more-card.is-active") ||
          menu?.querySelector?.(".topbar-more-card");
        if (target instanceof HTMLElement) target.focus();
      });
    } else {
      closeDetailsMenu(details);
    }
  }

  function repositionOpenGroups() {
    global.document?.querySelectorAll?.(`${SELECTOR}[open]`)?.forEach((details) => {
      portalOpenMenu(details);
    });
  }

  function onPointerDown(event) {
    const t = event.target;
    if (!(t instanceof Element)) return;
    const open = global.document.querySelector(`${SELECTOR}[open]`);
    if (!open) return;
    const id = open.dataset.meiAgmId;
    const menu =
      (id && global.document.querySelector(`.app-group-menu[${OWNER}="${id}"]`)) ||
      open.querySelector(MENU_SEL);
    if (open.contains(t)) return;
    if (menu instanceof Element && menu.contains(t)) return;
    open.open = false;
    closeDetailsMenu(open);
  }

  function onChromeRefreshed() {
    // Topbar HTML replaced: close and reclaim any orphan portaled menus.
    closeAllOpenGroups();
  }

  function onSpaNavigationComplete() {
    closeAllOpenGroups();
  }

  function onKeyDown(event) {
    const open = global.document.querySelector(`${SELECTOR}[open]`);
    if (!(open instanceof HTMLDetailsElement)) return;
    if (event.key === "Escape") {
      event.preventDefault();
      open.open = false;
      closeDetailsMenu(open);
      const summary = open.querySelector(":scope > summary");
      if (summary instanceof HTMLElement) summary.focus();
      return;
    }
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
      return;
    }
    const id = open.dataset.meiAgmId;
    const menu =
      (id && global.document.querySelector(`.app-group-menu[${OWNER}="${id}"]`)) ||
      open.querySelector(MENU_SEL);
    if (!(menu instanceof HTMLElement)) return;
    const cards = Array.from(menu.querySelectorAll(".topbar-more-card"));
    const current = cards.indexOf(global.document.activeElement);
    if (current < 0 || !cards.length) return;
    event.preventDefault();
    const delta = event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 1;
    cards[(current + delta + cards.length) % cards.length]?.focus?.();
  }

  function bind() {
    const doc = global.document;
    if (!doc || bind.bound) return;
    bind.bound = true;
    doc.addEventListener("toggle", onToggle, true);
    doc.addEventListener("pointerdown", onPointerDown, true);
    doc.addEventListener("keydown", onKeyDown, true);
    global.addEventListener("resize", repositionOpenGroups);
    global.addEventListener("scroll", repositionOpenGroups, true);
    doc.addEventListener("mei:host-chrome-refreshed", onChromeRefreshed);
    doc.addEventListener("mei:shell-layer-applied", onChromeRefreshed);
    doc.addEventListener("mei:spa-navigation-complete", onSpaNavigationComplete);
    doc.addEventListener("mei:spa-navigation-start", onSpaNavigationComplete);
    // Clicking an app link inside the portaled menu starts SPA navigation.
    doc.addEventListener(
      "click",
      (event) => {
        const t = event.target;
        if (!(t instanceof Element)) return;
        const menu = t.closest?.(".app-group-menu[" + PORTALED + "]");
        if (!menu) return;
        const link = t.closest?.("a[href]");
        if (link) closeAllOpenGroups();
      },
      true,
    );
  }

  bind.bound = false;
  if (global.document?.readyState === "loading") {
    global.document.addEventListener("DOMContentLoaded", bind, { once: true });
  } else {
    bind();
  }

  global.MeiTopbarAppGroupMenu = {
    repositionOpenGroups,
    restoreAllPortaled,
    closeAllOpenGroups,
  };
})(typeof window !== "undefined" ? window : globalThis);
