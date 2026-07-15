/**
 * 访问态浮动助手：由 `agent-panel.js` 在装配 `RT`（路由与 storage key）后调用。
 * `window.__meiAgentPanelInstallAccessFloat(api)` 注入，返回一组稳定引用供事件绑定/卸载。
 */
(function (w) {
  w.__meiAgentPanelInstallAccessFloat = function (api) {
    const {
      root,
      els,
      state,
      normalizeRouteMode,
      accessFloatingStorageKey,
      accessFloatingPositionStorageKey,
    } = api;
    const ACCESS_FLOATING_MARGIN_PX = 10;
    const ACCESS_FLOATING_DRAG_THRESHOLD_PX = 4;
    let accessFloatingDragState = null;

    function canUseAccessFab() {
      return !!(els.accessFloatingRoot && els.accessFab);
    }

    function isAccessFloatingMode() {
      const mode = normalizeRouteMode(root.dataset.mode);
      const accessLike = mode === "access" || mode === "copilot";
      return (
        accessLike &&
        !!els.accessFloatingRoot &&
        !!els.accessFab &&
        !!els.accessPanel
      );
    }

    function floatingBoundsHost() {
      const bootApi = window.__meiLangBoot || {};
      if (typeof bootApi.copilotFloatingOffsetParent === "function") {
        const host = bootApi.copilotFloatingOffsetParent(els.accessFloatingRoot);
        if (host) return host;
      }
      return null;
    }

    function floatingBoundsSize() {
      const bootApi = window.__meiLangBoot || {};
      if (typeof bootApi.resolveViewportOverlayBounds === "function") {
        const bounds = bootApi.resolveViewportOverlayBounds(els.accessFloatingRoot);
        if (bounds?.shell) {
          const width = bounds.shell.clientWidth || bounds.shell.offsetWidth || 0;
          const height = bounds.shell.clientHeight || bounds.shell.offsetHeight || 0;
          if (width > 0 && height > 0) {
            return { width, height };
          }
        }
      }
      if (typeof bootApi.copilotFloatingBoundsSize === "function") {
        const size = bootApi.copilotFloatingBoundsSize();
        if (size && size.width > 0 && size.height > 0) {
          return size;
        }
      }
      return {
        width: Number(window.innerWidth || 0),
        height: Number(window.innerHeight || 0),
      };
    }

    function isViewportFab() {
      return !!els.accessFloatingRoot?.classList.contains("mei-copilot-in-viewport");
    }

    function viewportFabBoot() {
      return window.__meiLangBoot || {};
    }

    function clampAccessFloatingPosition(left, top) {
      const inViewport = isViewportFab();
      if (!isAccessFloatingMode() && !inViewport) return null;
      if (inViewport) {
        const bootApi = viewportFabBoot();
        const toDesign =
          typeof bootApi.shellToViewportFabDesign === "function"
            ? bootApi.shellToViewportFabDesign(left, top)
            : { left, top };
        const clampDesign =
          typeof bootApi.clampViewportFabDesignPosition === "function"
            ? bootApi.clampViewportFabDesignPosition(toDesign.left, toDesign.top)
            : toDesign;
        const toShell =
          typeof bootApi.designToViewportFabShell === "function"
            ? bootApi.designToViewportFabShell(clampDesign.left, clampDesign.top)
            : clampDesign;
        return { left: toShell.left, top: toShell.top };
      }
      const width = Math.max(48, Number(els.accessFloatingRoot.offsetWidth || 68));
      const height = Math.max(48, Number(els.accessFloatingRoot.offsetHeight || 68));
      const bounds = floatingBoundsSize();
      const minLeft = ACCESS_FLOATING_MARGIN_PX;
      const minTop = ACCESS_FLOATING_MARGIN_PX;
      const maxLeft = Math.max(
        minLeft,
        Number(bounds.width || 0) - width - ACCESS_FLOATING_MARGIN_PX,
      );
      const maxTop = Math.max(
        minTop,
        Number(bounds.height || 0) - height - ACCESS_FLOATING_MARGIN_PX,
      );
      const nextLeft = Math.min(maxLeft, Math.max(minLeft, Math.round(Number(left) || 0)));
      const nextTop = Math.min(maxTop, Math.max(minTop, Math.round(Number(top) || 0)));
      return { left: nextLeft, top: nextTop };
    }

    function applyAccessFloatingPosition(left, top) {
      const inViewport = isViewportFab();
      if (!isAccessFloatingMode() && !inViewport) return null;
      if (inViewport) {
        const bootApi = viewportFabBoot();
        const toDesign =
          typeof bootApi.shellToViewportFabDesign === "function"
            ? bootApi.shellToViewportFabDesign(left, top)
            : { left, top };
        if (typeof bootApi.applyViewportFabDesignPosition === "function") {
          return bootApi.applyViewportFabDesignPosition(
            els.accessFloatingRoot,
            toDesign.left,
            toDesign.top,
          );
        }
      }
      const pos = clampAccessFloatingPosition(left, top);
      if (!pos) return null;
      els.accessFloatingRoot.style.left = String(pos.left) + "px";
      els.accessFloatingRoot.style.top = String(pos.top) + "px";
      els.accessFloatingRoot.style.right = "auto";
      els.accessFloatingRoot.style.bottom = "auto";
      els.accessFloatingRoot.dataset.positioned = "true";
      return pos;
    }

    function clearAccessFloatingPosition() {
      const inViewport = isViewportFab();
      if (!isAccessFloatingMode() && !inViewport) return;
      els.accessFloatingRoot.style.left = "";
      els.accessFloatingRoot.style.top = "";
      els.accessFloatingRoot.style.right = "";
      els.accessFloatingRoot.style.bottom = "";
      delete els.accessFloatingRoot.dataset.positioned;
      delete els.accessFloatingRoot.dataset.letterboxLeft;
      delete els.accessFloatingRoot.dataset.letterboxTop;
      delete els.accessFloatingRoot.dataset.fabDesignLeft;
      delete els.accessFloatingRoot.dataset.fabDesignTop;
      const bootApi = viewportFabBoot();
      if (inViewport && typeof bootApi.relocateAccessFabInLetterbox === "function") {
        bootApi.relocateAccessFabInLetterbox();
      }
    }

    function rememberAccessFloatingPosition(left, top) {
      if (!isAccessFloatingMode() && !isViewportFab()) return;
      if (isViewportFab()) {
        const designLeft = Number(els.accessFloatingRoot.dataset.fabDesignLeft);
        const designTop = Number(els.accessFloatingRoot.dataset.fabDesignTop);
        if (!Number.isFinite(designLeft) || !Number.isFinite(designTop)) return;
        try {
          localStorage.setItem(
            accessFloatingPositionStorageKey(),
            JSON.stringify({
              viewportDesign: true,
              designLeft,
              designTop,
            }),
          );
        } catch (_) {}
        return;
      }
      const pos = clampAccessFloatingPosition(left, top);
      if (!pos) return;
      try {
        localStorage.setItem(accessFloatingPositionStorageKey(), JSON.stringify(pos));
      } catch (_) {}
    }

    function restoreAccessFloatingPosition() {
      if (!isAccessFloatingMode() && !isViewportFab()) return;
      try {
        const raw = localStorage.getItem(accessFloatingPositionStorageKey());
        if (!raw) {
          clearAccessFloatingPosition();
          return;
        }
        const parsed = JSON.parse(raw);
        if (isViewportFab()) {
          const bootApi = viewportFabBoot();
          if (parsed?.viewportDesign === true) {
            const designLeft = Number(parsed.designLeft);
            const designTop = Number(parsed.designTop);
            if (!Number.isFinite(designLeft) || !Number.isFinite(designTop)) {
              clearAccessFloatingPosition();
              return;
            }
            if (typeof bootApi.applyViewportFabDesignPosition === "function") {
              bootApi.applyViewportFabDesignPosition(
                els.accessFloatingRoot,
                designLeft,
                designTop,
              );
            }
            return;
          }
          const legacyLeft = Number(parsed?.left);
          const legacyTop = Number(parsed?.top);
          if (Number.isFinite(legacyLeft) && Number.isFinite(legacyTop)) {
            const toDesign =
              typeof bootApi.shellToViewportFabDesign === "function"
                ? bootApi.shellToViewportFabDesign(legacyLeft, legacyTop)
                : { left: legacyLeft, top: legacyTop };
            const canvas =
              typeof bootApi.clampViewportFabDesignPosition === "function"
                ? bootApi.clampViewportFabDesignPosition(toDesign.left, toDesign.top)
                : toDesign;
            if (
              Math.abs(canvas.left - toDesign.left) > 2 ||
              Math.abs(canvas.top - toDesign.top) > 2
            ) {
              clearAccessFloatingPosition();
              return;
            }
            if (typeof bootApi.applyViewportFabDesignPosition === "function") {
              bootApi.applyViewportFabDesignPosition(
                els.accessFloatingRoot,
                toDesign.left,
                toDesign.top,
              );
              rememberAccessFloatingPosition();
            }
            return;
          }
          clearAccessFloatingPosition();
          return;
        }
        let left = Number(parsed && parsed.left);
        let top = Number(parsed && parsed.top);
        if (!Number.isFinite(left) || !Number.isFinite(top)) {
          clearAccessFloatingPosition();
          return;
        }
        const pos = applyAccessFloatingPosition(left, top);
        if (pos) rememberAccessFloatingPosition(pos.left, pos.top);
      } catch (_) {
        clearAccessFloatingPosition();
      }
    }

    function renderAccessFloatingPanel() {
      if (!isAccessFloatingMode()) return;
      const open = !!state.accessFloatingOpen;
      els.accessFloatingRoot.dataset.open = open ? "true" : "false";
      els.accessPanel.hidden = !open;
      els.accessFab.title = open ? "关闭助手对话框" : "打开助手对话框";
      els.accessFab.setAttribute("aria-label", open ? "关闭助手对话框" : "打开助手对话框");
      const bootApi = window.__meiLangBoot || {};
      if (typeof bootApi.relocateAccessChatOverlayInViewport === "function") {
        bootApi.relocateAccessChatOverlayInViewport();
      }
    }

    function rememberAccessFloatingPanel() {
      if (!isAccessFloatingMode()) return;
      try {
        localStorage.setItem(accessFloatingStorageKey(), state.accessFloatingOpen ? "1" : "0");
      } catch (_) {}
    }

    function reclampAccessFloatingInViewport() {
      const inViewport = isViewportFab();
      if (!isAccessFloatingMode() && !inViewport) return;
      const bootApi = viewportFabBoot();
      if (inViewport) {
        if (els.accessFloatingRoot.dataset.positioned === "true") {
          if (typeof bootApi.resyncViewportFabDesignPosition === "function") {
            bootApi.resyncViewportFabDesignPosition(els.accessFloatingRoot);
          }
        }
        if (typeof bootApi.relocateAccessFabInLetterbox === "function") {
          bootApi.relocateAccessFabInLetterbox();
        }
        return;
      }
      if (els.accessFloatingRoot.dataset.positioned === "true") {
        const left = Number(els.accessFloatingRoot.style.left);
        const top = Number(els.accessFloatingRoot.style.top);
        if (Number.isFinite(left) && Number.isFinite(top)) {
          const pos = clampAccessFloatingPosition(left, top);
          if (pos) {
            applyAccessFloatingPosition(pos.left, pos.top);
          } else {
            clearAccessFloatingPosition();
          }
        }
      } else {
        els.accessFloatingRoot.style.left = "";
        els.accessFloatingRoot.style.top = "";
      }
    }

    function syncAccessFloatingViewportMount() {
      if (!isAccessFloatingMode()) return;
      const bootApi = window.__meiLangBoot || {};
      if (typeof bootApi.relocateStageOverlaysInViewport === "function") {
        bootApi.relocateStageOverlaysInViewport();
      } else if (typeof bootApi.relocateCopilotInViewport === "function") {
        bootApi.relocateCopilotInViewport();
      }
      reclampAccessFloatingInViewport();
      const layout = bootApi.copilotFabLayout;
      if (layout && typeof layout.scheduleCopilotFabToolbarLayout === "function") {
        layout.scheduleCopilotFabToolbarLayout();
      }
    }

    function restoreAccessFloatingPanel() {
      if (!isAccessFloatingMode()) return;
      const bootApi = window.__meiLangBoot || {};
      if (typeof bootApi.relocateStageOverlaysInViewport === "function") {
        bootApi.relocateStageOverlaysInViewport();
      } else if (typeof bootApi.relocateCopilotInViewport === "function") {
        bootApi.relocateCopilotInViewport();
      }
      restoreAccessFloatingPosition();
      try {
        const saved = localStorage.getItem(accessFloatingStorageKey());
        state.accessFloatingOpen = saved === "1";
      } catch (_) {
        state.accessFloatingOpen = false;
      }
      renderAccessFloatingPanel();
      reclampAccessFloatingInViewport();
    }

    function toggleAccessFloatingPanel(next) {
      if (!isAccessFloatingMode()) return;
      if (typeof next === "boolean") {
        state.accessFloatingOpen = next;
      } else {
        state.accessFloatingOpen = !state.accessFloatingOpen;
      }
      rememberAccessFloatingPanel();
      renderAccessFloatingPanel();
      if (state.accessFloatingOpen && els.input) {
        window.setTimeout(function () {
          try {
            els.input.focus();
          } catch (_) {}
        }, 0);
      }
    }

    function shouldUseCopilotToolbar() {
      const bootApi = window.__meiLangBoot || {};
      const ctx = bootApi.copilotFabContext;
      if (ctx && typeof ctx.copilotFabContextActive === "function") {
        return ctx.copilotFabContextActive();
      }
      return false;
    }

    let lastFabTapAt = 0;

    function activateAccessFabTap() {
      if (!canUseAccessFab()) return;
      const now = Date.now();
      if (now - lastFabTapAt < 280) return;
      lastFabTapAt = now;
      const bootApi = window.__meiLangBoot || {};
      if (shouldUseCopilotToolbar()) {
        const toolbar = bootApi.copilotToolbar;
        if (toolbar && typeof toolbar.mount === "function" && !toolbar.uiState?.mounted) {
          toolbar.mount({ autoStart: false, apply: false, toolbarOpen: false });
        }
        if (toolbar && typeof toolbar.toggleToolbar === "function") {
          toolbar.toggleToolbar();
          return;
        }
      }
      if (isAccessFloatingMode()) {
        toggleAccessFloatingPanel();
      }
    }

    function beginAccessFloatingDrag(event) {
      if (!canUseAccessFab()) return;
      if (event && event.button != null && event.button !== 0) return;
      const host = floatingBoundsHost();
      const hostRect = host
        ? host.getBoundingClientRect()
        : { left: 0, top: 0 };
      const rect = els.accessFloatingRoot.getBoundingClientRect();
      const baseLeft = Number(rect.left || 0) - Number(hostRect.left || 0);
      const baseTop = Number(rect.top || 0) - Number(hostRect.top || 0);
      accessFloatingDragState = {
        pointerId: event ? event.pointerId : null,
        startX: Number(event && event.clientX),
        startY: Number(event && event.clientY),
        baseLeft,
        baseTop,
        moved: false,
        lastLeft: baseLeft,
        lastTop: baseTop,
      };
      state.accessFloatingDragMoved = false;
      els.accessFloatingRoot.dataset.dragging = "true";
    }

    function continueAccessFloatingDrag(event) {
      if (!accessFloatingDragState || !canUseAccessFab()) return;
      if (
        accessFloatingDragState.pointerId != null &&
        event &&
        event.pointerId != null &&
        event.pointerId !== accessFloatingDragState.pointerId
      ) {
        return;
      }
      const nextX = Number(event && event.clientX);
      const nextY = Number(event && event.clientY);
      if (!Number.isFinite(nextX) || !Number.isFinite(nextY)) return;
      const dx = nextX - accessFloatingDragState.startX;
      const dy = nextY - accessFloatingDragState.startY;
      if (
        !accessFloatingDragState.moved &&
        Math.hypot(dx, dy) < ACCESS_FLOATING_DRAG_THRESHOLD_PX
      ) {
        return;
      }
      if (!accessFloatingDragState.moved) {
        accessFloatingDragState.moved = true;
        state.accessFloatingDragMoved = true;
        try {
          if (els.accessFab && event && event.pointerId != null) {
            els.accessFab.setPointerCapture(event.pointerId);
          }
        } catch (_) {}
      }
      const pos = applyAccessFloatingPosition(
        accessFloatingDragState.baseLeft + dx,
        accessFloatingDragState.baseTop + dy,
      );
      if (!pos) return;
      accessFloatingDragState.lastLeft = pos.left;
      accessFloatingDragState.lastTop = pos.top;
      if (event && typeof event.preventDefault === "function") {
        event.preventDefault();
      }
    }

    function endAccessFloatingDrag(event) {
      if (!accessFloatingDragState) return;
      if (
        accessFloatingDragState.pointerId != null &&
        event &&
        event.pointerId != null &&
        event.pointerId !== accessFloatingDragState.pointerId
      ) {
        return;
      }
      const moved = !!accessFloatingDragState.moved;
      const left = accessFloatingDragState.lastLeft;
      const top = accessFloatingDragState.lastTop;
      accessFloatingDragState = null;
      if (els.accessFloatingRoot) {
        delete els.accessFloatingRoot.dataset.dragging;
      }
      try {
        if (els.accessFab && event && event.pointerId != null) {
          els.accessFab.releasePointerCapture(event.pointerId);
        }
      } catch (_) {}
      if (moved) {
        rememberAccessFloatingPosition(left, top);
        window.setTimeout(function () {
          state.accessFloatingDragMoved = false;
        }, 0);
        const layout = (window.__meiLangBoot || {}).copilotFabLayout;
        if (layout && typeof layout.scheduleCopilotFabToolbarLayout === "function") {
          layout.scheduleCopilotFabToolbarLayout();
        } else {
          const toolbar = (window.__meiLangBoot || {}).copilotToolbar;
          if (toolbar && typeof toolbar.syncLayout === "function") {
            toolbar.syncLayout({ toolbarOpenChanged: true });
          }
        }
        return;
      }
      activateAccessFabTap();
    }

    return {
      isAccessFloatingMode,
      canUseAccessFab,
      activateAccessFabTap,
      clampAccessFloatingPosition,
      applyAccessFloatingPosition,
      clearAccessFloatingPosition,
      rememberAccessFloatingPosition,
      restoreAccessFloatingPosition,
      renderAccessFloatingPanel,
      rememberAccessFloatingPanel,
      restoreAccessFloatingPanel,
      reclampAccessFloatingInViewport,
      syncAccessFloatingViewportMount,
      toggleAccessFloatingPanel,
      beginAccessFloatingDrag,
      continueAccessFloatingDrag,
      endAccessFloatingDrag,
    };
  };

  function onAccessFloatingWindowResize() {
    const bootApi = window.__meiLangBoot || {};
    if (typeof bootApi.relocateStageOverlaysInViewport === "function") {
      bootApi.relocateStageOverlaysInViewport();
      return;
    }
    if (typeof bootApi.reclampAccessFloatingInViewport === "function") {
      bootApi.reclampAccessFloatingInViewport();
    }
    const layout = bootApi.copilotFabLayout;
    if (layout && typeof layout.scheduleCopilotFabToolbarLayout === "function") {
      layout.scheduleCopilotFabToolbarLayout();
    }
  }
  window.addEventListener("resize", onAccessFloatingWindowResize, { passive: true });
  if (window.visualViewport) {
    window.visualViewport.addEventListener("resize", onAccessFloatingWindowResize);
  }
  window.addEventListener("meilang:viewport-stage-ready", () => {
    const bootApi = window.__meiLangBoot || {};
    if (typeof bootApi.syncAccessFloatingViewportMount === "function") {
      bootApi.syncAccessFloatingViewportMount();
    }
  });
  window.addEventListener("meilang:viewport-stage-layout", () => {
    const bootApi = window.__meiLangBoot || {};
    if (typeof bootApi.reclampAccessFloatingInViewport === "function") {
      bootApi.reclampAccessFloatingInViewport();
    }
    const layout = bootApi.copilotFabLayout;
    if (layout && typeof layout.scheduleCopilotFabToolbarLayout === "function") {
      layout.scheduleCopilotFabToolbarLayout();
    }
  });
})(window);
