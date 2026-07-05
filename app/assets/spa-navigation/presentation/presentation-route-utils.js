/**
 * Presentation helpers on app surface (Run/Copilot host routes retired per 0517 Phase C).
 */
(function initPresentationRouteUtils(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const RP = global.MeiRoutePredicates || {};

  function isPresentationSurfaceRoute(pathname) {
    if (typeof RP.isPresentationCapableRoute === "function") {
      return RP.isPresentationCapableRoute(pathname);
    }
    const path = String(pathname || global.location?.pathname || "");
    return /^\/apps\/[^/]+\/app(?:\/|$)/.test(path) || /^\/apps\/(?:app|access)\//.test(path);
  }

  function parsePresentationAppId(pathname) {
    if (typeof RP.appIdFromAppsPathname === "function") {
      const fromApps = String(RP.appIdFromAppsPathname(pathname) || "").trim();
      if (fromApps) return fromApps;
    }
    const path = String(pathname || global.location?.pathname || "");
    const appFirst = path.match(/^\/apps\/([^/]+)\/app(?:\/|$)/);
    if (appFirst && appFirst[1]) return appFirst[1];
    const legacy = path.match(
      /^\/apps\/(?:app|access|access-only|access_only|run|copilot|speaker)\/([^/]+)/,
    );
    return legacy && legacy[1] ? legacy[1] : "";
  }

  function rewriteStepRoute(route) {
    if (typeof RP.rewriteLegacyPresentationRoute === "function") {
      return RP.rewriteLegacyPresentationRoute(route);
    }
    if (typeof global.rewriteLegacyPresentationRoute === "function") {
      return global.rewriteLegacyPresentationRoute(route);
    }
    return String(route || "").trim();
  }

  function dispatchWorldAction(detail) {
    if (!detail || typeof detail !== "object") return false;
    if (boot.worldStageRuntime?.applyWorldTarget) {
      return Boolean(boot.worldStageRuntime.applyWorldTarget(detail));
    }
    try {
      global.dispatchEvent(
        new CustomEvent("mei:presentation-world-action", {
          detail,
          bubbles: false,
        }),
      );
      return true;
    } catch (_) {
      return false;
    }
  }

  boot.presentationRouteUtils = {
    isPresentationSurfaceRoute,
    parsePresentationAppId,
    rewriteStepRoute,
    dispatchWorldAction,
  };
  global.MeiPresentationRouteUtils = boot.presentationRouteUtils;
})(typeof window !== "undefined" ? window : globalThis);
