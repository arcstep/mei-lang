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
      const extent = measureStageContentExtent(stage, contentMaxWidth);
      runLayoutAudit(
        root,
        stage,
        designWidth,
        designHeight || contentHeight,
        extent.width,
        extent.height,
        extent.width,
      );
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
    const extent = measureStageContentExtent(stage, contentMaxWidth);
    runLayoutAudit(
      root,
      stage,
      designWidth,
      designHeight || contentHeight,
      extent.width,
      extent.height,
      extent.width,
    );
  }
