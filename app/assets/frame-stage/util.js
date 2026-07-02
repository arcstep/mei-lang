  function round(value) {
    return Math.round(value * 1000) / 1000;
  }

  function computeScale(mode, hostWidth, hostHeight, designWidth, designHeight, root) {
    if (hostWidth <= 0 || hostHeight <= 0 || designWidth <= 0 || designHeight <= 0) {
      return 1;
    }
    const sx = hostWidth / designWidth;
    const sy = hostHeight / designHeight;
    const normalized = String(mode || "contain").trim().toLowerCase();
    if (normalized === "cover") {
      return Math.max(sx, sy);
    }
    if (
      normalized === "fit-width" ||
      normalized === "fit_width" ||
      normalized === "width"
    ) {
      return sx;
    }
    // 访问态 contain：优先按宽度适配，高度不足时再缩小（纵向 letterbox）
    if (normalized === "contain" && root && !isManagePreviewRoute(root)) {
      const heightAtWidth = designHeight * sx;
      if (heightAtWidth <= hostHeight + 0.5) {
        return sx;
      }
      return Math.min(sx, sy);
    }
    return Math.min(sx, sy);
  }

  function overflowModeIsDebug(mode) {
    const value = String(mode || "").trim().toLowerCase();
    return value === "debug" || value === "scroll" || value === "visible";
  }

  /** 构建/管理端预览始终允许缩放；访问态仍依赖显式 viewport 配置。 */
  function viewportToolbarEnabled(root) {
    if (isManagePreviewRoute(root)) return true;
    return String(root?.dataset?.viewportExplicit || "").toLowerCase() === "true";
  }

  /** 管理端固定调试视口；访问/演示端固定裁切。以 data-route-mode 为准。 */
  function isManagePreviewRoute(root) {
    const route = String(root?.dataset?.routeMode || "").trim().toLowerCase();
    if (route === "manage" || route === "build") return true;
    if (
      route === "access" ||
      route === "app" ||
      route === "presentation" ||
      route === "run"
    ) {
      return false;
    }
    return overflowModeIsDebug(String(root?.dataset?.overflowMode || "clip"));
  }

  function showDesignBoundsEnabled(root) {
    const raw = String(root.dataset.showDesignBounds || "").trim().toLowerCase();
    return raw !== "false" && raw !== "0";
  }

  /** 仅访问态隐藏 chrome（app-view + chrome-none）；演示态虽无 chrome 但 preview 宿主仍走 shell 链。 */
  function isChromeNoneAccess() {
    return (
      document.body.classList.contains("app-view") &&
      document.body.classList.contains("chrome-none")
    );
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
    if (root && !isManagePreviewRoute(root)) {
      const accessHost = resolveAccessPreviewHost(root);
      if (accessHost) {
        return {
          hostWidth: Math.max(1, accessHost.width - safe.left - safe.right),
          hostHeight: Math.max(1, accessHost.height - safe.top - safe.bottom),
        };
      }
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

  /** 访问态：用 main / 滚动宿主可见区，避免窄屏断点下 viewport rect 高度塌缩。 */
  function resolveAccessPreviewHost(root) {
    const candidates = [
      root.closest(".main"),
      root.closest(".preview-pane-scroll"),
      root.parentElement,
      root,
    ];
    for (const node of candidates) {
      if (!(node instanceof HTMLElement)) continue;
      const rect = node.getBoundingClientRect();
      if (rect.width >= 1 && rect.height >= 1) {
        return { width: rect.width, height: rect.height };
      }
      if (node.clientWidth >= 1 && node.clientHeight >= 1) {
        return { width: node.clientWidth, height: node.clientHeight };
      }
    }
    const topbar = document.querySelector(".topbar");
    const statusbar = document.querySelector(".statusbar");
    const topH = topbar?.getBoundingClientRect().height || 0;
    const bottomH = statusbar?.getBoundingClientRect().height || 0;
    const vv = window.visualViewport;
    const width = vv?.width ?? window.innerWidth;
    const height = (vv?.height ?? window.innerHeight) - topH - bottomH;
    if (width >= 1 && height >= 1) {
      return { width, height };
    }
    return null;
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

  function buildRouteSlugFromPathname(pathname = window.location.pathname) {
    const match = String(pathname || "").match(/^\/apps\/([^/]+)\//);
    return match ? String(match[1] || "").trim().toLowerCase() : "";
  }

  function isBuildShellRoute(pathname = window.location.pathname) {
    const slug = buildRouteSlugFromPathname(pathname);
    return slug === "build" || slug === "manage";
  }

  function activeBuildTabSlug() {
    const shell = document.querySelector("[data-build-tab]");
    const fromShell =
      shell && String(shell.getAttribute("data-build-tab") || "").trim().toLowerCase();
    if (fromShell) return fromShell;
    try {
      return String(new URL(window.location.href).searchParams.get("tab") || "overview")
        .trim()
        .toLowerCase();
    } catch (_) {
      return "overview";
    }
  }

  /** 构建视图非 preview 标签时不应触发 dataset/metric 预取与 viewport 扫描。 */
  function shouldMountBuildPreviewRuntime() {
    if (!isBuildShellRoute()) return true;
    return activeBuildTabSlug() === "preview";
  }

  boot.shouldMountBuildPreviewRuntime = shouldMountBuildPreviewRuntime;
  boot.activeBuildTabSlug = activeBuildTabSlug;

