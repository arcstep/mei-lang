(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.spaNavigationMounted) return;
  boot.spaNavigationMounted = true;

  const RELOAD_APP_SCRIPTS = new Set([
    "/app-assets/frame-stage.js",
    "/app-assets/statusbar.js",
    "/app-assets/manage-tabs.js",
    "/app-assets/workspace-splitters.js",
    "/app-assets/source-tree-controls.js",
    "/app-assets/source-highlight.js",
    "/app-assets/opencode-panel.js",
  ]);
  const SPA_NAV_SCRIPT = "/app-assets/spa-navigation.js";
  const LOADING_DELAY_MS = 140;
  const LOADING_MIN_VISIBLE_MS = 180;
  let currentNavigationId = 0;
  let activeController = null;
  let loadingTimer = null;
  let loadingVisibleAt = 0;

  function createLoadingOverlay() {
    if (document.getElementById("mei-spa-loading")) return;
    const overlay = document.createElement("div");
    overlay.id = "mei-spa-loading";
    overlay.className = "spa-loading-overlay";
    overlay.innerHTML =
      '<div class="spa-loading-inner">' +
      '<img class="spa-loading-icon" src="/app-assets/favicon.svg" alt="loading"/>' +
      '<span class="spa-loading-text">加载中...</span>' +
      "</div>";
    document.body.appendChild(overlay);
  }

  function clearLoadingTimer() {
    if (loadingTimer) {
      clearTimeout(loadingTimer);
      loadingTimer = null;
    }
  }

  function showLoading() {
    clearLoadingTimer();
    loadingTimer = setTimeout(() => {
      createLoadingOverlay();
      const overlay = document.getElementById("mei-spa-loading");
      if (!overlay) return;
      overlay.classList.add("is-visible");
      loadingVisibleAt = Date.now();
      loadingTimer = null;
    }, LOADING_DELAY_MS);
  }

  function hideLoading() {
    clearLoadingTimer();
    const overlay = document.getElementById("mei-spa-loading");
    if (!overlay || !overlay.classList.contains("is-visible")) return;
    const elapsed = Date.now() - loadingVisibleAt;
    const finish = () => {
      overlay.classList.remove("is-visible");
    };
    if (elapsed < LOADING_MIN_VISIBLE_MS) {
      setTimeout(finish, LOADING_MIN_VISIBLE_MS - elapsed);
    } else {
      finish();
    }
  }

  function sameOrigin(url) {
    try {
      const parsed = new URL(url, window.location.href);
      return parsed.origin === window.location.origin;
    } catch (_) {
      return false;
    }
  }

  function shouldHandleUrl(url) {
    if (!sameOrigin(url)) return false;
    const parsed = new URL(url, window.location.href);
    return parsed.pathname.startsWith("/apps/");
  }

  function isSameLocation(url) {
    try {
      const next = new URL(url, window.location.href);
      const current = new URL(window.location.href);
      return (
        next.pathname === current.pathname &&
        next.search === current.search &&
        next.hash === current.hash
      );
    } catch (_) {
      return false;
    }
  }

  function resolveClickTarget(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (item instanceof HTMLAnchorElement && item.href) {
        return {
          url: item.href,
          target: item.getAttribute("target") || "",
          download: item.hasAttribute("download"),
        };
      }
      if (
        item instanceof HTMLElement &&
        item.tagName === "SL-BUTTON" &&
        item.hasAttribute("href")
      ) {
        return {
          url: item.getAttribute("href"),
          target: item.getAttribute("target") || "",
          download: item.hasAttribute("download"),
        };
      }
    }
    return null;
  }

  function shouldBypassSpaClick(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (
        item instanceof HTMLElement &&
        item.matches &&
        item.matches("a.manage-view-tab[data-manage-tab]")
      ) {
        return true;
      }
    }
    return false;
  }

  function normalizePath(rawUrl) {
    try {
      const parsed = new URL(rawUrl, window.location.href);
      return parsed.pathname;
    } catch (_) {
      return "";
    }
  }

  function collectBodyScripts(doc) {
    return Array.from(doc.body.querySelectorAll("script[src]"))
      .map((script) => script.getAttribute("src") || "")
      .map((src) => src.trim())
      .filter(Boolean);
  }

  function tagExistingBodyScripts() {
    Array.from(document.body.querySelectorAll("script[src]")).forEach((script) => {
      const src = script.getAttribute("src");
      if (!src) return;
      const path = normalizePath(src);
      if (!path || path === SPA_NAV_SCRIPT) return;
      if (path.startsWith("/workspace-components/")) {
        script.setAttribute("data-mei-persistent-script", path);
        return;
      }
      if (!path.startsWith("/app-assets/")) return;
      if (RELOAD_APP_SCRIPTS.has(path)) {
        script.setAttribute("data-mei-reload-script", path);
      } else {
        script.setAttribute("data-mei-persistent-script", path);
      }
    });
  }

  function disposeRuntimeHooks() {
    const names = [
      "disposeOpencodePanel",
      "disposeStatusBar",
      "disposeManageTabs",
      "disposeWorkspaceSplitters",
      "disposeFrameStage",
    ];
    names.forEach((name) => {
      const hook = boot[name];
      if (typeof hook === "function") {
        try {
          hook();
        } catch (_) {}
        boot[name] = null;
      }
    });
  }

  function loadScript(rawSrc, options) {
    const opts = options || {};
    const absolute = new URL(rawSrc, window.location.href).toString();
    if (opts.persistentKey) {
      const found = document.querySelector(
        'script[data-mei-persistent-script="' + opts.persistentKey + '"]',
      );
      if (found) return Promise.resolve();
    }
    if (opts.reloadKey) {
      document
        .querySelectorAll('script[data-mei-reload-script="' + opts.reloadKey + '"]')
        .forEach((node) => node.remove());
    }
    return new Promise((resolve, reject) => {
      const script = document.createElement("script");
      if (opts.module) script.type = "module";
      script.src = absolute;
      script.async = false;
      if (opts.persistentKey) {
        script.setAttribute("data-mei-persistent-script", opts.persistentKey);
      }
      if (opts.reloadKey) {
        script.setAttribute("data-mei-reload-script", opts.reloadKey);
      }
      script.onload = () => resolve();
      script.onerror = () => reject(new Error("failed to load script: " + rawSrc));
      document.body.appendChild(script);
    });
  }

  async function syncScriptsFromDocument(doc, navigationId) {
    const scripts = collectBodyScripts(doc);
    for (const src of scripts) {
      if (navigationId !== currentNavigationId) return;
      const path = normalizePath(src);
      if (!path) continue;
      if (path === SPA_NAV_SCRIPT) continue;
      if (path.startsWith("/workspace-components/")) {
        await loadScript(src, { module: true, persistentKey: path });
        continue;
      }
      if (!path.startsWith("/app-assets/")) continue;
      if (RELOAD_APP_SCRIPTS.has(path)) {
        const withBuster = path + "?spa=" + Date.now();
        await loadScript(withBuster, { reloadKey: path });
        continue;
      }
      await loadScript(src, { persistentKey: path });
    }
  }

  async function loadAndSwap(url, replaceHistory, navigationId, controller) {
    const response = await fetch(url, {
      credentials: "same-origin",
      headers: { "x-mei-spa-nav": "1" },
      signal: controller.signal,
    });
    if (!response.ok) throw new Error("navigation failed: " + response.status);
    const html = await response.text();
    if (navigationId !== currentNavigationId) return;
    const doc = new DOMParser().parseFromString(html, "text/html");
    const nextShell = doc.querySelector(".shell");
    const currentShell = document.querySelector(".shell");
    if (!nextShell || !currentShell) {
      window.location.assign(url);
      return;
    }
    disposeRuntimeHooks();
    document.title = doc.title || document.title;
    if (document.body.className !== doc.body.className) {
      document.body.className = doc.body.className;
    }
    currentShell.className = nextShell.className;
    const nextNodes = Array.from(nextShell.childNodes).map((node) =>
      node.cloneNode(true),
    );
    currentShell.replaceChildren(...nextNodes);
    if (replaceHistory) {
      window.history.replaceState({}, "", url);
    } else {
      window.history.pushState({}, "", url);
    }
    await syncScriptsFromDocument(doc, navigationId);
  }

  async function navigate(url, replaceHistory) {
    currentNavigationId += 1;
    const navigationId = currentNavigationId;
    if (activeController) {
      try {
        activeController.abort();
      } catch (_) {}
    }
    activeController = new AbortController();
    showLoading();
    try {
      await loadAndSwap(url, replaceHistory, navigationId, activeController);
    } catch (error) {
      if (error && error.name === "AbortError") return;
      console.error("[spa-navigation] fallback to hard reload", error);
      window.location.assign(url);
    } finally {
      if (navigationId === currentNavigationId) {
        hideLoading();
      }
    }
  }

  tagExistingBodyScripts();

  document.addEventListener(
    "click",
    (event) => {
      if (event.defaultPrevented) return;
      if (event.button !== 0) return;
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      if (shouldBypassSpaClick(event)) return;
      const target = resolveClickTarget(event);
      if (!target) return;
      if (target.download) return;
      if (target.target && target.target !== "_self") return;
      if (!shouldHandleUrl(target.url)) return;
      if (isSameLocation(target.url)) {
        event.preventDefault();
        return;
      }
      event.preventDefault();
      void navigate(target.url, false);
    },
    true,
  );

  window.addEventListener("popstate", () => {
    if (shouldHandleUrl(window.location.href)) {
      void navigate(window.location.href, true);
    }
  });
})();
