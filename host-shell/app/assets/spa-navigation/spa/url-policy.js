  function isManageSamePathNavigation(currentUrl, nextUrl) {
    return (
      currentUrl.pathname === nextUrl.pathname &&
      (isWorkspaceSurfaceRoute(currentUrl.pathname) ||
        isWorkspaceSurfaceRoute(nextUrl.pathname))
    );
  }

  function shouldReloadHostBundle(path, currentUrl, nextUrl) {
    if (path === "/app-bundles/manage.js") {
      const cur = currentUrl.pathname.startsWith("/apps/manage/");
      const next = nextUrl.pathname.startsWith("/apps/manage/");
      return cur !== next;
    }
    if (path === "/app-bundles/access.js") {
      const cur = isAppRoute(currentUrl.pathname);
      const next = isAppRoute(nextUrl.pathname);
      return cur !== next;
    }
    return false;
  }

  function syncManageTabFromUrl(url) {
    try {
      const tab = new URL(url, window.location.href).searchParams.get("tab");
      if (typeof boot.switchManageTab === "function") {
        boot.switchManageTab(tab || "", { updateUrl: false, emit: true });
      }
    } catch (_) {}
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
        const rawHref = item.getAttribute("href") || "";
        let absolute = rawHref;
        try {
          absolute = new URL(rawHref, window.location.href).href;
        } catch (_) {}
        return {
          url: absolute,
          target: item.getAttribute("target") || "",
          download: item.hasAttribute("download"),
        };
      }
    }
    return null;
  }

  function isSameAppWorkspaceSurfaceSwitch(currentUrl, nextUrl) {
    try {
      if (typeof isUnifiedViewRoute === "function" && isUnifiedViewRoute(currentUrl.pathname) && isUnifiedViewRoute(nextUrl.pathname)) {
        const fromApp =
          typeof appIdFromAppsPathname === "function"
            ? appIdFromAppsPathname(currentUrl.pathname)
            : "";
        const toApp =
          typeof appIdFromAppsPathname === "function"
            ? appIdFromAppsPathname(nextUrl.pathname)
            : "";
        return Boolean(fromApp && fromApp === toApp);
      }
      if (typeof isAppWorkspaceSurfaceRoute !== "function") return false;
      if (typeof appIdFromAppsPathname !== "function") return false;
      if (typeof isWorkspaceSurfaceRoute !== "function") return false;
      const from = currentUrl instanceof URL ? currentUrl : new URL(currentUrl, window.location.href);
      const to = nextUrl instanceof URL ? nextUrl : new URL(nextUrl, window.location.href);
      if (!isWorkspaceSurfaceRoute(from.pathname) || !isWorkspaceSurfaceRoute(to.pathname)) {
        return false;
      }
      const fromApp = appIdFromAppsPathname(from.pathname);
      const toApp = appIdFromAppsPathname(to.pathname);
      return Boolean(fromApp && fromApp === toApp);
    } catch (_) {
      return false;
    }
  }

  /** 配置/上传/模式切换/跨应用 Tab 整页导航；Config/Upload 明确 no-cache + full-page。 */
  function shouldBypassSpaClick(event) {
    const target = resolveClickTarget(event);
    if (
      target?.url &&
      isSameAppWorkspaceSurfaceSwitch(window.location.href, target.url)
    ) {
      return false;
    }
    const path = event.composedPath ? event.composedPath() : [];
    let appViewSurfaceSwitch = false;
    for (const item of path) {
      if (!(item instanceof HTMLElement) || !item.matches) continue;
      if (item.matches("sl-button[data-mei-app-view], .mode-tab-btn[data-mei-app-view]")) {
        appViewSurfaceSwitch = true;
        continue;
      }
      if (item.matches("a.app-tab, a.app-tab-sub")) {
        return true;
      }
      if (
        item.matches(
          "a.host-runtime-nav-link, a[data-runtime-node-link='1'], a.manage-view-tab[data-manage-tab], [data-mei-view='config'], [data-mei-view='upload'], [data-mei-view='app'], [data-mei-view='build'], [data-mei-view='runtime'], a[data-manage-config-link='1'], sl-button[data-mei-view]",
        )
      ) {
        return true;
      }
    }
    if (appViewSurfaceSwitch) {
      return false;
    }
    return false;
  }

  function shouldAbortRuntimeForBypassNavigation(event) {
    const path = event.composedPath ? event.composedPath() : [];
    let appViewSurfaceSwitch = false;
    for (const item of path) {
      if (!(item instanceof HTMLElement) || !item.matches) continue;
      if (item.matches("sl-button[data-mei-app-view], .mode-tab-btn[data-mei-app-view]")) {
        appViewSurfaceSwitch = true;
        continue;
      }
      if (item.matches("a.app-tab, a.app-tab-sub")) {
        return true;
      }
      if (
        item.matches(
          "[data-mei-view='config'], [data-mei-view='upload'], [data-mei-view='app'], [data-mei-view='build'], [data-mei-view='runtime'], a[data-manage-config-link='1'], sl-button[data-mei-view]",
        )
      ) {
        return true;
      }
    }
    if (appViewSurfaceSwitch) {
      return false;
    }
    return false;
  }

  function requestRuntimeAbort(reason, options) {
    const opts = options && typeof options === "object" ? options : {};
    try {
      window.dispatchEvent(
        new CustomEvent("mei:abort-runtime-queries", {
          detail: {
            reason: String(reason || "").trim(),
            clearCaches: opts.clearCaches,
          },
        }),
      );
    } catch (_) {}
  }

  function isConfigOrUploadPath(pathname) {
    return /^\/apps\/(?:config|upload)\//.test(String(pathname || ""));
  }

  function shouldForceFullPageNavigation(currentUrl, nextUrl) {
    const current = new URL(currentUrl, window.location.href);
    const next = new URL(nextUrl, window.location.href);
    if (isConfigOrUploadPath(next.pathname)) return true;
    if (isConfigOrUploadPath(current.pathname) && current.pathname !== next.pathname) {
      return true;
    }
    // 跨应用：capabilities / bootstrap 落在 document head，SPA 只换 shell 会对不齐。
    if (typeof appIdFromAppsPathname === "function") {
      const fromApp = String(appIdFromAppsPathname(current.pathname) || "").trim();
      const toApp = String(appIdFromAppsPathname(next.pathname) || "").trim();
      if (fromApp && toApp && fromApp !== toApp) return true;
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

  /** Build-tree clicks on layout/prototype are handled by MeiBuildTreePersist / inspect highlight. */
  function shouldDeferBuildTreeClick(event) {
    const path = event.composedPath ? event.composedPath() : [];
    let isBuildTreeLink = false;
    for (const item of path) {
      if (
        item instanceof HTMLElement &&
        item.matches?.("a.build-tree-link, a.build-tree-label--link")
      ) {
        isBuildTreeLink = true;
        break;
      }
    }
    if (!isBuildTreeLink) return false;
    if (typeof isWorkspaceSurfaceRoute === "function" && isWorkspaceSurfaceRoute()) {
      return true;
    }
    try {
      const boot = window.__meiLangBoot;
      if (typeof boot?.parseViewContext === "function") {
        const ctx = boot.parseViewContext(window.location.href);
        const surface = String(ctx?.surface || ctx?.mode || "").trim().toLowerCase();
        return surface === "layout" || surface === "prototype";
      }
    } catch (_) {}
    return false;
  }

