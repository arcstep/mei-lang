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
      '<div class="access-drilldown-overlay-status" data-drilldown-status="loading">正在加载明细表...</div>' +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="error" hidden>明细表加载失败，请稍后重试。</div>' +
      '<div class="access-drilldown-table-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-drilldown-table-host" data-drilldown-table-host="true"></div>' +
      "</div>" +
      "</div>" +
      '<div class="access-drilldown-overlay-body access-drilldown-overlay-body--structured" data-drilldown-body-mode="structured" hidden>' +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="loading">正在加载看板...</div>' +
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
      '<div class="access-scene-board-overlay-status access-drilldown-overlay-status" data-drilldown-status="loading">正在加载看板...</div>' +
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

  function setDrilldownOverlayStatus(root, status) {
    root
      .querySelectorAll("[data-drilldown-status]")
      .forEach((node) => node.toggleAttribute("hidden", node.dataset.drilldownStatus !== status));
  }

  function closeDrilldownOverlay() {
    const root = document.getElementById(DRILLDOWN_OVERLAY_ROOT_ID);
    if (!root) return;
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
    // 主屏在 overlay 期间未变，关闭时不广播 page 级 preview-updated，避免实时预警/典型案例等表格整页重查。
  }

  function closeSceneBoardOverlay() {
    const root = document.getElementById(SCENE_BOARD_OVERLAY_ROOT_ID);
    if (!root) return;
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

