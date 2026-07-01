/**
 * 访问/演示态外部 AI 链接漂浮入口：可拖拽，点击打开配置 URL。
 */
(function (w) {
  function positionStorageKey(appId) {
    return "mei-lang.access-external-ai-position." + String(appId || "default");
  }

  function bootAccessExternalAiFloat() {
    const root = document.getElementById("access-external-ai-floating-root");
    const fab = document.getElementById("access-external-ai-fab");
    if (!root || !fab) return null;

    const MARGIN_PX = 10;
    const DRAG_THRESHOLD_PX = 4;
    const storageKey = positionStorageKey(root.dataset.appId || "");
    let dragState = null;
    let dragMoved = false;

    function floatingBoundsHost() {
      const bootApi = w.__meiLangBoot || {};
      if (typeof bootApi.copilotFloatingOffsetParent === "function") {
        const host = bootApi.copilotFloatingOffsetParent(root);
        if (host) return host;
      }
      return null;
    }

    function floatingBoundsSize() {
      const bootApi = w.__meiLangBoot || {};
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

    function clampPosition(left, top) {
      const width = Math.max(48, Number(root.offsetWidth || 68));
      const height = Math.max(48, Number(root.offsetHeight || 68));
      const bounds = floatingBoundsSize();
      const minLeft = MARGIN_PX;
      const minTop = MARGIN_PX;
      const maxLeft = Math.max(
        minLeft,
        Number(bounds.width || 0) - width - MARGIN_PX,
      );
      const maxTop = Math.max(
        minTop,
        Number(bounds.height || 0) - height - MARGIN_PX,
      );
      return {
        left: Math.min(maxLeft, Math.max(minLeft, Math.round(Number(left) || 0))),
        top: Math.min(maxTop, Math.max(minTop, Math.round(Number(top) || 0))),
      };
    }

    function applyPosition(left, top) {
      const pos = clampPosition(left, top);
      root.style.left = String(pos.left) + "px";
      root.style.top = String(pos.top) + "px";
      root.style.right = "auto";
      root.style.bottom = "auto";
      root.dataset.positioned = "true";
      return pos;
    }

    function clearPosition() {
      root.style.left = "";
      root.style.top = "";
      root.style.right = "";
      root.style.bottom = "";
      delete root.dataset.positioned;
    }

    function rememberPosition(left, top) {
      const pos = clampPosition(left, top);
      try {
        localStorage.setItem(storageKey, JSON.stringify(pos));
      } catch (_) {}
    }

    function restorePosition() {
      try {
        const raw = localStorage.getItem(storageKey);
        if (!raw) {
          clearPosition();
          return;
        }
        const parsed = JSON.parse(raw);
        const left = Number(parsed && parsed.left);
        const top = Number(parsed && parsed.top);
        if (!Number.isFinite(left) || !Number.isFinite(top)) {
          clearPosition();
          return;
        }
        const pos = applyPosition(left, top);
        rememberPosition(pos.left, pos.top);
      } catch (_) {
        clearPosition();
      }
    }

    function beginDrag(event) {
      if (event && event.button != null && event.button !== 0) return;
      const host = floatingBoundsHost();
      const hostRect = host
        ? host.getBoundingClientRect()
        : { left: 0, top: 0 };
      const rect = root.getBoundingClientRect();
      dragState = {
        pointerId: event ? event.pointerId : null,
        startX: Number(event && event.clientX),
        startY: Number(event && event.clientY),
        baseLeft: Number(rect.left || 0) - Number(hostRect.left || 0),
        baseTop: Number(rect.top || 0) - Number(hostRect.top || 0),
        moved: false,
        lastLeft: Number(rect.left || 0),
        lastTop: Number(rect.top || 0),
      };
      dragMoved = false;
      root.dataset.dragging = "true";
      try {
        if (event && event.pointerId != null) {
          fab.setPointerCapture(event.pointerId);
        }
      } catch (_) {}
      if (event && typeof event.preventDefault === "function") {
        event.preventDefault();
      }
    }

    function continueDrag(event) {
      if (!dragState) return;
      if (
        dragState.pointerId != null &&
        event &&
        event.pointerId != null &&
        event.pointerId !== dragState.pointerId
      ) {
        return;
      }
      const nextX = Number(event && event.clientX);
      const nextY = Number(event && event.clientY);
      if (!Number.isFinite(nextX) || !Number.isFinite(nextY)) return;
      const dx = nextX - dragState.startX;
      const dy = nextY - dragState.startY;
      if (!dragState.moved && Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) {
        return;
      }
      dragState.moved = true;
      dragMoved = true;
      const pos = applyPosition(dragState.baseLeft + dx, dragState.baseTop + dy);
      dragState.lastLeft = pos.left;
      dragState.lastTop = pos.top;
      if (event && typeof event.preventDefault === "function") {
        event.preventDefault();
      }
    }

    function endDrag(event) {
      if (!dragState) return;
      if (
        dragState.pointerId != null &&
        event &&
        event.pointerId != null &&
        event.pointerId !== dragState.pointerId
      ) {
        return;
      }
      const moved = !!dragState.moved;
      const left = dragState.lastLeft;
      const top = dragState.lastTop;
      dragState = null;
      delete root.dataset.dragging;
      try {
        if (event && event.pointerId != null) {
          fab.releasePointerCapture(event.pointerId);
        }
      } catch (_) {}
      if (moved) {
        rememberPosition(left, top);
        window.setTimeout(function () {
          dragMoved = false;
        }, 0);
      }
    }

    function onFabClick(event) {
      if (dragMoved) {
        dragMoved = false;
        event.preventDefault();
      }
    }

    function onWindowResize() {
      if (root.dataset.positioned !== "true") return;
      const host = floatingBoundsHost();
      const hostRect = host
        ? host.getBoundingClientRect()
        : { left: 0, top: 0 };
      const rect = root.getBoundingClientRect();
      const pos = applyPosition(rect.left - hostRect.left, rect.top - hostRect.top);
      rememberPosition(pos.left, pos.top);
    }

    function syncViewportMount() {
      const bootApi = w.__meiLangBoot || {};
      if (typeof bootApi.relocateStageOverlaysInViewport === "function") {
        bootApi.relocateStageOverlaysInViewport();
      } else if (typeof bootApi.relocateCopilotInViewport === "function") {
        bootApi.relocateCopilotInViewport();
      }
      if (root.dataset.positioned === "true") {
        const left = Number(root.style.left);
        const top = Number(root.style.top);
        if (Number.isFinite(left) && Number.isFinite(top)) {
          applyPosition(left, top);
        }
      } else if (root.classList.contains("mei-copilot-in-viewport")) {
        root.style.left = "";
        root.style.top = "";
        root.style.right = "";
        root.style.bottom = "";
      }
    }

    function onPreviewUpdated() {
      syncViewportMount();
    }

    fab.addEventListener("click", onFabClick);
    fab.addEventListener("pointerdown", beginDrag);
    document.addEventListener("pointermove", continueDrag);
    document.addEventListener("pointerup", endDrag);
    document.addEventListener("pointercancel", endDrag);
    window.addEventListener("resize", onWindowResize);
    window.addEventListener("meilang:preview-updated", onPreviewUpdated);
    syncViewportMount();
    restorePosition();
    syncViewportMount();

    return function dispose() {
      fab.removeEventListener("click", onFabClick);
      fab.removeEventListener("pointerdown", beginDrag);
      document.removeEventListener("pointermove", continueDrag);
      document.removeEventListener("pointerup", endDrag);
      document.removeEventListener("pointercancel", endDrag);
      window.removeEventListener("resize", onWindowResize);
      window.removeEventListener("meilang:preview-updated", onPreviewUpdated);
    };
  }

  const boot = (w.__meiLangBoot = w.__meiLangBoot || {});
  if (typeof boot.disposeAccessExternalAiFloat === "function") {
    try {
      boot.disposeAccessExternalAiFloat();
    } catch (_) {}
    boot.disposeAccessExternalAiFloat = null;
  }
  boot.disposeAccessExternalAiFloat = bootAccessExternalAiFloat();
})(window);
