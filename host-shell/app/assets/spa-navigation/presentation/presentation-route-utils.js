/**
 * Presentation helpers on Access stage routes (`/apps/{app}/{stage}`).
 */
(function initPresentationRouteUtils(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const RP = global.MeiRoutePredicates || {};

  const RESERVED_STAGE_SEGMENTS = new Set([
    "view",
    "layout",
    "prototype",
    "app",
    "access",
    "build",
    "manage",
  ]);

  function isPresentationSurfaceRoute(pathname) {
    if (typeof RP.isAccessStageRoute === "function" && RP.isAccessStageRoute(pathname)) {
      return true;
    }
    if (typeof RP.isPresentationCapableRoute === "function") {
      return RP.isPresentationCapableRoute(pathname);
    }
    const path = String(pathname || global.location?.pathname || "");
    return (
      /^\/apps\/[^/]+\/[^/]+(?:\/|$)/.test(path) ||
      /^\/apps\/[^/]+\/(?:app|view)(?:\/|$)/.test(path) ||
      /^\/apps\/(?:app|access)\//.test(path)
    );
  }

  function parsePresentationAppId(pathname) {
    if (typeof RP.appIdFromAppsPathname === "function") {
      const fromApps = String(RP.appIdFromAppsPathname(pathname) || "").trim();
      if (fromApps) return fromApps;
    }
    const path = String(pathname || global.location?.pathname || "");
    const stageFirst = path.match(/^\/apps\/([^/]+)(?:\/|$)/);
    if (
      stageFirst &&
      stageFirst[1] &&
      !["app", "access", "view", "layout", "prototype"].includes(stageFirst[1])
    ) {
      return stageFirst[1];
    }
    const appFirst = path.match(/^\/apps\/([^/]+)\/(?:app|view)(?:\/|$)/);
    if (appFirst && appFirst[1]) return appFirst[1];
    const legacy = path.match(
      /^\/apps\/(?:app|access|access-only|access_only|run|copilot|speaker)\/([^/]+)/,
    );
    return legacy && legacy[1] ? legacy[1] : "";
  }

  function parsePresentationSceneId(pathname) {
    const path = String(pathname || global.location?.pathname || "");
    const stageMatch = path.match(/^\/apps\/[^/]+\/([^/?#]+)/);
    if (stageMatch) {
      const seg = String(stageMatch[1] || "").trim();
      if (seg && !RESERVED_STAGE_SEGMENTS.has(seg.toLowerCase())) return seg;
    }
    const sceneMatch = path.match(/\/scene\/([^/?#]+)/);
    if (sceneMatch) return String(sceneMatch[1] || "").trim();
    const mei = global.__mei;
    return String(mei?.active_scene_id || mei?.activeSceneId || "home").trim() || "home";
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
    parsePresentationSceneId,
    rewriteStepRoute,
    dispatchWorldAction,
  };
  global.MeiPresentationRouteUtils = boot.presentationRouteUtils;
})(typeof window !== "undefined" ? window : globalThis);
