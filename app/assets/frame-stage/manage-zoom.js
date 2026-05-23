    const gapBudget = detectCardGapBudget(stage);
    if (gapBudget.length) {
      diagnostics.push({
        severity: "info",
        code: "layout_audit_card_gap_budget_runtime",
        message: `检测到卡组 gap/留白偏离预算：${gapBudget.join("、")}`,
      });
    }
    publishLayoutAudit(root, diagnostics);
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
