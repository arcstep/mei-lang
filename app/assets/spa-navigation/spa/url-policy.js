  function isManageSamePathNavigation(currentUrl, nextUrl) {
    return (
      currentUrl.pathname === nextUrl.pathname &&
      (currentUrl.pathname.startsWith("/apps/manage/") ||
        currentUrl.pathname.startsWith("/apps/build/"))
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

  /** 构建页内 Tab 走客户端切换；配置/上传/模式切换/跨应用 Tab 整页导航。 */
  function shouldBypassSpaClick(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (!(item instanceof HTMLElement) || !item.matches) continue;
      if (
        item.matches(
          "a.host-runtime-nav-link, a.manage-view-tab[data-manage-tab], [data-mei-view='config'], [data-mei-view='upload'], [data-mei-view='app'], [data-mei-view='build'], [data-mei-view='runtime'], a[data-manage-config-link='1'], a.app-tab, a.app-tab-sub, sl-button[data-mei-view]",
        )
      ) {
        return true;
      }
    }
    return false;
  }

  function shouldAbortRuntimeForBypassNavigation(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (!(item instanceof HTMLElement) || !item.matches) continue;
      if (
        item.matches(
          "[data-mei-view='config'], [data-mei-view='upload'], [data-mei-view='app'], [data-mei-view='build'], [data-mei-view='runtime'], a[data-manage-config-link='1'], a.app-tab, a.app-tab-sub, sl-button[data-mei-view]",
        )
      ) {
        return true;
      }
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

  function normalizePath(rawUrl) {
    try {
      const parsed = new URL(rawUrl, window.location.href);
      return parsed.pathname;
    } catch (_) {
      return "";
    }
  }

