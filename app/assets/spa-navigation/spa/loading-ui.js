  function currentMainPane() {
    return document.querySelector("#workspace-root main.main");
  }

  function clearManageWorkspaceLoadingState() {
    const currentMain = currentMainPane();
    if (!currentMain) return;
    currentMain.removeAttribute("aria-busy");
    const overlay = currentMain.querySelector('[data-mei-manage-nav-loading="true"]');
    if (overlay) overlay.remove();
  }

  function showManageWorkspaceLoadingState(_url) {
    // 统一 view SPA 已用全局 loading + 访问历史；不再显示「正在切换预览」遮罩。
  }

  function loadingOverlayMarkup() {
    return (
      '<div class="spa-loading-inner">' +
      '<img class="spa-loading-icon" src="/app-assets/favicon.svg" alt="loading"/>' +
      '<div class="spa-loading-body">' +
      '<span class="spa-loading-text">加载中…</span>' +
      '<div class="spa-loading-track">' +
      '<div class="spa-loading-bar"><div class="spa-loading-bar-fill"></div></div>' +
      "</div>" +
      '<div class="spa-loading-detail"></div>' +
      "</div>" +
      "</div>"
    );
  }

  function createLoadingOverlay() {
    if (document.getElementById("mei-spa-loading")) return;
    const overlay = document.createElement("div");
    overlay.id = "mei-spa-loading";
    overlay.className = "spa-loading-overlay";
    overlay.innerHTML = loadingOverlayMarkup();
    document.body.appendChild(overlay);
  }

  function isSpaLoadingVisible() {
    const overlay = document.getElementById("mei-spa-loading");
    return Boolean(overlay && overlay.classList.contains("is-visible"));
  }

  function clearLoadingTimer() {
    if (loadingTimer) {
      clearTimeout(loadingTimer);
      loadingTimer = null;
    }
  }

  function shouldKeepLoadingVisible() {
    if (typeof boot.getLoadingProgressSession === "function") {
      const session = boot.getLoadingProgressSession();
      if (session) return true;
    }
    return Boolean(
      window.MeiPageLoadProgress &&
        typeof window.MeiPageLoadProgress.isTracking === "function" &&
        window.MeiPageLoadProgress.isTracking(),
    );
  }

  function scheduleLoadingShow() {
    clearLoadingTimer();
    loadingTimer = setTimeout(() => {
      loadingTimer = null;
      if (!shouldKeepLoadingVisible()) return;
      showLoadingNow();
    }, LOADING_SHOW_DELAY_MS);
  }

  function showLoading() {
    scheduleLoadingShow();
  }

  function hideLoading() {
    clearLoadingTimer();
    const overlay = document.getElementById("mei-spa-loading");
    if (!overlay) return;
    if (!overlay.classList.contains("is-visible")) {
      return;
    }
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

  function forceHideLoading() {
    clearLoadingTimer();
    const overlay = document.getElementById("mei-spa-loading");
    if (overlay) {
      overlay.classList.remove("is-visible");
    }
  }

  let drilldownProgressTimers = new WeakMap();

  function drilldownProgressHost(root) {
    if (!(root instanceof HTMLElement)) return null;
    return root.querySelector("[data-mei-drilldown-load-progress]");
  }

  function isDrilldownProgressVisible(root) {
    const host = drilldownProgressHost(root);
    return Boolean(host && host.classList.contains("is-progress-visible"));
  }

  function clearDrilldownProgressTimer(root) {
    const timer = drilldownProgressTimers.get(root);
    if (timer) {
      clearTimeout(timer);
      drilldownProgressTimers.delete(root);
    }
  }

  function revealDrilldownProgress(root) {
    const host = drilldownProgressHost(root);
    if (!host) return;
    const session =
      typeof boot.getActiveLoadSession === "function" ? boot.getActiveLoadSession() : null;
    if (!session || session.kind !== "drilldown") return;
    host.classList.add("is-progress-visible");
    const fallback = host.querySelector(".spa-loading-inline-fallback");
    const body = host.querySelector(".spa-loading-inline-body");
    if (fallback) fallback.hidden = true;
    if (body) body.hidden = false;
    session.uiShown = true;
    if (typeof boot.refreshLoadingProgressUi === "function") {
      boot.refreshLoadingProgressUi();
    }
  }

  function scheduleDrilldownProgressShow(root) {
    if (!(root instanceof HTMLElement)) return;
    clearDrilldownProgressTimer(root);
    const session =
      typeof boot.getActiveLoadSession === "function" ? boot.getActiveLoadSession() : null;
    const wallStartedAt = session?.wallStartedAt || Date.now();
    const delay = Math.max(0, LOADING_SHOW_DELAY_MS - (Date.now() - wallStartedAt));
    const timer = setTimeout(() => {
      drilldownProgressTimers.delete(root);
      if (!root.isConnected) return;
      revealDrilldownProgress(root);
    }, delay);
    drilldownProgressTimers.set(root, timer);
  }

  function showLoadingNow() {
    clearLoadingTimer();
    createLoadingOverlay();
    const overlay = document.getElementById("mei-spa-loading");
    if (!overlay) return;
    overlay.classList.add("is-visible");
    loadingVisibleAt = Date.now();
    const session =
      typeof boot.getActiveLoadSession === "function" ? boot.getActiveLoadSession() : null;
    if (session) session.uiShown = true;
    if (typeof boot.refreshLoadingProgressUi === "function") {
      boot.refreshLoadingProgressUi();
    }
  }

  function finishLoadingHide() {
    clearLoadingTimer();
    if (!isSpaLoadingVisible()) {
      forceHideLoading();
      return;
    }
    hideLoading();
  }

  async function finishNavigationUi(navigationId) {
    if (navigationId !== currentNavigationId && spaNavigationInFlight > 0) {
      return;
    }
    if (typeof boot.waitForLoadingProgressReady === "function") {
      await boot.waitForLoadingProgressReady(navigationId);
    }
    if (navigationId !== currentNavigationId && spaNavigationInFlight > 0) {
      return;
    }
    const session =
      typeof boot.getLoadSession === "function" ? boot.getLoadSession(navigationId) : null;
    if (session && !session.finalized && typeof boot.finalizeLoadSession === "function") {
      boot.finalizeLoadSession(session, { uiShown: Boolean(session.uiShown) });
    }
    finishLoadingHide();
    clearManageWorkspaceLoadingState();
    if (typeof boot.clearLoadingProgressSession === "function") {
      boot.clearLoadingProgressSession(navigationId);
    }
    if (typeof boot.refreshVisitHistoryPanel === "function") {
      boot.refreshVisitHistoryPanel();
    }
  }
  boot.scheduleDrilldownProgressShow = scheduleDrilldownProgressShow;
  boot.clearDrilldownProgressTimer = clearDrilldownProgressTimer;
  boot.isDrilldownProgressVisible = isDrilldownProgressVisible;
  boot.clearManageWorkspaceLoadingState = clearManageWorkspaceLoadingState;
