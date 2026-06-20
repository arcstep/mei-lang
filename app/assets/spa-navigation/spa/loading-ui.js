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

  function navigationTargetLabel(url) {
    try {
      const parsed = new URL(url, window.location.href);
      const file = String(parsed.searchParams.get("file") || "").trim();
      if (file) return file;
      const scene = String(parsed.searchParams.get("scene") || "").trim();
      if (scene) return `scene:${scene}`;
    } catch (_) {}
    return "目标预览";
  }

  function showManageWorkspaceLoadingState(url) {
    const currentUrl = new URL(window.location.href);
    const nextUrl = new URL(url, window.location.href);
    const isSameManageRoute =
      currentUrl.pathname === nextUrl.pathname &&
      currentUrl.pathname.startsWith("/apps/manage/");
    if (!isSameManageRoute) {
      clearManageWorkspaceLoadingState();
      return;
    }
    const currentMain = currentMainPane();
    if (!currentMain) return;
    currentMain.setAttribute("aria-busy", "true");
    let overlay = currentMain.querySelector('[data-mei-manage-nav-loading="true"]');
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.setAttribute("data-mei-manage-nav-loading", "true");
      overlay.style.cssText = [
        "position:absolute",
        "inset:0",
        "z-index:40",
        "display:grid",
        "place-items:center",
        "padding:24px",
        "background:linear-gradient(180deg, rgba(8,15,30,.42), rgba(8,15,30,.70))",
        "backdrop-filter:blur(2px)",
        "pointer-events:none",
      ].join(";");
      const card = document.createElement("div");
      card.style.cssText = [
        "display:grid",
        "gap:8px",
        "min-width:220px",
        "padding:16px 18px",
        "border-radius:14px",
        "border:1px solid rgba(96,165,250,.35)",
        "background:rgba(15,23,42,.88)",
        "box-shadow:0 12px 40px rgba(2,6,23,.28)",
        "color:#e2e8f0",
        "text-align:center",
      ].join(";");
      const title = document.createElement("strong");
      title.textContent = "正在切换预览";
      title.style.cssText = "font-size:14px;font-weight:700;color:#f8fafc;";
      const detail = document.createElement("span");
      detail.setAttribute("data-mei-manage-nav-target", "true");
      detail.style.cssText =
        "font-size:12px;line-height:1.5;color:#93c5fd;font-family:ui-monospace,SFMono-Regular,monospace;";
      const barTrack = document.createElement("div");
      barTrack.style.cssText =
        "height:4px;border-radius:999px;background:rgba(148,163,184,.22);overflow:hidden;";
      const barFill = document.createElement("div");
      barFill.setAttribute("data-mei-manage-loading-bar-fill", "true");
      barFill.style.cssText =
        "height:100%;width:0;border-radius:inherit;background:linear-gradient(90deg,#38bdf8,#60a5fa);transition:width 160ms ease;";
      barTrack.appendChild(barFill);
      const progressDetail = document.createElement("div");
      progressDetail.setAttribute("data-mei-manage-loading-detail", "true");
      progressDetail.style.cssText = "display:grid;gap:4px;text-align:left;";
      const hint = document.createElement("span");
      hint.textContent = "旧画面将被替换，请稍候...";
      hint.style.cssText = "font-size:11px;line-height:1.5;color:#94a3b8;";
      card.appendChild(title);
      card.appendChild(detail);
      card.appendChild(barTrack);
      card.appendChild(progressDetail);
      card.appendChild(hint);
      overlay.appendChild(card);
      if (getComputedStyle(currentMain).position === "static") {
        currentMain.style.position = "relative";
      }
      currentMain.appendChild(overlay);
    }
    const detail = overlay.querySelector('[data-mei-manage-nav-target="true"]');
    if (detail) {
      detail.textContent = navigationTargetLabel(url);
    }
    if (typeof boot.refreshLoadingProgressUi === "function") {
      boot.refreshLoadingProgressUi();
    }
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
