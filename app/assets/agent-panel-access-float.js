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

    function isAccessFloatingMode() {
      return (
        normalizeRouteMode(root.dataset.mode) === "access" &&
        !!els.accessFloatingRoot &&
        !!els.accessFab &&
        !!els.accessPanel
      );
    }

    function clampAccessFloatingPosition(left, top) {
      if (!isAccessFloatingMode()) return null;
      const width = Math.max(48, Number(els.accessFloatingRoot.offsetWidth || 68));
      const height = Math.max(48, Number(els.accessFloatingRoot.offsetHeight || 68));
      const minLeft = ACCESS_FLOATING_MARGIN_PX;
      const minTop = ACCESS_FLOATING_MARGIN_PX;
      const maxLeft = Math.max(
        minLeft,
        Number(window.innerWidth || 0) - width - ACCESS_FLOATING_MARGIN_PX,
      );
      const maxTop = Math.max(
        minTop,
        Number(window.innerHeight || 0) - height - ACCESS_FLOATING_MARGIN_PX,
      );
      const nextLeft = Math.min(maxLeft, Math.max(minLeft, Math.round(Number(left) || 0)));
      const nextTop = Math.min(maxTop, Math.max(minTop, Math.round(Number(top) || 0)));
      return { left: nextLeft, top: nextTop };
    }

    function applyAccessFloatingPosition(left, top) {
      if (!isAccessFloatingMode()) return null;
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
      if (!isAccessFloatingMode()) return;
      els.accessFloatingRoot.style.left = "";
      els.accessFloatingRoot.style.top = "";
      els.accessFloatingRoot.style.right = "";
      els.accessFloatingRoot.style.bottom = "";
      delete els.accessFloatingRoot.dataset.positioned;
    }

    function rememberAccessFloatingPosition(left, top) {
      if (!isAccessFloatingMode()) return;
      const pos = clampAccessFloatingPosition(left, top);
      if (!pos) return;
      try {
        localStorage.setItem(accessFloatingPositionStorageKey(), JSON.stringify(pos));
      } catch (_) {}
    }

    function restoreAccessFloatingPosition() {
      if (!isAccessFloatingMode()) return;
      try {
        const raw = localStorage.getItem(accessFloatingPositionStorageKey());
        if (!raw) {
          clearAccessFloatingPosition();
          return;
        }
        const parsed = JSON.parse(raw);
        const left = Number(parsed && parsed.left);
        const top = Number(parsed && parsed.top);
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
    }

    function rememberAccessFloatingPanel() {
      if (!isAccessFloatingMode()) return;
      try {
        localStorage.setItem(accessFloatingStorageKey(), state.accessFloatingOpen ? "1" : "0");
      } catch (_) {}
    }

    function restoreAccessFloatingPanel() {
      if (!isAccessFloatingMode()) return;
      restoreAccessFloatingPosition();
      try {
        const saved = localStorage.getItem(accessFloatingStorageKey());
        state.accessFloatingOpen = saved === "1";
      } catch (_) {
        state.accessFloatingOpen = false;
      }
      renderAccessFloatingPanel();
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

    function beginAccessFloatingDrag(event) {
      if (!isAccessFloatingMode()) return;
      if (event && event.button != null && event.button !== 0) return;
      const rect = els.accessFloatingRoot.getBoundingClientRect();
      accessFloatingDragState = {
        pointerId: event ? event.pointerId : null,
        startX: Number(event && event.clientX),
        startY: Number(event && event.clientY),
        baseLeft: Number(rect.left || 0),
        baseTop: Number(rect.top || 0),
        moved: false,
        lastLeft: Number(rect.left || 0),
        lastTop: Number(rect.top || 0),
      };
      state.accessFloatingDragMoved = false;
      els.accessFloatingRoot.dataset.dragging = "true";
      try {
        if (els.accessFab && event && event.pointerId != null) {
          els.accessFab.setPointerCapture(event.pointerId);
        }
      } catch (_) {}
      if (event && typeof event.preventDefault === "function") {
        event.preventDefault();
      }
    }

    function continueAccessFloatingDrag(event) {
      if (!accessFloatingDragState || !isAccessFloatingMode()) return;
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
      accessFloatingDragState.moved = true;
      state.accessFloatingDragMoved = true;
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
      }
    }

    return {
      isAccessFloatingMode,
      clampAccessFloatingPosition,
      applyAccessFloatingPosition,
      clearAccessFloatingPosition,
      rememberAccessFloatingPosition,
      restoreAccessFloatingPosition,
      renderAccessFloatingPanel,
      rememberAccessFloatingPanel,
      restoreAccessFloatingPanel,
      toggleAccessFloatingPanel,
      beginAccessFloatingDrag,
      continueAccessFloatingDrag,
      endAccessFloatingDrag,
    };
  };
})(window);
