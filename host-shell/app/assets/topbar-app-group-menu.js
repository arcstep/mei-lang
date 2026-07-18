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
    const menuWidth = Math.min(520, Math.max(240, menu.offsetWidth || 240));
    let left = Math.round(rect.left);
    if (vw > 0 && left + menuWidth > vw - 8) {
      left = Math.max(8, Math.round(vw - menuWidth - 8));
    }
    menu.style.position = "fixed";
    menu.style.top = `${Math.round(rect.bottom + 4)}px`;
    menu.style.left = `${left}px`;
    menu.style.right = "auto";
    menu.style.zIndex = menuZIndex();
    menu.style.maxWidth = `min(520px, ${Math.max(160, vw - 16)}px)`;
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
      portalOpenMenu(details);
      global.requestAnimationFrame(() => {
        if (details.open) portalOpenMenu(details);
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

  function scrollActiveChipsIntoView(root) {
    const scope = root instanceof Element ? root : global.document;
    if (!scope) return;
    const strips = scope.querySelectorAll?.(
      "[data-mei-stage-strip], [data-mei-admin-strip]",
    );
    if (!strips) return;
    Array.from(strips).forEach((strip) => {
      if (!(strip instanceof HTMLElement)) return;
      const active = strip.querySelector(".is-active, .topbar-chip.is-active");
      if (!(active instanceof HTMLElement)) return;
      try {
        active.scrollIntoView({
          inline: "nearest",
          block: "nearest",
          behavior: "instant",
        });
      } catch (_) {
        active.scrollIntoView(false);
      }
    });
  }

  function onChromeRefreshed() {
    // Topbar HTML replaced: reclaim any orphan portaled menus.
    restoreAllPortaled();
    scrollActiveChipsIntoView(global.document);
  }

  function onSpaNavigationComplete() {
    closeAllOpenGroups();
    scrollActiveChipsIntoView(global.document);
  }

  function bind() {
    const doc = global.document;
    if (!doc || bind.bound) return;
    bind.bound = true;
    doc.addEventListener("toggle", onToggle, true);
    doc.addEventListener("pointerdown", onPointerDown, true);
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
    scrollActiveChipsIntoView(doc);
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
    scrollActiveChipsIntoView,
  };
})(typeof window !== "undefined" ? window : globalThis);
