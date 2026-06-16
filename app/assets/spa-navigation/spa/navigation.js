  async function loadAndSwap(url, replaceHistory, navigationId) {
    const fetchController = new AbortController();
    const fetchTimer = setTimeout(() => fetchController.abort(), SPA_FETCH_TIMEOUT_MS);
    let response;
    try {
      response = await fetch(url, {
        credentials: "same-origin",
        headers: { "x-mei-spa-nav": "1" },
        signal: fetchController.signal,
      });
    } finally {
      clearTimeout(fetchTimer);
    }
    if (!response.ok) throw new Error("navigation failed: " + response.status);
    const html = await response.text();
    if (typeof boot.recordLoadingNavigationResponse === "function") {
      boot.recordLoadingNavigationResponse(response, navigationId, html.length);
    }
    if (navigationId !== currentNavigationId) return false;
    const doc = new DOMParser().parseFromString(html, "text/html");
    const nextShell = doc.querySelector(".shell");
    const currentShell = document.querySelector(".shell");
    if (!nextShell || !currentShell) {
      const err = new Error("spa shell missing in response");
      err.meiSpaHardNav = true;
      throw err;
    }
    if (shouldForceHardNavForSceneBundleSwitch(doc)) {
      const err = new Error("scene bundle changed; fallback to hard navigation");
      err.meiSpaHardNav = true;
      throw err;
    }
    const currentUrl = new URL(window.location.href);
    const nextUrl = new URL(url, window.location.href);
    const preserveManageWorkspace = shouldPreserveManageWorkspace(currentUrl, nextUrl);
    disposeRuntimeHooks({
      preserveAgentPanel: preserveManageWorkspace,
      preserveStatusBar: preserveManageWorkspace,
      preserveManageTabs: preserveManageWorkspace,
      preserveWorkspaceSplitters: preserveManageWorkspace,
      preserveFrameStage: preserveManageWorkspace,
      preserveSourceTreeControls: preserveManageWorkspace,
      preserveSourceHighlight: preserveManageWorkspace,
    });
    if (preserveManageWorkspace && typeof window.__meiClearRuntimePerfDiagnostics === "function") {
      try {
        window.__meiClearRuntimePerfDiagnostics("SPA 换文件");
      } catch (_) {}
    }
    document.title = doc.title || document.title;
    if (document.body.className !== doc.body.className) {
      document.body.className = doc.body.className;
    }
    if (preserveManageWorkspace) {
      const swapped = swapManageWorkspace(doc, url, replaceHistory);
      if (!swapped) {
        replaceShellFromDoc(doc, url, replaceHistory);
      }
    } else {
      replaceShellFromDoc(doc, url, replaceHistory);
    }
    if (navigationId !== currentNavigationId) return false;
    if (typeof boot.markLoadingRenderSwapDone === "function") {
      boot.markLoadingRenderSwapDone(navigationId);
    }
    runPostSpaWork(doc, url, navigationId, currentUrl, nextUrl);
    return true;
  }

  async function navigateInternal(url, replaceHistory) {
    currentNavigationId += 1;
    const navigationId = currentNavigationId;
    spaNavigationInFlight += 1;
    boot._spaInFlight = spaNavigationInFlight;
    requestRuntimeAbort("spa_navigation");
    closeDrilldownOverlay();
    let currentUrl = null;
    let nextUrl = null;
    try {
      currentUrl = new URL(window.location.href);
      nextUrl = new URL(url, window.location.href);
    } catch (_) {}
    const manageSamePath =
      currentUrl && nextUrl && isManageSamePathNavigation(currentUrl, nextUrl);
    if (manageSamePath) {
      showManageWorkspaceLoadingState(url);
    } else {
      showManageWorkspaceLoadingState(url);
      showLoadingNow();
    }
    if (typeof boot.beginLoadingProgressSession === "function") {
      boot.beginLoadingProgressSession(navigationId, url);
    }
    try {
      const completed = await loadAndSwap(url, replaceHistory, navigationId);
      if (!completed && navigationId === currentNavigationId) {
        console.warn("[spa-navigation] navigation superseded", url);
      }
    } catch (error) {
      if (typeof boot.abortLoadingProgressSession === "function") {
        boot.abortLoadingProgressSession(navigationId, "navigation_error");
      }
      console.error("[spa-navigation] navigation failed", error);
      if (error && error.name === "AbortError") {
        console.warn("[spa-navigation] fetch timeout", url);
        if (window.MeiHostHttpFeedback && typeof window.MeiHostHttpFeedback.notify === "function") {
          window.MeiHostHttpFeedback.notify({
            status: 504,
            url: url,
            title: "页面加载超时",
            message: "SPA 导航等待宿主响应超时，请稍后重试或刷新页面。",
          });
        }
        return;
      }
      if (error && error.meiSpaHardNav) {
        window.location.assign(url);
        return;
      }
      const statusMatch =
        error && error.message && String(error.message).match(/navigation failed:\s*(\d{3})/);
      const status = statusMatch ? Number(statusMatch[1]) : 500;
      if (window.MeiHostHttpFeedback && typeof window.MeiHostHttpFeedback.notify === "function") {
        window.MeiHostHttpFeedback.notify({
          status: status,
          url: url,
          title: status === 404 ? "页面不存在" : "页面加载失败",
          message:
            (error && error.message ? String(error.message) : "导航失败") +
            "。可尝试刷新或联系管理员。",
        });
      } else {
        window.location.assign(url);
      }
    } finally {
      spaNavigationInFlight = Math.max(0, spaNavigationInFlight - 1);
      boot._spaInFlight = spaNavigationInFlight;
      await finishNavigationUi(navigationId);
    }
  }

  boot.navigateSpa = function (url, replaceHistory) {
    return navigateInternal(url, !!replaceHistory);
  };

