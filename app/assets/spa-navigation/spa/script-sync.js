  const SCENE_BUNDLE_PATH_PREFIX = "/workspace-components/bundles/";

  function isSceneBundlePath(path) {
    return Boolean(path && path.startsWith(SCENE_BUNDLE_PATH_PREFIX));
  }

  function isWorkspaceComponentModulePath(path) {
    return path.startsWith("/workspace-components/") && !isSceneBundlePath(path);
  }

  function findSceneBundleSrcInDoc(doc) {
    if (!doc) return "";
    for (const script of doc.querySelectorAll("script[src]")) {
      const src = (script.getAttribute("src") || "").trim();
      if (!src) continue;
      const path = normalizePath(src);
      if (isSceneBundlePath(path) || script.getAttribute("data-mei-scene-bundle") === "true") {
        return src;
      }
    }
    return "";
  }

  function currentSceneBundlePath() {
    const node = document.querySelector('script[data-mei-scene-bundle="true"]');
    if (!node) return "";
    const persisted = (node.getAttribute("data-mei-persistent-script") || "").trim();
    if (persisted) return persisted;
    return normalizePath(node.getAttribute("src") || "");
  }

  function shouldForceHardNavForSceneBundleSwitch(doc) {
    const nextBundleSrc = findSceneBundleSrcInDoc(doc);
    if (!nextBundleSrc) return false;
    const nextPath = normalizePath(nextBundleSrc);
    if (!nextPath) return false;
    const currentPath = currentSceneBundlePath();
    if (!currentPath) return false;
    return currentPath !== nextPath;
  }

  function removeExistingSceneBundleScripts() {
    document.querySelectorAll('script[data-mei-scene-bundle="true"]').forEach((node) => {
      node.remove();
    });
  }

  async function syncSceneBundleFromDoc(doc, navigationId) {
    const bundleSrc = findSceneBundleSrcInDoc(doc);
    if (!bundleSrc) {
      removeExistingSceneBundleScripts();
      return true;
    }
    const path = normalizePath(bundleSrc);
    const alreadyLoaded = document.querySelector(
      'script[data-mei-scene-bundle="true"][data-mei-persistent-script="' + path + '"]',
    );
    if (alreadyLoaded) return true;
    removeExistingSceneBundleScripts();
    if (navigationId !== currentNavigationId) return false;
    await loadScript(bundleSrc, {
      module: true,
      persistentKey: path,
      sceneBundle: true,
      softFail: true,
    });
    if (navigationId === currentNavigationId) {
      wakeRuntimeAfterSceneBundleLoaded();
    }
    return navigationId === currentNavigationId;
  }

  function collectBodyScripts(doc) {
    if (!doc) return [];
    return Array.from(doc.querySelectorAll("script[src]"))
      .map((script) => script.getAttribute("src") || "")
      .map((src) => src.trim())
      .filter(Boolean);
  }

  function tagExistingBodyScripts() {
    Array.from(document.querySelectorAll("script[src]")).forEach((script) => {
      const src = script.getAttribute("src");
      if (!src) return;
      const path = normalizePath(src);
      if (!path || path === SPA_NAV_SCRIPT) return;
      if (isSceneBundlePath(path) || script.getAttribute("data-mei-scene-bundle") === "true") {
        script.setAttribute("data-mei-scene-bundle", "true");
        script.setAttribute("data-mei-persistent-script", path);
        return;
      }
      if (isWorkspaceComponentModulePath(path)) {
        script.setAttribute("data-mei-persistent-script", path);
        return;
      }
      if (path.startsWith("/app-assets/")) {
        if (RELOAD_APP_SCRIPTS.has(path)) {
          script.setAttribute("data-mei-reload-script", path);
        } else {
          script.setAttribute("data-mei-persistent-script", path);
        }
        return;
      }
      if (path.startsWith("/app-bundles/")) {
        if (RELOAD_BUNDLE_SCRIPTS.has(path)) {
          script.setAttribute("data-mei-reload-script", path);
        } else {
          script.setAttribute("data-mei-persistent-script", path);
        }
      }
    });
  }

  function disposeRuntimeHooks(options) {
    const opts = options || {};
    const names = [
      "disposeAgentPanel",
      "disposeStatusBar",
      "disposeManageTabs",
      "disposeWorkspaceSplitters",
      "disposeFrameStage",
      "disposeSourceTreeControls",
      "disposeSourceHighlight",
    ];
    names.forEach((name) => {
      if (opts.preserveAgentPanel && name === "disposeAgentPanel") return;
      if (opts.preserveStatusBar && name === "disposeStatusBar") return;
      if (opts.preserveManageTabs && name === "disposeManageTabs") return;
      if (opts.preserveWorkspaceSplitters && name === "disposeWorkspaceSplitters") return;
      if (opts.preserveFrameStage && name === "disposeFrameStage") return;
      if (opts.preserveSourceTreeControls && name === "disposeSourceTreeControls") return;
      if (opts.preserveSourceHighlight && name === "disposeSourceHighlight") return;
      const hook = boot[name];
      if (typeof hook === "function") {
        try {
          hook();
        } catch (_) {}
        boot[name] = null;
      }
    });
  }

