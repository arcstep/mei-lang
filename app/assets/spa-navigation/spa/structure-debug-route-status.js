/**
 * Phase 8.5: FAB structure focus → host statusbar debug route (click to copy).
 */
(function initStructureDebugRouteStatus(global) {
  "use strict";

  const COPIED_MS = 1600;

  function debugRouteChip() {
    return global.document?.getElementById?.("mei-status-debug-route") || null;
  }

  function buildTempStageRoute(detail) {
    const appId =
      String(global.document?.body?.getAttribute("data-app-id") || "").trim() ||
      (typeof appIdFromAppsPathname === "function"
        ? appIdFromAppsPathname(global.location?.pathname)
        : "");
    const scope = String(detail?.preview_scope || "").trim().replace(/^\/+|\/+$/g, "");
    const nodeId = String(detail?.node_id || "").trim();
    const target = scope || (nodeId ? `node/${nodeId}` : "");
    if (!appId || !target) return "";
    if (typeof canonicalTempStagePath === "function") {
      return canonicalTempStagePath(appId, target);
    }
    return `/apps/${appId}/~/${target}`;
  }

  function setChipRoute(route) {
    const chip = debugRouteChip();
    if (!(chip instanceof HTMLElement)) return;
    const value = String(route || "").trim();
    if (!value) {
      chip.hidden = true;
      chip.textContent = "";
      chip.removeAttribute("data-route");
      chip.removeAttribute("data-copied");
      return;
    }
    chip.hidden = false;
    chip.setAttribute("data-route", value);
    chip.removeAttribute("data-copied");
    chip.textContent = value;
    chip.title = "点击复制调试路由";
  }

  async function copyRoute(route) {
    const text = String(route || "").trim();
    if (!text) return false;
    try {
      if (global.navigator?.clipboard?.writeText) {
        await global.navigator.clipboard.writeText(text);
        return true;
      }
    } catch (_) {}
    try {
      const ta = global.document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.left = "-9999px";
      global.document.body.appendChild(ta);
      ta.select();
      const ok = global.document.execCommand("copy");
      ta.remove();
      return ok;
    } catch (_) {
      return false;
    }
  }

  function flashCopied(chip) {
    if (!(chip instanceof HTMLElement)) return;
    chip.setAttribute("data-copied", "1");
    chip.textContent = "已复制";
    chip.title = "已复制到剪贴板";
    global.setTimeout(() => {
      const route = chip.getAttribute("data-route") || "";
      chip.removeAttribute("data-copied");
      chip.textContent = route;
      chip.title = "点击复制调试路由";
    }, COPIED_MS);
  }

  function onStructureFocus(event) {
    const route = buildTempStageRoute(event?.detail || {});
    setChipRoute(route);
  }

  function onChipClick(event) {
    const chip = event?.currentTarget;
    if (!(chip instanceof HTMLElement)) return;
    const route = chip.getAttribute("data-route") || chip.textContent || "";
    copyRoute(route).then((ok) => {
      if (ok) flashCopied(chip);
    });
  }

  function bind() {
    const chip = debugRouteChip();
    if (chip instanceof HTMLElement && chip.dataset.bound !== "1") {
      chip.dataset.bound = "1";
      chip.addEventListener("click", onChipClick);
    }
  }

  global.document?.addEventListener?.("mei:structure-focus", onStructureFocus);
  if (global.document?.readyState === "loading") {
    global.document.addEventListener("DOMContentLoaded", bind);
  } else {
    bind();
  }
  // Thin-shell chrome may inject statusbar later.
  global.document?.addEventListener?.("mei:shell-chrome-ready", bind);
  global.setInterval(bind, 2000);
})(typeof window !== "undefined" ? window : globalThis);
