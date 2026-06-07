/**
 * 路由解析、管理页 tab、会话绑定与 localStorage/sessionStorage 键工厂。由 agent-panel 主文件装配 `RT`。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallRouting = function (api) {
    const root = api.root;
    const boot = api.boot;
    const $U = api.$U;

    function currentTarget() {
      const params = new URLSearchParams(window.location.search);
      const fromUrl = params.get("file") || params.get("target");
      if (fromUrl && String(fromUrl).trim()) return String(fromUrl).trim();
      const sceneRouteTarget = String(root.dataset.sceneTarget || "").trim();
      if (sceneRouteTarget) return sceneRouteTarget;
      return String(root.dataset.file || root.dataset.target || "").trim();
    }

    function currentManageTab() {
      const params = new URLSearchParams(window.location.search);
      const raw = String(params.get("tab") || root.dataset.viewTab || "preview")
        .trim()
        .toLowerCase();
      if (raw === "source" || raw === "diff" || raw === "diagnostics") return raw;
      return "preview";
    }

    function setManageTab(tab) {
      const next = String(tab || "").trim().toLowerCase();
      if (!next) return currentManageTab();
      if (typeof boot.switchManageTab === "function") {
        return boot.switchManageTab(next);
      }
      const url = new URL(window.location.href);
      url.searchParams.set("tab", next);
      window.location.assign(url.toString());
      return next;
    }

    function normalizeTargetKey(target) {
      return $U.normalizeFilePath(target);
    }

    function currentTargetKey() {
      return normalizeTargetKey(currentTarget());
    }

    function currentAppKey() {
      const fromDataset = String(root.dataset.app || "").trim();
      try {
        const path = window.location.pathname || "";
        const prefixes = [
          "/apps/build/",
          "/apps/manage/",
          "/apps/app/",
          "/apps/access/",
        ];
        for (const prefix of prefixes) {
          if (!path.startsWith(prefix)) continue;
          let rest = path.slice(prefix.length);
          const sceneSeg = "/scene/";
          const sceneIdx = rest.indexOf(sceneSeg);
          if (sceneIdx >= 0) {
            rest = rest.slice(0, sceneIdx);
          }
          const slashQ = rest.indexOf("/?");
          if (slashQ >= 0) rest = rest.slice(0, slashQ);
          rest = rest.replace(/\/+$/, "");
          if (rest) return rest;
          break;
        }
      } catch (_) {}
      return fromDataset;
    }

    function currentSceneId() {
      return String(root.dataset.scene || "").trim();
    }

    function normalizeRouteMode(value) {
      const mode = String(value || "").trim().toLowerCase();
      if (mode === "access" || mode === "app" || mode === "run") return "access";
      return "manage";
    }

    /** 与 SSR `data-history-actions` / `data-source-views` 一致：访问壳不拉作者写回 diff。 */
    function panelAuthoringEnabled() {
      const history = String(root.dataset.historyActions || "").trim().toLowerCase();
      const sourceViews = String(root.dataset.sourceViews || "").trim().toLowerCase();
      if (history === "true" || sourceViews === "true") return true;
      if (history === "false" && sourceViews === "false") return false;
      return normalizeRouteMode(root.dataset.mode) !== "access";
    }

    function sessionBindingKind() {
      return "scene";
    }

    function currentSessionBindingFingerprint() {
      const sid = currentSceneId() || "__no_scene__";
      return "scene:" + sid;
    }

    function sessionBindingStorageKey() {
      const sid = currentSceneId() || "__no_scene__";
      return "scene:" + sid;
    }

    function sessionStorageKey() {
      return "mei-lang.agent.session." + currentAppKey() + "." + sessionBindingStorageKey();
    }

    function modeStorageKey() {
      return "mei-lang.agent.mode." + currentAppKey() + "." + sessionBindingStorageKey();
    }

    function accessFloatingStorageKey() {
      return "mei-lang.agent.access-floating." + currentAppKey();
    }

    function accessFloatingPositionStorageKey() {
      return "mei-lang.agent.access-floating-position." + currentAppKey();
    }

    function revertedStorageKey() {
      return "mei-lang.agent.reverted." + currentAppKey() + "." + sessionBindingStorageKey();
    }

    function deltaDebugStorageKey(sessionId) {
      const sid = String(sessionId || "").trim();
      if (!sid) return "";
      return "mei-lang.agent.delta-debug." + currentAppKey() + "." + sid;
    }

    function normalizeAgentMode(value) {
      const normalizedRoute = normalizeRouteMode(root.dataset.mode);
      const allowed = String(root.dataset.allowedModes || "")
        .split(",")
        .map(function (item) {
          const raw = String(item || "").trim().toLowerCase();
          if (raw === "plan") return "ask";
          if (raw === "ask" || raw === "build") return raw;
          return "";
        })
        .filter(Boolean);
      if (!allowed.length) {
        if (normalizedRoute === "access") {
          allowed.push("ask");
        } else {
          allowed.push("build");
        }
      }
      const defaultFromDataset = String(root.dataset.defaultAgentMode || "").trim().toLowerCase();
      const fallback =
        allowed.indexOf(defaultFromDataset) >= 0
          ? defaultFromDataset
          : allowed[0];
      const raw = String(value || "").trim().toLowerCase();
      const mapped = raw === "plan" ? "ask" : raw === "ask" ? "ask" : "build";
      return allowed.indexOf(mapped) >= 0 ? mapped : fallback;
    }

    function buildBoundSessionTitle(targetKey) {
      const params = new URLSearchParams();
      params.set("app", String(root.dataset.app || ""));
      params.set("bind", "scene");
      params.set("scene", currentSceneId() || "");
      params.set("anchor", String(targetKey || "").trim());
      return "MEI|" + params.toString();
    }

    function parseBoundSessionTitle(title) {
      const value = String(title || "");
      if (!value.startsWith("MEI|")) return null;
      try {
        const params = new URLSearchParams(value.slice(4));
        const app = String(params.get("app") || "").trim();
        if (!app) return null;
        const bindRaw = String(params.get("bind") || "").trim().toLowerCase();
        if (bindRaw === "scene") {
          const scene = String(params.get("scene") || "").trim();
          const anchor = normalizeTargetKey(params.get("anchor") || "");
          if (!scene) return null;
          return {
            app: app,
            bind: "scene",
            scene: scene,
            anchor: anchor,
            target: anchor,
          };
        }
        const target = normalizeTargetKey(params.get("file") || params.get("target") || "");
        const scene = String(params.get("scene") || "").trim();
        if (!target && !scene) return null;
        return {
          app: app,
          bind: "scene",
          scene: scene || "__legacy_file__",
          anchor: target,
          target: target,
        };
      } catch (_) {
        return null;
      }
    }

    return {
      currentTarget: currentTarget,
      currentManageTab: currentManageTab,
      setManageTab: setManageTab,
      normalizeTargetKey: normalizeTargetKey,
      currentTargetKey: currentTargetKey,
      currentAppKey: currentAppKey,
      currentSceneId: currentSceneId,
      sessionBindingKind: sessionBindingKind,
      currentSessionBindingFingerprint: currentSessionBindingFingerprint,
      sessionBindingStorageKey: sessionBindingStorageKey,
      sessionStorageKey: sessionStorageKey,
      modeStorageKey: modeStorageKey,
      accessFloatingStorageKey: accessFloatingStorageKey,
      accessFloatingPositionStorageKey: accessFloatingPositionStorageKey,
      revertedStorageKey: revertedStorageKey,
      deltaDebugStorageKey: deltaDebugStorageKey,
      normalizeAgentMode: normalizeAgentMode,
      normalizeRouteMode: normalizeRouteMode,
      panelAuthoringEnabled: panelAuthoringEnabled,
      buildBoundSessionTitle: buildBoundSessionTitle,
      parseBoundSessionTitle: parseBoundSessionTitle,
    };
  };
})(window);
