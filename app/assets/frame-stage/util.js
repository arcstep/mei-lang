  function round(value) {
    return Math.round(value * 1000) / 1000;
  }

  function computeScale(mode, hostWidth, hostHeight, designWidth, designHeight) {
    if (hostWidth <= 0 || hostHeight <= 0 || designWidth <= 0 || designHeight <= 0) {
      return 1;
    }
    const sx = hostWidth / designWidth;
    const sy = hostHeight / designHeight;
    const fit = Math.min(sx, sy);
    if (mode === "cover") return Math.max(sx, sy);
    return fit;
  }

  function overflowModeIsDebug(mode) {
    const value = String(mode || "").trim().toLowerCase();
    return value === "debug" || value === "scroll" || value === "visible";
  }

  /** 仅 frame.props.viewport 显式配置；profile 默认（page-flow）不提供缩放工具栏。 */
  function viewportToolbarEnabled(root) {
    return String(root?.dataset?.viewportExplicit || "").toLowerCase() === "true";
  }

  /** 管理端固定调试视口；访问端固定裁切。以 data-route-mode 为准。 */
  function isManagePreviewRoute(root) {
    const route = String(root?.dataset?.routeMode || "").trim().toLowerCase();
    if (route === "manage") return true;
    if (route === "access") return false;
    return overflowModeIsDebug(String(root?.dataset?.overflowMode || "clip"));
  }

  function showDesignBoundsEnabled(root) {
    const raw = String(root.dataset.showDesignBounds || "").trim().toLowerCase();
    return raw !== "false" && raw !== "0";
  }

  function isChromeNoneAccess() {
    return document.body.classList.contains("chrome-none");
  }

  function readSafeInsets(root, overflowMode) {
    const inDebug = overflowModeIsDebug(overflowMode);
    return {
      top: Number((inDebug ? root.dataset.editSafeTop : root.dataset.safeTop) || 0),
      right: Number((inDebug ? root.dataset.editSafeRight : root.dataset.safeRight) || 0),
      bottom: Number((inDebug ? root.dataset.editSafeBottom : root.dataset.safeBottom) || 0),
      left: Number((inDebug ? root.dataset.editSafeLeft : root.dataset.safeLeft) || 0),
    };
  }

  /** 宿主 = 包裹 viewport 的可用区域；chrome=none 时退化为浏览器窗口。 */
  function resolveHostSize(root, safe) {
    if (isChromeNoneAccess()) {
      const vv = window.visualViewport;
      const width = vv?.width ?? window.innerWidth;
      const height = vv?.height ?? window.innerHeight;
      return {
        hostWidth: Math.max(1, width - safe.left - safe.right),
        hostHeight: Math.max(1, height - safe.top - safe.bottom),
      };
    }
    const rect = root.getBoundingClientRect();
    if (rect.width >= 1 && rect.height >= 1) {
      return {
        hostWidth: Math.max(1, rect.width - safe.left - safe.right),
        hostHeight: Math.max(1, rect.height - safe.top - safe.bottom),
      };
    }
    return {
      hostWidth: Math.max(1, window.innerWidth - safe.left - safe.right),
      hostHeight: Math.max(1, window.innerHeight - safe.top - safe.bottom),
    };
  }

  /**
   * 管理端：用 preview-pane-scroll 的可见区域作宿主，避免 viewport 随画布撑高导致 fit 缩放反馈振荡。
   */
  function resolveManageHostSize(root, safe) {
    const scrollPane = root.closest(".preview-pane-scroll");
    if (scrollPane && scrollPane.clientWidth >= 1 && scrollPane.clientHeight >= 1) {
      const toolbar = root.querySelector(":scope > .preview-viewport-toolbar");
      const toolbarHeight = toolbar?.offsetHeight || 0;
      return {
        hostWidth: Math.max(1, scrollPane.clientWidth - safe.left - safe.right),
        hostHeight: Math.max(
          1,
          scrollPane.clientHeight - safe.top - safe.bottom - toolbarHeight,
        ),
      };
    }
    const host = root.parentElement;
    if (host && host.clientWidth >= 1 && host.clientHeight >= 1) {
      return {
        hostWidth: Math.max(1, host.clientWidth - safe.left - safe.right),
        hostHeight: Math.max(1, host.clientHeight - safe.top - safe.bottom),
      };
    }
    return resolveHostSize(root, safe);
  }

  const viewportUpdateQueued = new WeakMap();
  const viewportLayoutApplying = new WeakMap();

  function manageLayoutKey(
    contentWidth,
    contentHeight,
    appliedZoom,
    hostWidth,
    hostHeight,
    canvasWidth,
  ) {
    return [
      Math.round(contentWidth),
      Math.round(contentHeight),
      round(appliedZoom),
      Math.round(hostWidth),
      Math.round(hostHeight),
      Math.round(canvasWidth),
    ].join(":");
  }


