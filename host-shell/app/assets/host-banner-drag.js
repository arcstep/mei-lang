/**
 * 宿主提示横幅拖拽：为心跳 / HTTP 错误气泡提供拖动手柄。
 */
(() => {
  const MARGIN_PX = 8;
  const DRAG_THRESHOLD_PX = 4;

  function clampPosition(left, top, width, height) {
    const viewportWidth = Number(window.innerWidth || 0);
    const viewportHeight = Number(window.innerHeight || 0);
    const minLeft = MARGIN_PX;
    const minTop = MARGIN_PX;
    const maxLeft = Math.max(minLeft, viewportWidth - width - MARGIN_PX);
    const maxTop = Math.max(minTop, viewportHeight - height - MARGIN_PX);
    return {
      left: Math.min(maxLeft, Math.max(minLeft, Math.round(Number(left) || 0))),
      top: Math.min(maxTop, Math.max(minTop, Math.round(Number(top) || 0))),
    };
  }

  function readStoredPosition(storageKey) {
    if (!storageKey) return null;
    try {
      const raw = sessionStorage.getItem(storageKey);
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      const left = Number(parsed?.left);
      const top = Number(parsed?.top);
      if (!Number.isFinite(left) || !Number.isFinite(top)) return null;
      return { left, top };
    } catch (_) {
      return null;
    }
  }

  function storePosition(storageKey, left, top) {
    if (!storageKey) return;
    try {
      sessionStorage.setItem(storageKey, JSON.stringify({ left, top }));
    } catch (_) {}
  }

  function resolveZIndex(banner, options) {
    if (options?.zIndex != null && options.zIndex !== "") {
      return String(options.zIndex);
    }
    const root = banner.closest("[id^='mei-host-']");
    if (root) {
      const z = getComputedStyle(root).zIndex;
      if (z && z !== "auto") return z;
    }
    return "5800";
  }

  function applyFloatingPosition(banner, left, top, options) {
    const rect = banner.getBoundingClientRect();
    const width = Math.max(1, rect.width || banner.offsetWidth || 0);
    const height = Math.max(1, rect.height || banner.offsetHeight || 0);
    const pos = clampPosition(left, top, width, height);
    banner.classList.add("mei-host-banner--floating");
    banner.style.position = "fixed";
    banner.style.left = pos.left + "px";
    banner.style.top = pos.top + "px";
    banner.style.right = "auto";
    banner.style.bottom = "auto";
    banner.style.margin = "0";
    banner.style.zIndex = resolveZIndex(banner, options);
    if (!banner.style.width) {
      banner.style.width = width + "px";
    }
    return pos;
  }

  function attachHostBannerDrag(banner, options) {
    if (!banner || banner.dataset.meiDragAttached === "true") return banner;
    banner.dataset.meiDragAttached = "true";
    banner.classList.add("mei-host-banner--draggable");

    const storageKey = String(options?.storageKey || "");
    const handle = document.createElement("button");
    handle.type = "button";
    handle.className = "mei-host-banner__drag";
    handle.setAttribute("aria-label", "拖动提示");
    handle.title = "拖动";
    handle.innerHTML = '<span class="mei-host-banner__drag-grip" aria-hidden="true"></span>';
    banner.appendChild(handle);

    const stored = readStoredPosition(storageKey);
    if (stored) {
      applyFloatingPosition(banner, stored.left, stored.top, options);
    }

    let dragState = null;

    function onPointerDown(event) {
      if (event.button != null && event.button !== 0) return;
      const rect = banner.getBoundingClientRect();
      dragState = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        baseLeft: rect.left,
        baseTop: rect.top,
        moved: false,
      };
      banner.dataset.dragging = "true";
      try {
        handle.setPointerCapture(event.pointerId);
      } catch (_) {}
      event.preventDefault();
    }

    function onPointerMove(event) {
      if (!dragState || event.pointerId !== dragState.pointerId) return;
      const dx = event.clientX - dragState.startX;
      const dy = event.clientY - dragState.startY;
      if (!dragState.moved && Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) return;
      dragState.moved = true;
      applyFloatingPosition(
        banner,
        dragState.baseLeft + dx,
        dragState.baseTop + dy,
        options,
      );
      event.preventDefault();
    }

    function finishDrag(event) {
      if (!dragState || event.pointerId !== dragState.pointerId) return;
      if (dragState.moved) {
        const rect = banner.getBoundingClientRect();
        storePosition(storageKey, rect.left, rect.top);
      }
      dragState = null;
      delete banner.dataset.dragging;
      try {
        handle.releasePointerCapture(event.pointerId);
      } catch (_) {}
    }

    handle.addEventListener("pointerdown", onPointerDown);
    handle.addEventListener("pointermove", onPointerMove);
    handle.addEventListener("pointerup", finishDrag);
    handle.addEventListener("pointercancel", finishDrag);

    return banner;
  }

  window.MeiHostBannerDrag = { attach: attachHostBannerDrag };
})();
