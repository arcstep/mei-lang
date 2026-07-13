  function drilldownLoadingStatusHtml(fallbackText) {
    return (
      '<div class="access-drilldown-overlay-status spa-loading-inline spa-loading-inline--kind-drilldown" data-drilldown-status="loading" data-mei-drilldown-load-progress="true">' +
      '<span class="spa-loading-inline-fallback">' +
      String(fallbackText || "正在加载…") +
      "</span>" +
      '<div class="spa-loading-inline-body" hidden>' +
      '<span class="spa-loading-text">下钻加载中…</span>' +
      '<div class="spa-loading-track">' +
      '<div class="spa-loading-bar"><div class="spa-loading-bar-fill"></div></div>' +
      "</div>" +
      '<div class="spa-loading-detail"></div>' +
      "</div>" +
      "</div>"
    );
  }

  function ensureDrilldownOverlayRoot() {
    let root = document.getElementById(DRILLDOWN_OVERLAY_ROOT_ID);
    if (root) {
      return root;
    }
    root = document.createElement("div");
    root.id = DRILLDOWN_OVERLAY_ROOT_ID;
    root.className = "access-drilldown-overlay";
    root.setAttribute("hidden", "hidden");
    root.innerHTML =
      '<div class="access-drilldown-overlay-backdrop" data-drilldown-close="mask"></div>' +
      '<section class="access-drilldown-overlay-panel" role="dialog" aria-modal="true" aria-label="指标下钻明细">' +
      '<header class="access-drilldown-overlay-head">' +
      '<div class="access-drilldown-overlay-head-meta">' +
      '<div class="access-drilldown-overlay-title" data-drilldown-title="true"></div>' +
      '<div class="access-drilldown-overlay-note" data-drilldown-note="true" hidden></div>' +
      "</div>" +
      '<button type="button" class="access-drilldown-overlay-close" data-drilldown-close="button" aria-label="关闭">×</button>' +
      "</header>" +
      '<div class="access-drilldown-panel-hero" data-drilldown-hero="true" hidden>' +
      '<div class="access-drilldown-panel-hero-title" data-drilldown-hero-title="true"></div>' +
      '<div class="access-drilldown-panel-hero-note" data-drilldown-hero-note="true" hidden></div>' +
      "</div>" +
      '<div class="access-drilldown-overlay-tabs" data-drilldown-tabs="true" hidden></div>' +
      '<div class="access-drilldown-overlay-body" data-drilldown-body-mode="generic">' +
      drilldownLoadingStatusHtml("正在加载明细表...") +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="error" hidden>明细表加载失败，请稍后重试。</div>' +
      '<div class="access-drilldown-table-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-drilldown-table-host" data-drilldown-table-host="true"></div>' +
      "</div>" +
      "</div>" +
      '<div class="access-drilldown-overlay-body access-drilldown-overlay-body--structured" data-drilldown-body-mode="structured" hidden>' +
      drilldownLoadingStatusHtml("正在加载看板...") +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="error" hidden>看板加载失败，请稍后重试。</div>' +
      '<div class="access-drilldown-structured-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-drilldown-structured-layout" data-drilldown-structured-layout="true"></div>' +
      "</div>" +
      "</div>" +
      "</section>";
    root.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (!target.dataset.drilldownClose) return;
      closeDrilldownOverlay();
    });
    document.body.appendChild(root);
    return root;
  }

  function ensureSceneBoardOverlayRoot() {
    let root = document.getElementById(SCENE_BOARD_OVERLAY_ROOT_ID);
    if (root) {
      return root;
    }
    root = document.createElement("div");
    root.id = SCENE_BOARD_OVERLAY_ROOT_ID;
    root.className = "access-scene-board-overlay access-drilldown-overlay";
    root.setAttribute("hidden", "hidden");
    root.innerHTML =
      '<div class="access-scene-board-overlay-backdrop access-drilldown-overlay-backdrop" data-scene-board-close="mask"></div>' +
      '<section class="access-scene-board-overlay-panel access-drilldown-overlay-panel" role="dialog" aria-modal="true" aria-label="看板明细">' +
      '<header class="access-scene-board-overlay-head access-drilldown-overlay-head">' +
      '<div class="access-scene-board-overlay-head-meta access-drilldown-overlay-head-meta">' +
      '<div class="access-scene-board-overlay-title access-drilldown-overlay-title" data-drilldown-title="true"></div>' +
      '<div class="access-scene-board-overlay-note access-drilldown-overlay-note" data-drilldown-note="true" hidden></div>' +
      "</div>" +
      '<button type="button" class="access-scene-board-overlay-close access-drilldown-overlay-close" data-scene-board-close="button" aria-label="关闭">×</button>' +
      "</header>" +
      '<div class="access-scene-board-overlay-body access-drilldown-overlay-body--structured">' +
      drilldownLoadingStatusHtml("正在加载看板...") +
      '<div class="access-scene-board-overlay-status access-drilldown-overlay-status" data-drilldown-status="error" hidden>看板加载失败，请稍后重试。</div>' +
      '<div class="access-scene-board-structured-shell access-drilldown-structured-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-scene-board-structured-layout access-drilldown-structured-layout" data-drilldown-structured-layout="true"></div>' +
      "</div>" +
      "</div>" +
      "</section>";
    root.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (!target.dataset.sceneBoardClose) return;
      closeSceneBoardOverlay();
    });
    document.body.appendChild(root);
    return root;
  }

  function setDrilldownOverlayStatus(root, status, failure = {}) {
    if (!(root instanceof HTMLElement)) return "";
    if (status === "loading") {
      delete root.dataset.meiClientErrorTraceId;
      root.querySelectorAll('[data-drilldown-status="error"]').forEach((node) => {
        if (!(node instanceof HTMLElement)) return;
        const base = String(node.dataset.meiErrorBaseText || node.textContent || "").trim();
        node.dataset.meiErrorBaseText = base;
        node.textContent = base;
      });
    }
    let traceId = String(
      failure?.traceId || root.dataset.meiClientErrorTraceId || "",
    ).trim();
    if (status === "error" && !traceId) {
      const config =
        failure?.config && typeof failure.config === "object"
          ? failure.config
          : root.__meiDrilldownErrorConfig || {};
      traceId = String(
        recordPopupDebugIssue({
          level: "error",
          message: String(failure?.message || "二级看板进入加载失败状态"),
          phase: String(failure?.phase || "drilldown_visible_error"),
          detail: failure?.detail || {},
          config,
          datasetId: failure?.datasetId || "",
          metricId: failure?.metricId || "",
          root,
          stack: failure?.stack || "",
        }) || "",
      ).trim();
    }
    if (status === "error") {
      root.querySelectorAll('[data-drilldown-status="error"]').forEach((node) => {
        if (!(node instanceof HTMLElement)) return;
        const base = String(
          failure?.userMessage ||
            node.dataset.meiErrorBaseText ||
            node.textContent ||
            "看板加载失败，请稍后重试。",
        ).trim();
        node.dataset.meiErrorBaseText = base;
        node.textContent = traceId ? `${base}（追踪编号：${traceId}）` : base;
      });
    }
    root
      .querySelectorAll("[data-drilldown-status]")
      .forEach((node) => node.toggleAttribute("hidden", node.dataset.drilldownStatus !== status));
    if (status === "loading" && typeof boot.scheduleDrilldownProgressShow === "function") {
      boot.scheduleDrilldownProgressShow(root);
    }
    if (
      (status === "ready" || status === "error") &&
      typeof boot.completeDrilldownLoadSession === "function"
    ) {
      void boot.completeDrilldownLoadSession({
        outcome: status === "ready" ? "ready" : "error",
        root,
      });
    }
    return traceId;
  }

  function abortDrilldownLoadIfNeeded(root) {
    if (typeof boot.clearDrilldownProgressTimer === "function" && root) {
      boot.clearDrilldownProgressTimer(root);
    }
    if (typeof boot.abortDrilldownLoadSession === "function") {
      boot.abortDrilldownLoadSession();
    }
  }

  function closeDrilldownOverlay() {
    if (typeof boot.useUnifiedLayer2 === "function" && boot.useUnifiedLayer2()) {
      if (typeof boot.closeLayer2Stack === "function") {
        boot.closeLayer2Stack();
      }
      return;
    }
    const root = document.getElementById(DRILLDOWN_OVERLAY_ROOT_ID);
    if (!root) return;
    abortDrilldownLoadIfNeeded(root);
    cleanupStructuredDrilldownWatcher(root);
    root.setAttribute("hidden", "hidden");
    root.classList.remove("is-open");
    for (const selector of [
      '[data-drilldown-table-host="true"]',
      '[data-drilldown-filter-host="true"]',
      '[data-drilldown-zone-host]',
      '[data-drilldown-structured-layout="true"]',
    ]) {
      root.querySelectorAll(selector).forEach((host) => {
        if (host instanceof HTMLElement) {
          host.replaceChildren();
        }
      });
    }
    document.body.classList.remove("access-drilldown-open");
    // 主屏在 overlay 期间未变，关闭时不广播 page 级 preview-updated，避免关联表格整页重查。
  }

  function installOverlayCloseDelegation() {
    if (boot.overlayCloseDelegationInstalled) return;
    boot.overlayCloseDelegationInstalled = true;
    document.addEventListener(
      "click",
      (event) => {
        const target = event.target;
        if (!(target instanceof Element)) return;
        const sceneBoardRoot = document.getElementById(SCENE_BOARD_OVERLAY_ROOT_ID);
        if (
          sceneBoardRoot instanceof HTMLElement &&
          !sceneBoardRoot.hidden &&
          sceneBoardRoot.classList.contains("is-open") &&
          target.closest(`#${SCENE_BOARD_OVERLAY_ROOT_ID} [data-scene-board-close]`)
        ) {
          closeSceneBoardOverlay();
          return;
        }
        const drilldownRoot = document.getElementById(DRILLDOWN_OVERLAY_ROOT_ID);
        if (
          drilldownRoot instanceof HTMLElement &&
          !drilldownRoot.hidden &&
          drilldownRoot.classList.contains("is-open") &&
          target.closest(`#${DRILLDOWN_OVERLAY_ROOT_ID} [data-drilldown-close]`)
        ) {
          closeDrilldownOverlay();
        }
      },
      true,
    );
  }

  function closeSceneBoardOverlay() {
    if (typeof boot.useUnifiedLayer2 === "function" && boot.useUnifiedLayer2()) {
      if (typeof boot.closeLayer2Stack === "function") {
        boot.closeLayer2Stack();
      }
      return;
    }
    const root = document.getElementById(SCENE_BOARD_OVERLAY_ROOT_ID);
    if (!root) return;
    abortDrilldownLoadIfNeeded(root);
    cleanupStructuredDrilldownWatcher(root);
    root.setAttribute("hidden", "hidden");
    root.classList.remove("is-open");
    for (const selector of ['[data-drilldown-structured-layout="true"]']) {
      root.querySelectorAll(selector).forEach((host) => {
        if (host instanceof HTMLElement) {
          host.replaceChildren();
        }
      });
    }
    document.body.classList.remove("access-scene-board-open");
  }

