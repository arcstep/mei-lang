(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (typeof boot.disposeFrameStage === "function") {
    try {
      boot.disposeFrameStage();
    } catch (_) {}
    boot.disposeFrameStage = null;
  }
  const tracked = new WeakMap();
  const observers = new Set();

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

  function queueUpdateViewport(root) {
    if (viewportUpdateQueued.get(root)) return;
    viewportUpdateQueued.set(root, true);
    requestAnimationFrame(() => {
      viewportUpdateQueued.delete(root);
      updateViewport(root);
    });
  }

  function invalidateManageLayout(root) {
    delete root.dataset.meiLayoutKey;
    root.dataset.meiRelayoutPass = "0";
    const timers = manageRelayoutTimers.get(root);
    if (timers?.t500) clearTimeout(timers.t500);
    manageRelayoutTimers.delete(root);
  }

  function applyFluidWidthLayout(shell, stage, contentMaxWidth, hostWidth) {
    const stageWidth = Math.min(contentMaxWidth, hostWidth);
    shell.style.width = `${round(stageWidth)}px`;
    shell.style.height = "auto";
    shell.style.maxHeight = "none";
    shell.style.overflow = "visible";
    stage.style.width = `${round(stageWidth)}px`;
    stage.style.height = "auto";
    stage.style.maxHeight = "none";
    stage.style.transform = "none";
    removeDesignBounds(shell);
  }

  function removeViewportChrome(root, shell) {
    root?.querySelector(".preview-viewport-chrome")?.remove();
    shell?.querySelector(".preview-viewport-chrome")?.remove();
  }

  function debugLayoutContainer(shell) {
    return shell?.querySelector(".preview-stage-scale-wrap") || shell;
  }

  function ensureStageScaleWrap(shell, stage) {
    let wrap = shell.querySelector(".preview-stage-scale-wrap");
    if (!wrap) {
      wrap = document.createElement("div");
      wrap.className = "preview-stage-scale-wrap";
      if (stage.parentElement === shell) {
        shell.insertBefore(wrap, stage);
        wrap.appendChild(stage);
      } else {
        shell.appendChild(wrap);
        if (stage.parentElement !== wrap) {
          wrap.appendChild(stage);
        }
      }
    }
    return wrap;
  }

  function ensureStageScaleInner(wrap) {
    let inner = wrap.querySelector(":scope > .preview-stage-scale-inner");
    if (!inner) {
      inner = document.createElement("div");
      inner.className = "preview-stage-scale-inner";
      [...wrap.childNodes].forEach((node) => inner.appendChild(node));
      wrap.appendChild(inner);
    }
    return inner;
  }

  const MANAGE_ZOOM_STORAGE_KEY = "mei-manage-preview-zoom";
  const MANAGE_ZOOM_MIN = 0.1;
  const MANAGE_ZOOM_MAX = 2;

  function initManagePreviewZoom(root) {
    if (!root.dataset.previewZoom) {
      root.dataset.previewZoom =
        localStorage.getItem(MANAGE_ZOOM_STORAGE_KEY) || "fit";
    }
  }

  function computeManageFitScale(
    hostWidth,
    hostHeight,
    contentWidth,
    contentHeight,
  ) {
    if (hostWidth <= 0 || hostHeight <= 0 || contentWidth <= 0 || contentHeight <= 0) {
      return 1;
    }
    const sx = hostWidth / contentWidth;
    const sy = hostHeight / contentHeight;
    const fit = Math.min(sx, sy);
    return fit > 0 ? fit : 1;
  }

  function resolveManagePreviewZoom(root, fitScale) {
    initManagePreviewZoom(root);
    const raw = String(root.dataset.previewZoom || "fit").trim().toLowerCase();
    if (raw === "fit" || raw === "auto") return fitScale;
    const value = Number(raw);
    if (Number.isFinite(value) && value > 0) {
      return Math.min(MANAGE_ZOOM_MAX, Math.max(MANAGE_ZOOM_MIN, value));
    }
    return fitScale;
  }

  function setManagePreviewZoom(root, value) {
    root.dataset.previewZoom = value;
    try {
      localStorage.setItem(MANAGE_ZOOM_STORAGE_KEY, value);
    } catch (_) {}
    invalidateManageLayout(root);
    updateManageZoomToolbar(root);
    updateViewport(root);
  }

  function stepManagePreviewZoom(root, delta) {
    initManagePreviewZoom(root);
    const fitScale = Number(root.dataset.meiFitScale || 1);
    const current =
      root.dataset.previewZoom === "fit"
        ? fitScale
        : resolveManagePreviewZoom(root, fitScale);
    const next = Math.min(
      MANAGE_ZOOM_MAX,
      Math.max(MANAGE_ZOOM_MIN, round(current + delta)),
    );
    setManagePreviewZoom(root, String(next));
  }

  function ensureManageZoomToolbar(root) {
    let toolbar = root.querySelector(":scope > .preview-viewport-toolbar");
    const shell = root.querySelector(".preview-stage-shell");
    if (!toolbar) {
      toolbar = document.createElement("div");
      toolbar.className = "preview-viewport-toolbar";
      const bar = document.createElement("div");
      bar.className = "preview-viewport-zoom-bar";
      bar.dataset.previewZoomBar = "true";
      const title = document.createElement("span");
      title.className = "preview-viewport-zoom-title";
      title.textContent = "视窗";
      bar.appendChild(title);
      const presets = [
        ["fit", "自适应"],
        ["1", "100%"],
        ["0.75", "75%"],
        ["0.5", "50%"],
        ["minus", "−"],
        ["plus", "+"],
      ];
      presets.forEach(([value, label]) => {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "preview-viewport-zoom-btn";
        btn.dataset.previewZoom = value;
        btn.textContent = label;
        if (value === "minus") btn.title = "缩小";
        if (value === "plus") btn.title = "放大";
        bar.appendChild(btn);
      });
      const readout = document.createElement("span");
      readout.className = "preview-viewport-zoom-readout";
      readout.dataset.previewZoomReadout = "true";
      readout.textContent = "—";
      bar.appendChild(readout);
      toolbar.appendChild(bar);
      const chrome = document.createElement("div");
      chrome.className = "preview-viewport-chrome";
      chrome.setAttribute("aria-hidden", "true");
      toolbar.appendChild(chrome);
      root.insertBefore(toolbar, shell || root.firstChild);
    }
    const orphanChrome = root.querySelector(":scope > .preview-viewport-chrome");
    const toolbarChrome = toolbar.querySelector(".preview-viewport-chrome");
    if (orphanChrome && toolbarChrome && orphanChrome !== toolbarChrome) {
      toolbarChrome.replaceWith(orphanChrome);
    } else if (orphanChrome && !toolbarChrome) {
      toolbar.appendChild(orphanChrome);
    }
    return toolbar;
  }

  function updateManageZoomToolbar(root) {
    ensureManageZoomToolbar(root);
    const bar = root.querySelector("[data-preview-zoom-bar]");
    if (!bar) return;
    const mode = String(root.dataset.previewZoom || "fit");
    const applied = Number(root.dataset.meiAppliedZoom || 1);
    bar.querySelectorAll("[data-preview-zoom]").forEach((btn) => {
      const zoom = String(btn.dataset.previewZoom || "");
      const isStep = zoom === "minus" || zoom === "plus";
      const isActive =
        !isStep &&
        (zoom === mode ||
          (mode !== "fit" &&
            zoom !== "minus" &&
            zoom !== "plus" &&
            Math.abs(Number(zoom) - Number(mode)) < 0.001));
      btn.classList.toggle("is-active", isActive);
    });
    const readout = bar.querySelector("[data-preview-zoom-readout]");
    if (readout) {
      readout.textContent = `${Math.round(applied * 100)}%`;
    }
  }

  function unlockStageLayoutForDebug(stage, designHeight) {
    stage.style.overflow = "visible";
    stage.style.height = "auto";
    stage.style.minHeight = `${round(designHeight)}px`;
    stage.style.maxHeight = "none";
    if (stage.dataset.meiDebugLayoutUnlocked !== "true") {
      const rows = getComputedStyle(stage).gridTemplateRows;
      if (rows && rows !== "none") {
        const parts = rows.split(/\s+/).filter((row) => row && row !== "none");
        stage.style.gridTemplateRows = parts
          .map((row, index) => {
            if (index === 0) return row;
            if (/minmax|fr/i.test(row)) return "auto";
            if (Number.isFinite(Number.parseFloat(row))) return "auto";
            return row;
          })
          .join(" ");
      }
      stage.dataset.meiDebugLayoutUnlocked = "true";
    }
    stage.querySelectorAll(".preview-card, .preview-panel-body").forEach((node) => {
      node.style.overflow = "visible";
      node.style.maxHeight = "none";
    });
  }

  function resolveCanvasWidth(root, designWidth) {
    const cap = Number(root.dataset.contentMaxWidth || 0);
    const declared = Number(designWidth || root.dataset.designWidth || 0);
    if (cap > 0 && declared > 0) return Math.min(cap, declared);
    return declared;
  }

  function clearDebugOverlayNodes(container) {
    container
      ?.querySelectorAll(
        ".preview-design-bounds, .preview-design-overflow-veil, .preview-design-overflow-veil-x",
      )
      .forEach((node) => node.remove());
  }

  /** 在布局坐标（未缩放）下测量；勿用缩放后的 getBoundingClientRect（边栏变宽→zoom 变大→假横向溢出）。 */
  function measureStageContentExtent(stage, contentMaxWidth) {
    const cap = contentMaxWidth > 0 ? contentMaxWidth : 0;
    if (cap > 0 && stage.offsetWidth > 0 && stage.offsetWidth <= cap + 2) {
      const height = Math.max(stage.offsetHeight, stage.scrollHeight);
      return { width: cap, height: Math.max(1, height) };
    }
    let maxRight = 0;
    let maxBottom = 0;
    let hasContent = false;
    stage
      .querySelectorAll(".preview-card, .panel-heading, .preview-panel-body")
      .forEach((node) => {
        if (
          node.closest(
            ".preview-design-bounds, .preview-design-overflow-veil, .preview-design-overflow-veil-x",
          )
        ) {
          return;
        }
        const right = node.offsetLeft + node.offsetWidth;
        const bottom = node.offsetTop + node.offsetHeight;
        if (node.offsetWidth < 1 || node.offsetHeight < 1) return;
        hasContent = true;
        if (right > maxRight) maxRight = right;
        if (bottom > maxBottom) maxBottom = bottom;
      });
    let width = hasContent ? Math.max(1, maxRight, stage.offsetWidth) : stage.offsetWidth;
    let height = hasContent ? Math.max(1, maxBottom, stage.offsetHeight) : stage.offsetHeight;
    if (cap > 0 && width <= cap + 2) {
      width = cap;
    }
    return { width, height };
  }

  function measureStageContentSize(
    stage,
    canvasWidth,
    designHeight,
    fluidHeight,
    contentMaxWidth,
  ) {
    const extent = measureStageContentExtent(stage, contentMaxWidth);
    return {
      width: Math.max(canvasWidth, extent.width),
      extentWidth: extent.width,
      height: Math.max(fluidHeight ? extent.height : designHeight, extent.height),
      extentHeight: extent.height,
    };
  }

  function shouldShowHorizontalOverflowVeil(canvasWidth, extentWidth, contentMaxWidth) {
    const cap = contentMaxWidth > 0 ? contentMaxWidth : 0;
    if (cap > 0) {
      if (extentWidth <= cap + 2) return false;
      if (extentWidth <= canvasWidth + 2) return false;
    }
    return extentWidth > canvasWidth + 2;
  }

  function removeDesignBounds(shell) {
    const container = debugLayoutContainer(shell);
    container?.querySelector(".preview-design-bounds")?.remove();
    container?.querySelector(".preview-design-overflow-veil")?.remove();
    container?.querySelector(".preview-design-overflow-veil-x")?.remove();
    shell?.querySelector(".preview-design-bounds")?.remove();
    shell?.querySelector(".preview-design-overflow-veil")?.remove();
    shell?.querySelector(".preview-design-overflow-veil-x")?.remove();
    removeViewportChrome(shell?.closest('[data-mei-frame-viewport="true"]'), shell);
  }

  function ensureDesignBounds(shell, designWidth, designHeight, scale) {
    let bounds = shell.querySelector(".preview-design-bounds");
    if (!bounds) {
      bounds = document.createElement("div");
      bounds.className = "preview-design-bounds";
      bounds.setAttribute("aria-hidden", "true");
      shell.appendChild(bounds);
    }
    bounds.style.width = `${round(designWidth * scale)}px`;
    bounds.style.height = `${round(designHeight * scale)}px`;
    return bounds;
  }

  /** 分辨率标签在 viewport 首行（管理端 SSR + 文档流）；仅更新文案，避免 shell overflow 裁切。 */
  function ensureViewportChrome(
    root,
    designWidth,
    designHeight,
    aspectRatio,
    contentHeight,
    fluidHeight,
  ) {
    if (!root) return;
    let chrome =
      root.querySelector(".preview-viewport-toolbar .preview-viewport-chrome") ||
      root.querySelector(":scope > .preview-viewport-chrome");
    if (!chrome) {
      const shell = root.querySelector(".preview-stage-shell");
      chrome = document.createElement("div");
      chrome.className = "preview-viewport-chrome";
      chrome.setAttribute("aria-hidden", "true");
      root.insertBefore(chrome, shell || root.firstChild);
    }
    const aspectSuffix =
      aspectRatio && String(aspectRatio).trim()
        ? ` · ${String(aspectRatio).trim()}`
        : "";
    const heightLabel =
      fluidHeight && contentHeight > 0
        ? Math.round(contentHeight)
        : Math.round(designHeight);
    const heightSuffix = fluidHeight ? " (内容高)" : "";
    let chromeText = `${Math.round(designWidth)} × ${heightLabel}${heightSuffix}${aspectSuffix}`;
    const measuredWidth = Number(root.dataset.meiContentWidth || 0);
    const measuredHeight = Number(root.dataset.meiContentHeight || 0);
    if (
      measuredWidth > designWidth + 1 ||
      measuredHeight > designHeight + 1
    ) {
      chromeText += ` · 实测 ${Math.round(measuredWidth)}×${Math.round(measuredHeight)}`;
    }
    const appliedZoom = Number(root.dataset.meiAppliedZoom || 0);
    if (appliedZoom > 0) {
      chromeText += ` · 缩放 ${Math.round(appliedZoom * 100)}%`;
    }
    chrome.textContent = chromeText;
    chrome.style.position = "";
    chrome.style.top = "";
    chrome.style.left = "";
    chrome.style.bottom = "";
    chrome.style.transform = "";
  }

  function ensureOverflowVeilLabel(veil, axis, overflowPx, contentSize, designSize) {
    let label = veil.querySelector(".preview-design-overflow-veil-label");
    if (!label) {
      label = document.createElement("span");
      label.className = "preview-design-overflow-veil-label";
      veil.appendChild(label);
    }
    const axisLabel = axis === "x" ? "横向" : "纵向";
    label.textContent = `${axisLabel}溢出 +${Math.round(overflowPx)}px（内容 ${Math.round(contentSize)} / 设计 ${Math.round(designSize)}）· 访问态将裁切`;
  }

  function ensureOverflowVeil(
    shell,
    designWidth,
    designHeight,
    scale,
    contentWidth,
    contentHeight,
    extentWidth,
    contentMaxWidth,
  ) {
    let veilY = shell.querySelector(":scope > .preview-design-overflow-veil:not(.preview-design-overflow-veil-x)");
    let veilX = shell.querySelector(":scope > .preview-design-overflow-veil-x");
    if (contentHeight > designHeight + 1) {
      if (!veilY) {
        veilY = document.createElement("div");
        veilY.className = "preview-design-overflow-veil";
        veilY.setAttribute("aria-hidden", "true");
        shell.appendChild(veilY);
      }
      const top = round(designHeight * scale);
      const height = round((contentHeight - designHeight) * scale);
      veilY.style.top = `${top}px`;
      veilY.style.left = "0";
      veilY.style.width = `${round(designWidth * scale)}px`;
      veilY.style.height = `${height}px`;
      ensureOverflowVeilLabel(
        veilY,
        "y",
        contentHeight - designHeight,
        contentHeight,
        designHeight,
      );
    } else {
      veilY?.remove();
    }
    const overflowExtentWidth = extentWidth > 0 ? extentWidth : contentWidth;
    if (
      shouldShowHorizontalOverflowVeil(
        designWidth,
        overflowExtentWidth,
        contentMaxWidth,
      )
    ) {
      if (!veilX) {
        veilX = document.createElement("div");
        veilX.className =
          "preview-design-overflow-veil preview-design-overflow-veil-x";
        veilX.setAttribute("aria-hidden", "true");
        shell.appendChild(veilX);
      }
      const left = round(designWidth * scale);
      const width = round((overflowExtentWidth - designWidth) * scale);
      veilX.style.top = "0";
      veilX.style.left = `${left}px`;
      veilX.style.width = `${width}px`;
      veilX.style.height = `${round(Math.max(designHeight, contentHeight) * scale)}px`;
      ensureOverflowVeilLabel(
        veilX,
        "x",
        overflowExtentWidth - designWidth,
        overflowExtentWidth,
        designWidth,
      );
    } else {
      veilX?.remove();
    }
  }

  function computeDebugScale(
    fluidHeight,
    scaleMode,
    hostWidth,
    hostHeight,
    designWidth,
    designHeight,
  ) {
    const sx = hostWidth / designWidth;
    const mode = String(scaleMode || "fit-width").trim().toLowerCase();
    if (fluidHeight || mode === "fit-width" || mode === "width" || mode === "fit_width") {
      return sx > 0 ? sx : 1;
    }
    if (mode === "cover" || mode === "contain") {
      return computeScale(mode, hostWidth, hostHeight, designWidth, designHeight);
    }
    return sx > 0 ? sx : 1;
  }

  function applyDebugPreviewLayout(
    root,
    shell,
    stage,
    hostWidth,
    hostHeight,
    designWidth,
    designHeight,
    fluidHeight,
    contentMaxWidth,
  ) {
    const canvasWidth =
      contentMaxWidth > 0 ? Math.min(contentMaxWidth, designWidth) : designWidth;
    if (contentMaxWidth > 0 && designWidth > canvasWidth + 0.5) {
      root.style.justifyItems = "start";
    }
    initManagePreviewZoom(root);
    ensureManageZoomToolbar(root);
    const wrap = ensureStageScaleWrap(shell, stage);
    const inner = ensureStageScaleInner(wrap);
    clearDebugOverlayNodes(inner);
    inner.style.transform = "none";
    inner.style.width = "";
    inner.style.height = "";

    unlockStageLayoutForDebug(stage, designHeight);
    stage.style.width = `${round(canvasWidth)}px`;
    stage.style.height = "auto";
    stage.style.maxHeight = "none";
    stage.style.transform = "none";
    stage.style.zoom = "";
    stage.style.transformOrigin = "top left";

    let {
      width: contentWidth,
      extentWidth,
      height: contentHeight,
    } = measureStageContentSize(
      stage,
      canvasWidth,
      designHeight,
      fluidHeight,
      contentMaxWidth,
    );

    const fitScale = computeManageFitScale(
      hostWidth,
      hostHeight,
      contentWidth,
      contentHeight,
    );
    const appliedZoom = resolveManagePreviewZoom(root, fitScale);
    const aspectRatio = String(root.dataset.aspectRatio || "").trim();
    const layoutKey = manageLayoutKey(
      contentWidth,
      contentHeight,
      appliedZoom,
      hostWidth,
      hostHeight,
      canvasWidth,
    );
    if (root.dataset.meiLayoutKey === layoutKey) {
      clearDebugOverlayNodes(inner);
      ensureDesignBounds(inner, canvasWidth, fluidHeight ? contentHeight : designHeight, 1);
      ensureOverflowVeil(
        inner,
        canvasWidth,
        designHeight,
        1,
        contentWidth,
        contentHeight,
        extentWidth,
        contentMaxWidth,
      );
      ensureViewportChrome(
        root,
        canvasWidth,
        designHeight,
        aspectRatio,
        contentHeight,
        fluidHeight,
      );
      updateManageZoomToolbar(root);
      return;
    }
    root.dataset.meiLayoutKey = layoutKey;
    root.dataset.meiFitScale = String(round(fitScale));
    root.dataset.meiAppliedZoom = String(round(appliedZoom));
    root.dataset.meiFrameScale = String(round(appliedZoom));

    inner.style.width = `${round(contentWidth)}px`;
    inner.style.height = `${round(contentHeight)}px`;
    inner.style.transformOrigin = "top left";
    inner.style.transform =
      Math.abs(appliedZoom - 1) > 0.001 ? `scale(${appliedZoom})` : "none";

    const shellWidth = round(contentWidth * appliedZoom);
    const shellHeight = round(contentHeight * appliedZoom);

    wrap.style.display = "block";
    wrap.style.position = "relative";
    wrap.style.overflow = "visible";
    wrap.style.width = `${shellWidth}px`;
    wrap.style.height = `${shellHeight}px`;
    wrap.style.margin = "0";

    shell.style.display = "block";
    shell.style.overflow = "visible";
    shell.style.maxHeight = "none";
    shell.style.position = "relative";
    shell.style.margin = "0";
    shell.style.justifyContent = "";
    shell.style.alignItems = "";
    shell.style.width = `${shellWidth}px`;
    shell.style.height = `${shellHeight}px`;

    root.dataset.meiContentWidth = String(Math.round(contentWidth));
    root.dataset.meiContentHeight = String(Math.round(contentHeight));

    ensureViewportChrome(
      root,
      canvasWidth,
      designHeight,
      aspectRatio,
      contentHeight,
      fluidHeight,
    );
    const boundsHeight = fluidHeight ? contentHeight : designHeight;
    ensureDesignBounds(inner, canvasWidth, boundsHeight, 1);
    ensureOverflowVeil(
      inner,
      canvasWidth,
      designHeight,
      1,
      contentWidth,
      contentHeight,
      extentWidth,
      contentMaxWidth,
    );
    updateManageZoomToolbar(root);

    scheduleManageViewportRelayout(root, contentHeight);
  }

  const manageRelayoutTimers = new WeakMap();

  /** 异步内容（字体/图表）稳定前最多补测一次，避免每帧触发 ResizeObserver 振荡。 */
  function scheduleManageViewportRelayout(root, contentHeight) {
    const pass = Number(root.dataset.meiRelayoutPass || 0);
    if (pass >= 1) return;
    const prev = manageRelayoutTimers.get(root) || {};
    if (prev.t500) return;
    const prevH = Number(root.dataset.meiScheduledContentHeight || 0);
    if (pass > 0 && Math.abs(prevH - contentHeight) < 2) return;
    root.dataset.meiScheduledContentHeight = String(Math.round(contentHeight));
    prev.t500 = setTimeout(() => {
      const timers = manageRelayoutTimers.get(root) || {};
      timers.t500 = null;
      root.dataset.meiRelayoutPass = "1";
      delete root.dataset.meiLayoutKey;
      updateViewport(root);
    }, 500);
    manageRelayoutTimers.set(root, prev);
  }

  /**
   * 访问 / 运行态：contain（默认）或 cover；contain 等比铺满宿主且不裁切，信纸区居中。
   */
  function applyCanvasScaleLayout(
    root,
    shell,
    stage,
    scaleMode,
    hostWidth,
    hostHeight,
    designWidth,
    designHeight,
  ) {
    removeDesignBounds(shell);
    const mode = String(scaleMode || "contain").trim().toLowerCase();
    const scale = computeScale(mode, hostWidth, hostHeight, designWidth, designHeight);
    const scaleText = round(scale);
    const shellWidth = round(designWidth * scale);
    const shellHeight = round(designHeight * scale);

    shell.style.display = "flex";
    shell.style.alignItems = "center";
    shell.style.justifyContent = "center";
    shell.style.width = `${shellWidth}px`;
    shell.style.height = `${shellHeight}px`;
    shell.style.maxWidth = "none";
    shell.style.maxHeight = "none";
    shell.style.margin = "0";
    shell.style.flex = "none";
    shell.style.position = "relative";
    shell.style.overflow = "hidden";
    shell.style.boxSizing = "border-box";

    stage.style.width = `${round(designWidth)}px`;
    stage.style.height = `${round(designHeight)}px`;
    stage.style.minHeight = "";
    stage.style.flexShrink = "0";
    stage.style.transformOrigin = "center center";
    stage.style.transform =
      Math.abs(scaleText - 1) > 0.001 ? `scale(${scaleText})` : "none";
    stage.style.zoom = "";

    root.dataset.meiFrameScale = String(scaleText);
    root.dataset.meiManageRelayout = "";
  }

  function syncChromeNoneViewportBox(root) {
    if (!isChromeNoneAccess()) return;
    root.style.width = "100vw";
    root.style.height = "100vh";
    root.style.maxWidth = "100vw";
    root.style.maxHeight = "100vh";
    if (window.visualViewport) {
      root.style.width = `${window.visualViewport.width}px`;
      root.style.height = `${window.visualViewport.height}px`;
    }
  }

  function updateViewport(root) {
    const designWidthDeclared = Number(root.dataset.designWidth || 0);
    const designWidth = resolveCanvasWidth(root, designWidthDeclared);
    const designHeight = Number(root.dataset.designHeight || 0);
    const contentMaxWidth = Number(root.dataset.contentMaxWidth || 0);
    const contentHeightAttr = Number(root.dataset.contentHeight || 0);
    const fluidHeight = root.dataset.contentFluidHeight === "true";
    const contentHeight = contentHeightAttr || designHeight;
    const scaleMode = String(root.dataset.scaleMode || "contain").trim().toLowerCase();
    const editScaleMode = String(root.dataset.editScaleMode || "contain").trim().toLowerCase();
    const overflowMode = String(root.dataset.overflowMode || "clip").trim().toLowerCase();
    const safe = readSafeInsets(root, overflowMode);
    const shell = root.querySelector(".preview-stage-shell");
    const stage = root.querySelector(".preview-stage");
    if (!shell || !stage || !designWidth) return;

    syncChromeNoneViewportBox(root);
    const { hostWidth, hostHeight } = isManagePreviewRoute(root)
      ? resolveManageHostSize(root, safe)
      : resolveHostSize(root, safe);

    if (isManagePreviewRoute(root)) {
      if (viewportLayoutApplying.get(root)) return;
      viewportLayoutApplying.set(root, true);
      try {
        applyDebugPreviewLayout(
          root,
          shell,
          stage,
          hostWidth,
          hostHeight,
        designWidthDeclared,
        designHeight || contentHeight,
        fluidHeight,
        contentMaxWidth > 0 ? contentMaxWidth : 0,
      );
      } finally {
        requestAnimationFrame(() => viewportLayoutApplying.delete(root));
      }
      return;
    }

    if (contentMaxWidth > 0 && !fluidHeight) {
      applyFluidWidthLayout(shell, stage, contentMaxWidth, hostWidth);
      return;
    }

    if (!designHeight && !contentHeight) return;
    applyCanvasScaleLayout(
      root,
      shell,
      stage,
      scaleMode,
      hostWidth,
      hostHeight,
      designWidth,
      designHeight || contentHeight,
    );
  }

  function observeHostResize(root) {
    const hosts = [];
    let node = root.parentElement;
    while (node && node !== document.body) {
      hosts.push(node);
      node = node.parentElement;
    }
    if (isChromeNoneAccess()) {
      hosts.push(document.body);
    }
    const seen = new Set();
    const callbacks = hosts
      .filter((host) => {
        if (!host || host === root || seen.has(host)) return false;
        seen.add(host);
        return true;
      })
      .map((host) => {
        const observer = new ResizeObserver(() => queueUpdateViewport(root));
        observer.observe(host);
        observers.add(observer);
        return observer;
      });
    return callbacks;
  }

  function observeViewport(root) {
    if (tracked.has(root)) {
      if (isManagePreviewRoute(root)) invalidateManageLayout(root);
      queueUpdateViewport(root);
      return;
    }
    const observersForRoot = [];
    const manage = isManagePreviewRoute(root);
    const scrollPane = root.closest(".preview-pane-scroll");
    const resizeTarget = manage && scrollPane ? scrollPane : root;
    const rootObserver = new ResizeObserver(() => queueUpdateViewport(root));
    rootObserver.observe(resizeTarget);
    observersForRoot.push(rootObserver);
    observers.add(rootObserver);

    if (!manage) {
      const hostObservers = observeHostResize(root);
      if (hostObservers?.length) observersForRoot.push(...hostObservers);
    }

    tracked.set(root, observersForRoot);
    invalidateManageLayout(root);
    updateViewport(root);
  }

  function scan() {
    document
      .querySelectorAll('[data-mei-frame-viewport="true"]')
      .forEach((root) => observeViewport(root));
  }

  function scheduleViewportRelayout() {
    requestAnimationFrame(() => {
      document
        .querySelectorAll('[data-mei-frame-viewport="true"]')
        .forEach((root) => {
          if (isManagePreviewRoute(root)) invalidateManageLayout(root);
          queueUpdateViewport(root);
        });
    });
  }

  let domReadyHandler = null;
  if (document.readyState === "loading") {
    domReadyHandler = () => scan();
    document.addEventListener("DOMContentLoaded", domReadyHandler, { once: true });
  } else {
    scan();
  }

  function onManageTabChange(event) {
    const tab = event?.detail?.tab;
    if (!tab || tab === "preview") {
      scheduleViewportRelayout();
    }
  }

  function onWindowResize() {
    scheduleViewportRelayout();
  }

  window.addEventListener("resize", onWindowResize);
  window.visualViewport?.addEventListener("resize", onWindowResize);
  window.visualViewport?.addEventListener("scroll", onWindowResize);
  window.addEventListener("meilang:preview-updated", scan);
  document.addEventListener("mei:manage-tab-change", onManageTabChange);
  document.addEventListener("click", (event) => {
    const btn = event.target.closest("[data-preview-zoom]");
    if (!btn) return;
    const root = btn.closest('[data-mei-frame-viewport="true"]');
    if (!root || !isManagePreviewRoute(root)) return;
    const mode = String(btn.dataset.previewZoom || "");
    if (mode === "minus") {
      stepManagePreviewZoom(root, -0.1);
      return;
    }
    if (mode === "plus") {
      stepManagePreviewZoom(root, 0.1);
      return;
    }
    setManagePreviewZoom(root, mode);
  });

  boot.scheduleFrameViewportRelayout = scheduleViewportRelayout;
  boot.disposeFrameStage = () => {
    window.removeEventListener("resize", onWindowResize);
    window.visualViewport?.removeEventListener("resize", onWindowResize);
    window.visualViewport?.removeEventListener("scroll", onWindowResize);
    window.removeEventListener("meilang:preview-updated", scan);
    document.removeEventListener("mei:manage-tab-change", onManageTabChange);
    if (domReadyHandler) {
      document.removeEventListener("DOMContentLoaded", domReadyHandler);
      domReadyHandler = null;
    }
    observers.forEach((observer) => {
      try {
        observer.disconnect();
      } catch (_) {}
    });
    observers.clear();
    tracked.clear();
    boot.scheduleFrameViewportRelayout = null;
  };
})();
